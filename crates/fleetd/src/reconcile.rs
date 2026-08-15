//! Pure startup reconciliation. Decides what to do with persisted non-terminal
//! units and the unit-ids that currently have a running container. Docker-free
//! and exhaustively unit-tested; the server applies the actions via the Runner +
//! Store.

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Non-terminal unit with a running container: reap it + emit a synthetic Halt.
    HaltWithContainer(String),
    /// Non-terminal unit with no container (died Queued/awaiting-permit): Halt only.
    HaltNoContainer(String),
    /// A running container whose unit is not non-terminal (terminal/unknown): reap.
    ReapStray(String),
}

/// `persisted_nonterminal` = unit-ids whose stored phase is not a terminal one.
/// `running` = unit-ids that currently have a container.
pub fn reconcile(persisted_nonterminal: &[String], running: &[String]) -> Vec<Action> {
    let mut out = Vec::new();
    for u in persisted_nonterminal {
        if running.iter().any(|r| r == u) {
            out.push(Action::HaltWithContainer(u.clone()));
        } else {
            out.push(Action::HaltNoContainer(u.clone()));
        }
    }
    for c in running {
        if !persisted_nonterminal.iter().any(|u| u == c) {
            out.push(Action::ReapStray(c.clone()));
        }
    }
    out
}

/// Steady-state (periodic) reconciliation, run **while drivers are live** — unlike
/// [`reconcile`] (startup), which assumes no live drivers exist and so halts every
/// persisted non-terminal unit. Here a unit with a live in-memory driver is
/// *healthy* and must be left completely alone.
///
/// * `persisted_nonterminal` = unit-ids whose stored phase is non-terminal.
/// * `live` = unit-ids that currently have a live in-memory driver (`AppState.units`).
/// * `running` = unit-ids that currently have a running container.
///
/// Emits:
/// * `HaltWithContainer` / `HaltNoContainer` for a **stranded** unit — non-terminal
///   in the store but with no live driver (its driver died / was never resumed).
/// * `ReapStray` for a container whose unit is *neither* live *nor* persisted
///   non-terminal (a genuine orphan).
///
/// A healthy unit (non-terminal **and** live) never appears in the output, even
/// when it owns a running container — this is the safety property that lets the
/// pass run repeatedly on a timer without halting in-flight work.
pub fn reconcile_live(
    persisted_nonterminal: &[String],
    live: &[String],
    running: &[String],
) -> Vec<Action> {
    let mut out = Vec::new();
    // Stranded units: non-terminal in the store but with no live driver.
    for u in persisted_nonterminal {
        if live.iter().any(|l| l == u) {
            continue; // healthy — a live driver owns it; leave it alone.
        }
        if running.iter().any(|r| r == u) {
            out.push(Action::HaltWithContainer(u.clone()));
        } else {
            out.push(Action::HaltNoContainer(u.clone()));
        }
    }
    // Stray containers: running, but the unit is neither live nor persisted
    // non-terminal (a healthy unit's container is skipped by the `live` check).
    for c in running {
        let known = live.iter().any(|l| l == c) || persisted_nonterminal.iter().any(|u| u == c);
        if !known {
            out.push(Action::ReapStray(c.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_quadrants() {
        // a: non-terminal + running; b: non-terminal, no container; c: running stray.
        let actions = reconcile(&["a".into(), "b".into()], &["a".into(), "c".into()]);
        assert!(actions.contains(&Action::HaltWithContainer("a".into())));
        assert!(actions.contains(&Action::HaltNoContainer("b".into())));
        assert!(actions.contains(&Action::ReapStray("c".into())));
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn empty_inputs_yield_nothing() {
        assert!(reconcile(&[], &[]).is_empty());
    }

    #[test]
    fn all_nonterminal_without_containers_are_halted() {
        let actions = reconcile(&["x".into(), "y".into()], &[]);
        assert_eq!(
            actions,
            vec![
                Action::HaltNoContainer("x".into()),
                Action::HaltNoContainer("y".into())
            ]
        );
    }

    #[test]
    fn steady_state_spares_live_units_but_reaps_strays_and_halts_stranded() {
        // u1: healthy — non-terminal, live driver, running container → untouched.
        // u2: stranded — non-terminal, no driver, no container → HaltNoContainer.
        // u3: stray — running container whose unit is neither live nor persisted → ReapStray.
        let actions = reconcile_live(
            &["u1".into(), "u2".into()], // persisted non-terminal
            &["u1".into()],              // live drivers
            &["u1".into(), "u3".into()], // running containers
        );
        assert_eq!(
            actions.len(),
            2,
            "healthy u1 must produce no action: {actions:?}"
        );
        assert!(
            actions.contains(&Action::HaltNoContainer("u2".into())),
            "stranded u2 halted"
        );
        assert!(
            actions.contains(&Action::ReapStray("u3".into())),
            "stray u3 reaped"
        );
        assert!(
            !actions.iter().any(|a| matches!(a,
                Action::HaltWithContainer(x) | Action::HaltNoContainer(x) | Action::ReapStray(x)
                    if x == "u1")),
            "healthy live unit u1 must never be touched"
        );
    }

    #[test]
    fn steady_state_halts_stranded_unit_that_still_has_a_container() {
        // Non-terminal, no live driver, but a container is still up → reap + halt.
        let actions = reconcile_live(&["u9".into()], &[], &["u9".into()]);
        assert_eq!(actions, vec![Action::HaltWithContainer("u9".into())]);
    }

    #[test]
    fn steady_state_no_drift_is_a_no_op() {
        // Every non-terminal unit is live and has its container; nothing to do.
        let actions = reconcile_live(
            &["u1".into(), "u2".into()],
            &["u1".into(), "u2".into()],
            &["u1".into(), "u2".into()],
        );
        assert!(
            actions.is_empty(),
            "steady state with no drift yields no actions: {actions:?}"
        );
    }
}
