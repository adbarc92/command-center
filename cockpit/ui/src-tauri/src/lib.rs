use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

// LANE-A → SHELL contract: the dashboard's read-seam Tauri commands (§6.1/§6.2).
mod dashboard;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // LANE-A → SHELL contract: register the dashboard read-seam commands so the
        // frontend's `invoke('halyard_status'|'halyard_queue'|'audience_health'|
        // 'audience_posts')` resolve. Additive only — remove nothing here.
        .invoke_handler(tauri::generate_handler![
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
            let sidecar = app.shell().sidecar("fleetd-serve")?;
            let (mut rx, _child) = sidecar.spawn()?;
            tauri::async_runtime::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        CommandEvent::Stdout(b) => {
                            log::info!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
                        }
                        CommandEvent::Stderr(b) => {
                            log::warn!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
                        }
                        CommandEvent::Terminated(t) => log::warn!("fleetd exited: {:?}", t.code),
                        _ => {}
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
