use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestDocument {
    pub app: AppSection,
    #[serde(default)]
    pub window: Option<WindowSection>,
    #[serde(default)]
    pub windows: Vec<WindowSection>,
    #[serde(default)]
    pub dev: Option<DevSection>,
    pub build: BuildSection,
    #[serde(default)]
    pub bundle: Option<BundleSection>,
    #[serde(default)]
    pub native: Option<NativeSection>,
    #[serde(default)]
    pub capabilities: BTreeMap<String, CapabilitySection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppSection {
    pub name: String,
    #[serde(default)]
    pub identifier: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindowSection {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default = "default_true")]
    pub resizable: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
}

impl Default for WindowSection {
    fn default() -> Self {
        Self {
            id: None,
            title: None,
            width: None,
            height: None,
            resizable: true,
            visible: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DevSection {
    pub url: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildSection {
    pub frontend_dist: PathBuf,
    pub entry: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BundleSection {
    #[serde(default)]
    pub icon: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NativeSection {
    #[serde(default)]
    pub dialog: Option<DialogSection>,
    #[serde(default)]
    pub clipboard: Option<ClipboardSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DialogSection {
    #[serde(default)]
    pub backend: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClipboardSection {
    #[serde(default)]
    pub backend: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CapabilitySection {
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub protocols: Vec<String>,
    #[serde(default)]
    pub allowed_navigation_origins: Vec<String>,
    #[serde(default)]
    pub allow_remote_navigation: bool,
}

const fn default_true() -> bool {
    true
}
