/// Print enabled compile-time features and build profile.
pub(crate) fn run_features() {
    let version = env!("CARGO_PKG_VERSION");

    // Check each feature at compile time
    let features: &[(&str, &str, bool)] = &[
        // Providers
        (
            "provider-openai",
            "Providers",
            cfg!(feature = "provider-openai"),
        ),
        (
            "provider-anthropic",
            "Providers",
            cfg!(feature = "provider-anthropic"),
        ),
        (
            "provider-test",
            "Providers",
            cfg!(feature = "provider-test"),
        ),
        (
            "provider-bedrock",
            "Providers",
            cfg!(feature = "provider-bedrock"),
        ),
        (
            "provider-vertex",
            "Providers",
            cfg!(feature = "provider-vertex"),
        ),
        (
            "provider-azure",
            "Providers",
            cfg!(feature = "provider-azure"),
        ),
        // Assets
        ("embed-ui", "Assets", cfg!(feature = "embed-ui")),
        ("embed-docs", "Assets", cfg!(feature = "embed-docs")),
        ("embed-catalog", "Assets", cfg!(feature = "embed-catalog")),
        // Databases
        (
            "database-sqlite",
            "Databases",
            cfg!(feature = "database-sqlite"),
        ),
        (
            "database-postgres",
            "Databases",
            cfg!(feature = "database-postgres"),
        ),
        // Infrastructure
        ("redis", "Infrastructure", cfg!(feature = "redis")),
        ("otlp", "Infrastructure", cfg!(feature = "otlp")),
        ("sso", "Infrastructure", cfg!(feature = "sso")),
        ("saml", "Infrastructure", cfg!(feature = "saml")),
        ("cel", "Infrastructure", cfg!(feature = "cel")),
        ("prometheus", "Infrastructure", cfg!(feature = "prometheus")),
        // Secrets
        ("vault", "Secrets", cfg!(feature = "vault")),
        ("secrets-aws", "Secrets", cfg!(feature = "secrets-aws")),
        ("secrets-azure", "Secrets", cfg!(feature = "secrets-azure")),
        ("secrets-gcp", "Secrets", cfg!(feature = "secrets-gcp")),
        // Storage & Processing
        (
            "s3-storage",
            "Storage & Processing",
            cfg!(feature = "s3-storage"),
        ),
        (
            "document-extraction-basic",
            "Storage & Processing",
            cfg!(feature = "document-extraction-basic"),
        ),
        (
            "document-extraction-full",
            "Storage & Processing",
            cfg!(feature = "document-extraction-full"),
        ),
        (
            "virus-scan",
            "Storage & Processing",
            cfg!(feature = "virus-scan"),
        ),
        // Validation & Export
        (
            "json-schema",
            "Validation & Export",
            cfg!(feature = "json-schema"),
        ),
        (
            "response-validation",
            "Validation & Export",
            cfg!(feature = "response-validation"),
        ),
        (
            "csv-export",
            "Validation & Export",
            cfg!(feature = "csv-export"),
        ),
        // Tools
        ("forecasting", "Tools", cfg!(feature = "forecasting")),
        ("wizard", "Tools", cfg!(feature = "wizard")),
        // Documentation
        ("utoipa", "Documentation", cfg!(feature = "utoipa")),
    ];

    // Infer build profile from enabled features
    let profile = if cfg!(feature = "full") {
        "full"
    } else if cfg!(feature = "headless") {
        "headless"
    } else if cfg!(feature = "standard") {
        "standard"
    } else if cfg!(feature = "minimal") {
        "minimal"
    } else if cfg!(feature = "tiny") {
        "tiny"
    } else {
        "custom"
    };

    println!("Hadrian Gateway v{version}\n");
    println!("Build profile: {profile}");
    match profile {
        "full" => println!("  (full = standard + saml, doc-extraction-full, virus-scan)\n"),
        "headless" => {
            println!("  (headless = full features without embedded assets — UI, docs, catalog)\n")
        }
        "standard" => println!(
            "  (standard = minimal + redis, otlp, doc-extraction-basic, postgres, embed-docs, prometheus, cel, utoipa, sso, forecasting, json-schema, response-validation, csv-export)\n"
        ),
        "minimal" => {
            println!("  (minimal = tiny + sqlite, embed-catalog, embed-ui, wizard)\n")
        }
        "tiny" => {
            println!(
                "  (tiny = openai, anthropic, test providers only, no database, no embedded assets)\n"
            )
        }
        _ => println!(),
    }

    println!("Compile-time features:");

    let mut current_group = "";
    for &(name, group, enabled) in features {
        if group != current_group {
            if !current_group.is_empty() {
                println!();
            }
            println!("  {group}:");
            current_group = group;
        }
        let status = if enabled { "enabled" } else { "disabled" };
        println!("    {name:<32} {status}");
    }
}
