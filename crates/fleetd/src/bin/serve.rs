//! `serve` — start the fleetd HTTP/WS server. The cockpit (and `curl`) talk to
//! this. Defaults to 127.0.0.1:8787; override with CC_ADDR. Persists to CC_DB
//! (default ./fleet.db).

use fleetd::local_docker::LocalDockerRunner;
use fleetd::server::{reconcile_on_startup, reconcile_tick, router, AppState};
use fleetd::store::Store;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Load a .env from the cwd (or a parent) if present — see .env.example.
    let _ = dotenvy::dotenv();

    let addr = std::env::var("CC_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let db = std::env::var("CC_DB").unwrap_or_else(|_| "fleet.db".into());
    let store = Store::open(std::path::Path::new(&db)).unwrap_or_else(|e| panic!("open {db}: {e}"));
    let state = AppState::new(Arc::new(Mutex::new(store)));

    // Reap orphan containers + mark stranded units Halted before serving.
    let image = std::env::var("CC_IMAGE").unwrap_or_else(|_| "cc-agent:dev".into());
    reconcile_on_startup(&state, &LocalDockerRunner::new(image.clone())).await;

    // Steady-state reconcile loop: on a timer, reap orphan containers + halt
    // stranded units WITHOUT disturbing units that still have a live driver.
    // CC_RECONCILE_SECS=0 disables it (e.g. for a single-shot / test process).
    let reconcile_secs: u64 = std::env::var("CC_RECONCILE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    if reconcile_secs > 0 {
        let loop_state = state.clone();
        let loop_image = image.clone();
        tokio::spawn(async move {
            let runner = LocalDockerRunner::new(loop_image);
            let mut tick = tokio::time::interval(Duration::from_secs(reconcile_secs));
            tick.tick().await; // consume the immediate first tick — startup already reconciled.
            loop {
                tick.tick().await;
                reconcile_tick(&loop_state, &runner).await;
            }
        });
        println!("reconcile loop: every {reconcile_secs}s");
    }

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("fleetd listening on http://{addr} (db: {db})");
    axum::serve(listener, app).await.unwrap();
}
