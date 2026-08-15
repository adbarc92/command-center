//! Parse token/cost usage from Claude Code's `--output-format stream-json`
//! output. The terminal `{"type":"result", ...}` record carries
//! `total_cost_usd` and a `usage` object (verified in Spike 2).

use crate::runner::Usage;

/// Find the `result` record in newline-delimited JSON stdout and extract usage.
/// Returns `None` if no parseable result record is present.
pub fn parse_usage(stdout: &[String]) -> Option<Usage> {
    for line in stdout {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("result") {
            continue;
        }
        let usage = v.get("usage");
        let tokens_in = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0);
        let tokens_out = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0);
        let cost_usd = v
            .get("total_cost_usd")
            .and_then(|c| c.as_f64())
            .unwrap_or(0.0);
        return Some(Usage {
            tokens_in,
            tokens_out,
            cost_usd,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real result-record shape captured in spikes/SPIKE-RESULTS.md.
    const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.20697875,"usage":{"input_tokens":9457,"cache_creation_input_tokens":25535,"cache_read_input_tokens":0,"output_tokens":4},"modelUsage":{"claude-opus-4-8[1m]":{"costUSD":0.20697875}}}"#;

    #[test]
    fn extracts_cost_and_tokens_from_result_record() {
        let lines = vec![
            r#"{"type":"system","subtype":"init"}"#.to_string(),
            r#"{"type":"assistant"}"#.to_string(),
            RESULT_LINE.to_string(),
        ];
        let u = parse_usage(&lines).expect("usage parsed");
        assert_eq!(u.tokens_in, 9457);
        assert_eq!(u.tokens_out, 4);
        assert!((u.cost_usd - 0.20697875).abs() < 1e-9);
    }

    #[test]
    fn ignores_non_result_and_garbage_lines() {
        let lines = vec![
            "not json at all".to_string(),
            r#"{"type":"assistant","message":{}}"#.to_string(),
        ];
        assert!(parse_usage(&lines).is_none());
    }

    #[test]
    fn tolerates_missing_fields() {
        let lines = vec![r#"{"type":"result"}"#.to_string()];
        let u = parse_usage(&lines).expect("result with no usage still yields zeros");
        assert_eq!(u.tokens_in, 0);
        assert_eq!(u.cost_usd, 0.0);
    }
}
