use std::path::{Path, PathBuf};
use std::process::Command;

use axion_core::{AppConfig, Builder, RunMode};

use crate::cli::DoctorArgs;
use crate::commands::dev::dev_server_is_reachable;
use crate::error::AxionCliError;

pub fn run(args: DoctorArgs) -> Result<(), AxionCliError> {
    println!("Axion doctor");
    print_tool_status("cargo", &["--version"]);
    print_tool_status("rustc", &["--version"]);
    print_manifest_status(&args.manifest_path)?;
    print_servo_status(servo_path_for_manifest(&args.manifest_path).as_deref());
    Ok(())
}

fn print_tool_status(program: &str, args: &[&str]) {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("{program}: ok ({})", version.trim());
        }
        Ok(output) => {
            let error = String::from_utf8_lossy(&output.stderr);
            println!("{program}: failed ({})", error.trim());
        }
        Err(error) => {
            println!("{program}: missing ({error})");
        }
    }
}

fn print_manifest_status(manifest_path: &Path) -> Result<(), AxionCliError> {
    if !manifest_path.exists() {
        println!("manifest: missing ({})", manifest_path.display());
        return Ok(());
    }

    let config = axion_manifest::load_app_config_from_path(manifest_path)?;
    println!(
        "manifest: ok (app={}, windows={})",
        config.identity.name,
        config.windows.len()
    );
    for line in manifest_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in build_asset_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in bundle_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in runtime_diagnostic_lines(&config) {
        println!("{line}");
    }
    println!("{}", dev_server_diagnostic_line(&config));
    Ok(())
}

fn manifest_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(identifier) = &config.identity.identifier {
        lines.push(format!("app.identifier: {identifier}"));
    }
    if let Some(version) = &config.identity.version {
        lines.push(format!("app.version: {version}"));
    }
    if let Some(description) = &config.identity.description {
        lines.push(format!("app.description: {description}"));
    }
    if !config.identity.authors.is_empty() {
        lines.push(format!(
            "app.authors: {}",
            config.identity.authors.join(", ")
        ));
    }
    if let Some(homepage) = &config.identity.homepage {
        lines.push(format!("app.homepage: {homepage}"));
    }
    lines.push(format!(
        "native.dialog.backend: {}",
        config.native.dialog.backend.as_str()
    ));

    lines.extend(config
        .windows
        .iter()
        .flat_map(|window| {
            let window_id = window.id.as_str();
            let window_line = format!(
                "window.{window_id}: title={:?}, size={}x{}, visible={}, resizable={}",
                window.title, window.width, window.height, window.visible, window.resizable
            );
            let capability_line = match config.capabilities.get(window_id) {
                Some(capability) => {
                    let bridge_status = if capability
                        .protocols
                        .iter()
                        .any(|protocol| protocol == "axion")
                    {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    format!(
                        "capabilities.{window_id}: bridge={bridge_status}, commands={}, events={}, protocols={}, navigation_origins={}, remote_navigation={}",
                        list_or_none(&capability.commands),
                        list_or_none(&capability.events),
                        list_or_none(&capability.protocols),
                        list_or_none(&capability.allowed_navigation_origins),
                        capability.allow_remote_navigation
                    )
                }
                None => format!("capabilities.{window_id}: none (bridge=disabled)"),
            };

            [window_line, capability_line]
        })
        .collect::<Vec<_>>());
    lines
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

fn build_asset_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let frontend_dist = &config.build.frontend_dist;
    let entry = &config.build.entry;

    match axion_packager::validate_web_assets(frontend_dist, entry) {
        Ok(validation) => vec![
            format!("frontend_dist: ok ({})", frontend_dist.display()),
            format!(
                "entry: ok ({}; relative={})",
                entry.display(),
                validation.relative_entry.display()
            ),
        ],
        Err(error) => {
            let mut lines = vec![format!("build assets: invalid ({error})")];
            lines.push(format!("frontend_dist: {}", frontend_dist.display()));
            lines.push(format!("entry: {}", entry.display()));
            lines
        }
    }
}

fn bundle_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    match config.bundle.icon.as_deref() {
        Some(icon) => match axion_packager::validate_bundle_icon(Some(icon)) {
            Ok(_) => vec![format!("bundle.icon: ok ({})", icon.display())],
            Err(error) => vec![format!("bundle.icon: invalid ({error})")],
        },
        None => vec!["bundle.icon: not configured".to_owned()],
    }
}

