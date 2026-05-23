//! Microsandbox runtime adapter.
//!
//! Wraps the [`microsandbox`] SDK so each Hadrian session corresponds to
//! one local microVM. Microsandbox runs in-process (no daemon, no
//! endpoint) — booting a VM, streaming command I/O, and tearing it down
//! all happen via direct Rust calls.

#![cfg(feature = "runtime-microsandbox")]

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::Stream;
use microsandbox::{ExecEvent as MsExecEvent, NetworkPolicy, Sandbox};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    config::MicrosandboxConfig,
    runtimes::{
        ExecEvent, ExecHandle, ExecRequest, NetworkMode, RuntimeCapabilities, RuntimeError,
        RuntimeResult, SessionHandle, SessionSpec, ShellRuntime, ShellSession,
    },
};

/// `ShellRuntime` implementation backed by microsandbox microVMs.
///
/// Each `start_session` boots a fresh VM — the previous pre-warm pool
/// was removed because pooled VMs were reused across tenants without a
/// filesystem reset, opening a cross-tenant data leak window for any
/// session whose `SessionSpec` didn't force a fresh VM. Cold-start
/// cost is paid per request; revisit pooling later only if we add
/// per-tenant keying or snapshot-restore.
pub struct MicrosandboxRuntime {
    config: MicrosandboxConfig,
}

impl MicrosandboxRuntime {
    pub fn new(config: MicrosandboxConfig) -> Self {
        Self { config }
    }
}

fn cpus_for_sdk(cpus: u32) -> u8 {
    // SDK takes u8 (0..=255). Anything beyond 255 vCPUs is operator
    // error; clamp rather than silently wrapping.
    cpus.min(u8::MAX as u32) as u8
}

