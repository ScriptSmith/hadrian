//! Microsandbox runtime adapter.
//!
//! Wraps the [`microsandbox`] SDK so each Hadrian session corresponds to
//! one local microVM. Microsandbox runs in-process (no daemon, no
//! endpoint) — booting a VM, streaming command I/O, and tearing it down
//! all happen via direct Rust calls.

#![cfg(feature = "runtime-microsandbox")]

use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use microsandbox::{ExecEvent as MsExecEvent, Sandbox};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    config::MicrosandboxConfig,
    runtimes::{
        EgressPolicy, ExecEvent, ExecHandle, ExecRequest, NetworkMode, RuntimeCapabilities,
        RuntimeError, RuntimeResult, SessionHandle, SessionSpec, ShellRuntime, ShellSession,
    },
};

/// `ShellRuntime` implementation backed by microsandbox microVMs.
pub struct MicrosandboxRuntime {
    config: MicrosandboxConfig,
    /// Pre-warm pool of booted sandboxes ready for immediate use.
    /// Only consumed for sessions whose SessionSpec doesn't require
    /// secrets, skills, or other build-time configuration that would
    /// invalidate a pre-booted VM.
    pool: Arc<Mutex<VecDeque<Sandbox>>>,
}

impl MicrosandboxRuntime {
    pub fn new(config: MicrosandboxConfig) -> Self {
        let pool: Arc<Mutex<VecDeque<Sandbox>>> = Arc::new(Mutex::new(VecDeque::new()));
        let runtime = Self {
            config: config.clone(),
            pool: pool.clone(),
        };

        // Kick off the initial pool fill in the background so app
        // startup isn't blocked on VM boots.
        let target = config.prewarm_pool_size;
        if target > 0 {
            let cfg = config.clone();
            crate::compat::spawn_detached(async move {
                info!(pool_target = target, "Filling microsandbox pre-warm pool");
                for _ in 0..target {
                    match boot_default_sandbox(&cfg).await {
                        Ok(sandbox) => {
                            let mut guard = pool.lock().await;
                            guard.push_back(sandbox);
                            debug!(pool_size = guard.len(), "Pre-warmed sandbox added to pool");
                        }
                        Err(e) => {
                            warn!(error = %e, "Pre-warm boot failed; pool may be undersized");
                        }
                    }
                }
            });
        }

        runtime
    }

    /// True if `spec` requires VM-creation-time configuration that
    /// can't be applied to a pre-warmed sandbox.
    fn spec_requires_fresh_vm(spec: &SessionSpec) -> bool {
        let EgressPolicy {
            ref allow_hosts,
            ref secrets,
        } = spec.egress_policy;
        !allow_hosts.is_empty()
            || !secrets.is_empty()
            || !spec.mounted_skills.is_empty()
            || spec.cpu_limit.is_some()
            || spec.mem_limit_bytes.is_some()
    }
}

/// Boot one sandbox with the runtime's default image/cpu/memory and a
/// random name. Used both for pre-warm fill and for fresh-VM fallback.
async fn boot_default_sandbox(config: &MicrosandboxConfig) -> RuntimeResult<Sandbox> {
    let name = format!("hadrian-pool-{}", Uuid::new_v4());
    let cpus = config.cpus as u8;
    let memory_mb = config.memory_mb;
    Sandbox::builder(name)
        .image(config.image.clone())
        .cpus(cpus)
        .memory(memory_mb)
        .replace()
        .create()
        .await
        .map_err(|e| RuntimeError::Backend(format!("microsandbox create (pool): {e}")))
}

