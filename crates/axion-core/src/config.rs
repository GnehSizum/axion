use std::collections::BTreeMap;
use std::path::PathBuf;

use url::Url;

use crate::WindowConfig;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIdentity {
    pub name: String,
    pub identifier: Option<String>,
}

impl AppIdentity {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            identifier: None,
        }
    }

    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevServerConfig {
    pub url: Url,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildConfig {
    pub frontend_dist: PathBuf,
    pub entry: PathBuf,
}

impl BuildConfig {
    pub fn new(frontend_dist: impl Into<PathBuf>, entry: impl Into<PathBuf>) -> Self {
        Self {
            frontend_dist: frontend_dist.into(),
            entry: entry.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilityConfig {
    pub commands: Vec<String>,
    pub events: Vec<String>,
    pub protocols: Vec<String>,
    pub allowed_navigation_origins: Vec<String>,
    pub allow_remote_navigation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    pub identity: AppIdentity,
    pub windows: Vec<WindowConfig>,
    pub dev: Option<DevServerConfig>,
    pub build: BuildConfig,
    pub capabilities: BTreeMap<String, CapabilityConfig>,
}

impl AppConfig {
    pub fn primary_window(&self) -> Option<&WindowConfig> {
        self.windows.first()
    }
}
