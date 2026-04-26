use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axion_core::{Builder, RunMode};
use axion_runtime::{DiagnosticsReport, DiagnosticsWindowReport};

use crate::cli::SelfTestArgs;
use crate::error::AxionCliError;

pub fn run(args: SelfTestArgs) -> Result<(), AxionCliError> {
    let report = run_self_test(&args)?;

    if let Some(path) = &args.report_path {
        write_report_json(path, &report)?;
    }

    if args.json {
        println!("{}", report.to_diagnostics_json());
        return Ok(());
    }

    if !args.quiet {
        print_human_report(&report, args.report_path.as_deref());
    }

    Ok(())
}

fn print_human_report(report: &SelfTestReport, report_path: Option<&Path>) {
    println!("Axion self-test");
    println!("manifest: {}", report.manifest_path.display());
    println!("app: {}", report.app_name);
    if let Some(identifier) = &report.identifier {
        println!("identifier: {identifier}");
    }
    if let Some(version) = &report.version {
        println!("version: {version}");
    }
    if let Some(description) = &report.description {
        println!("description: {description}");
    }
    if !report.authors.is_empty() {
        println!("authors: {}", report.authors.join(", "));
    }
    if let Some(homepage) = &report.homepage {
        println!("homepage: {homepage}");
    }
    println!("windows: {}", report.window_count);
    for window in &report.windows {
        println!(
            "window.{}: title={:?}, bridge={}, commands={}, events={}, protocols={}, runtime_commands={}, runtime_events={}",
            window.id,
            window.title,
            if window.bridge_enabled {
                "enabled"
            } else {
                "disabled"
            },
            list_or_none(&window.configured_commands),
            list_or_none(&window.configured_events),
            list_or_none(&window.configured_protocols),
            window.runtime_command_count,
            window.runtime_event_count,
        );
        println!(
            "window.{}.host_events: {}",
            window.id,
            list_or_none(&window.host_events)
        );
        println!(
            "window.{}.navigation: trusted_origins={}, allowed_origins={}, remote_navigation={}",
            window.id,
            list_or_none(&window.trusted_origins),
            list_or_none(&window.allowed_navigation_origins),
            window.allow_remote_navigation,
        );
    }
    println!("frontend_dist: {}", report.frontend_dist.display());
    println!("entry: {}", report.entry.display());
    println!(
        "native_dialog_backend: {} (configured={})",
        report.dialog_backend, report.configured_dialog_backend
    );
    match &report.icon {
        Some(icon) => println!("bundle_icon: {}", icon.display()),
        None => println!("bundle_icon: not configured"),
    }
    println!("runtime_errors: false");
    println!("host_events: {}", list_or_none(&report.host_events));
    println!("staged_app_dir: {}", report.staged_app_dir.display());
    println!("asset_manifest: {}", report.asset_manifest_path.display());
    if let Some(path) = report_path {
        println!("diagnostics_report: {}", path.display());
    }
    if report.artifacts_removed {
        println!("artifacts: removed");
    } else {
        println!("artifacts: kept");
    }
    println!("result: ok");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfTestReport {
    exported_at_unix_seconds: u64,
    manifest_path: PathBuf,
    app_name: String,
    identifier: Option<String>,
    version: Option<String>,
    description: Option<String>,
    authors: Vec<String>,
    homepage: Option<String>,
    window_count: usize,
    windows: Vec<SelfTestWindowReport>,
    frontend_dist: PathBuf,
    entry: PathBuf,
    configured_dialog_backend: String,
    dialog_backend: String,
    icon: Option<PathBuf>,
    host_events: Vec<String>,
    staged_app_dir: PathBuf,
    asset_manifest_path: PathBuf,
    artifacts_removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfTestWindowReport {
    id: String,
    title: String,
    bridge_enabled: bool,
    configured_commands: Vec<String>,
    configured_events: Vec<String>,
    configured_protocols: Vec<String>,
    runtime_command_count: usize,
    runtime_event_count: usize,
    host_events: Vec<String>,
    trusted_origins: Vec<String>,
    allowed_navigation_origins: Vec<String>,
    allow_remote_navigation: bool,
}

fn run_self_test(args: &SelfTestArgs) -> Result<SelfTestReport, AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config.clone()).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let diagnostics = axion_runtime::diagnostic_report(&app, RunMode::Production);
    if diagnostics.has_errors() {
        let message = diagnostics
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(std::io::Error::other(format!(
            "runtime diagnostics reported errors: {message}"
        ))
        .into());
    }

    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| default_output_dir(&args.manifest_path, &launch_config.app_name));
    let artifact = axion_packager::stage_web_assets(
        launch_config.frontend_dist.clone(),
        launch_config.packaged_entry.clone(),
        &output_dir,
    )?;
    let icon = axion_packager::validate_bundle_icon(config.bundle.icon.as_deref())?;
    let host_events = diagnostics
        .windows
        .iter()
        .flat_map(|window| window.host_events.iter().cloned())
        .fold(Vec::new(), |mut events, event| {
            if !events.contains(&event) {
                events.push(event);
            }
            events
        });
    let windows = launch_config
        .windows
        .iter()
        .map(|window| {
            let capability = config.capabilities.get(&window.id);
            let diagnostic = diagnostics
                .windows
                .iter()
                .find(|diagnostic| diagnostic.window_id == window.id);
            let configured_protocols = capability
                .map(|capability| capability.protocols.clone())
                .unwrap_or_default();

            SelfTestWindowReport {
                id: window.id.clone(),
                title: window.title.clone(),
                bridge_enabled: configured_protocols
                    .iter()
                    .any(|protocol| protocol == "axion"),
                configured_commands: capability
                    .map(|capability| capability.commands.clone())
                    .unwrap_or_default(),
                configured_events: capability
                    .map(|capability| capability.events.clone())
                    .unwrap_or_default(),
                configured_protocols,
                runtime_command_count: diagnostic
                    .map(|diagnostic| diagnostic.command_count)
                    .unwrap_or_default(),
                runtime_event_count: diagnostic
                    .map(|diagnostic| diagnostic.event_count)
                    .unwrap_or_default(),
                host_events: diagnostic
                    .map(|diagnostic| diagnostic.host_events.clone())
                    .unwrap_or_default(),
                trusted_origins: diagnostic
                    .map(|diagnostic| diagnostic.trusted_origins.clone())
                    .unwrap_or_default(),
                allowed_navigation_origins: diagnostic
                    .map(|diagnostic| diagnostic.allowed_navigation_origins.clone())
                    .unwrap_or_default(),
                allow_remote_navigation: diagnostic
                    .map(|diagnostic| diagnostic.allow_remote_navigation)
                    .unwrap_or_default(),
            }
        })
        .collect();
    let staged_app_dir = artifact.app_dir.clone();
    let asset_manifest_path = artifact.asset_manifest_path.clone();
    let artifacts_removed = if args.keep_artifacts {
        false
    } else {
        std::fs::remove_dir_all(&output_dir)?;
        true
    };

    Ok(SelfTestReport {
        exported_at_unix_seconds: current_unix_timestamp_secs(),
        manifest_path: args.manifest_path.clone(),
        app_name: launch_config.app_name.clone(),
        identifier: launch_config.identifier.clone(),
        version: launch_config.version.clone(),
        description: launch_config.description.clone(),
        authors: launch_config.authors.clone(),
        homepage: launch_config.homepage.clone(),
        window_count: launch_config.windows.len(),
        windows,
        frontend_dist: launch_config.frontend_dist.clone(),
        entry: launch_config.packaged_entry.clone(),
        configured_dialog_backend: diagnostics.configured_dialog_backend.as_str().to_owned(),
        dialog_backend: diagnostics.dialog_backend.as_str().to_owned(),
        icon,
        host_events,
        staged_app_dir,
        asset_manifest_path,
        artifacts_removed,
    })
}

