//! The pure, sync swarm admission core: lane admission against the dual
//! guardrail, and branch-slug sanitization. No async, no I/O — exhaustively
//! unit-testable, mirroring `fleet_core::gate`.

/// Sanitize a planner-chosen lane title into a git-ref-safe, length-bounded
/// slug. Uniqueness comes from the `{idx}-` prefix the caller adds, so this is
/// purely cosmetic and never the uniqueness key.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in title.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(32).collect();
    let trimmed = trimmed.trim_matches('-').to_string();
    if trimmed.is_empty() { "lane".into() } else { trimmed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sanitizes_charset_length_and_empty() {
        assert_eq!(slug("Add Auth!!"), "add-auth");
        assert_eq!(slug("  spaced  out  "), "spaced-out");
        assert_eq!(slug("🚀🚀🚀"), "lane");          // non-ascii → fallback
        assert_eq!(slug(""), "lane");
        assert_eq!(slug(&"x".repeat(100)).len(), 32); // truncated
    }
}