#[async_trait]
impl ShellRuntime for MicrosandboxRuntime {
    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            passthrough_only: false,
            // Slice 1D enables secret injection via microsandbox's
            // SecretBuilder (placeholder substitution at the TLS proxy).
            secret_injection: true,
            // Hostname-based egress allowlist without an accompanying
            // secret isn't natively supported by microsandbox's
            // NetworkPolicy (which is IP-based). Adding it would
            // require DNS-resolution + per-IP rules; punt for now and
            // recommend operators scope egress via secret mounts.
            egress_allowlist: false,
            skill_mount: true,
            file_io: true,
            network_isolation_modes: vec![NetworkMode::Full],
            max_session_duration: None,
        }
    }

    async fn start_session(&self, spec: SessionSpec) -> RuntimeResult<SessionHandle> {
        // Validate capability requirements before spinning up a VM.
        if !spec.egress_policy.allow_hosts.is_empty() {
            return Err(RuntimeError::Unsupported("egress_allowlist"));
        }

        // Fast path: if the spec only needs a default-config VM and the
        // pool has one ready, hand it over and trigger an async refill.
        if !Self::spec_requires_fresh_vm(&spec) && self.config.prewarm_pool_size > 0 {
            let pooled = {
                let mut guard = self.pool.lock().await;
                guard.pop_front()
            };
            if let Some(sandbox) = pooled {
                let session_id = sandbox.name().to_string();
                debug!(
                    stage = "microsandbox_pool_checkout",
                    session_id = %session_id,
                    "Reusing pre-warmed sandbox"
                );

                // Refill in the background.
                let cfg = self.config.clone();
                let pool = self.pool.clone();
                crate::compat::spawn_detached(async move {
                    match boot_default_sandbox(&cfg).await {
                        Ok(s) => pool.lock().await.push_back(s),
                        Err(e) => warn!(error = %e, "Pool refill failed"),
                    }
                });

                return Ok(SessionHandle::new(
                    session_id,
                    Box::new(MicrosandboxSession {
                        sandbox: Arc::new(Mutex::new(Some(sandbox))),
                    }),
                ));
            }
            debug!(
                stage = "microsandbox_pool_empty",
                "Pre-warm pool empty; falling back to fresh VM"
            );
        }

        let session_id = spec
            .session_id_hint
            .unwrap_or_else(|| format!("hadrian-{}", Uuid::new_v4()));

        let cpus: u8 = spec
            .cpu_limit
            .map(|c| c.ceil() as u8)
            .unwrap_or(self.config.cpus as u8);
        let memory_mb: u32 = spec
            .mem_limit_bytes
            .map(|b| (b / (1024 * 1024)) as u32)
            .unwrap_or(self.config.memory_mb);

        info!(
            stage = "microsandbox_starting",
            session_id = %session_id,
            image = %self.config.image,
            cpus,
            memory_mb,
            "Creating microsandbox VM"
        );

        let mut builder = Sandbox::builder(session_id.clone())
            .image(self.config.image.clone())
            .cpus(cpus)
            .memory(memory_mb)
            .replace();

        // Wire each requested SecretMount into microsandbox's
        // SecretBuilder. Each mount gives the guest a placeholder env
        // var (e.g. `$MSB_GITHUB_TOKEN`); the TLS-intercepting proxy
        // substitutes the real value when outbound requests are
        // destined for one of the allowed hosts. The model never sees
        // the raw secret value.
        for mount in &spec.egress_policy.secrets {
            if mount.allowed_hosts.is_empty() {
                return Err(RuntimeError::Backend(format!(
                    "secret mount {:?} has no allowed_hosts",
                    mount.placeholder
                )));
            }
            let placeholder = mount.placeholder.clone();
            let value = mount.value.clone();
            let hosts = mount.allowed_hosts.clone();
            builder = builder.secret(move |s| {
                let mut sb = s.env(&placeholder).value(&value);
                for h in &hosts {
                    sb = if h.contains('*') {
                        sb.allow_host_pattern(h)
                    } else {
                        sb.allow_host(h)
                    };
                }
                sb
            });
        }

        let sandbox = builder
            .create()
            .await
            .map_err(|e| RuntimeError::Backend(format!("microsandbox create: {e}")))?;

        // Mount any requested skill bundles via the VM's filesystem
        // API. Each skill's files are written under its `mount_path`;
        // intermediate directories are created on demand.
        for skill in &spec.mounted_skills {
            mount_skill_into_sandbox(&sandbox, skill).await?;
            debug!(
                stage = "microsandbox_skill_mounted",
                skill_id = %skill.skill_id,
                mount_path = %skill.mount_path,
                file_count = skill.files.len(),
                "Mounted skill bundle"
            );
        }

        Ok(SessionHandle::new(
            session_id,
            Box::new(MicrosandboxSession {
                sandbox: Arc::new(Mutex::new(Some(sandbox))),
            }),
        ))
    }
}

/// Write a skill's files into a freshly-booted sandbox. Creates the
/// mount directory and any subdirectories implied by file paths.
async fn mount_skill_into_sandbox(
    sandbox: &Sandbox,
    skill: &crate::runtimes::SkillMount,
) -> RuntimeResult<()> {
    let fs = sandbox.fs();
    // Root directory first.
    fs.mkdir(&skill.mount_path)
        .await
        .map_err(|e| RuntimeError::Backend(format!("mkdir {}: {e}", skill.mount_path)))?;
    for file in &skill.files {
        let full_path = join_paths(&skill.mount_path, &file.relative_path);
        // Ensure parent directory exists.
        if let Some(parent) = parent_of(&full_path)
            && parent != skill.mount_path
        {
            // mkdir is idempotent in microsandbox? If not, the
            // backend returns AlreadyExists which we ignore.
            let _ = fs.mkdir(&parent).await;
        }
        fs.write(&full_path, file.content.as_ref())
            .await
            .map_err(|e| RuntimeError::Backend(format!("write {full_path}: {e}")))?;
    }
    Ok(())
}