fn runtime_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let app = match Builder::new().apply_config(config.clone()).build() {
        Ok(app) => app,
        Err(error) => return vec![format!("runtime: invalid ({error})")],
    };
    let report = axion_runtime::diagnostic_report(&app, RunMode::Production);
    let mut lines = vec![format!(
        "runtime: app={}, mode={}, windows={}, errors={}, configured_dialog_backend={}, dialog_backend={}, resource_policy={}",
        report.app_name,
        report.mode,
        report.window_count,
        report.has_errors(),
        report.configured_dialog_backend.as_str(),
        report.dialog_backend.as_str(),
        report.resource_policy
    )];
    for window in report.windows {
        lines.push(format!(
            "runtime.window.{}: bridge={}, commands={}, events={}, frontend_events={}, host_events={}, startup_events={}, lifecycle_events={}, trusted_origins={}, navigation_origins={}, remote_navigation={}, csp={}",
            window.window_id,
            if window.bridge_enabled { "enabled" } else { "disabled" },
            window.command_count,
            window.event_count,
            list_or_none(&window.frontend_events),
            list_or_none(&window.host_events),
            window.startup_event_count,
            list_or_none(&window.lifecycle_events),
            list_or_none(&window.trusted_origins),
            list_or_none(&window.allowed_navigation_origins),
            window.allow_remote_navigation,
            window.content_security_policy,
        ));
    }
    for issue in report.issues {
        lines.push(format!(
            "runtime.issue.{:?}: {}",
            issue.severity, issue.message
        ));
    }
    lines
}

fn dev_server_diagnostic_line(config: &AppConfig) -> String {
    dev_server_diagnostic_line_with(config, dev_server_is_reachable)
}

fn dev_server_diagnostic_line_with(
    config: &AppConfig,
    is_reachable: impl Fn(&AppConfig) -> bool,
) -> String {
    let Some(dev_server) = &config.dev else {
        return "dev_server: not configured".to_owned();
    };

    if is_reachable(config) {
        format!("dev_server: reachable ({})", dev_server.url)
    } else {
        format!("dev_server: unreachable ({})", dev_server.url)
    }
}

fn print_servo_status(servo_path: Option<&Path>) {
    if let Some(servo_path) = servo_path {
        println!("servo: ok ({})", servo_path.display());
    } else {
        println!("servo: missing (searched manifest ancestors)");
    }
}

