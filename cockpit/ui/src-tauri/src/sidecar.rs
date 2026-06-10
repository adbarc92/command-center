//! Sidecar supervisor for the bundled `fleetd-serve` daemon.
//!
//! The cockpit talks to fleetd over HTTP/WS on `127.0.0.1:8787`. This module
//! owns that process for the whole app lifetime and guarantees three things the
//! prior bare `spawn()` did not:
//!
//! - **Health-gate**: after (re)spawn we poll `GET /health` until it answers OK,
//!   then emit `fleetd://status { status: "ready" }` so the UI can lift any
//!   "starting up" gate. (`App.svelte` also polls `/health` directly, so the
//!   event is an optimisation, not a hard dependency.)
//! - **Restart on crash**: if the child exits while the app is still running, we
//!   respawn it (with a small backoff) and re-gate on health.
//! - **Kill on close**: the supervisor holds the `CommandChild` and kills it on
//!   `ExitRequested`, leaving no orphaned `fleetd-serve` process.
//!
//! The supervisor lives entirely in Rust (a Tauri-managed `SidecarSupervisor`
//! plus a background async task) so it survives webview reloads — a JS-side
//! babysitter would be torn down on every reload.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

/// fleetd's bound address; mirrors `crates/fleetd/src/bin/serve.rs` (CC_ADDR
/// default). The supervisor health-gates against `http://{ADDR}/health`.
const FLEETD_ADDR: &str = "127.0.0.1:8787";

/// Event channel the UI can subscribe to for sidecar lifecycle transitions.
const STATUS_EVENT: &str = "fleetd://status";

/// How long to wait for `/health` to come up after a (re)spawn before giving up
/// on this attempt and restarting.
const HEALTH_GATE_TIMEOUT: Duration = Duration::from_secs(20);
/// Poll interval while waiting for the health gate.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(300);
/// Backoff between a crash and the next respawn, so a hard-crashing binary
/// doesn't spin the CPU.
const RESTART_BACKOFF: Duration = Duration::from_secs(2);

/// Lifecycle status surfaced to the UI via [`STATUS_EVENT`].
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    /// Process (re)spawned; health not yet confirmed.
    Starting,
    /// `/health` answered OK — cockpit may talk to fleetd.
    Ready,
    /// Process exited; a restart is pending (or the app is shutting down).
    Down,
}

#[derive(Clone, Serialize)]
struct StatusPayload {
    status: Status,
    /// Set on `Down` transitions when we know why (process exit code).
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
}

/// Tauri-managed handle holding the current child so the shutdown hook can kill
/// it. `shutting_down` flips on `ExitRequested` so the supervisor loop stops
/// respawning and exits cleanly.
#[derive(Default)]
pub struct SidecarSupervisor {
    child: Mutex<Option<CommandChild>>,
    shutting_down: Mutex<bool>,
}

impl SidecarSupervisor {
    fn store_child(&self, child: CommandChild) {
        *self.child.lock().unwrap() = Some(child);
    }

    fn take_child(&self) -> Option<CommandChild> {
        self.child.lock().unwrap().take()
    }

    fn is_shutting_down(&self) -> bool {
        *self.shutting_down.lock().unwrap()
    }

    /// Mark the app as shutting down and kill the running sidecar (idempotent).
    /// Called from the `ExitRequested` handler so no `fleetd-serve` is orphaned.
    pub fn shutdown(&self) {
        *self.shutting_down.lock().unwrap() = true;
        if let Some(child) = self.take_child() {
            // Best-effort: if the child already died this is a no-op error.
            let _ = child.kill();
        }
    }
}

/// Spawn the supervisor background task. Call once from `.setup()`.
///
/// Returns immediately; the task runs on the Tauri async runtime and keeps the
/// sidecar alive (health-gating + restarting) until [`SidecarSupervisor::shutdown`]
/// is invoked.
pub fn spawn_supervisor(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        supervise(app).await;
    });
}

