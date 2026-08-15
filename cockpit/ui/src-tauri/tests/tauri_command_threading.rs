//! Guard test: no new `#[tauri::command]` may block the main event-loop thread.
//!
//! A *synchronous* `#[tauri::command]` is invoked on Tauri's main event-loop thread, so any
//! blocking work inside it freezes the whole window. This project has now hit that defect
//! twice:
//!
//!   1. The embedding commands (P3) — fixed by making them `async`.
//!   2. `plugin_launch` (Phase-6 smoke item 1.5) — a `docker compose build` with a 20-minute
//!      budget ran inline, freezing the UI from the tab click until the stack came up. Fixed
//!      in `db74a47` by dispatching the start sequence to a dedicated OS thread. See
//!      `spikes/SPIKE-RESULTS.md` → "Smoke run 1".
//!
//! Both were caught by a human watching a window, and both passed every automated gate on the
//! way in. This test makes the *class* mechanical. A command is acceptable when it is either:
//!
//!   * `async fn` — Tauri runs it on a worker, off the event loop; or
//!   * dispatching — it hands its work to a thread and returns.
//!
//! Anything else must be named in `MAIN_THREAD_DEBT`.
//!
//! **This is a ratchet, not a clean bill of health.** The list below is the debt that already
//! existed when the guard was written; the entries marked UNBOUNDED are genuine freeze risks
//! that simply have not been fixed yet. The value is that a *new* blocking command fails here
//! immediately, instead of waiting to be discovered as a frozen window six weeks later.
//!
//! It also pins the 1.5 fix from the Rust side: delete the `thread::spawn` from
//! `plugin_launch` and this test goes red, because the command becomes sync, non-dispatching,
//! and absent from the list.

use std::fs;
use std::path::{Path, PathBuf};

/// Synchronous commands that predate this guard, and why each is tolerated.
///
/// Do NOT add an entry here to make a red build green — make the command `async` instead, or
/// dispatch its work to a thread. An entry is a promise that someone looked and decided the
/// freeze risk was acceptable.
const MAIN_THREAD_DEBT: &[(&str, &str)] = &[
    (
        "plugins_list",
        "BOUNDED: read_dir over at most two shallow roots, one small JSON parse per entry. \
         No network, no subprocess. Cost is a few milliseconds.",
    ),
    (
        "halyard_status",
        "UNBOUNDED: spawns the halyard CLI via Command::output() with NO timeout. A hung or \
         slow CLI freezes the window for as long as it hangs — the same shape as the 1.5 \
         defect, just with a different blocking call.",
    ),
    (
        "halyard_queue",
        "UNBOUNDED: same call path as halyard_status, same risk.",
    ),
    (
        "scan_local_projects",
        "UNBOUNDED: recursive walkdir over operator-configured scan roots, plus a file read \
         per project found. Cost scales with the size of the operator's disk, not with \
         anything this repo controls.",
    ),
];

#[derive(Debug)]
struct TauriCommand {
    name: String,
    is_async: bool,
    dispatches: bool,
    file: String,
    line: usize,
}

impl TauriCommand {
    /// Runs off the event loop one way or the other.
    fn is_safe(&self) -> bool {
        self.is_async || self.dispatches
    }
}

#[test]
fn no_new_command_blocks_the_main_event_loop() {
    let commands = collect_commands();

    assert!(
        !commands.is_empty(),
        "found no #[tauri::command] at all — the scanner is broken, not the code"
    );

    let offenders: Vec<&TauriCommand> = commands
        .iter()
        .filter(|c| !c.is_safe())
        .filter(|c| !MAIN_THREAD_DEBT.iter().any(|(name, _)| *name == c.name))
        .collect();

    assert!(
        offenders.is_empty(),
        "\n\
         These #[tauri::command] functions are synchronous and do not dispatch their work, so\n\
         they run on Tauri's main event-loop thread. If any of them blocks — a subprocess, an\n\
         HTTP call, a filesystem walk — the entire window freezes for that long.\n\n\
         {}\n\n\
         Fix by making the command `async fn` (Tauri then runs it on a worker), or by handing\n\
         the blocking work to a thread and returning immediately, as `plugin_launch` does.\n\
         Only add to MAIN_THREAD_DEBT in tests/tauri_command_threading.rs if you have looked\n\
         at the work it does and concluded the freeze risk is genuinely acceptable.\n",
        offenders
            .iter()
            .map(|c| format!("  - {} ({}:{})", c.name, c.file, c.line))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Keeps the debt list honest: an entry that no longer names a sync command is stale, either
/// because the command was fixed (delete the entry and keep the win) or renamed/removed.
#[test]
fn the_debt_list_has_no_stale_entries() {
    let commands = collect_commands();

    let stale: Vec<&str> = MAIN_THREAD_DEBT
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| {
            !commands
                .iter()
                .any(|c| c.name == *name && !c.is_safe())
        })
        .collect();

    assert!(
        stale.is_empty(),
        "\n\
         MAIN_THREAD_DEBT names commands that are no longer synchronous main-thread commands:\n\
         {}\n\n\
         If one was fixed, delete its entry so the ratchet tightens. If one was renamed or\n\
         removed, delete its entry too.\n",
        stale
            .iter()
            .map(|n| format!("  - {n}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn collect_commands() -> Vec<TauriCommand> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();

    let mut out = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&src)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        scan_source(&rel, &text, &mut out);
    }
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn scan_source(file: &str, text: &str, out: &mut Vec<TauriCommand>) {
    let lines: Vec<&str> = text.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[tauri::command]" {
            continue;
        }
        // The signature is the next line declaring a fn (attributes may sit between).
        let Some(sig_idx) = (i + 1..lines.len()).find(|&j| lines[j].contains("fn ")) else {
            continue;
        };
        let sig = lines[sig_idx];
        let Some(name) = fn_name(sig) else { continue };

        out.push(TauriCommand {
            name,
            is_async: sig.contains("async fn"),
            dispatches: body_after(&lines, sig_idx).contains("thread::spawn"),
            file: file.to_string(),
            line: sig_idx + 1,
        });
    }
}

fn fn_name(sig: &str) -> Option<String> {
    let after = sig.split("fn ").nth(1)?;
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The function body: from the first `{` at or after the signature to its matching `}`.
/// Brace counting is naive about braces inside strings and comments, which is fine here —
/// the only question asked of the result is whether it contains `thread::spawn`.
fn body_after(lines: &[&str], sig_idx: usize) -> String {
    let rest = lines[sig_idx..].join("\n");
    let Some(start) = rest.find('{') else {
        return String::new();
    };
    let mut depth = 0i32;
    for (offset, ch) in rest[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return rest[start..start + offset + 1].to_string();
                }
            }
            _ => {}
        }
    }
    rest[start..].to_string()
}
