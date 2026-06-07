//! `serve` — start the fleetd HTTP/WS server. The cockpit (and `curl`) talk to
//! this. Defaults to 127.0.0.1:8787; override with CC_ADDR. Persists to CC_DB
//! (default ./fleet.db).

use fleetd::local_docker::LocalDockerRunner;
use fleetd::server::{reconcile_on_startup, router, AppState};
use fleetd::store::Store;
use std::sync::{Arc, Mutex};

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
    reconcile_on_startup(&state, &LocalDockerRunner::new(image)).await;

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("fleetd listening on http://{addr} (db: {db})");
    axum::serve(listener, app).await.unwrap();
}
