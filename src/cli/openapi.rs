/// Export OpenAPI specification to file or stdout (JSON format)
#[cfg(feature = "utoipa")]
pub(crate) fn run_openapi_export(output: Option<String>) {
    let spec = crate::openapi::ApiDoc::build();
    let content =
        serde_json::to_string_pretty(&spec).expect("Failed to serialize OpenAPI spec to JSON");

    match output {
        Some(path) => {
            std::fs::write(&path, &content)
                .unwrap_or_else(|e| panic!("Failed to write to {}: {}", path, e));
            eprintln!("OpenAPI spec written to {}", path);
        }
        None => {
            println!("{}", content);
        }
    }
}

/// Export JSON schema for the configuration file to file or stdout
#[cfg(feature = "json-schema")]
pub(crate) fn run_schema_export(output: Option<String>) {
    let content = crate::config::GatewayConfig::json_schema_string();

    match output {
        Some(path) => {
            std::fs::write(&path, &content)
                .unwrap_or_else(|e| panic!("Failed to write to {}: {}", path, e));
            eprintln!("Config JSON schema written to {}", path);
        }
        None => {
            println!("{}", content);
        }
    }
}
