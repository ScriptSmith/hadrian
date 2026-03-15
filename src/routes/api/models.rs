use axum::{Extension, Json, extract::State};
use serde::Serialize;

use super::ApiError;
use crate::AppState;

/// Combined models response with provider-prefixed model IDs.
#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CombinedModelsResponse {
    /// List of available models
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    data: Vec<serde_json::Value>,
}

/// List available models
///
/// Lists all models available from all configured providers.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/models",
    tag = "models",
    responses(
        (status = 200, description = "List of available models", body = CombinedModelsResponse),
        (status = 400, description = "Bad request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.models", skip(state, auth))]
pub async fn api_v1_models(
    State(state): State<AppState>,
    auth: Option<Extension<crate::auth::AuthenticatedRequest>>,
) -> Result<Json<CombinedModelsResponse>, ApiError> {
    use futures::future::join_all;

    // Create futures for fetching models from all providers in parallel
    let fetch_futures: Vec<_> = state
        .config
        .providers
        .iter()
        .map(|(provider_name, provider_config)| {
            let provider_name = provider_name.to_owned();
            let http_client = state.http_client.clone();
            let circuit_breakers = state.circuit_breakers.clone();

            async move {
                let models_result = crate::providers::list_models_for_config(
                    provider_config,
                    &provider_name,
                    &http_client,
                    &circuit_breakers,
                )
                .await;
                (provider_name, models_result)
            }
        })
        .collect();

    // Fetch from all providers in parallel
    let results = join_all(fetch_futures).await;

    // Collect successful results and enrich with catalog data
    let mut all_models = Vec::new();
    for (provider_name, models_result) in results {
        if let Ok(models_response) = models_result {
            // Get the provider config for catalog lookup
            let provider_config = state.config.providers.get(&provider_name);

            // Resolve the catalog provider ID for this provider
            let catalog_provider_id = provider_config.and_then(|pc| {
                crate::catalog::resolve_catalog_provider_id(
                    pc.provider_type_name(),
                    pc.base_url(),
                    pc.catalog_provider(),
                )
            });

            // Prefix each model ID with the provider name and enrich with catalog + config data
            for model in models_response.data {
                let prefixed_id = format!("{}/{}", provider_name, model.id);
                let mut model_json = model.extra;
                if let Some(obj) = model_json.as_object_mut() {
                    obj.insert("id".to_string(), serde_json::Value::String(prefixed_id));

                    // Look up catalog enrichment and config override
                    let enrichment = catalog_provider_id
                        .as_ref()
                        .and_then(|pid| state.model_catalog.lookup(pid, &model.id));
                    let model_config =
                        provider_config.and_then(|pc| pc.get_model_config(&model.id));

                    // Merge metadata: config wins if present, else catalog, else omit.
                    // Only enrich if at least one source has data.
                    if enrichment.is_some() || model_config.is_some() {
                        // Capabilities: config overrides catalog
                        if let Some(ref caps) = model_config.and_then(|mc| mc.capabilities.as_ref())
                        {
                            obj.insert(
                                "capabilities".to_string(),
                                serde_json::to_value(caps).unwrap_or_default(),
                            );
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "capabilities".to_string(),
                                serde_json::to_value(&e.capabilities).unwrap_or_default(),
                            );
                        }

                        // Context length: config > provider response > catalog
                        if let Some(ctx_len) = model_config.and_then(|mc| mc.context_length) {
                            obj.insert(
                                "context_length".to_string(),
                                serde_json::Value::Number(ctx_len.into()),
                            );
                        } else if !obj.contains_key("context_length")
                            && let Some(ctx_len) =
                                enrichment.as_ref().and_then(|e| e.limits.context_length)
                        {
                            obj.insert(
                                "context_length".to_string(),
                                serde_json::Value::Number(ctx_len.into()),
                            );
                        }

                        // Max output tokens
                        if let Some(max_out) = model_config.and_then(|mc| mc.max_output_tokens) {
                            obj.insert(
                                "max_output_tokens".to_string(),
                                serde_json::Value::Number(max_out.into()),
                            );
                        } else if let Some(max_out) =
                            enrichment.as_ref().and_then(|e| e.limits.max_output_tokens)
                        {
                            obj.insert(
                                "max_output_tokens".to_string(),
                                serde_json::Value::Number(max_out.into()),
                            );
                        }

                        // Modalities: config overrides catalog
                        if let Some(ref mods) = model_config.and_then(|mc| mc.modalities.as_ref()) {
                            obj.insert(
                                "modalities".to_string(),
                                serde_json::to_value(mods).unwrap_or_default(),
                            );
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "modalities".to_string(),
                                serde_json::to_value(&e.modalities).unwrap_or_default(),
                            );
                        }

                        // Tasks: config overrides catalog
                        let tasks = model_config
                            .filter(|mc| !mc.tasks.is_empty())
                            .map(|mc| &mc.tasks)
                            .or(enrichment
                                .as_ref()
                                .filter(|e| !e.tasks.is_empty())
                                .map(|e| &e.tasks));
                        if let Some(tasks) = tasks {
                            obj.insert(
                                "tasks".to_string(),
                                serde_json::to_value(tasks).unwrap_or_default(),
                            );
                        }

                        // Catalog pricing for display (from catalog only)
                        if let Some(ref e) = enrichment {
                            obj.insert(
                                "catalog_pricing".to_string(),
                                serde_json::to_value(&e.catalog_pricing).unwrap_or_default(),
                            );
                        }

                        // Family: config overrides catalog
                        if let Some(family) = model_config
                            .and_then(|mc| mc.family.as_ref())
                            .or(enrichment.as_ref().and_then(|e| e.family.as_ref()))
                        {
                            obj.insert(
                                "family".to_string(),
                                serde_json::Value::String(family.clone()),
                            );
                        }

                        // Open weights: config overrides catalog
                        if let Some(ow) = model_config.and_then(|mc| mc.open_weights) {
                            obj.insert("open_weights".to_string(), serde_json::Value::Bool(ow));
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "open_weights".to_string(),
                                serde_json::Value::Bool(e.open_weights),
                            );
                        }

                        // Image generation metadata (config only)
                        if let Some(mc) = model_config {
                            if !mc.image_sizes.is_empty() {
                                obj.insert(
                                    "image_sizes".to_string(),
                                    serde_json::to_value(&mc.image_sizes).unwrap_or_default(),
                                );
                            }
                            if !mc.image_qualities.is_empty() {
                                obj.insert(
                                    "image_qualities".to_string(),
                                    serde_json::to_value(&mc.image_qualities).unwrap_or_default(),
                                );
                            }
                            if let Some(max) = mc.max_images {
                                obj.insert(
                                    "max_images".to_string(),
                                    serde_json::Value::Number(max.into()),
                                );
                            }
                            if !mc.voices.is_empty() {
                                obj.insert(
                                    "voices".to_string(),
                                    serde_json::to_value(&mc.voices).unwrap_or_default(),
                                );
                            }
                        }
                    }

                    // Sovereignty: merge provider → model override (independent of catalog)
                    let provider_sov = provider_config.and_then(|pc| pc.sovereignty());
                    let model_sov = model_config.and_then(|mc| mc.sovereignty.as_ref());
                    if let Some(merged) =
                        crate::config::SovereigntyMetadata::merge(provider_sov, model_sov)
                            .filter(|m| !m.is_empty())
                    {
                        obj.insert(
                            "sovereignty".to_string(),
                            serde_json::to_value(&merged).unwrap_or_default(),
                        );
                    }
                } else {
                    model_json = serde_json::json!({ "id": prefixed_id });
                }
                all_models.push(model_json);
            }
        }
        // Skip providers that fail to return models
    }

    // Mark all static models with source
    for model in &mut all_models {
        if let Some(obj) = model.as_object_mut() {
            obj.insert(
                "source".to_string(),
                serde_json::Value::String("static".to_string()),
            );
        }
    }

    // Include dynamic models from the authenticated user's and org's providers (if any).
    // Falls back to the default anonymous user when API auth is disabled.
    let user_id_for_models = auth
        .as_ref()
        .and_then(|Extension(a)| a.user_id())
        .or(state.default_user_id);

    if let (Some(user_id), Some(services)) = (user_id_for_models, state.services.as_ref()) {
        // Look up the user's org membership for building scoped model IDs
        let org_membership = services
            .users
            .get_org_memberships_for_user(user_id)
            .await
            .ok()
            .and_then(|m| m.into_iter().next());

        let org_slug = org_membership.as_ref().map(|m| m.org_slug.as_str());

        // Helper: resolve models for a dynamic provider (with 5-minute cache)
        let resolve_models = |provider: &crate::models::DynamicProvider| {
            let provider = provider.clone();
            let http_client = state.http_client.clone();
            let circuit_breakers = state.circuit_breakers.clone();
            let secrets = state.secrets.clone();
            let cache = state.cache.clone();
            async move {
                if !provider.models.is_empty() {
                    return provider.models;
                }

                // Check cache for previously discovered models
                let cache_key = format!("gw:provider:models:{}", provider.id);
                if let Some(ref cache) = cache
                    && let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await
                    && let Ok(models) = serde_json::from_slice::<Vec<String>>(&bytes)
                {
                    return models;
                }

                let Ok(config) = crate::routing::resolver::dynamic_provider_to_config(
                    &provider,
                    secrets.as_ref(),
                )
                .await
                else {
                    return Vec::new();
                };
                let models: Vec<String> = crate::providers::list_models_for_config(
                    &config,
                    &provider.name,
                    &http_client,
                    &circuit_breakers,
                )
                .await
                .map(|r| r.data.into_iter().map(|m| m.id).collect())
                .unwrap_or_default();

                // Cache the discovered models for 5 minutes
                if !models.is_empty()
                    && let Some(ref cache) = cache
                    && let Ok(bytes) = serde_json::to_vec(&models)
                {
                    let _ = cache
                        .set_bytes(&cache_key, &bytes, std::time::Duration::from_secs(300))
                        .await;
                }

                models
            }
        };

        // Collect all enabled providers across scopes, auto-paginating through cursor pages
        #[cfg(not(target_arch = "wasm32"))]
        type ProviderPageFn = Box<
            dyn Fn(
                    crate::db::repos::ListParams,
                ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = crate::db::DbResult<
                                    crate::db::repos::ListResult<crate::models::DynamicProvider>,
                                >,
                            > + Send,
                    >,
                > + Send,
        >;
        #[cfg(target_arch = "wasm32")]
        type ProviderPageFn = Box<
            dyn Fn(
                crate::db::repos::ListParams,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = crate::db::DbResult<
                                crate::db::repos::ListResult<crate::models::DynamicProvider>,
                            >,
                        >,
                >,
            >,
        >;
        let collect_all_enabled = |fetch_page: ProviderPageFn| async move {
            let mut all = Vec::new();
            let mut params = crate::db::repos::ListParams {
                limit: Some(100),
                ..Default::default()
            };
            loop {
                let Ok(page) = fetch_page(params.clone()).await else {
                    break;
                };
                all.extend(page.items);
                if !page.has_more {
                    break;
                }
                match page.cursors.next {
                    Some(cursor) => {
                        params.cursor = Some(cursor);
                    }
                    None => break,
                }
            }
            all
        };

        // Fetch user and org providers concurrently
        let user_providers_fut = {
            let services = services.clone();
            collect_all_enabled(Box::new(move |params| {
                let services = services.clone();
                Box::pin(async move {
                    services
                        .providers
                        .list_enabled_by_user(user_id, params)
                        .await
                })
            }))
        };

        let org_providers_fut = {
            let services = services.clone();
            let org_membership = org_membership.clone();
            async move {
                if let Some(ref membership) = org_membership {
                    let org_id = membership.org_id;
                    collect_all_enabled(Box::new(move |params| {
                        let services = services.clone();
                        Box::pin(async move {
                            services.providers.list_enabled_by_org(org_id, params).await
                        })
                    }))
                    .await
                } else {
                    Vec::new()
                }
            }
        };

        let project_providers_fut = {
            let services = services.clone();
            async move {
                let Ok(project_memberships) = services
                    .users
                    .get_project_memberships_for_user(user_id)
                    .await
                else {
                    return Vec::new();
                };
                let futs: Vec<_> = project_memberships
                    .iter()
                    .map(|m| {
                        let services = services.clone();
                        let project_id = m.project_id;
                        let project_slug = m.project_slug.clone();
                        async move {
                            let providers = collect_all_enabled(Box::new(move |params| {
                                let services = services.clone();
                                Box::pin(async move {
                                    services
                                        .providers
                                        .list_enabled_by_project(project_id, params)
                                        .await
                                })
                            }))
                            .await;
                            (project_slug, providers)
                        }
                    })
                    .collect();
                futures::future::join_all(futs).await
            }
        };

        let team_providers_fut = {
            let services = services.clone();
            async move {
                let Ok(team_memberships) =
                    services.users.get_team_memberships_for_user(user_id).await
                else {
                    return Vec::new();
                };
                let futs: Vec<_> = team_memberships
                    .iter()
                    .map(|m| {
                        let services = services.clone();
                        let team_id = m.team_id;
                        let team_slug = m.team_slug.clone();
                        let org_id = m.org_id;
                        async move {
                            let org_slug = services
                                .organizations
                                .get_by_id(org_id)
                                .await
                                .ok()
                                .flatten()
                                .map(|o| o.slug)
                                .unwrap_or_default();
                            let providers = collect_all_enabled(Box::new(move |params| {
                                let services = services.clone();
                                Box::pin(async move {
                                    services
                                        .providers
                                        .list_enabled_by_team(team_id, params)
                                        .await
                                })
                            }))
                            .await;
                            (org_slug, team_slug, providers)
                        }
                    })
                    .collect();
                futures::future::join_all(futs).await
            }
        };

        let (user_providers, org_providers, project_groups, team_groups) = tokio::join!(
            user_providers_fut,
            org_providers_fut,
            project_providers_fut,
            team_providers_fut,
        );

        // Resolve models for all providers concurrently within each scope
        let user_futs: Vec<_> = user_providers
            .iter()
            .map(|p| async move { (p, resolve_models(p).await) })
            .collect();
        let org_futs: Vec<_> = org_providers
            .iter()
            .map(|p| async move { (p, resolve_models(p).await) })
            .collect();
        let project_futs: Vec<_> = project_groups
            .iter()
            .flat_map(|(slug, providers)| {
                providers
                    .iter()
                    .map(move |p| async move { (slug.as_str(), p, resolve_models(p).await) })
            })
            .collect();

        let team_futs: Vec<_> = team_groups
            .iter()
            .flat_map(|(org_slug, team_slug, providers)| {
                providers.iter().map(move |p| async move {
                    (
                        org_slug.as_str(),
                        team_slug.as_str(),
                        p,
                        resolve_models(p).await,
                    )
                })
            })
            .collect();

        let (user_results, org_results, project_results, team_results) = tokio::join!(
            futures::future::join_all(user_futs),
            futures::future::join_all(org_futs),
            futures::future::join_all(project_futs),
            futures::future::join_all(team_futs),
        );

        // User-owned dynamic providers
        for (provider, model_names) in &user_results {
            let provider_name = &provider.name;
            for model_name in model_names {
                let scoped_id = if let Some(slug) = org_slug {
                    format!(":org/{slug}/:user/{user_id}/{provider_name}/{model_name}")
                } else {
                    format!(":user/{user_id}/{provider_name}/{model_name}")
                };
                all_models.push(serde_json::json!({
                    "id": scoped_id,
                    "object": "model",
                    "owned_by": provider_name,
                    "source": "dynamic",
                    "provider_name": provider_name,
                }));
            }
        }

        // Organization-owned dynamic providers
        if let Some(ref membership) = org_membership {
            for (provider, model_names) in &org_results {
                let provider_name = &provider.name;
                for model_name in model_names {
                    let scoped_id =
                        format!(":org/{}/{provider_name}/{model_name}", membership.org_slug);
                    all_models.push(serde_json::json!({
                        "id": scoped_id,
                        "object": "model",
                        "owned_by": provider_name,
                        "source": "dynamic",
                        "provider_name": provider_name,
                    }));
                }
            }
        }

        // Project-owned dynamic providers
        {
            let org_slug_for_project = org_membership
                .as_ref()
                .map(|m| m.org_slug.as_str())
                .unwrap_or("unknown");

            for (project_slug, provider, model_names) in &project_results {
                let provider_name = &provider.name;
                for model_name in model_names {
                    let scoped_id = format!(
                        ":org/{org_slug_for_project}/:project/{project_slug}/{provider_name}/{model_name}"
                    );
                    all_models.push(serde_json::json!({
                        "id": scoped_id,
                        "object": "model",
                        "owned_by": provider_name,
                        "source": "dynamic",
                        "provider_name": provider_name,
                    }));
                }
            }
        }

        // Team-owned dynamic providers
        for (org_slug, team_slug, provider, model_names) in &team_results {
            if org_slug.is_empty() {
                continue;
            }
            let provider_name = &provider.name;
            for model_name in model_names {
                let scoped_id =
                    format!(":org/{org_slug}/:team/{team_slug}/{provider_name}/{model_name}");
                all_models.push(serde_json::json!({
                    "id": scoped_id,
                    "object": "model",
                    "owned_by": provider_name,
                    "source": "dynamic",
                    "provider_name": provider_name,
                }));
            }
        }
    }

    Ok(Json(CombinedModelsResponse { data: all_models }))
}
