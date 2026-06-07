//! `fleetd` — the SP1 fleet daemon library: the isolation seam (`Runner`), the
//! host git/PR seam (`Forge`), their fakes, and the lifecycle `driver`. The
//! HTTP/WS server and `LocalDockerRunner` land in later Phase 1/2 steps.

pub mod claude_meter;
pub mod driver;
pub mod fake;
pub mod forge;
pub mod gh_forge;
pub mod local_docker;
pub mod reconcile;
pub mod retry;
pub mod runner;
pub mod server;
pub mod steps;
pub mod store;
