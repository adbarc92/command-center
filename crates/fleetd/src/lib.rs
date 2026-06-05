//! `fleetd` — the SP1 fleet daemon library: the isolation seam (`Runner`), the
//! host git/PR seam (`Forge`), their fakes, and the lifecycle `driver`. The
//! HTTP/WS server and `LocalDockerRunner` land in later Phase 1/2 steps.

pub mod driver;
pub mod fake;
pub mod forge;
pub mod runner;
