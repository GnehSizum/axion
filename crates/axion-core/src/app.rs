use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use url::Url;

use crate::{AppConfig, WindowConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Development,
    Production,
}

impl Display for RunMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Development => formatter.write_str("development"),
            Self::Production => formatter.write_str("production"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEntrypoint {
    DevServer(Url),
    Packaged(PathBuf),
}

impl Display for RunEntrypoint {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DevServer(url) => write!(formatter, "{url}"),
            Self::Packaged(path) => write!(formatter, "{}", path.display()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowPlan {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub visible: bool,
}

impl From<&WindowConfig> for WindowPlan {
    fn from(value: &WindowConfig) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            title: value.title.clone(),
            width: value.width,
            height: value.height,
            resizable: value.resizable,
            visible: value.visible,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowLaunchConfig {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub visible: bool,
}

impl From<&WindowConfig> for WindowLaunchConfig {
    fn from(value: &WindowConfig) -> Self {
        Self {
            id: value.id.as_str().to_owned(),
            title: value.title.clone(),
            width: value.width,
            height: value.height,
            resizable: value.resizable,
            visible: value.visible,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchEntrypoint {
    DevServer(Url),
    Packaged(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeLaunchConfig {
    pub app_name: String,
    pub identifier: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub mode: RunMode,
    pub entrypoint: LaunchEntrypoint,
    pub frontend_dist: PathBuf,
    pub packaged_entry: PathBuf,
    pub windows: Vec<WindowLaunchConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePlan {
    pub app_name: String,
    pub identifier: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub mode: RunMode,
    pub entrypoint: RunEntrypoint,
    pub frontend_dist: PathBuf,
    pub windows: Vec<WindowPlan>,
    pub notes: Vec<String>,
}

impl Display for RuntimePlan {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "app: {}", self.app_name)?;
        if let Some(identifier) = &self.identifier {
            writeln!(formatter, "identifier: {identifier}")?;
        }
        if let Some(version) = &self.version {
            writeln!(formatter, "version: {version}")?;
        }
        if let Some(description) = &self.description {
            writeln!(formatter, "description: {description}")?;
        }
        if !self.authors.is_empty() {
            writeln!(formatter, "authors: {}", self.authors.join(", "))?;
        }
        if let Some(homepage) = &self.homepage {
            writeln!(formatter, "homepage: {homepage}")?;
        }
        writeln!(formatter, "mode: {}", self.mode)?;
        writeln!(formatter, "entrypoint: {}", self.entrypoint)?;
        writeln!(formatter, "frontend_dist: {}", self.frontend_dist.display())?;
        writeln!(formatter, "windows: {}", self.windows.len())?;

        for window in &self.windows {
            writeln!(
                formatter,
                "- {}: '{}' ({}x{}, visible={}, resizable={})",
                window.id,
                window.title,
                window.width,
                window.height,
                window.visible,
                window.resizable
            )?;
        }

        if !self.notes.is_empty() {
            writeln!(formatter, "notes:")?;
            for note in &self.notes {
                writeln!(formatter, "- {note}")?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHandle {
    app_name: String,
}

impl AppHandle {
    pub fn app_name(&self) -> &str {
        &self.app_name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    config: AppConfig,
}

impl App {
    pub(crate) fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn handle(&self) -> AppHandle {
        AppHandle {
            app_name: self.config.identity.name.clone(),
        }
    }

    pub fn runtime_launch_config(&self, mode: RunMode) -> RuntimeLaunchConfig {
        let entrypoint = match (mode, self.config.dev.as_ref()) {
            (RunMode::Development, Some(dev)) => LaunchEntrypoint::DevServer(dev.url.clone()),
            (RunMode::Development, None) | (RunMode::Production, _) => {
                LaunchEntrypoint::Packaged(self.config.build.entry.clone())
            }
        };

        RuntimeLaunchConfig {
            app_name: self.config.identity.name.clone(),
            identifier: self.config.identity.identifier.clone(),
            version: self.config.identity.version.clone(),
            description: self.config.identity.description.clone(),
            authors: self.config.identity.authors.clone(),
            homepage: self.config.identity.homepage.clone(),
            mode,
            entrypoint,
            frontend_dist: self.config.build.frontend_dist.clone(),
            packaged_entry: self.config.build.entry.clone(),
            windows: self
                .config
                .windows
                .iter()
                .map(WindowLaunchConfig::from)
                .collect(),
        }
    }

    pub fn runtime_plan(&self, mode: RunMode) -> RuntimePlan {
        let launch_config = self.runtime_launch_config(mode);
        let mut notes = vec![
            "runtime startup is available through axion-runtime; enable `servo-runtime` for Servo-backed desktop windows"
                .to_owned(),
            "each declared window maps to one native window and one primary WebView in the desktop runtime"
                .to_owned(),
        ];

        let entrypoint = match &launch_config.entrypoint {
            LaunchEntrypoint::DevServer(url) => RunEntrypoint::DevServer(url.clone()),
            LaunchEntrypoint::Packaged(path) => {
                if matches!(launch_config.mode, RunMode::Development) {
                    notes.push(
                    "development URL is absent, so the packaged entry is used as the fallback plan"
                        .to_owned(),
                );
                }
                RunEntrypoint::Packaged(path.clone())
            }
        };

        RuntimePlan {
            app_name: launch_config.app_name,
            identifier: launch_config.identifier,
            version: launch_config.version,
            description: launch_config.description,
            authors: launch_config.authors,
            homepage: launch_config.homepage,
            mode: launch_config.mode,
            entrypoint,
            frontend_dist: launch_config.frontend_dist,
            windows: launch_config
                .windows
                .iter()
                .map(|window| WindowPlan {
                    id: window.id.clone(),
                    title: window.title.clone(),
                    width: window.width,
                    height: window.height,
                    resizable: window.resizable,
                    visible: window.visible,
                })
                .collect(),
            notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use url::Url;

    use super::{App, LaunchEntrypoint, RunEntrypoint, RunMode};
    use crate::{
        AppConfig, AppIdentity, BuildConfig, CapabilityConfig, DevServerConfig, WindowConfig,
    };

    fn test_app(dev_url: Option<&str>) -> App {
        App::new(AppConfig {
            identity: AppIdentity::new("axion-test").with_identifier("dev.axion.test"),
            windows: vec![WindowConfig::main("Axion Test")],
            dev: dev_url.map(|url| DevServerConfig {
                url: Url::parse(url).expect("test URL must parse"),
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            capabilities: BTreeMap::from([(
                "main".to_owned(),
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )]),
        })
    }

    #[test]
    fn launch_config_uses_dev_server_in_development() {
        let app = test_app(Some("http://127.0.0.1:3000"));
        let launch_config = app.runtime_launch_config(RunMode::Development);

        assert_eq!(launch_config.app_name, "axion-test");
        assert_eq!(launch_config.identifier.as_deref(), Some("dev.axion.test"));
        assert_eq!(launch_config.frontend_dist, PathBuf::from("frontend"));
        assert_eq!(
            launch_config.packaged_entry,
            PathBuf::from("frontend/index.html")
        );
        assert_eq!(launch_config.windows.len(), 1);
        assert!(matches!(
            launch_config.entrypoint,
            LaunchEntrypoint::DevServer(_)
        ));
    }

    #[test]
    fn launch_config_falls_back_to_packaged_entry_without_dev_server() {
        let app = test_app(None);
        let launch_config = app.runtime_launch_config(RunMode::Development);

        assert_eq!(
            launch_config.entrypoint,
            LaunchEntrypoint::Packaged(PathBuf::from("frontend/index.html"))
        );
        assert_eq!(
            launch_config.packaged_entry,
            PathBuf::from("frontend/index.html")
        );
    }

    #[test]
    fn runtime_plan_reports_packaged_fallback_note() {
        let app = test_app(None);
        let runtime_plan = app.runtime_plan(RunMode::Development);

        assert!(matches!(
            runtime_plan.entrypoint,
            RunEntrypoint::Packaged(_)
        ));
        assert!(
            runtime_plan
                .notes
                .iter()
                .any(|note| note.contains("fallback plan"))
        );
    }
}
