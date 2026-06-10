use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(rename = "apiVersion")]
    pub api_version: u32,
    #[serde(default)]
    pub icon: String,
    pub url: String,
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub webview: WebviewCfg,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lifecycle {
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub build: Option<BuildStep>,
    pub start: String,
    #[serde(default)]
    pub stop: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub health: Probe,
    pub ready: Probe,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildStep {
    pub cmd: String,
    #[serde(default)]
    pub args: BTreeMap<String, String>,
    #[serde(default = "default_build_timeout")]
    pub timeout: u64,
}
fn default_build_timeout() -> u64 { 1_200_000 }

#[derive(Debug, Clone, Deserialize)]
pub struct Probe {
    pub url: String,
    #[serde(rename = "okStatus", default = "default_ok_status")]
    pub ok_status: Vec<u16>,
    #[serde(default = "default_probe_timeout")]
    pub timeout: u64,
    #[serde(default = "default_probe_interval")]
    pub interval: u64,
}
fn default_ok_status() -> Vec<u16> { vec![200] }
fn default_probe_timeout() -> u64 { 180_000 }
fn default_probe_interval() -> u64 { 1_000 }

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WebviewCfg {
    #[serde(default)]
    pub popups: Popups,
    #[serde(rename = "externalLinks", default)]
    pub external_links: ExternalLinks,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Popups { #[default] Allow, Block }

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalLinks { #[default] InApp, SystemBrowser }

pub const SUPPORTED_API_VERSIONS: &[u32] = &[1];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("unsupported apiVersion: {0}")]
    UnsupportedApiVersion(u32),
}

impl Manifest {
    pub fn from_json(s: &str) -> Result<Manifest, serde_json::Error> {
        serde_json::from_str(s)
    }
    /// Effective window title (defaults to `name`).
    pub fn window_title(&self) -> String {
        self.webview.title.clone().unwrap_or_else(|| self.name.clone())
    }
    pub fn validate(&self) -> Result<(), ManifestError> {
        if !SUPPORTED_API_VERSIONS.contains(&self.api_version) {
            return Err(ManifestError::UnsupportedApiVersion(self.api_version));
        }
        Ok(())
    }
    /// Resolve `lifecycle.cwd` relative to the manifest's own directory;
    /// absolute paths pass through unchanged. No cwd → manifest dir.
    pub fn resolved_cwd(&self, manifest_dir: &Path) -> PathBuf {
        match &self.lifecycle.cwd {
            None => manifest_dir.to_path_buf(),
            Some(c) => {
                let p = Path::new(c);
                if p.is_absolute() { p.to_path_buf() } else { manifest_dir.join(p) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const AUDIENCE_JSON: &str = r#"{
      "id": "audience",
      "name": "Audience",
      "apiVersion": 1,
      "icon": "icon.svg",
      "url": "http://localhost:3000",
      "lifecycle": {
        "managed": true,
        "cwd": "D:/MajorProjects/CURRENT/audience",
        "build": { "cmd": "docker compose -f docker-compose.prod.yml build",
                   "args": { "NODE_ENV": "development" }, "timeout": 1200000 },
        "start": "docker compose -f docker-compose.prod.yml up",
        "stop": "docker compose -f docker-compose.prod.yml down",
        "env": { "NODE_ENV": "development" },
        "health": { "url": "http://localhost:8080/health", "okStatus": [200], "timeout": 180000, "interval": 1000 },
        "ready": { "url": "http://localhost:3000", "okStatus": [200, 302], "timeout": 180000, "interval": 1000 }
      }
    }"#;

    #[test]
    fn parses_a_full_manifest() {
        let m = Manifest::from_json(AUDIENCE_JSON).expect("should parse");
        assert_eq!(m.id, "audience");
        assert_eq!(m.api_version, 1);
        assert_eq!(m.url, "http://localhost:3000");
        assert_eq!(m.lifecycle.health.ok_status, vec![200]);
        assert_eq!(m.lifecycle.ready.ok_status, vec![200, 302]);
        // webview block omitted → defaults
        assert_eq!(m.webview.popups, Popups::Allow);
        assert_eq!(m.webview.external_links, ExternalLinks::InApp);
        assert_eq!(m.window_title(), "Audience"); // title defaults to name
    }

    #[test]
    fn rejects_unknown_api_version() {
        let json = AUDIENCE_JSON.replace("\"apiVersion\": 1", "\"apiVersion\": 99");
        let m = Manifest::from_json(&json).unwrap();
        assert!(matches!(m.validate(), Err(ManifestError::UnsupportedApiVersion(99))));
    }

    #[test]
    fn accepts_supported_api_version() {
        let m = Manifest::from_json(AUDIENCE_JSON).unwrap();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn resolves_relative_cwd_against_manifest_dir() {
        let mut m = Manifest::from_json(AUDIENCE_JSON).unwrap();
        m.lifecycle.cwd = Some("backend".into());
        let resolved = m.resolved_cwd(Path::new("/plugins/audience"));
        assert_eq!(resolved, Path::new("/plugins/audience/backend"));
    }

    #[test]
    fn keeps_absolute_cwd_as_is() {
        let m = Manifest::from_json(AUDIENCE_JSON).unwrap(); // cwd is absolute D:/...
        let resolved = m.resolved_cwd(Path::new("/plugins/audience"));
        assert_eq!(resolved, Path::new("D:/MajorProjects/CURRENT/audience"));
    }

    #[test]
    fn uses_manifest_dir_when_cwd_is_absent() {
        let mut m = Manifest::from_json(AUDIENCE_JSON).unwrap();
        m.lifecycle.cwd = None;
        assert_eq!(m.resolved_cwd(Path::new("/plugins/audience")), Path::new("/plugins/audience"));
    }
}
