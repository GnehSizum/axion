use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{
    App, AppConfig, AppIdentity, AxionError, BuildConfig, BundleConfig, CapabilityConfig,
    DevServerConfig, WindowConfig,
};

#[derive(Clone, Debug, Default)]
pub struct Builder {
    identity: Option<AppIdentity>,
    windows: Vec<WindowConfig>,
    dev: Option<DevServerConfig>,
    build: Option<BuildConfig>,
    bundle: BundleConfig,
    capabilities: BTreeMap<String, CapabilityConfig>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_config(mut self, config: AppConfig) -> Self {
        self.identity = Some(config.identity);
        self.windows = config.windows;
        self.dev = config.dev;
        self.build = Some(config.build);
        self.bundle = config.bundle;
        self.capabilities = config.capabilities;
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        let identity = self
            .identity
            .take()
            .unwrap_or_else(|| AppIdentity::new(name.clone()));
        self.identity = Some(AppIdentity {
            name,
            identifier: identity.identifier,
            version: identity.version,
            description: identity.description,
            authors: identity.authors,
            homepage: identity.homepage,
        });
        self
    }

    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        let identity = self
            .identity
            .take()
            .unwrap_or_else(|| AppIdentity::new(String::new()));
        self.identity = Some(identity.with_identifier(identifier));
        self
    }

    pub fn with_window(mut self, window: WindowConfig) -> Self {
        self.windows.push(window);
        self
    }

    pub fn with_dev_server(mut self, dev: DevServerConfig) -> Self {
        self.dev = Some(dev);
        self
    }

    pub fn with_build(mut self, build: BuildConfig) -> Self {
        self.build = Some(build);
        self
    }

    pub fn with_bundle(mut self, bundle: BundleConfig) -> Self {
        self.bundle = bundle;
        self
    }

    pub fn with_capability(
        mut self,
        window_id: impl Into<String>,
        capability: CapabilityConfig,
    ) -> Self {
        self.capabilities.insert(window_id.into(), capability);
        self
    }

    pub fn build(self) -> Result<App, AxionError> {
        let identity = self.identity.ok_or(AxionError::MissingAppName)?;
        if identity.name.trim().is_empty() {
            return Err(AxionError::MissingAppName);
        }

        if self.windows.is_empty() {
            return Err(AxionError::MissingWindow);
        }

        let mut window_ids = BTreeSet::new();
        for window in &self.windows {
            if window.id.as_str().trim().is_empty() {
                return Err(AxionError::InvalidWindowId);
            }
            if !window_ids.insert(window.id.as_str().to_owned()) {
                return Err(AxionError::DuplicateWindowId {
                    window_id: window.id.as_str().to_owned(),
                });
            }
            if window.title.trim().is_empty() {
                return Err(AxionError::InvalidWindowTitle {
                    window_id: window.id.as_str().to_owned(),
                });
            }
            if window.width == 0 || window.height == 0 {
                return Err(AxionError::InvalidWindowSize {
                    window_id: window.id.as_str().to_owned(),
                });
            }
        }
        for window_id in self.capabilities.keys() {
            if !window_ids.contains(window_id) {
                return Err(AxionError::UnknownCapabilityWindow {
                    window_id: window_id.clone(),
                });
            }
        }

        let build = self.build.ok_or(AxionError::MissingBuildConfig)?;
        if is_path_empty(&build.frontend_dist) {
            return Err(AxionError::MissingFrontendDist);
        }
        if is_path_empty(&build.entry) {
            return Err(AxionError::MissingBuildEntry);
        }

        Ok(App::new(AppConfig {
            identity,
            windows: self.windows,
            dev: self.dev,
            build,
            bundle: self.bundle,
            capabilities: self.capabilities,
        }))
    }
}

fn is_path_empty(path: &Path) -> bool {
    path.as_os_str().is_empty()
}

#[cfg(test)]
mod tests {
    use super::Builder;
    use crate::{
        AppConfig, AppIdentity, AxionError, BuildConfig, BundleConfig, WindowConfig, WindowId,
    };

    fn valid_builder() -> Builder {
        Builder::new()
            .with_name("axion-test")
            .with_build(BuildConfig::new("frontend", "frontend/index.html"))
    }

    #[test]
    fn builder_rejects_empty_window_id() {
        let error = valid_builder()
            .with_window(WindowConfig::new(WindowId::new(""), "Test", 960, 720))
            .build()
            .expect_err("empty window id should fail");

        assert!(matches!(error, AxionError::InvalidWindowId));
    }

    #[test]
    fn builder_rejects_zero_window_size() {
        let error = valid_builder()
            .with_window(WindowConfig::new(WindowId::main(), "Test", 0, 720))
            .build()
            .expect_err("zero window width should fail");

        assert!(matches!(error, AxionError::InvalidWindowSize { .. }));
    }

    #[test]
    fn builder_rejects_duplicate_window_ids() {
        let error = valid_builder()
            .with_window(WindowConfig::new(WindowId::main(), "Main", 960, 720))
            .with_window(WindowConfig::new(WindowId::main(), "Duplicate", 800, 600))
            .build()
            .expect_err("duplicate window ids should fail");

        assert!(matches!(error, AxionError::DuplicateWindowId { .. }));
    }

    #[test]
    fn builder_rejects_capabilities_for_unknown_windows() {
        let error = valid_builder()
            .with_window(WindowConfig::new(WindowId::main(), "Main", 960, 720))
            .with_capability("settings", Default::default())
            .build()
            .expect_err("unknown capability window should fail");

        assert!(matches!(error, AxionError::UnknownCapabilityWindow { .. }));
    }

    #[test]
    fn builder_preserves_bundle_config() {
        let icon = std::path::PathBuf::from("icons/app.icns");
        let app = valid_builder()
            .with_window(WindowConfig::new(WindowId::main(), "Main", 960, 720))
            .with_bundle(BundleConfig::new().with_icon(&icon))
            .build()
            .expect("bundle config should build");

        assert_eq!(app.config().bundle.icon.as_ref(), Some(&icon));
    }

    #[test]
    fn apply_config_preserves_bundle_config() {
        let icon = std::path::PathBuf::from("icons/app.icns");
        let app = Builder::new()
            .apply_config(AppConfig {
                identity: AppIdentity::new("axion-test"),
                windows: vec![WindowConfig::new(WindowId::main(), "Main", 960, 720)],
                dev: None,
                build: BuildConfig::new("frontend", "frontend/index.html"),
                bundle: BundleConfig::new().with_icon(&icon),
                capabilities: Default::default(),
            })
            .build()
            .expect("config should build");

        assert_eq!(app.config().bundle.icon.as_ref(), Some(&icon));
    }
}
