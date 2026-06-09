//! `fleet-core` — the pure domain of the SP1 fleet engine: the unit lifecycle
//! state machine, the autonomy-ladder tiers, and the evidence-based review gate.
//!
//! This crate has no IO, no Docker, and no async. It is the part of `fleetd`
//! whose decisions must be exhaustively unit-testable. See
//! `docs/superpowers/specs/2026-06-05-command-center-sp1-design.md`.

mod event;
mod gate;
mod phase;
mod tier;
mod transition;

pub use event::{
    ArtifactKind, Command, ErrorScope, Event, IterationKind, LogStream, Severity,
};
pub use gate::{gate_met, GateConfig, ReviewSnapshot};
pub use phase::{Phase, TERMINAL_PHASE_STRS};
pub use tier::Tier;
pub use transition::{transition, Trigger};