fn join_paths(base: &str, rel: &str) -> String {
    let base = base.trim_end_matches('/');
    let rel = rel.trim_start_matches('/');
    format!("{base}/{rel}")
}

fn parent_of(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    let idx = trimmed.rfind('/')?;
    if idx == 0 {
        return Some("/".to_string());
    }
    Some(trimmed[..idx].to_string())
}

/// One live microsandbox session.
///
/// The inner `Option<Sandbox>` lets `terminate()` consume the Sandbox
/// without requiring `&mut self` (which the trait doesn't provide).
struct MicrosandboxSession {
    sandbox: Arc<Mutex<Option<Sandbox>>>,
}

#[async_trait]
impl ShellSession for MicrosandboxSession {
    async fn exec(&self, cmd: ExecRequest) -> RuntimeResult<ExecHandle> {
        let guard = self.sandbox.lock().await;
        let sandbox = guard
            .as_ref()
            .ok_or_else(|| RuntimeError::Backend("session already terminated".into()))?;

        // stdin: SDK supports a Pipe mode for streaming; for a one-shot
        // bytes payload from the model we'd ideally use `stdin_bytes`
        // before exec, but `shell_stream` is the simplest path and
        // doesn't accept stdin. Fall back to redirecting via heredoc in
        // the command when stdin is provided.
        let script = match cmd.stdin {
            Some(bytes) => {
                let stdin_text = String::from_utf8_lossy(&bytes);
                // Quote the heredoc terminator with an unlikely marker
                // so the model's stdin can't accidentally close it.
                format!(
                    "{} <<'__HADRIAN_STDIN_EOF__'\n{}\n__HADRIAN_STDIN_EOF__",
                    cmd.command, stdin_text
                )
            }
            None => cmd.command.clone(),
        };

        let mut handle = sandbox
            .shell_stream(script)
            .await
            .map_err(|e| RuntimeError::Backend(format!("microsandbox shell_stream: {e}")))?;

        // Bridge the SDK's UnboundedReceiver<MsExecEvent> into our
        // Stream<ExecEvent>. We can't move `handle` directly across a
        // .await in a stream::unfold without owning it, so we drain it
        // into our own channel via a detached task.
        let (tx, rx) = mpsc::channel::<ExecEvent>(32);
        crate::compat::spawn_detached(async move {
            while let Some(ev) = handle.recv().await {
                let mapped = match ev {
                    MsExecEvent::Started { .. } => continue, // skip
                    MsExecEvent::Stdout(b) => ExecEvent::Stdout(b),
                    MsExecEvent::Stderr(b) => ExecEvent::Stderr(b),
                    MsExecEvent::Exited { code } => ExecEvent::Exit { code, signal: None },
                    MsExecEvent::Failed(f) => {
                        warn!(
                            stage = "exec_failed",
                            error = ?f,
                            "microsandbox exec failed to start"
                        );
                        ExecEvent::Exit {
                            code: -1,
                            signal: None,
                        }
                    }
                };
                if tx.send(mapped).await.is_err() {
                    return;
                }
            }
        });

        Ok(ExecHandle {
            output: Box::pin(receiver_stream(rx)),
        })
    }

    async fn write_file(&self, path: &str, bytes: Bytes) -> RuntimeResult<()> {
        let guard = self.sandbox.lock().await;
        let sandbox = guard
            .as_ref()
            .ok_or_else(|| RuntimeError::Backend("session already terminated".into()))?;
        sandbox
            .fs()
            .write(path, bytes.as_ref())
            .await
            .map_err(|e| RuntimeError::Backend(format!("write_file {path}: {e}")))
    }

    async fn read_file(&self, path: &str) -> RuntimeResult<Bytes> {
        let guard = self.sandbox.lock().await;
        let sandbox = guard
            .as_ref()
            .ok_or_else(|| RuntimeError::Backend("session already terminated".into()))?;
        sandbox
            .fs()
            .read(path)
            .await
            .map_err(|e| RuntimeError::Backend(format!("read_file {path}: {e}")))
    }

    async fn terminate(&self) -> RuntimeResult<()> {
        let mut guard = self.sandbox.lock().await;
        let Some(sandbox) = guard.take() else {
            return Ok(());
        };
        match sandbox.stop_and_wait().await {
            Ok(status) => {
                info!(
                    stage = "microsandbox_stopped",
                    exit_code = status.code().unwrap_or(-1),
                    "microsandbox VM stopped cleanly"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    stage = "microsandbox_stop_failed",
                    error = %e,
                    "Failed to stop microsandbox VM"
                );
                Err(RuntimeError::Backend(format!("stop_and_wait: {e}")))
            }
        }
    }
}

fn receiver_stream<T>(mut rx: mpsc::Receiver<T>) -> impl Stream<Item = T> + Send
where
    T: Send + 'static,
{
    futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
}
