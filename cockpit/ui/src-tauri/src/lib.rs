use tauri::Manager;

mod plugins;
// LANE-A → SHELL contract: the dashboard's read-seam Tauri commands (§6.1/§6.2).
mod dashboard;
// LANE-B → HOST: fleetd-serve sidecar supervisor (health-gate / restart / kill).
mod sidecar;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(plugins::manager::PluginManager::default())
        // LANE-B → HOST: holds the live fleetd-serve child so the shutdown hook
        // can kill it (no orphaned sidecar) and the supervisor can restart it.
        .manage(sidecar::SidecarSupervisor::default())
        // LANE-A → SHELL contract: register the dashboard read-seam commands so the
        // frontend's `invoke('halyard_status'|'halyard_queue'|'audience_health'|
        // 'audience_posts')` resolve. Additive only — remove nothing here.
        .invoke_handler(tauri::generate_handler![
            plugins::manager::plugins_list,
            plugins::manager::plugin_launch,
            dashboard::halyard_status,
            dashboard::halyard_queue,
            dashboard::audience_health,
            dashboard::audience_posts,
        ])
        .setup(|app| {
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
                api.prevent_exit();
                // LANE-B → HOST: stop the fleetd-serve sidecar first so the
                // supervisor doesn't respawn it as we tear down, and no
                // orphaned process is left behind.
                app_handle
                    .state::<sidecar::SidecarSupervisor>()
                    .shutdown();
                let mgr = app_handle.state::<plugins::manager::PluginManager>();
                mgr.stop_all_owned(30_000); // total budget; kept under the OS force-kill ceiling
                app_handle.exit(0);
            }
        });
}
