use harness_conformance::{run_all, CaseOutcome, KitConfig};
use std::time::Duration;

fn fake_config(mode: &str) -> KitConfig {
    let mut cfg = KitConfig::new(vec![env!("CARGO_BIN_EXE_harness-fake").to_string()]);
    cfg.env = vec![
        ("HARNESS_FAKE_MODE".into(), mode.into()),
        ("HARNESS_FAKE_STEP_MS".into(), "20".into()),
    ];
    cfg.wall_clock = Duration::from_secs(10);
    cfg.grace = Duration::from_secs(2);
    cfg
}

#[test]
fn conformant_fake_passes_every_case() {
    let reports = run_all(&fake_config("conformant"));
    assert!(!reports.is_empty());
    for report in &reports {
        assert_eq!(
            report.outcome,
            CaseOutcome::Pass,
            "case `{}` did not pass",
            report.name
        );
    }
}
