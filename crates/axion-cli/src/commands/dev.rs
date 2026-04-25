use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use axion_core::{App, AppConfig, Builder, RunMode};

use crate::cli::DevArgs;
use crate::error::AxionCliError;

pub fn run(args: DevArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let dev_server_status = dev_server_status(app.config());
    let packaged_fallback_status = packaged_fallback_status(&app);
    let launch_mode = select_launch_mode_with_packaged_fallback(
        &dev_server_status,
        &packaged_fallback_status,
        args.fallback_packaged,
    );

    println!("Axion development diagnostics");
    for line in dev_diagnostic_lines(
        &app,
        &args.manifest_path,
        &dev_server_status,
        &packaged_fallback_status,
        args.fallback_packaged,
    ) {
        println!("{line}");
    }

    if args.launch {
        let launch_mode = launch_mode?;
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

    let plan_mode = launch_mode
        .as_ref()
        .copied()
        .unwrap_or(RunMode::Development);
    let plan = app.runtime_plan(plan_mode);

    println!("runtime_plan:");
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PackagedFallbackStatus {
    Available { url: String },
    Unavailable { reason: String },
}

impl PackagedFallbackStatus {
    fn summary(&self, fallback_enabled: bool, selected: bool) -> String {
        match self {
            Self::Available { url } if selected => {
                format!("selected ({url})")
            }
            Self::Available { url } if fallback_enabled => {
                format!("enabled ({url})")
            }
            Self::Available { url } => {
                format!("disabled; available with --fallback-packaged ({url})")
            }
            Self::Unavailable { reason } if fallback_enabled => {
                format!("enabled but unavailable ({reason})")
            }
            Self::Unavailable { reason } => {
                format!("disabled; unavailable ({reason})")
            }
        }
    }

    fn launch_error(&self) -> std::io::Error {
        let message = match self {
            Self::Available { .. } => "packaged fallback is available".to_owned(),
            Self::Unavailable { reason } => {
                format!("packaged fallback is unavailable: {reason}")
            }
        };

        std::io::Error::other(message)
    }

    fn entry_url(&self) -> Option<&str> {
        match self {
            Self::Available { url } => Some(url),
            Self::Unavailable { .. } => None,
        }
    }
}

#[cfg(test)]
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

fn select_launch_mode_with_packaged_fallback(
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    fallback_packaged: bool,
) -> Result<RunMode, AxionCliError> {
    match dev_server_status {
        DevServerStatus::Reachable { .. } => Ok(RunMode::Development),
        _ if fallback_packaged => match packaged_fallback_status {
            PackagedFallbackStatus::Available { .. } => Ok(RunMode::Production),
            PackagedFallbackStatus::Unavailable { .. } => {
                Err(packaged_fallback_status.launch_error().into())
            }
        },
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

fn packaged_fallback_status(app: &App) -> PackagedFallbackStatus {
    match axion_runtime::launch_request(app, RunMode::Production) {
        Ok(request) => match request.target {
            axion_runtime::RuntimeLaunchTarget::AppProtocol(app_protocol) => {
                PackagedFallbackStatus::Available {
                    url: app_protocol.initial_url.to_string(),
                }
            }
            axion_runtime::RuntimeLaunchTarget::DevServer(url) => {
                PackagedFallbackStatus::Unavailable {
                    reason: format!("production launch unexpectedly resolved to {url}"),
                }
            }
        },
        Err(error) => PackagedFallbackStatus::Unavailable {
            reason: error.to_string(),
        },
    }
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

fn dev_diagnostic_lines(
    app: &App,
    manifest_path: &std::path::Path,
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    fallback_packaged: bool,
) -> Vec<String> {
    let launch_mode = select_launch_mode_with_packaged_fallback(
        dev_server_status,
        packaged_fallback_status,
        fallback_packaged,
    );
    let selected_packaged_fallback = matches!(launch_mode, Ok(RunMode::Production));

    let mut lines = vec![
        format!("manifest: {}", manifest_path.display()),
        format!(
            "launch_mode: {}",
            launch_mode
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|error| format!("blocked ({error})"))
        ),
        format!("dev_server: {}", dev_server_status.summary()),
        format!(
            "packaged_fallback: {}",
            packaged_fallback_status.summary(fallback_packaged, selected_packaged_fallback)
        ),
        "window_entries:".to_owned(),
    ];

    lines.extend(window_entry_lines(
        app,
        dev_server_status,
        packaged_fallback_status,
        launch_mode.ok(),
    ));
    lines
}

fn window_entry_lines(
    app: &App,
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    launch_mode: Option<RunMode>,
) -> Vec<String> {
    let entry = match launch_mode {
        Some(RunMode::Development) => match dev_server_status {
            DevServerStatus::Reachable { url } => {
                format!("{url} (development)")
            }
            _ => "unavailable (development server is not reachable)".to_owned(),
        },
        Some(RunMode::Production) => packaged_fallback_status
            .entry_url()
            .map(|url| format!("{url} (packaged fallback)"))
            .unwrap_or_else(|| "unavailable (packaged fallback is invalid)".to_owned()),
        None => "unavailable (launch blocked)".to_owned(),
    };

    app.config()
        .windows
        .iter()
        .map(|window| format!("- {}: {entry}", window.id.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{AppConfig, AppIdentity, BuildConfig, RunMode, WindowConfig, WindowId};
    use url::Url;

    use super::{
        DevServerStatus, PackagedFallbackStatus, dev_diagnostic_lines, dev_server_status,
        dev_server_status_with, packaged_fallback_status, select_launch_mode,
        select_launch_mode_with_packaged_fallback,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-dev-test-{unique}-{serial}"))
    }

    fn config_with_dev_url(dev_url: Option<&str>) -> AppConfig {
        AppConfig {
            identity: AppIdentity::new("axion-cli-test"),
            windows: vec![WindowConfig::main("CLI Test")],
            dev: dev_url.map(|value| axion_core::DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            capabilities: Default::default(),
        }
    }

    fn app_with_frontend(dev_url: Option<&str>, window_count: usize) -> axion_core::App {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let windows = if window_count == 1 {
            vec![WindowConfig::main("CLI Test")]
        } else {
            vec![
                WindowConfig::main("CLI Test"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 480, 360),
            ]
        };

        axion_core::Builder::new()
            .apply_config(AppConfig {
                identity: AppIdentity::new("axion-cli-test"),
                windows,
                dev: dev_url.map(|value| axion_core::DevServerConfig {
                    url: Url::parse(value).expect("test URL must parse"),
                }),
                build: BuildConfig::new(&frontend, &entry),
                bundle: Default::default(),
                capabilities: Default::default(),
            })
            .build()
            .unwrap()
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
    fn launch_mode_requires_available_packaged_fallback() {
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = PackagedFallbackStatus::Unavailable {
            reason: "missing index.html".to_owned(),
        };

        assert!(
            select_launch_mode_with_packaged_fallback(&dev_status, &fallback_status, true).is_err()
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

    #[test]
    fn dev_diagnostics_report_reachable_development_entries() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 2);
        let dev_status = DevServerStatus::Reachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            false,
        );

        assert!(lines.iter().any(|line| line == "launch_mode: development"));
        assert!(
            lines
                .iter()
                .any(|line| line == "dev_server: reachable (http://127.0.0.1:3000/)")
        );
        assert!(
            lines.iter().any(|line| {
                line == "packaged_fallback: disabled; available with --fallback-packaged (axion://app/index.html)"
            })
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: http://127.0.0.1:3000/ (development)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- settings: http://127.0.0.1:3000/ (development)")
        );
    }

    #[test]
    fn dev_diagnostics_report_blocked_launch_without_fallback() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 1);
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            false,
        );

        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("launch_mode: blocked"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: unavailable (launch blocked)")
        );
    }

    #[test]
    fn dev_diagnostics_report_selected_packaged_fallback_entries() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 1);
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            true,
        );

        assert!(lines.iter().any(|line| line == "launch_mode: production"));
        assert!(
            lines
                .iter()
                .any(|line| line == "packaged_fallback: selected (axion://app/index.html)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: axion://app/index.html (packaged fallback)")
        );
    }
}
