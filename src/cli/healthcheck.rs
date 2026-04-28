//! `hadrian healthcheck` subcommand.
//!
//! Issues a single GET against `/health/live` and exits 0/1. Used by the
//! Docker image's `HEALTHCHECK` so the runtime image can drop `curl`.

use std::time::Duration;

pub async fn run_healthcheck(
    config_path: Option<&str>,
    url_override: Option<String>,
    timeout_secs: u64,
) {
    let url = match url_override {
        Some(u) => u,
        None => match resolve_url_from_config(config_path) {
            Ok(u) => u,
            Err(err) => {
                eprintln!("healthcheck: could not resolve URL from config: {err}");
                std::process::exit(1);
            }
        },
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("healthcheck: could not build HTTP client: {err}");
            std::process::exit(1);
        }
    };

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            std::process::exit(0);
        }
        Ok(resp) => {
            eprintln!("healthcheck: {url} returned status {}", resp.status());
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("healthcheck: request to {url} failed: {err}");
            std::process::exit(1);
        }
    }
}

fn resolve_url_from_config(config_path: Option<&str>) -> Result<String, String> {
    let path = config_path.ok_or_else(|| {
        "no --config supplied and no --url override; pass one of them".to_string()
    })?;
    let config = crate::config::GatewayConfig::from_file(path).map_err(|e| e.to_string())?;
    let host = match config.server.host.to_string().as_str() {
        // 0.0.0.0 isn't dialable; map back to loopback for the local probe.
        "0.0.0.0" => "127.0.0.1".to_string(),
        "::" => "[::1]".to_string(),
        other => {
            // Wrap bare IPv6 addresses in brackets for URL syntax.
            if other.contains(':') && !other.starts_with('[') {
                format!("[{other}]")
            } else {
                other.to_string()
            }
        }
    };
    Ok(format!("http://{host}:{}/health/live", config.server.port))
}