/// The supervisor loop: (re)spawn → pipe logs → health-gate → wait-for-exit →
/// restart, until shutdown is requested.
async fn supervise(app: AppHandle) {
    loop {
        if supervisor_state(&app).is_shutting_down() {
            return;
        }

        emit_status(&app, Status::Starting, None);

        // Spawn the bundled sidecar (binaries/fleetd-serve-<triple>).
        let spawned = match app.shell().sidecar("fleetd-serve") {
            Ok(cmd) => cmd.spawn(),
            Err(e) => {
                log::error!("fleetd: failed to resolve sidecar: {e}");
                emit_status(&app, Status::Down, None);
                if backoff_or_exit(&app).await {
                    return;
                }
                continue;
            }
        };

        let (mut rx, child) = match spawned {
            Ok(pair) => pair,
            Err(e) => {
                log::error!("fleetd: failed to spawn sidecar: {e}");
                emit_status(&app, Status::Down, None);
                if backoff_or_exit(&app).await {
                    return;
                }
                continue;
            }
        };

        // Hand the child to the managed supervisor so the shutdown hook can kill
        // it. If shutdown raced us to here, kill immediately and bail.
        {
            let state = supervisor_state(&app);
            if state.is_shutting_down() {
                let _ = child.kill();
                return;
            }
            state.store_child(child);
        }

        log::info!("fleetd: sidecar spawned; health-gating on http://{FLEETD_ADDR}/health");

        // Pipe logs and watch for termination. The event loop runs until the
        // child's stdout/stderr close (i.e. it exits), giving us `exit_code`.
        let exit_code = pump_events(&app, &mut rx).await;

        // Drop the (now-dead) child handle.
        let _ = supervisor_state(&app).take_child();

        if supervisor_state(&app).is_shutting_down() {
            // Expected exit during shutdown — don't restart, don't alarm the UI.
            log::info!("fleetd: sidecar stopped during shutdown");
            return;
        }

        log::warn!("fleetd: sidecar exited (code {exit_code:?}); restarting");
        emit_status(&app, Status::Down, exit_code);

        if backoff_or_exit(&app).await {
            return;
        }
    }
}

/// Pump stdout/stderr to the log and gate on `/health` on first spawn. Returns
/// the child's exit code (when known) once its event stream closes.
async fn pump_events(
    app: &AppHandle,
    rx: &mut tauri::async_runtime::Receiver<CommandEvent>,
) -> Option<i32> {
    // Kick off the health gate concurrently with log piping so a slow-binding
    // fleetd still streams its logs while we wait.
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if health_gate(&app).await {
                emit_status(&app, Status::Ready, None);
                log::info!("fleetd: health OK — cockpit may connect");
            } else {
                log::warn!("fleetd: health gate timed out; UI will retry via /health poll");
            }
        });
    }

    let mut exit_code = None;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(b) => {
                log::info!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
            }
            CommandEvent::Stderr(b) => {
                log::warn!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
            }
            CommandEvent::Terminated(t) => {
                exit_code = t.code;
                log::warn!("fleetd exited: {:?}", t.code);
            }
            _ => {}
        }
    }
    exit_code
}

/// Poll `GET /health` until it answers 2xx or the gate times out. Returns
/// `true` if fleetd became healthy.
async fn health_gate(app: &AppHandle) -> bool {
    let url = format!("http://{FLEETD_ADDR}/health");
    let deadline = std::time::Instant::now() + HEALTH_GATE_TIMEOUT;
    let client = reqwest::Client::new();

    loop {
        if supervisor_state(app).is_shutting_down() {
            return false;
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return true,
            _ => {}
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }
}

/// Sleep the restart backoff, returning `true` if shutdown was requested while
/// we waited (caller should exit the loop).
async fn backoff_or_exit(app: &AppHandle) -> bool {
    if supervisor_state(app).is_shutting_down() {
        return true;
    }
    tokio::time::sleep(RESTART_BACKOFF).await;
    supervisor_state(app).is_shutting_down()
}

fn supervisor_state(app: &AppHandle) -> tauri::State<'_, SidecarSupervisor> {
    app.state::<SidecarSupervisor>()
}

fn emit_status(app: &AppHandle, status: Status, code: Option<i32>) {
    let _ = app.emit(STATUS_EVENT, StatusPayload { status, code });
}
