use std::path::PathBuf;

use super::{default_config_path, default_config_toml, default_data_dir};

/// Initialize a new configuration file
pub(crate) fn run_init(output: Option<String>, force: bool, use_wizard: bool) {
    if use_wizard {
        #[cfg(feature = "wizard")]
        run_init_wizard(output, force);
        #[cfg(not(feature = "wizard"))]
        {
            let _ = (output, force);
            eprintln!("Error: The interactive wizard requires the 'wizard' feature to be enabled.");
            eprintln!("Rebuild with: cargo build --features wizard");
            eprintln!("Or use 'gateway init' without --wizard for a default config.");
            std::process::exit(1);
        }
    } else {
        run_init_default(output, force);
    }
}

/// Run the interactive configuration wizard.
#[cfg(feature = "wizard")]
fn run_init_wizard(output: Option<String>, force: bool) {
    match crate::wizard::run() {
        Ok(result) => {
            // Use the wizard's suggested path or override with --output
            let output_path = output.map(PathBuf::from).unwrap_or(result.path);

            if output_path.exists() && !force {
                eprintln!(
                    "Config file already exists: {}\nUse --force to overwrite.",
                    output_path.display()
                );
                std::process::exit(1);
            }

            // Create parent directories if needed
            if let Some(parent) = output_path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Failed to create directory {}: {}", parent.display(), e);
                std::process::exit(1);
            }

            // Create data directory if needed
            if let Some(data_dir) = default_data_dir()
                && let Err(e) = std::fs::create_dir_all(&data_dir)
            {
                eprintln!(
                    "Warning: Failed to create data directory {}: {}",
                    data_dir.display(),
                    e
                );
            }

            if let Err(e) = std::fs::write(&output_path, &result.config) {
                eprintln!("Failed to write config file: {}", e);
                std::process::exit(1);
            }

            println!();
            println!("Created config file: {}", output_path.display());
            println!();
            println!("To start the gateway, run:");
            println!("  gateway serve --config {}", output_path.display());
        }
        Err(crate::wizard::WizardError::Cancelled) => {
            println!("Wizard cancelled.");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Wizard error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Create a default configuration file (non-interactive).
fn run_init_default(output: Option<String>, force: bool) {
    let Some(output_path) = output.map(PathBuf::from).or_else(default_config_path) else {
        eprintln!("Could not determine default config path. Please specify one with --output.");
        std::process::exit(1);
    };

    if output_path.exists() && !force {
        eprintln!(
            "Config file already exists: {}\nUse --force to overwrite.",
            output_path.display()
        );
        std::process::exit(1);
    }

    // Create parent directories if needed
    if let Some(parent) = output_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Failed to create directory {}: {}", parent.display(), e);
        std::process::exit(1);
    }

    // Determine data directory and expand paths
    let data_dir = default_data_dir().unwrap_or_else(|| PathBuf::from("."));
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "Failed to create data directory {}: {}",
            data_dir.display(),
            e
        );
        std::process::exit(1);
    }

    let config_content = default_config_toml().replace(
        "~/.local/share/hadrian/hadrian.db",
        &data_dir.join("hadrian.db").to_string_lossy(),
    );

    if let Err(e) = std::fs::write(&output_path, config_content) {
        eprintln!("Failed to write config file: {}", e);
        std::process::exit(1);
    }

    println!("Created config file: {}", output_path.display());
    println!("Database will be stored at: {}", data_dir.display());
    println!();
    println!("To start the gateway, run:");
    println!("  gateway serve");
    println!();
    println!("For interactive configuration, use:");
    println!("  gateway init --wizard");
}
