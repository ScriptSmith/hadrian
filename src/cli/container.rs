//! `hadrian container` — boot a one-off shell container using the configured
//! runtime (microsandbox / opensandbox), stage files, and run commands or an
//! interactive shell. Mirrors how the Responses-API shell tool provisions a
//! session so operators can reproduce and debug agent behavior locally.

#![cfg(feature = "server")]

use std::{
    io::{Write, stderr, stdin, stdout},
    sync::Arc,
    time::Duration,
};

use futures_util::StreamExt;

use super::resolve_config_path;
use crate::{
    config::{self, ShellRuntimeConfig},
    runtimes::{EgressPolicy, ExecEvent, ExecRequest, NetworkMode, SessionSpec, ShellRuntime},
};

/// Entry point for the `container` subcommand.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_container(
    explicit_config_path: Option<&str>,
    exec: Vec<String>,
    files: Vec<String>,
    allow_hosts: Vec<String>,
    memory_mb: Option<u64>,
    cpus: Option<f64>,
    timeout_secs: u64,
) {
    let (config_path, _) = match resolve_config_path(explicit_config_path) {
        Ok(v) => v,
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

    let runtime = match build_runtime(&config.features.shell) {
        Ok(rt) => rt,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };

    // Egress allowlist: explicit `--allow-host` wins, else the operator's
    // configured ceiling, else `*` (the point of this command is debugging, so
    // default to reachable rather than deny-all).
    let limits = &config.features.server_tools.shell_limits;
    let allow_hosts = if !allow_hosts.is_empty() {
        allow_hosts
    } else if !limits.allowed_egress_hosts.is_empty() {
        limits.allowed_egress_hosts.clone()
    } else {
        vec!["*".to_string()]
    };
    let network_mode = if allow_hosts.iter().any(|h| h == "*") {
        NetworkMode::Full
    } else {
        NetworkMode::AllowList
    };

    let spec = SessionSpec {
        network_mode: Some(network_mode),
        egress_policy: EgressPolicy {
            allow_hosts: allow_hosts.clone(),
            secrets: Vec::new(),
        },
        mounted_skills: Vec::new(),
        cpu_limit: cpus.or(limits.default_cpu_limit),
        mem_limit_bytes: memory_mb
            .or_else(|| limits.default_mem_limit_mb.map(u64::from))
            .map(|mb| mb * 1024 * 1024),
        session_id_hint: None,
    };

    eprintln!(
        "Starting {} container (egress: {})…",
        config.features.shell.name(),
        allow_hosts.join(", ")
    );
    let session = match runtime.start_session(spec).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start container: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("Container ready: {}", session.session_id);

    // Stage input files into /mnt/data, mirroring input_file staging.
    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Failed to read {path}: {e}");
                continue;
            }
        };
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let dest = format!("/mnt/data/{filename}");
        match session.write_file(&dest, bytes.into()).await {
            Ok(()) => eprintln!("Staged {path} → {dest}"),
            Err(e) => eprintln!("Failed to stage {path}: {e}"),
        }
    }

    let timeout = Duration::from_secs(timeout_secs);
    let mut last_exit = 0;

    if exec.is_empty() {
        // Interactive shell: one command per line until EOF (Ctrl-D) or `exit`.
        eprintln!("Interactive shell. Type `exit` or Ctrl-D to quit.\n");
        loop {
            print!("container$ ");
            let _ = stdout().flush();
            let mut line = String::new();
            match stdin().read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    eprintln!("stdin error: {e}");
                    break;
                }
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "exit" || line == "quit" {
                break;
            }
            last_exit = run_command(&session, line, timeout).await;
        }
    } else {
        for cmd in &exec {
            last_exit = run_command(&session, cmd, timeout).await;
        }
    }

    if let Err(e) = session.terminate().await {
        eprintln!("Warning: failed to terminate container cleanly: {e}");
    }
    std::process::exit(last_exit);
}

/// Build the runtime from `[features.shell]`. Passthrough runtimes don't
/// execute locally (the model's client or OpenAI does), so they're rejected
/// with a hint. Returns the human-facing error message on failure.
fn build_runtime(shell: &ShellRuntimeConfig) -> Result<Arc<dyn ShellRuntime>, String> {
    match shell {
        ShellRuntimeConfig::None => Err(
            "No shell runtime configured. Set `[features.shell]` to `microsandbox` or \
             `opensandbox` in your config."
                .to_string(),
        ),
        ShellRuntimeConfig::PassthroughOpenAI | ShellRuntimeConfig::ClientPassthrough => {
            Err(format!(
                "The `{}` runtime executes shell calls outside Hadrian (OpenAI's container or the \
             API client), so there's nothing to run locally. Configure `microsandbox` or \
             `opensandbox` to use this command.",
                shell.name()
            ))
        }
        #[cfg(feature = "runtime-microsandbox")]
        ShellRuntimeConfig::Microsandbox(cfg) => Ok(Arc::new(
            crate::runtimes::MicrosandboxRuntime::new(cfg.clone()),
        )),
        #[cfg(feature = "runtime-opensandbox")]
        ShellRuntimeConfig::OpenSandbox(cfg) => Ok(Arc::new(
            crate::runtimes::OpenSandboxRuntime::new(cfg.clone(), reqwest::Client::new()),
        )),
    }
}

/// Run one command, streaming stdout/stderr to the terminal. Returns the exit
/// code (or 1 if the command couldn't be launched).
async fn run_command(
    session: &crate::runtimes::SessionHandle,
    command: &str,
    timeout: Duration,
) -> i32 {
    let handle = match session
        .exec(ExecRequest {
            command: command.to_string(),
            stdin: None,
            timeout: Some(timeout),
        })
        .await
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!("exec failed: {e}");
            return 1;
        }
    };

    let mut output = handle.output;
    let mut exit_code = 0;
    while let Some(event) = output.next().await {
        match event {
            ExecEvent::Stdout(bytes) => {
                let _ = stdout().write_all(&bytes);
                let _ = stdout().flush();
            }
            ExecEvent::Stderr(bytes) => {
                let _ = stderr().write_all(&bytes);
                let _ = stderr().flush();
            }
            ExecEvent::Exit { code, .. } => exit_code = code,
        }
    }
    exit_code
}
