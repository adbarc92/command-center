//! U4 (spec §4, §6): filesystem discovery + raw reads for the `local` dashboard source.
//! No markdown parsing here — the TS adapter parses. Discovery is bounded-recursive,
//! prunes heavy dirs, does NOT follow symlinks, and treats every dir with docs/STATUS.md
//! as a project (nested markers allowed). `roadmap_hash` (SHA-256 over raw bytes) feeds
//! the Phase-2 write-back CAS.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    #[serde(default)]
    pub scan_roots: Vec<String>,
    #[serde(default = "default_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub pins: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}
fn default_depth() -> usize { 5 }

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocalProjectDoc {
    pub project_dir: String,
    pub status_text: Option<String>,
    pub roadmap_text: Option<String>,
    pub roadmap_hash: Option<String>,
    pub is_pinned: bool,
}

const PRUNE: &[&str] = &[".git", "node_modules", "target", "dist"];

fn normalize(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn is_excluded(path: &str, excludes: &[String]) -> bool {
    let p = path.to_lowercase();
    excludes.iter().any(|e| p.contains(&e.replace('\\', "/").to_lowercase()))
}

/// Read a project dir's STATUS.md/ROADMAP.md into a doc (raw text; hash over raw bytes).
fn read_project(dir: &Path, is_pinned: bool) -> LocalProjectDoc {
    let status_text = std::fs::read_to_string(dir.join("docs/STATUS.md")).ok();
    let roadmap_bytes = std::fs::read(dir.join("ROADMAP.md")).ok();
    let roadmap_hash = roadmap_bytes.as_ref().map(|b| {
        let mut h = Sha256::new();
        h.update(b);
        format!("{:x}", h.finalize())
    });
    let roadmap_text = roadmap_bytes.and_then(|b| String::from_utf8(b).ok());
    LocalProjectDoc {
        project_dir: normalize(dir),
        status_text,
        roadmap_text,
        roadmap_hash,
        is_pinned,
    }
}

/// Bounded-recursive discovery: every dir containing docs/STATUS.md is a project
/// (including nested). Prunes PRUNE dirs, skips symlinks, respects excludes + depth.
fn discover(root: &Path, max_depth: usize, excludes: &[String], out: &mut Vec<PathBuf>) {
    let walker = walkdir::WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && PRUNE.contains(&name.as_ref()))
                && !is_excluded(&normalize(e.path()), excludes)
        });
    for entry in walker.flatten() {
        if entry.file_type().is_dir() && entry.path().join("docs/STATUS.md").is_file() {
            out.push(entry.path().to_path_buf());
        }
    }
}

#[tauri::command]
pub fn scan_local_projects(config: ScanConfig) -> Result<Vec<LocalProjectDoc>, String> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for root in &config.scan_roots {
        discover(Path::new(root), config.max_depth, &config.excludes, &mut dirs);
    }
    let discovered: std::collections::HashSet<String> =
        dirs.iter().map(|p| normalize(p)).collect();

    let mut docs: Vec<LocalProjectDoc> = dirs.iter().map(|d| read_project(d, false)).collect();
    // Pins: included even without a marker; skip a pin already auto-discovered.
    for pin in &config.pins {
        let norm = pin.replace('\\', "/");
        if discovered.contains(&norm) {
            continue;
        }
        docs.push(read_project(Path::new(pin), true));
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn make_project(root: &Path, rel: &str, status: &str) {
        let dir = root.join(rel);
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("docs/STATUS.md"), status).unwrap();
    }

    #[test]
    fn discovers_nested_marked_projects_and_prunes() {
        let root = tmp();
        make_project(&root, "alpha", "---\nstage: Build\n---\n");
        make_project(&root, "mono/services/api", "---\nstage: Spec\n---\n"); // depth-3 nested
        fs::create_dir_all(root.join("node_modules/pkg/docs")).unwrap();
        fs::write(root.join("node_modules/pkg/docs/STATUS.md"), "x").unwrap(); // must be pruned
        let cfg = ScanConfig { scan_roots: vec![root.to_string_lossy().into()], max_depth: 5, pins: vec![], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        let dirs: Vec<&str> = docs.iter().map(|d| d.project_dir.as_str()).collect();
        assert!(dirs.iter().any(|d| d.ends_with("/alpha")));
        assert!(dirs.iter().any(|d| d.ends_with("/mono/services/api")));
        assert!(!dirs.iter().any(|d| d.contains("node_modules")));
    }

    #[test]
    fn hashes_roadmap_over_raw_bytes() {
        let root = tmp();
        make_project(&root, "beta", "---\nstage: Build\n---\n");
        fs::write(root.join("beta/ROADMAP.md"), "## X\n<!-- cc-item id=x status=open -->\n").unwrap();
        let cfg = ScanConfig { scan_roots: vec![root.to_string_lossy().into()], max_depth: 5, pins: vec![], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        let beta = docs.iter().find(|d| d.project_dir.ends_with("/beta")).unwrap();
        assert!(beta.roadmap_hash.as_ref().unwrap().len() == 64); // hex sha256
    }

    #[test]
    fn roadmap_hash_is_over_raw_bytes_even_when_not_utf8() {
        let root = tmp();
        make_project(&root, "gamma", "---\nstage: Build\n---\n");
        std::fs::write(root.join("gamma/ROADMAP.md"), [0x23, 0x20, 0xff, 0xfe, 0x0a]).unwrap();
        let cfg = ScanConfig { scan_roots: vec![root.to_string_lossy().into()], max_depth: 5, pins: vec![], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        let gamma = docs.iter().find(|d| d.project_dir.ends_with("/gamma")).unwrap();
        assert!(gamma.roadmap_hash.as_ref().unwrap().len() == 64); // hash computed despite invalid UTF-8
        assert!(gamma.roadmap_text.is_none()); // decode fails, proving hash path is independent of decode
    }

    #[test]
    fn pinned_unmarked_dir_is_included() {
        let root = tmp();
        let pin = root.join("pinned-no-marker");
        fs::create_dir_all(&pin).unwrap();
        let cfg = ScanConfig { scan_roots: vec![], max_depth: 5, pins: vec![pin.to_string_lossy().into()], excludes: vec![] };
        let docs = scan_local_projects(cfg).unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].is_pinned);
        assert!(docs[0].status_text.is_none());
    }
}
