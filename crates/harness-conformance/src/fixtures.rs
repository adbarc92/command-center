//! The synthetic work order the kit sends. It names no real repository.

use harness_protocol::{Caps, Repo, Tier, WorkItem, WorkItemKind, WorkOrder};

pub fn work_order(tier: Tier) -> WorkOrder {
    WorkOrder {
        unit_id: "conformance-1".into(),
        work_item: WorkItem {
            kind: WorkItemKind::RoadmapItem,
            reference: "conformance#kit".into(),
            fingerprint: None,
        },
        tier,
        task: "Conformance kit synthetic unit. Do not modify any repository.".into(),
        repo: Repo {
            url: "https://example.invalid/conformance.git".into(),
            slug: "example/conformance".into(),
            base_branch: "main".into(),
        },
        branch: "agent/conformance-1".into(),
        test_cmd: "true".into(),
        caps: Caps {
            usd: 0.10,
            wall_clock_secs: 30,
            min_review_rounds: 1,
        },
        resume: None,
    }
}
