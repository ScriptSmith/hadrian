use super::resolve_config_path;
use crate::{config, db, models, observability, services};

/// Run the bootstrap command: create initial org, SSO config, and API key from config.
pub(crate) async fn run_bootstrap(explicit_config_path: Option<&str>, dry_run: bool) {
    // Resolve config path
    let (config_path, _) = match resolve_config_path(explicit_config_path) {
        Ok((path, is_new)) => (path, is_new),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let config = match config::GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config from {}: {e}", config_path.display());
            std::process::exit(1);
        }
    };

    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    let bootstrap = match &config.auth.bootstrap {
        Some(b) => b.clone(),
        None => {
            eprintln!("Error: No [auth.bootstrap] section in config file.");
            eprintln!("Add an [auth.bootstrap] section with initial_org and/or initial_api_key.");
            std::process::exit(1);
        }
    };

    if config.database.is_none() {
        eprintln!("Error: Database is not configured. Bootstrap requires a database.");
        std::process::exit(1);
    }

    if dry_run {
        println!("=== Bootstrap Dry Run ===");
        println!("Config: {}", config_path.display());
        if let Some(ref org) = bootstrap.initial_org {
            println!("  Create org: slug={}, name={}", org.slug, org.name);
            #[cfg(feature = "sso")]
            if let Some(ref sso) = org.sso {
                println!(
                    "  Configure SSO: provider={}, issuer={}",
                    sso.provider_type,
                    sso.issuer.as_deref().unwrap_or("(none)")
                );
                if !sso.allowed_email_domains.is_empty() {
                    println!("  Email domains: {:?}", sso.allowed_email_domains);
                }
            }
            if !org.admin_identities.is_empty() {
                println!("  Admin identities: {:?}", org.admin_identities);
            }
        }
        if !bootstrap.auto_verify_domains.is_empty() {
            println!("  Auto-verify domains: {:?}", bootstrap.auto_verify_domains);
        }
        if let Some(ref key) = bootstrap.initial_api_key {
            println!("  Create API key: name={}", key.name);
        }
        println!("=== No changes applied (dry run) ===");
        std::process::exit(0);
    }

    // Connect to database and run migrations
    let db = match db::DbPool::from_config(&config.database).await {
        Ok(pool) => {
            if let Err(e) = pool.run_migrations().await {
                eprintln!("Error: Database migrations failed: {e}");
                std::process::exit(1);
            }
            std::sync::Arc::new(pool)
        }
        Err(e) => {
            eprintln!("Error: Failed to connect to database: {e}");
            std::process::exit(1);
        }
    };

    let file_storage: std::sync::Arc<dyn services::FileStorage> =
        std::sync::Arc::new(services::DatabaseFileStorage::new(db.clone()));
    let max_cel = config.auth.rbac.max_expression_length;
    let services = services::Services::new(db.clone(), file_storage, max_cel);

    let api_key_prefix = config.auth.api_key_config().generation_prefix();
    let mut summary = Vec::new();

    // 1. Create org if configured
    let org_id = if let Some(ref org_config) = bootstrap.initial_org {
        match services
            .organizations
            .create(models::CreateOrganization {
                slug: org_config.slug.clone(),
                name: org_config.name.clone(),
            })
            .await
        {
            Ok(org) => {
                let msg = format!("Created organization: {} ({})", org.slug, org.id);
                tracing::info!("{msg}");
                summary.push(msg);
                Some(org.id)
            }
            Err(db::DbError::Conflict(_)) => {
                let existing = services
                    .organizations
                    .get_by_slug(&org_config.slug)
                    .await
                    .unwrap_or(None);
                if let Some(org) = existing {
                    let msg = format!("Organization already exists: {} ({})", org.slug, org.id);
                    tracing::info!("{msg}");
                    summary.push(msg);
                    Some(org.id)
                } else {
                    eprintln!("Error: Organization conflict but not found by slug");
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Error creating organization: {e}");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // 2. Configure SSO if specified
    #[cfg(feature = "sso")]
    if let Some(ref org_config) = bootstrap.initial_org
        && let (Some(sso_config), Some(oid)) = (&org_config.sso, org_id)
    {
        // Check if SSO config already exists
        let existing = services.org_sso_configs.get_by_org_id(oid).await;
        if let Ok(Some(_)) = existing {
            let msg = format!("SSO config already exists for org {oid}");
            tracing::info!("{msg}");
            summary.push(msg);
        } else {
            // Initialize secret manager for SSO (reuse same logic as AppState)
            let secret_manager: std::sync::Arc<dyn crate::secrets::SecretManager> =
                match crate::init::init_secret_manager(&config).await {
                    Ok(sm) => sm,
                    Err(e) => {
                        eprintln!("Error initializing secret manager for SSO: {e}");
                        std::process::exit(1);
                    }
                };

            let provider_type = match sso_config.provider_type.as_str() {
                "saml" => models::SsoProviderType::Saml,
                _ => models::SsoProviderType::Oidc,
            };

            let create_input = models::CreateOrgSsoConfig {
                provider_type,
                issuer: sso_config.issuer.clone(),
                discovery_url: sso_config.discovery_url.clone(),
                client_id: sso_config.client_id.clone(),
                client_secret: sso_config.client_secret.clone(),
                redirect_uri: sso_config.redirect_uri.clone(),
                allowed_email_domains: sso_config.allowed_email_domains.clone(),
                ..Default::default()
            };

            match services
                .org_sso_configs
                .create(oid, create_input, secret_manager.as_ref())
                .await
            {
                Ok(created) => {
                    let msg = format!("Created SSO config for org {oid} ({})", created.id);
                    tracing::info!("{msg}");
                    summary.push(msg);

                    // Auto-verify domains
                    for domain in &bootstrap.auto_verify_domains {
                        if sso_config.allowed_email_domains.contains(domain) {
                            match services
                                .domain_verifications
                                .create_auto_verified(created.id, domain)
                                .await
                            {
                                Ok(_) => {
                                    let msg = format!("Auto-verified domain: {domain}");
                                    tracing::info!("{msg}");
                                    summary.push(msg);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to auto-verify domain {domain}: {e}");
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error creating SSO config: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    // 3. Create API key if configured
    if let Some(ref key_config) = bootstrap.initial_api_key {
        let oid = if let Some(oid) = org_id {
            oid
        } else {
            eprintln!("Error: initial_api_key requires initial_org to be configured.");
            std::process::exit(1);
        };

        // Check if key already exists (idempotent)
        match services
            .api_keys
            .get_by_name_and_org(oid, &key_config.name)
            .await
        {
            Ok(Some(existing)) => {
                let msg = format!(
                    "API key already exists: {} ({})",
                    existing.name, existing.id
                );
                tracing::info!("{msg}");
                summary.push(msg);
            }
            Ok(None) => {
                let owner = models::ApiKeyOwner::Organization { org_id: oid };
                match services
                    .api_keys
                    .create(
                        models::CreateApiKey {
                            name: key_config.name.clone(),
                            owner,
                            budget_limit_cents: None,
                            budget_period: None,
                            expires_at: None,
                            scopes: None,
                            allowed_models: None,
                            ip_allowlist: None,
                            rate_limit_rpm: None,
                            rate_limit_tpm: None,
                        },
                        &api_key_prefix,
                    )
                    .await
                {
                    Ok(created) => {
                        let msg = format!(
                            "Created API key: {} ({})",
                            created.api_key.name, created.api_key.id
                        );
                        tracing::info!("{msg}");
                        summary.push(msg);
                        // Print the raw key to stdout (only shown once)
                        println!("{}", created.key);
                    }
                    Err(e) => {
                        eprintln!("Error creating API key: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error checking for existing API key: {e}");
                std::process::exit(1);
            }
        }
    }

    // Print summary
    eprintln!();
    eprintln!("=== Bootstrap Summary ===");
    for line in &summary {
        eprintln!("  {line}");
    }
    if summary.is_empty() {
        eprintln!("  No changes made (nothing configured in [auth.bootstrap])");
    }
    eprintln!("=========================");
}
