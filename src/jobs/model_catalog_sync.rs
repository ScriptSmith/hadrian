//! Model catalog sync worker for updating the catalog from models.dev.
//!
//! This module provides a background worker that periodically fetches the
//! model catalog from models.dev and updates the registry with the latest
//! model metadata including capabilities, pricing, and limits.
//!
//! The sync is designed to be resilient:
//! - Errors don't crash the worker, just log and retry next interval
//! - The embedded catalog serves as a fallback when sync fails
//! - Initial sync runs immediately on startup

use std::time::Instant;

use reqwest::Client;

use crate::{catalog::ModelCatalogRegistry, config::ModelCatalogConfig};

/// Results from a single sync run.
#[derive(Debug)]
pub struct SyncRunResult {
    /// Number of models in the catalog after sync.
    pub model_count: usize,
    /// Duration of the sync in milliseconds.
    pub duration_ms: u64,
}

/// Starts the model catalog sync worker as a background task.
///
/// The worker runs in a loop, fetching the catalog at the configured interval.
/// It will run indefinitely until the task is cancelled.
pub async fn start_model_catalog_sync_worker(
    registry: ModelCatalogRegistry,
    config: ModelCatalogConfig,
    http_client: Client,
) {
    if !config.enabled {
        tracing::info!("Model catalog sync worker disabled by configuration");
        return;
    }

    tracing::info!(
        sync_interval_secs = config.sync_interval_secs,
        api_url = %config.api_url,
        "Starting model catalog sync worker"
    );

    let interval = std::time::Duration::from_secs(config.sync_interval_secs);

    // Run initial sync immediately
    match run_sync(&registry, &config, &http_client).await {
        Ok(result) => {
            tracing::info!(
                model_count = result.model_count,
                duration_ms = result.duration_ms,
                "Initial model catalog sync complete"
            );
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Initial model catalog sync failed, using embedded catalog"
            );
        }
    }

    // Then run at configured interval
    loop {
        tokio::time::sleep(interval).await;

        match run_sync(&registry, &config, &http_client).await {
            Ok(result) => {
                tracing::debug!(
                    model_count = result.model_count,
                    duration_ms = result.duration_ms,
                    "Model catalog sync complete"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Model catalog sync failed, keeping existing data"
                );
            }
        }
    }
}

/// Run a single sync pass, fetching the catalog from the API.
async fn run_sync(
    registry: &ModelCatalogRegistry,
    config: &ModelCatalogConfig,
    http_client: &Client,
) -> Result<SyncRunResult, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();

    // Fetch the catalog from the API
    let response = http_client
        .get(&config.api_url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    let json = response.text().await?;

    // Parse and load into registry
    registry.load_from_json(&json)?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let model_count = registry.model_count();

    Ok(SyncRunResult {
        model_count,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_run_result() {
        let result = SyncRunResult {
            model_count: 100,
            duration_ms: 500,
        };

        assert_eq!(result.model_count, 100);
        assert_eq!(result.duration_ms, 500);
    }
}