impl SelfTestReport {
    fn to_diagnostics_json(&self) -> String {
        self.to_diagnostics_report().to_json()
    }

    fn to_diagnostics_report(&self) -> DiagnosticsReport {
        let windows = self
            .windows
            .iter()
            .map(|window| DiagnosticsWindowReport {
                id: window.id.clone(),
                title: window.title.clone(),
                bridge_enabled: window.bridge_enabled,
                configured_commands: window.configured_commands.clone(),
                configured_events: window.configured_events.clone(),
                configured_protocols: window.configured_protocols.clone(),
                runtime_command_count: window.runtime_command_count,
                runtime_event_count: window.runtime_event_count,
                host_events: window.host_events.clone(),
                trusted_origins: window.trusted_origins.clone(),
                allowed_navigation_origins: window.allowed_navigation_origins.clone(),
                allow_remote_navigation: window.allow_remote_navigation,
            })
            .collect();

        DiagnosticsReport {
            source: "axion-cli self-test".to_owned(),
            exported_at_unix_seconds: Some(self.exported_at_unix_seconds),
            manifest_path: Some(self.manifest_path.clone()),
            app_name: self.app_name.clone(),
            identifier: self.identifier.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            authors: self.authors.clone(),
            homepage: self.homepage.clone(),
            mode: Some("production".to_owned()),
            window_count: self.window_count,
            windows,
            frontend_dist: Some(self.frontend_dist.clone()),
            entry: Some(self.entry.clone()),
            configured_dialog_backend: Some(self.configured_dialog_backend.clone()),
            dialog_backend: Some(self.dialog_backend.clone()),
            icon: self.icon.clone(),
            host_events: self.host_events.clone(),
            staged_app_dir: Some(self.staged_app_dir.clone()),
            asset_manifest_path: Some(self.asset_manifest_path.clone()),
            artifacts_removed: Some(self.artifacts_removed),
            result: "ok".to_owned(),
        }
    }
}

