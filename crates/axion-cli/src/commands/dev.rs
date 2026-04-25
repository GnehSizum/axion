use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use axion_core::{AppConfig, Builder, RunMode};

use crate::cli::DevArgs;
use crate::error::AxionCliError;

pub fn run(args: DevArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let dev_server_status = dev_server_status(app.config());

    if args.launch {
        let launch_mode = select_launch_mode(&dev_server_status, args.fallback_packaged)?;
        match (&dev_server_status, launch_mode) {
            (DevServerStatus::Reachable { url }, RunMode::Development) => {
                println!("Axion dev launch: using reachable dev server at {url}");
            }
            (_, RunMode::Production) => {
                println!("Axion dev launch fallback: launching packaged app.");
            }
            _ => {}
        }

        axion_runtime::run(app, launch_mode)?;
        return Ok(());
    }

    let plan = app.runtime_plan(RunMode::Development);

    println!("Axion development plan");
    println!("manifest: {}", args.manifest_path.display());
    println!("dev_server: {}", dev_server_status.summary());
    println!("{plan}");

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DevServerStatus {
    Unconfigured,
    InvalidEndpoint { url: String },
    Unreachable { url: String },
    Reachable { url: String },
}

impl DevServerStatus {
    fn summary(&self) -> String {
        match self {
            Self::Unconfigured => "unconfigured".to_owned(),
            Self::InvalidEndpoint { url } => format!("invalid endpoint ({url})"),
            Self::Unreachable { url } => format!("unreachable ({url})"),
            Self::Reachable { url } => format!("reachable ({url})"),
        }
    }

    fn launch_error(&self) -> std::io::Error {
        let message = match self {
            Self::Unconfigured => {
                "dev server is not configured; add [dev] url = \"http://127.0.0.1:3000\" or pass --fallback-packaged".to_owned()
            }
            Self::InvalidEndpoint { url } => {
                format!(
                    "dev server URL does not include a usable host and port: {url}; fix [dev].url or pass --fallback-packaged"
                )
            }
            Self::Unreachable { url } => {
                format!(
                    "dev server is not reachable at {url}; start the frontend dev server or pass --fallback-packaged"
                )
            }
            Self::Reachable { .. } => "dev server is reachable".to_owned(),
        };

        std::io::Error::other(message)
    }
}

fn select_launch_mode(
    dev_server_status: &DevServerStatus,
    fallback_packaged: bool,
) -> Result<RunMode, AxionCliError> {
    match dev_server_status {
        DevServerStatus::Reachable { .. } => Ok(RunMode::Development),
        _ if fallback_packaged => Ok(RunMode::Production),
        _ => Err(dev_server_status.launch_error().into()),
    }
}

fn dev_server_status(config: &AppConfig) -> DevServerStatus {
    dev_server_status_with(config, |host, port| {
        let Ok(addresses) = (host, port).to_socket_addrs() else {
            return None;
        };

        let timeout = Duration::from_millis(300);
        Some(
            addresses
                .into_iter()
                .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok()),
        )
    })
}

fn dev_server_status_with(
    config: &AppConfig,
    is_reachable: impl Fn(&str, u16) -> Option<bool>,
) -> DevServerStatus {
    let Some(dev_server) = &config.dev else {
        return DevServerStatus::Unconfigured;
    };
    let url = dev_server.url.to_string();

    let Some(host) = dev_server.url.host_str() else {
        return DevServerStatus::InvalidEndpoint { url };
    };
    let Some(port) = dev_server.url.port_or_known_default() else {
        return DevServerStatus::InvalidEndpoint { url };
    };

    match is_reachable(host, port) {
        Some(true) => DevServerStatus::Reachable { url },
        Some(false) => DevServerStatus::Unreachable { url },
        None => DevServerStatus::InvalidEndpoint { url },
    }
}

pub(crate) fn dev_server_is_reachable(config: &AppConfig) -> bool {
    matches!(dev_server_status(config), DevServerStatus::Reachable { .. })
}

#[cfg(test)]
mod tests {
    use axion_core::{AppConfig, AppIdentity, BuildConfig, RunMode, WindowConfig};
    use url::Url;

    use super::{DevServerStatus, dev_server_status, dev_server_status_with, select_launch_mode};

    fn config_with_dev_url(dev_url: Option<&str>) -> AppConfig {
        AppConfig {
            identity: AppIdentity::new("axion-cli-test"),
            windows: vec![WindowConfig::main("CLI Test")],
            dev: dev_url.map(|value| axion_core::DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            capabilities: Default::default(),
        }
    }

    #[test]
    fn launch_mode_errors_without_dev_server_by_default() {
        let status = dev_server_status(&config_with_dev_url(None));

        assert_eq!(status, DevServerStatus::Unconfigured);
        assert!(select_launch_mode(&status, false).is_err());
        assert_eq!(
            select_launch_mode(&status, true).expect("fallback should select production"),
            RunMode::Production
        );
    }

    #[test]
    fn launch_mode_uses_reachable_dev_server() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));
        let status = dev_server_status_with(&config, |_host, _port| Some(true));

        assert!(matches!(status, DevServerStatus::Reachable { .. }));
        assert_eq!(
            select_launch_mode(&status, false).expect("reachable dev server should launch dev"),
            RunMode::Development
        );
    }

    #[test]
    fn dev_server_status_reports_unreachable_server() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));

        assert!(matches!(
            dev_server_status_with(&config, |_host, _port| Some(false)),
            DevServerStatus::Unreachable { .. }
        ));
    }

    #[test]
    fn dev_server_status_reports_invalid_endpoint() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));

        assert!(matches!(
            dev_server_status_with(&config, |_host, _port| None),
            DevServerStatus::InvalidEndpoint { .. }
        ));
    }
}
