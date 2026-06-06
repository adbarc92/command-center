//! `serve` — start the fleetd HTTP/WS server. The cockpit (and `curl`) talk to
//! this. Defaults to 127.0.0.1:8787; override with CC_ADDR.

use fleetd::server::{router, AppState};

#[tokio::main]
async fn main() {
    // Load a .env from the cwd (or a parent) if present — see .env.example.
    let _ = dotenvy::dotenv();

    let addr = std::env::var("CC_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let app = router(AppState::default());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("fleetd listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
