use std::collections::BTreeMap;

use crate::{
    App, AppConfig, AppIdentity, AxionError, BuildConfig, BundleConfig, CapabilityConfig,
    DevServerConfig, NativeConfig, WindowConfig,
};

#[derive(Clone, Debug, Default)]
pub struct Builder {
    identity: Option<AppIdentity>,
    windows: Vec<WindowConfig>,
    dev: Option<DevServerConfig>,
    build: Option<BuildConfig>,
    bundle: BundleConfig,
    native: NativeConfig,
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
        self.native = config.native;
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

    pub fn with_native(mut self, native: NativeConfig) -> Self {
        self.native = native;
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
        let build = self.build.ok_or(AxionError::MissingBuildConfig)?;
        let mut config = AppConfig {
            identity,
            windows: self.windows,
            dev: self.dev,
            build,
            bundle: self.bundle,
            native: self.native,
            capabilities: self.capabilities,
        };
        config.resolve_capabilities()?;
        config.validate()?;
        Ok(App::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::Builder;
    use crate::{
        AppConfig, AppIdentity, AxionError, BuildConfig, BundleConfig, ClipboardConfig,
        DialogConfig, LifecycleConfig, NativeConfig, WindowConfig, WindowId,
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
    fn builder_rejects_path_like_app_names() {
        for name in [".", "..", "../outside", "one/two", "one\\two", "nul\0name"] {
            assert!(matches!(
                valid_builder()
                    .with_name(name)
                    .with_window(WindowConfig::main("Test"))
                    .build(),
                Err(AxionError::InvalidAppName)
            ));
        }
    }

    #[test]
    fn builder_and_direct_config_reject_zero_close_timeout() {
        let error = valid_builder()
            .with_window(WindowConfig::main("Test"))
            .with_native(
                NativeConfig::new().with_lifecycle(LifecycleConfig::new().with_close_timeout_ms(0)),
            )
            .build()
            .expect_err("zero timeout must fail");
        assert!(matches!(error, AxionError::InvalidCloseTimeout));
        let mut config = valid_builder()
            .with_window(WindowConfig::main("Test"))
            .build()
            .unwrap()
            .config()
            .clone();
        config.native.lifecycle.close_timeout_ms = 0;
        assert!(matches!(
            config.validate(),
            Err(AxionError::InvalidCloseTimeout)
        ));
    }

    #[test]
    fn builder_resolves_profiles_and_rejects_tampered_cached_permissions() {
        let mut config = valid_builder()
            .with_window(WindowConfig::main("Test"))
            .with_capability(
                "main",
                crate::CapabilityConfig {
                    profiles: vec!["app-info".to_owned()],
                    ..Default::default()
                },
            )
            .build()
            .unwrap()
            .config()
            .clone();
        assert!(
            config.capabilities["main"]
                .commands
                .iter()
                .any(|command| command == "app.ping")
        );
        assert_eq!(config.capabilities["main"].protocols, vec!["axion"]);
        config
            .capabilities
            .get_mut("main")
            .unwrap()
            .commands
            .push("fs.remove".to_owned());
        assert!(matches!(
            config.validate(),
            Err(AxionError::InvalidCapability { .. })
        ));
        assert!(Builder::new().apply_config(config).build().is_err());
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
                native: NativeConfig::new(),
                capabilities: Default::default(),
            })
            .build()
            .expect("config should build");

        assert_eq!(app.config().bundle.icon.as_ref(), Some(&icon));
    }

    #[test]
    fn builder_preserves_native_config() {
        let app = valid_builder()
            .with_window(WindowConfig::new(WindowId::main(), "Main", 960, 720))
            .with_native(
                NativeConfig::new()
                    .with_dialog(DialogConfig::system())
                    .with_clipboard(ClipboardConfig::system())
                    .with_lifecycle(LifecycleConfig::new().with_close_timeout_ms(1500)),
            )
            .build()
            .expect("native config should build");

        assert_eq!(app.config().native.dialog, DialogConfig::system());
        assert_eq!(app.config().native.clipboard, ClipboardConfig::system());
        assert_eq!(app.config().native.lifecycle.close_timeout_ms, 1500);
    }
}
