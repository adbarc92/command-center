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
            vec![Action::HaltNoContainer("x".into()), Action::HaltNoContainer("y".into())]
        );
    }
}
