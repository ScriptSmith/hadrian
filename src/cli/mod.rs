mod bootstrap;
mod features;
mod init;
mod migrate;
mod openapi;
mod server;
#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
mod worker;

use std::path::PathBuf;

use clap::Parser;

/// CLI arguments for Hadrian Gateway
#[derive(Parser, Debug)]
#[command(version, about = "Hadrian AI Gateway", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to config file (defaults to ~/.config/hadrian/hadrian.toml if it exists,
    /// otherwise creates a default config)
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Disable automatic browser opening on startup
    #[arg(long, global = true)]
    no_browser: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Start the gateway server (default)
    Serve,
    /// Export the OpenAPI specification (JSON format)
    Openapi {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Export the JSON schema for the configuration file
    Schema {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Initialize a new configuration file
    Init {
        /// Path to create the config file (defaults to ~/.config/hadrian/hadrian.toml)
        #[arg(short, long)]
        output: Option<String>,
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
        /// Run interactive configuration wizard
        #[arg(short, long)]
        wizard: bool,
    },
    /// Run the file processing worker (for queue-based file processing)
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    Worker {
        /// Unique consumer name for this worker instance (defaults to random UUID)
        #[arg(long)]
        consumer_name: Option<String>,
        /// Number of jobs to process per batch (default: 10)
        #[arg(long, default_value = "10")]
        batch_size: usize,
        /// Block timeout in milliseconds when waiting for jobs (default: 5000)
        #[arg(long, default_value = "5000")]
        block_timeout_ms: u64,
        /// Whether to claim pending messages from other consumers (default: true)
        #[arg(long, default_value = "true")]
        claim_pending: bool,
        /// Max idle time in ms before a pending message can be claimed (default: 60000)
        #[arg(long, default_value = "60000")]
        pending_timeout_ms: u64,
    },
    /// Run database migrations and exit
    ///
    /// Useful for Kubernetes init containers or CI/CD pipelines.
    /// Connects to the database, runs any pending migrations, and exits.
    Migrate,
    /// Bootstrap organizations, SSO configs, and API keys from config.
    ///
    /// Reads [auth.bootstrap] from hadrian.toml and creates the initial org,
    /// SSO configuration, auto-verified domains, and API key. Idempotent:
    /// safe to run repeatedly (skips resources that already exist).
    /// Operates directly against the database (no HTTP server needed).
    Bootstrap {
        /// Preview changes without applying them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Show enabled compile-time features
    Features,
}

/// Dispatch to the appropriate subcommand handler.
pub async fn dispatch(args: Args) {
    match args.command {
        Some(Command::Openapi { output }) => {
            #[cfg(feature = "utoipa")]
            openapi::run_openapi_export(output);
            #[cfg(not(feature = "utoipa"))]
            {
                let _ = output;
                eprintln!("Error: OpenAPI export requires the 'utoipa' feature to be enabled");
                std::process::exit(1);
            }
        }
        Some(Command::Schema { output }) => {
            #[cfg(feature = "json-schema")]
            openapi::run_schema_export(output);
            #[cfg(not(feature = "json-schema"))]
            {
                let _ = output;
                eprintln!("Error: JSON schema export requires the 'json-schema' feature");
                std::process::exit(1);
            }
        }
        Some(Command::Init {
            output,
            force,
            wizard,
        }) => {
            init::run_init(output, force, wizard);
        }
        #[cfg(any(
            feature = "document-extraction-basic",
            feature = "document-extraction-full"
        ))]
        Some(Command::Worker {
            consumer_name,
            batch_size,
            block_timeout_ms,
            claim_pending,
            pending_timeout_ms,
        }) => {
            worker::run_worker(
                args.config.as_deref(),
                consumer_name,
                batch_size,
                block_timeout_ms,
                claim_pending,
                pending_timeout_ms,
            )
            .await;
        }
        Some(Command::Migrate) => {
            migrate::run_migrate(args.config.as_deref()).await;
        }
        Some(Command::Bootstrap { dry_run }) => {
            bootstrap::run_bootstrap(args.config.as_deref(), dry_run).await;
        }
        Some(Command::Features) => {
            features::run_features();
        }
        Some(Command::Serve) | None => {
            server::run_server(args.config.as_deref(), args.no_browser).await;
        }
    }
}

/// Default configuration for zero-config startup.
/// Uses SQLite for storage and in-memory cache for simplicity.
pub(crate) fn default_config_toml() -> &'static str {
    r#"# Hadrian AI Gateway Configuration
# Generated automatically for local development

[server]
host = "127.0.0.1"
port = 8080
# Allow providers on localhost (e.g. Ollama)
allow_loopback_urls = true

# CORS: Allow local development origins
[server.cors]
enabled = true
allowed_origins = ["http://localhost:8080", "http://127.0.0.1:8080"]
allow_credentials = true

# SQLite database for persistent storage
[database]
type = "sqlite"
path = "~/.local/share/hadrian/hadrian.db"

# In-memory cache for rate limiting and sessions
[cache]
type = "memory"

# Web UI enabled and served from embedded assets
[ui]
enabled = true

# Example provider configuration (uncomment and add your API key)
# [providers.openai]
# type = "open_ai"
# api_key = "${OPENAI_API_KEY}"
#
# [providers.anthropic]
# type = "anthropic"
# api_key = "${ANTHROPIC_API_KEY}"
"#
}

/// Get the default config directory path.
#[cfg(feature = "wizard")]
pub(crate) fn default_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("hadrian"))
}

/// Get the default config directory path.
#[cfg(not(feature = "wizard"))]
pub(crate) fn default_config_dir() -> Option<PathBuf> {
    None
}

/// Get the default config file path.
pub fn default_config_path() -> Option<PathBuf> {
    default_config_dir().map(|p| p.join("hadrian.toml"))
}

/// Get the default data directory path.
#[cfg(feature = "wizard")]
pub fn default_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("hadrian"))
}

/// Get the default data directory path.
#[cfg(not(feature = "wizard"))]
pub fn default_data_dir() -> Option<PathBuf> {
    None
}

/// Resolve the config path, creating default config if necessary.
/// Returns the config path and whether it was newly created.
pub(crate) fn resolve_config_path(explicit_path: Option<&str>) -> Result<(PathBuf, bool), String> {
    // If explicit path is provided, use it
    if let Some(path) = explicit_path {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("Config file not found: {}", path.display()));
        }
        return Ok((path, false));
    }

    // Check for hadrian.toml in current directory
    let cwd_config = PathBuf::from("hadrian.toml");
    if cwd_config.exists() {
        return Ok((cwd_config, false));
    }

    // Check for config in default location
    if let Some(default_path) = default_config_path()
        && default_path.exists()
    {
        return Ok((default_path, false));
    }

    // No config found - create default config
    create_default_config()
}

/// Create the default configuration file and data directory.
fn create_default_config() -> Result<(PathBuf, bool), String> {
    let config_dir = default_config_dir().ok_or("Could not determine config directory")?;
    let config_path = config_dir.join("hadrian.toml");
    let data_dir = default_data_dir().ok_or("Could not determine data directory")?;

    // Create directories
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;

    // Write default config with expanded path
    let config_content = default_config_toml().replace(
        "~/.local/share/hadrian/hadrian.db",
        &data_dir.join("hadrian.db").to_string_lossy(),
    );
    std::fs::write(&config_path, config_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok((config_path, true))
}
