use crate::plugins::manifest::Manifest;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

/// Scan each root for `<root>/<plugin>/app-plugin.json`. Later roots override
/// earlier ones on `id` collision (so the user dir wins over packaged).
/// Manifests that fail to parse or fail `validate()` are skipped.
pub fn discover(roots: &[&Path]) -> Vec<DiscoveredPlugin> {
    use std::collections::BTreeMap;
    let mut by_id: BTreeMap<String, DiscoveredPlugin> = BTreeMap::new();
    for root in roots {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => continue, // missing root is fine
        };
        for entry in entries.flatten() {
            let mani_path = entry.path().join("app-plugin.json");
            let text = match std::fs::read_to_string(&mani_path) {
                Ok(t) => t,
                Err(e) => { log::warn!("app-plugins: skipping {} — unreadable: {e}", mani_path.display()); continue }
            };
            let manifest = match Manifest::from_json(&text) {
                Ok(m) => m,
                Err(e) => { log::warn!("app-plugins: skipping {} — parse error: {e}", mani_path.display()); continue }
            };
            if let Err(e) = manifest.validate() {
                log::warn!("app-plugins: skipping {} — validation failed: {e}", mani_path.display());
                continue;
            }
            by_id.insert(
                manifest.id.clone(),
                DiscoveredPlugin { dir: entry.path(), manifest },
            );
        }
    }
    by_id.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_manifests_and_dedupes_by_id_with_user_dir_winning() {
        let tmp = std::env::temp_dir().join("appplugins_disc_test");
        let _ = fs::remove_dir_all(&tmp);
        let packaged = tmp.join("packaged");
        let user = tmp.join("user");
        fs::create_dir_all(packaged.join("audience")).unwrap();
        fs::create_dir_all(user.join("audience")).unwrap();
        // both define id "audience"; user dir should win
        let base = r#"{"id":"audience","name":"NAME","apiVersion":1,"url":"http://localhost:3000",
            "lifecycle":{"start":"x","health":{"url":"h"},"ready":{"url":"r"}}}"#;
        fs::write(packaged.join("audience/app-plugin.json"), base.replace("NAME", "Packaged")).unwrap();
        fs::write(user.join("audience/app-plugin.json"), base.replace("NAME", "User")).unwrap();

        let found = discover(&[packaged.as_path(), user.as_path()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.name, "User"); // later root wins on id collision
    }

    #[test]
    fn skips_unparseable_and_version_refused_manifests() {
        let tmp = std::env::temp_dir().join("appplugins_disc_skip_test");
        let _ = fs::remove_dir_all(&tmp);
        let root = tmp.join("root");
        for sub in ["good", "garbage", "badversion", "nomanifest"] {
            fs::create_dir_all(root.join(sub)).unwrap();
        }
        let valid = r#"{"id":"good","name":"Good","apiVersion":1,"url":"http://localhost:3000",
            "lifecycle":{"start":"x","health":{"url":"h"},"ready":{"url":"r"}}}"#;
        fs::write(root.join("good/app-plugin.json"), valid).unwrap();
        fs::write(root.join("garbage/app-plugin.json"), "{ not json").unwrap();
        // parses fine but validate() refuses the unsupported apiVersion → skipped
        fs::write(root.join("badversion/app-plugin.json"), valid.replace("\"apiVersion\":1", "\"apiVersion\":99").replace("\"id\":\"good\"", "\"id\":\"bad\"")).unwrap();
        // "nomanifest" dir has no app-plugin.json at all → skipped

        let found = discover(&[root.as_path()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].manifest.id, "good");
    }
}