fn write_report_json(path: &Path, report: &SelfTestReport) -> Result<(), AxionCliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(path, report.to_diagnostics_json())?;
    Ok(())
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs()
}

fn default_output_dir(manifest_path: &Path, app_name: &str) -> PathBuf {
    static SELF_TEST_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_nanos();
    let counter = SELF_TEST_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("target")
        .join("axion")
        .join(app_name)
        .join("self-test")
        .join(format!("{unique}-{counter}"))
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{default_output_dir, run_self_test, write_report_json};
    use crate::cli::SelfTestArgs;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-self-test-{unique}-{serial}"))
    }

    fn write_project(root: &Path) -> PathBuf {
        let frontend = root.join("frontend");
        let icons = root.join("icons");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&icons).unwrap();
        fs::write(frontend.join("index.html"), "<!doctype html><html></html>").unwrap();
        fs::write(frontend.join("app.js"), "console.log('axion');").unwrap();
        fs::write(icons.join("app.icns"), "icon").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(
            &manifest,
            r#"
[app]
name = "self-test-app"
identifier = "dev.axion.self-test"
version = "1.2.3"
description = "Self-test fixture"
authors = ["Axion Tests"]
homepage = "https://example.dev/self-test"

[window]
id = "main"
title = "Self Test"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[bundle]
icon = "icons/app.icns"

[capabilities.main]
commands = ["app.ping"]
events = ["app.log"]
protocols = ["axion"]
"#,
        )
        .unwrap();
        manifest
    }

    fn write_multi_window_project(root: &Path) -> PathBuf {
        let frontend = root.join("frontend");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(frontend.join("index.html"), "<!doctype html><html></html>").unwrap();
        fs::write(frontend.join("app.js"), "console.log('axion');").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(
            &manifest,
            r#"
[app]
name = "self-test-multi-window"

[[windows]]
id = "main"
title = "Main"

[[windows]]
id = "settings"
title = "Settings"
width = 520
height = 420

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = ["app.ping", "app.info"]
events = ["app.log"]
protocols = ["axion"]

[capabilities.settings]
commands = ["window.info", "window.focus", "window.set_title"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
"#,
        )
        .unwrap();
        manifest
    }

    #[test]
    fn default_output_dir_is_unique_and_workspace_local() {
        let first = default_output_dir(Path::new("/tmp/demo/axion.toml"), "hello-axion");
        let second = default_output_dir(Path::new("/tmp/demo/axion.toml"), "hello-axion");

        assert!(first.starts_with("/tmp/demo/target/axion/hello-axion/self-test"));
        assert_ne!(first, second);
    }

    #[test]
    fn self_test_stages_and_removes_artifacts_by_default() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let manifest = write_project(&root);
        let output_dir = root.join("self-test-output");

        let report = run_self_test(&SelfTestArgs {
            manifest_path: manifest,
            output_dir: Some(output_dir.clone()),
            report_path: None,
            json: false,
            quiet: false,
            keep_artifacts: false,
        })
        .expect("self-test should pass");

        assert_eq!(report.app_name, "self-test-app");
        assert_eq!(report.identifier.as_deref(), Some("dev.axion.self-test"));
        assert_eq!(report.version.as_deref(), Some("1.2.3"));
        assert_eq!(report.description.as_deref(), Some("Self-test fixture"));
        assert_eq!(report.authors, vec!["Axion Tests".to_owned()]);
        assert_eq!(
            report.homepage.as_deref(),
            Some("https://example.dev/self-test")
        );
        assert!(report.icon.as_ref().is_some_and(|icon| {
            icon.file_name()
                .is_some_and(|file_name| file_name == "app.icns")
        }));
        assert_eq!(report.configured_dialog_backend, "headless");
        assert_eq!(report.dialog_backend, "headless");
        assert_eq!(report.windows.len(), 1);
        assert_eq!(report.windows[0].id, "main");
        assert!(report.windows[0].bridge_enabled);
        assert_eq!(
            report.windows[0].configured_commands,
            vec!["app.ping".to_owned()]
        );
        assert_eq!(
            report.windows[0].configured_events,
            vec!["app.log".to_owned()]
        );
        assert_eq!(
            report.windows[0].configured_protocols,
            vec!["axion".to_owned()]
        );
        assert_eq!(report.windows[0].runtime_command_count, 1);
        assert_eq!(report.windows[0].runtime_event_count, 1);
        assert!(
            report.windows[0]
                .trusted_origins
                .contains(&"axion://app".to_owned())
        );
        assert!(report.host_events.contains(&"app.ready".to_owned()));
        assert!(report.artifacts_removed);
        assert!(!output_dir.exists());
    }

    #[test]
    fn self_test_can_keep_artifacts() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let manifest = write_project(&root);
        let output_dir = root.join("kept-output");

        let report = run_self_test(&SelfTestArgs {
            manifest_path: manifest,
            output_dir: Some(output_dir.clone()),
            report_path: None,
            json: false,
            quiet: false,
            keep_artifacts: true,
        })
        .expect("self-test should pass");

        assert!(!report.artifacts_removed);
        assert!(report.staged_app_dir.exists());
        assert!(report.asset_manifest_path.exists());
    }

    #[test]
    fn self_test_reports_multi_window_manifests() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let manifest = write_multi_window_project(&root);
        let output_dir = root.join("multi-window-output");

        let report = run_self_test(&SelfTestArgs {
            manifest_path: manifest,
            output_dir: Some(output_dir.clone()),
            report_path: None,
            json: false,
            quiet: false,
            keep_artifacts: false,
        })
        .expect("multi-window self-test should pass");

        assert_eq!(report.app_name, "self-test-multi-window");
        assert_eq!(report.window_count, 2);
        assert_eq!(report.windows.len(), 2);
        assert_eq!(report.windows[0].id, "main");
        assert_eq!(
            report.windows[0].configured_commands,
            vec!["app.info".to_owned(), "app.ping".to_owned()]
        );
        assert_eq!(report.windows[0].runtime_command_count, 2);
        assert_eq!(report.windows[1].id, "settings");
        assert_eq!(
            report.windows[1].configured_commands,
            vec![
                "window.focus".to_owned(),
                "window.info".to_owned(),
                "window.set_title".to_owned()
            ]
        );
        assert_eq!(report.windows[1].runtime_command_count, 3);
        assert_eq!(
            report.windows[1].allowed_navigation_origins,
            vec!["https://docs.example".to_owned()]
        );
        assert!(report.host_events.contains(&"app.ready".to_owned()));
        assert!(
            report
                .host_events
                .contains(&"window.close_requested".to_owned())
        );
        assert!(report.host_events.contains(&"window.focused".to_owned()));
        assert!(report.artifacts_removed);
    }

    #[test]
    fn self_test_report_serializes_diagnostics_json() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let manifest = write_project(&root);
        let output_dir = root.join("json-output");

        let report = run_self_test(&SelfTestArgs {
            manifest_path: manifest,
            output_dir: Some(output_dir),
            report_path: None,
            json: false,
            quiet: false,
            keep_artifacts: false,
        })
        .expect("self-test should pass");
        let json = report.to_diagnostics_json();

        assert!(json.contains("\"schema\":\"axion.diagnostics-report.v1\""));
        assert!(json.contains("\"source\":\"axion-cli self-test\""));
        assert!(json.contains("\"app_name\":\"self-test-app\""));
        assert!(json.contains("\"window_count\":1"));
        assert!(json.contains("\"configured_commands\":[\"app.ping\"]"));
        assert!(json.contains("\"result\":\"ok\""));
    }

    #[test]
    fn self_test_report_writes_json_file() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let manifest = write_project(&root);
        let output_dir = root.join("write-report-output");
        let report_path = root.join("reports").join("self-test.json");

        let report = run_self_test(&SelfTestArgs {
            manifest_path: manifest,
            output_dir: Some(output_dir),
            report_path: None,
            json: false,
            quiet: false,
            keep_artifacts: false,
        })
        .expect("self-test should pass");

        write_report_json(&report_path, &report).expect("report json should be written");
        let contents = fs::read_to_string(report_path).expect("report json should be readable");
        assert!(contents.contains("\"schema\":\"axion.diagnostics-report.v1\""));
        assert!(contents.contains("\"source\":\"axion-cli self-test\""));
    }
}