#[async_trait]
impl ShellRuntime for MicrosandboxRuntime {
    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            passthrough_only: false,
            client_executes: false,
            // Microsandbox supports placeholder substitution at the TLS
            // proxy via SecretBuilder so the model never sees the raw
            // secret value.
            secret_injection: true,
            // Hostname-based egress allowlists are first-class in the
            // microsandbox 0.4 NetworkPolicy builder (`allow_domains` /
            // `allow_domain_suffixes`). With those rules wired up, this
            // runtime matches opensandbox's deny-by-default + explicit
            // allowlist semantics.
            egress_allowlist: true,
            skill_mount: true,
            file_io: true,
            network_isolation_modes: vec![
                NetworkMode::None,
                NetworkMode::AllowList,
                NetworkMode::Full,
            ],
            max_session_duration: None,
        }
    }

    async fn start_session(&self, spec: SessionSpec) -> RuntimeResult<SessionHandle> {
        let session_id = spec
            .session_id_hint
            .unwrap_or_else(|| format!("hadrian-{}", Uuid::new_v4()));

        let cpus: u8 = spec
            .cpu_limit
            .map(|c| c.ceil().clamp(1.0, u8::MAX as f64) as u8)
            .unwrap_or_else(|| cpus_for_sdk(self.config.cpus));
        let memory_mb: u32 = spec
            .mem_limit_bytes
            .map(|b| (b / (1024 * 1024)) as u32)
            .unwrap_or(self.config.memory_mb);

        // Translate Hadrian's `allow_hosts` patterns into a concrete
        // `NetworkPolicy`. The shapes we accept (from the OpenAI shell
        // tool spec via `resolve_shell_environment`):
        //   - `[]`        → no egress (deny-all). Matches opensandbox.
        //   - `["*"]`     → public internet + DNS to the gateway resolver.
        //                   Other private egress and the IMDS endpoint at
        //                   169.254.169.254 stay blocked. See
        //                   `build_network_policy`.
        //   - exact name  → `allow_domains([name])`.
        //   - `*.suffix`  → `allow_domain_suffixes([suffix])`.
        // Per-secret allow-host rules still apply on top via
        // `secret().allow_host()` below.
        let policy = build_network_policy(&spec.egress_policy.allow_hosts);

        info!(
            stage = "microsandbox_starting",
            session_id = %session_id,
            image = %self.config.image,
            cpus,
            memory_mb,
            egress_hosts = ?spec.egress_policy.allow_hosts,
            "Creating microsandbox VM"
        );

        let mut builder = Sandbox::builder(session_id.clone())
            .image(self.config.image.clone())
            .cpus(cpus)
            .memory(memory_mb)
            .replace()
            .network(|n| n.policy(policy));

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

/// Build a [`NetworkPolicy`] from a Hadrian `allow_hosts` list (already
/// intersected with the operator allowlist by `resolve_shell_environment`).
///
/// Rules:
/// - Empty list → deny-all in both directions. The runtime defers to
///   `NetworkPolicy::none()`; matches opensandbox's default semantics
///   so the model can't quietly assume the public internet is reachable
///   when no per-request `network_policy` was supplied.
/// - `"*"` anywhere in the list → public internet on any port plus DNS to
///   the per-sandbox gateway resolver (a private/CGN address `public_only()`
///   would otherwise refuse). All other private/loopback/link-local/metadata
///   egress stays denied, so SSRF surface is limited to name resolution.
/// - Otherwise → deny by default, allow only the supplied hostnames /
///   `*.suffix` patterns. Patterns are normalized: a leading `*.` is
///   recognised as a `Destination::DomainSuffix`; everything else is
///   treated as an exact `Destination::Domain`.
fn build_network_policy(allow_hosts: &[String]) -> NetworkPolicy {
    if allow_hosts.is_empty() {
        return NetworkPolicy::none();
    }
    if allow_hosts.iter().any(|h| h == "*") {
        // Public internet on any port, plus DNS to the per-sandbox gateway
        // resolver. `public_only()` alone denies the gateway (a private/CGN
        // address in the 100.96.0.0/11 pool), so name resolution is refused;
        // the explicit `Host` rule on :53 reaches the resolver without
        // opening any other private/LAN egress. IMDS (169.254.169.254) and
        // loopback stay blocked.
        return NetworkPolicy::builder()
            .default_deny()
            .egress(|e| e.allow_public())
            .egress(|e| e.allow_host().udp().tcp().port(53))
            .build()
            .unwrap_or_else(|err| {
                warn!(
                    stage = "microsandbox_public_policy_build_failed",
                    error = %err,
                    "public egress policy build failed; falling back to public_only"
                );
                NetworkPolicy::public_only()
            });
    }

    let mut domains: Vec<String> = Vec::new();
    let mut suffixes: Vec<String> = Vec::new();
    for h in allow_hosts {
        let h = h.trim().trim_end_matches('.');
        if let Some(rest) = h.strip_prefix("*.") {
            // Strip a redundant trailing dot the operator might have
            // typed (`*.example.com.`).
            suffixes.push(rest.trim_end_matches('.').to_string());
        } else {
            domains.push(h.to_string());
        }
    }

    // Egress is the only thing this allowlist gates. We don't publish
    // inbound ports for shell-tool VMs, so denying ingress is benign
    // (avoids the extra `Action` import path).
    let policy = NetworkPolicy::builder().default_deny().egress(|e| {
        if !suffixes.is_empty() {
            e.allow_domain_suffixes(suffixes.iter().cloned());
        }
        if !domains.is_empty() {
            e.allow_domains(domains.iter().cloned());
        }
        e
    });
    match policy.build() {
        Ok(p) => p,
        Err(err) => {
            // Builder errors only happen on invalid patterns (e.g. an
            // empty domain string). Fall back to deny-all rather than
            // silently allowing more than the operator asked for.
            warn!(
                stage = "microsandbox_policy_build_failed",
                error = %err,
                hosts = ?allow_hosts,
                "NetworkPolicy build failed; falling back to deny-all"
            );
            NetworkPolicy::none()
        }
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
        // Reject path traversal: treat the SkillService output as
        // untrusted on this code path. `..` or absolute prefixes
        // would let one skill clobber arbitrary paths inside the VM.
        let safe_rel = sanitize_skill_relative_path(&file.relative_path)?;
        let full_path = join_paths(&skill.mount_path, &safe_rel);
        // Ensure parent directory exists.
        if let Some(parent) = parent_of(&full_path)
            && parent != skill.mount_path
        {
            // microsandbox returns an AlreadyExists-style error if
            // the directory is already there; log other failures so
            // we don't silently swallow real problems on `write`
            // later. Per CLAUDE.md memory: prefer correctness + debug
            // logs over silent error swallowing.
            if let Err(e) = fs.mkdir(&parent).await {
                let msg = e.to_string();
                let already_exists = msg.contains("AlreadyExists")
                    || msg.contains("already exists")
                    || msg.contains("EEXIST");
                if !already_exists {
                    debug!(
                        path = %parent,
                        error = %msg,
                        "Non-AlreadyExists mkdir error inside sandbox (continuing)"
                    );
                }
            }
        }
        fs.write(&full_path, file.content.as_ref())
            .await
            .map_err(|e| RuntimeError::Backend(format!("write {full_path}: {e}")))?;
    }
    Ok(())
}

/// Reject path traversal in skill-file relative paths. Returns the
/// cleaned path (leading slashes stripped) or a `RuntimeError::Backend`
/// on any `..` segment, absolute prefix, or non-normal component.
fn sanitize_skill_relative_path(rel: &str) -> RuntimeResult<String> {
    use std::path::{Component, Path};
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(RuntimeError::Backend(format!(
            "skill relative_path must not be absolute: {rel}"
        )));
    }
    let mut cleaned = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => cleaned.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(RuntimeError::Backend(format!(
                    "skill relative_path must not contain traversal: {rel}"
                )));
            }
        }
    }
    cleaned.to_str().map(str::to_string).ok_or_else(|| {
        RuntimeError::Backend(format!("skill relative_path is not valid UTF-8: {rel}"))
    })
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