fn servo_path_for_manifest(manifest_path: &Path) -> Option<PathBuf> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    for ancestor in manifest_dir.ancestors() {
        let candidate = ancestor.join("servo");
        if candidate.join("components").join("servo").exists() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{
        AppConfig, AppIdentity, BuildConfig, CapabilityConfig, DevServerConfig, WindowConfig,
        WindowId,
    };
    use url::Url;

    use super::{
        build_asset_diagnostic_lines, bundle_diagnostic_lines, dev_server_diagnostic_line,
        dev_server_diagnostic_line_with, manifest_diagnostic_lines, runtime_diagnostic_lines,
        servo_path_for_manifest,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-doctor-test-{unique}-{serial}"))
    }

    #[test]
    fn servo_path_searches_manifest_ancestors() {
        let root = temp_dir();
        let app_dir = root.join("examples").join("hello");
        fs::create_dir_all(root.join("servo").join("components").join("servo")).unwrap();
        fs::create_dir_all(&app_dir).unwrap();

        assert_eq!(
            servo_path_for_manifest(&app_dir.join("axion.toml")),
            Some(root.join("servo"))
        );
    }

    #[test]
    fn manifest_diagnostics_include_windows_and_bridge_status() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test")
                .with_identifier("dev.axion.doctor")
                .with_version("1.2.3")
                .with_description("Doctor test app")
                .with_authors(["Axion Maintainers"])
                .with_homepage("https://example.dev/doctor"),
            windows: vec![
                WindowConfig::main("Main"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 480, 360),
            ],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([(
                "main".to_owned(),
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned(), "window.info".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                    allow_remote_navigation: false,
                },
            )]),
        };

        let lines = manifest_diagnostic_lines(&config);

        assert!(
            lines
                .iter()
                .any(|line| line == "app.identifier: dev.axion.doctor")
        );
        assert!(lines.iter().any(|line| line == "app.version: 1.2.3"));
        assert!(
            lines
                .iter()
                .any(|line| line == "app.description: Doctor test app")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "app.authors: Axion Maintainers")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "app.homepage: https://example.dev/doctor")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "native.dialog.backend: headless")
        );
        assert!(lines.iter().any(|line| line.contains("window.main")));
        assert!(
            lines
                .iter()
                .any(|line| line == "capabilities.main: bridge=enabled, commands=app.ping,window.info, events=app.log, protocols=axion, navigation_origins=https://docs.example, remote_navigation=false")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "capabilities.settings: none (bridge=disabled)")
        );
    }

    #[test]
    fn build_asset_diagnostics_report_valid_paths() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert_eq!(
            lines,
            vec![
                format!("frontend_dist: ok ({})", frontend.display()),
                format!("entry: ok ({}; relative=index.html)", entry.display())
            ]
        );
    }

    #[test]
    fn bundle_diagnostics_report_icon_status() {
        let root = temp_dir();
        let icon = root.join("icons").join("app.icns");
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(&icon, "icon").unwrap();

        let mut config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: axion_core::BundleConfig::new().with_icon(&icon),
            native: Default::default(),
            capabilities: Default::default(),
        };

        assert_eq!(
            bundle_diagnostic_lines(&config),
            vec![format!("bundle.icon: ok ({})", icon.display())]
        );

        config.bundle = axion_core::BundleConfig::new().with_icon(root.join("missing.icns"));
        assert!(
            bundle_diagnostic_lines(&config)
                .first()
                .is_some_and(|line| line.starts_with("bundle.icon: invalid"))
        );
    }

    #[test]
    fn runtime_diagnostics_report_launch_plan() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([(
                "main".to_owned(),
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )]),
        };

        let lines = runtime_diagnostic_lines(&config);

        assert!(lines.iter().any(|line| line.starts_with(
            "runtime: app=doctor-test, mode=production, windows=1, errors=false, configured_dialog_backend=headless, dialog_backend=headless, resource_policy="
        )));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("runtime.window.main: bridge=enabled"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("events=1, frontend_events=app.log, host_events=app.ready,window.created,window.close_requested,window.closed,window.resized,window.focused,window.blurred,window.moved,window.redraw_failed"))
        );
        assert!(lines.iter().any(|line| line.contains(
            "lifecycle_events=window.created,window.close_requested,window.closed,window.resized,window.focused,window.blurred,window.moved,window.redraw_failed"
        )));
    }

    #[test]
    fn runtime_diagnostics_report_multi_window_capabilities() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-multi-window"),
            windows: vec![
                WindowConfig::main("Main"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 520, 420),
            ],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([
                (
                    "main".to_owned(),
                    CapabilityConfig {
                        commands: vec!["app.ping".to_owned(), "app.info".to_owned()],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["axion".to_owned()],
                        allowed_navigation_origins: Vec::new(),
                        allow_remote_navigation: false,
                    },
                ),
                (
                    "settings".to_owned(),
                    CapabilityConfig {
                        commands: vec![
                            "window.info".to_owned(),
                            "window.focus".to_owned(),
                            "window.set_title".to_owned(),
                        ],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["axion".to_owned()],
                        allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                        allow_remote_navigation: false,
                    },
                ),
            ]),
        };

        let lines = runtime_diagnostic_lines(&config);

        assert!(lines.iter().any(|line| line.starts_with(
            "runtime: app=doctor-multi-window, mode=production, windows=2, errors=false, configured_dialog_backend=headless, dialog_backend=headless"
        )));
        assert!(lines.iter().any(|line| {
            line.starts_with("runtime.window.main: bridge=enabled, commands=2")
                && line.contains("frontend_events=app.log")
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with("runtime.window.settings: bridge=enabled, commands=3")
                && line.contains("navigation_origins=https://docs.example")
        }));
    }

    #[test]
    fn build_asset_diagnostics_report_missing_and_outside_entry() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = root.join("index.html");

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert_eq!(
            lines,
            vec![
                format!(
                    "build assets: invalid (frontend_dist '{}' must exist and be a directory)",
                    frontend.display()
                ),
                format!("frontend_dist: {}", frontend.display()),
                format!("entry: {}", entry.display())
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn build_asset_diagnostics_report_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        let external = root.join("external.txt");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();
        fs::write(&external, "external").unwrap();
        symlink(&external, frontend.join("external.txt")).unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("build assets: invalid"))
        );
        assert!(lines[0].contains("must not contain symlinks"));
    }

    #[test]
    fn build_asset_diagnostics_report_reserved_asset_manifest_path() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();
        fs::write(frontend.join("axion-assets.json"), "{}").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("build assets: invalid"))
        );
        assert!(lines[0].contains("reserved generated asset path"));
    }

    #[test]
    fn dev_server_diagnostics_report_unconfigured() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        assert_eq!(
            dev_server_diagnostic_line(&config),
            "dev_server: not configured"
        );
    }

    #[test]
    fn dev_server_diagnostics_report_reachable() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: Some(DevServerConfig {
                url: Url::parse("http://127.0.0.1:3000").unwrap(),
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        assert_eq!(
            dev_server_diagnostic_line_with(&config, |_| true),
            "dev_server: reachable (http://127.0.0.1:3000/)"
        );
        assert_eq!(
            dev_server_diagnostic_line_with(&config, |_| false),
            "dev_server: unreachable (http://127.0.0.1:3000/)"
        );
    }
}
