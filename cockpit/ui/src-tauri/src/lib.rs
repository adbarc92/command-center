use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// Gate-5 shutdown re-entrancy guard (smoke item 1.9b).
///
/// `AppHandle::exit` re-emits `RunEvent::ExitRequested`. If the handler calls
/// `api.prevent_exit()` unconditionally, the exit it requests re-enters the handler, which
/// prevents it again — so the process never exits and spins the event loop at ~100% of one
/// core with no window. Measured in Smoke run 2: 309 s of CPU burned after a graceful close.
///
/// The FIRST exit request must be prevented, so teardown gets to run. Every later one must be
/// allowed through.
#[derive(Default)]
pub struct ShutdownGuard(AtomicBool);

impl ShutdownGuard {
    /// True exactly once — for the first exit request only.
    pub fn should_prevent_exit(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
}

/// One guard per process; there is exactly one `run()` per process.
static SHUTDOWN_GUARD: ShutdownGuard = ShutdownGuard(AtomicBool::new(false));

mod plugins;
// LANE-A → SHELL contract: the dashboard's read-seam Tauri commands (§6.1/§6.2).
mod dashboard;
// LANE-B → HOST: fleetd-serve sidecar supervisor (health-gate / restart / kill).
mod sidecar;
// U4 (spec §4, §6): filesystem discovery + raw reads for the `local` dashboard source.
mod local_projects;
// PLUGIN RUNTIME (Lane S integration):
//  - `view_plugins`: the `ccplugin://` scheme serving sandboxed view-plugin assets (Lane V).
//  - `embedding`: the app-plugin child-webview show/hide/set_rect commands (Lane A).
mod embedding;
mod view_plugins;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(plugins::manager::PluginManager::default())
        // LANE-B → HOST: holds the live fleetd-serve child so the shutdown hook
        // can kill it (no orphaned sidecar) and the supervisor can restart it.
        .manage(sidecar::SidecarSupervisor::default())
        // PLUGIN RUNTIME (Lane S): warm-pool bookkeeping for app-plugin child webviews.
        .manage(embedding::WebviewPool::default())
        // PLUGIN RUNTIME (Lane S): serve sandboxed view-plugin assets over `ccplugin://`
        // with the plugin-doc CSP + ACAO headers (P4 spike findings). Registered on the
        // builder so it works in BOTH `tauri dev` and a packaged build.
        .register_uri_scheme_protocol(view_plugins::SCHEME, |ctx, req| {
            view_plugins::respond(ctx.app_handle(), req)
        })
        // LANE-A → SHELL contract: register the dashboard read-seam commands so the
        // frontend's `invoke('halyard_status'|'halyard_queue'|'audience_health'|
        // 'audience_posts')` resolve. Additive only — remove nothing here.
        .invoke_handler(tauri::generate_handler![
            plugins::manager::plugins_list,
            plugins::manager::plugin_launch,
            // PLUGIN RUNTIME (Lane S): app-plugin child-webview embedding (Lane A bundle).
            embedding::plugin_show,
            embedding::plugin_hide,
            embedding::plugin_set_rect,
            dashboard::halyard_status,
            dashboard::halyard_queue,
            dashboard::audience_health,
            dashboard::audience_posts,
            local_projects::scan_local_projects,
        ])
        .setup(|app| {
            // LANE-P → HOST: activate the updater runtime. Lane B already wired
            // the `plugins.updater` *config* (endpoints + pubkey) in
            // tauri.conf.json; registering the plugin is what makes the app
            // actually able to check for / install updates at runtime. Desktop
            // only (no updater on mobile). Additive — the proven shutdown path
            // and the rest of the wiring are untouched.
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Babysit the fleetd daemon as a sidecar: the cockpit talks to it on
            // 127.0.0.1:8787. Bundled as binaries/fleetd-serve-<target-triple>.
            // The supervisor (Rust, survives webview reloads) health-gates on
            // /health, restarts on crash, and is killed on window close.
            sidecar::spawn_supervisor(app.handle());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                // `app_handle.exit(0)` below re-emits `ExitRequested`. Preventing that
                // re-entry too would call this handler forever: the process never exits and
                // spins the event loop at ~100% of a core with no window (Gate-5 item 1.9b,
                // measured at 309 s of CPU in Smoke run 2). Let every later request through.
                if !SHUTDOWN_GUARD.should_prevent_exit() {
                    return;
                }
                api.prevent_exit();
                // LANE-B → HOST: stop the fleetd-serve sidecar first so the
                // supervisor doesn't respawn it as we tear down, and no
                // orphaned process is left behind.
                app_handle.state::<sidecar::SidecarSupervisor>().shutdown();
                let mgr = app_handle.state::<plugins::manager::PluginManager>();
                mgr.stop_all_owned(30_000); // total budget; kept under the OS force-kill ceiling
                app_handle.exit(0);
                // LANE-P note — the cosmetic "failed to send message to the
                // webview" line on clean exit is upstream/benign, not ours:
                //   - it is the Display of `Error::FailedToSendMessage`
                //     (tauri-runtime/src/lib.rs), logged via `log::error!("{e}")`
                //     INSIDE wry (tauri-runtime-wry), when a final in-flight
                //     message reaches a webview whose event loop is already
                //     closing — i.e. strictly AFTER this handler reaped the
                //     sidecar above, so the shutdown path is unaffected.
                //   - it only surfaces in DEV: the log plugin is registered
                //     under `cfg!(debug_assertions)` only, so release bundles
                //     have no logger installed and the line never reaches users.
                //   - it is deliberately NOT suppressed: tauri-plugin-log's
                //     `.filter` keys on `log::Metadata` (target + level only, no
                //     message body), so dropping it would mean muting the entire
                //     `tauri_runtime_wry` error target and hiding real runtime
                //     errors. Not worth weakening observability for a dev-only
                //     cosmetic line.
            }
        });
}

#[cfg(test)]
mod shutdown_guard_tests {
    use super::ShutdownGuard;

    /// Pins Gate-5 item 1.9b. `AppHandle::exit` re-emits `ExitRequested`; if that re-entry is
    /// prevented too, the handler calls itself forever and the process spins instead of
    /// exiting. Delete the `swap` in `should_prevent_exit` and this goes red.
    #[test]
    fn only_the_first_exit_request_is_prevented() {
        let guard = ShutdownGuard::default();

        assert!(
            guard.should_prevent_exit(),
            "the first ExitRequested must be prevented so teardown can run"
        );
        assert!(
            !guard.should_prevent_exit(),
            "the exit(0) re-entry must NOT be prevented, or the app never exits"
        );
        assert!(
            !guard.should_prevent_exit(),
            "every later exit request must also be allowed through"
        );
    }
}