/// Internal sentinel returned by the per-iteration `select!` inside
/// the exec drain task: a delivered SDK event, the upstream channel
/// closing cleanly, or the per-command deadline elapsing.
enum DrainOutcome {
    Event(MsExecEvent),
    Eof,
    Timeout,
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
        //
        // Per-call random terminator: a fixed marker is forge-able
        // (an adversarial model could embed the literal string in
        // stdin to escape into shell). The UUID makes collision
        // probability ~2^-122. Single-quoting prevents variable
        // expansion inside the body.
        let script = match cmd.stdin {
            Some(bytes) => {
                let stdin_text = String::from_utf8_lossy(&bytes);
                let terminator = format!("__HADRIAN_STDIN_{}__", uuid::Uuid::new_v4().simple());
                format!(
                    "{} <<'{terminator}'\n{stdin_text}\n{terminator}",
                    cmd.command
                )
            }
            None => cmd.command.clone(),
        };

        let mut handle = sandbox
            .shell_stream(script)
            .await
            .map_err(|e| RuntimeError::Backend(format!("microsandbox shell_stream: {e}")))?;

        // Honor the per-command timeout in `ExecRequest`. The SDK's
        // `shell_stream` doesn't apply the `ExecOptionsBuilder.timeout`
        // value to the streaming path (`exec_stream_inner` discards it
        // — only `exec_with_opts` enforces it after collecting), so we
        // wrap the drain loop with `tokio::time::timeout` ourselves and
        // call the SDK's `handle.kill()` on elapse. Exit code 124 is
        // the coreutils convention for "command timed out."
        //
        // Stash a clone of the underlying `ExecHandle` first so the kill
        // path doesn't fight the drain task for ownership.
        let kill_after = cmd.timeout;
        let exec_id = handle.id();
        // Bridge the SDK's UnboundedReceiver<MsExecEvent> into our
        // Stream<ExecEvent>. We can't move `handle` directly across a
        // .await in a stream::unfold without owning it, so we drain it
        // into our own channel via a detached task.
        let (tx, rx) = mpsc::channel::<ExecEvent>(32);
        crate::compat::spawn_detached(async move {
            let deadline = kill_after.map(|d| tokio::time::Instant::now() + d);
            let mut exit_seen = false;
            loop {
                let next = if let Some(dl) = deadline {
                    tokio::select! {
                        ev = handle.recv() => ev.map(DrainOutcome::Event)
                            .unwrap_or(DrainOutcome::Eof),
                        _ = tokio::time::sleep_until(dl) => DrainOutcome::Timeout,
                    }
                } else {
                    match handle.recv().await {
                        Some(ev) => DrainOutcome::Event(ev),
                        None => DrainOutcome::Eof,
                    }
                };

                match next {
                    DrainOutcome::Event(ev) => {
                        let mapped = match ev {
                            MsExecEvent::Started { .. } => continue, // skip
                            MsExecEvent::Stdout(b) => ExecEvent::Stdout(b),
                            MsExecEvent::Stderr(b) => ExecEvent::Stderr(b),
                            MsExecEvent::Exited { code } => {
                                exit_seen = true;
                                ExecEvent::Exit { code, signal: None }
                            }
                            MsExecEvent::Failed(f) => {
                                warn!(
                                    stage = "exec_failed",
                                    error = ?f,
                                    exec_id = %exec_id,
                                    "microsandbox exec failed to start"
                                );
                                exit_seen = true;
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
                    DrainOutcome::Eof => return,
                    DrainOutcome::Timeout => {
                        warn!(
                            stage = "exec_timeout",
                            exec_id = %exec_id,
                            timeout_ms = kill_after.map(|d| d.as_millis() as u64),
                            "microsandbox shell command exceeded timeout; killing"
                        );
                        // Best-effort kill of the underlying process.
                        if let Err(e) = handle.kill().await {
                            warn!(
                                stage = "exec_kill_failed",
                                exec_id = %exec_id,
                                error = ?e,
                                "Failed to kill timed-out microsandbox exec"
                            );
                        }
                        // Brief grace period to collect anything the
                        // process emitted between the timeout firing
                        // and the kill landing. After that, emit our
                        // own `Exit { code: 124 }` so the caller's
                        // stream terminates promptly.
                        let grace =
                            tokio::time::Instant::now() + std::time::Duration::from_millis(500);
                        loop {
                            let res = tokio::select! {
                                ev = handle.recv() => Some(ev),
                                _ = tokio::time::sleep_until(grace) => None,
                            };
                            match res {
                                Some(Some(MsExecEvent::Stdout(b))) => {
                                    let _ = tx.send(ExecEvent::Stdout(b)).await;
                                }
                                Some(Some(MsExecEvent::Stderr(b))) => {
                                    let _ = tx.send(ExecEvent::Stderr(b)).await;
                                }
                                Some(Some(MsExecEvent::Exited { code })) => {
                                    let _ = tx.send(ExecEvent::Exit { code, signal: None }).await;
                                    return;
                                }
                                Some(Some(_)) => continue,
                                // Stream closed or grace elapsed.
                                Some(None) | None => break,
                            }
                        }
                        if !exit_seen {
                            let _ = tx
                                .send(ExecEvent::Exit {
                                    code: 124,
                                    signal: None,
                                })
                                .await;
                        }
                        return;
                    }
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
