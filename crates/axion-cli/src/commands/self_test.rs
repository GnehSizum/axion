use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axion_core::{Builder, RunMode};

use crate::cli::SelfTestArgs;
use crate::error::AxionCliError;

pub fn run(args: SelfTestArgs) -> Result<(), AxionCliError> {
    let report = run_self_test(&args)?;

    println!("Axion self-test");
    println!("manifest: {}", report.manifest_path.display());
    println!("app: {}", report.app_name);
    println!("windows: {}", report.window_count);
    println!("frontend_dist: {}", report.frontend_dist.display());
    println!("entry: {}", report.entry.display());
    println!("runtime_errors: false");
    println!("host_events: {}", list_or_none(&report.host_events));
    println!("staged_app_dir: {}", report.staged_app_dir.display());
    println!("asset_manifest: {}", report.asset_manifest_path.display());
    if report.artifacts_removed {
        println!("artifacts: removed");
    } else {
        println!("artifacts: kept");
    }
    println!("result: ok");

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelfTestReport {
    manifest_path: PathBuf,
    app_name: String,
    window_count: usize,
    frontend_dist: PathBuf,
    entry: PathBuf,
    host_events: Vec<String>,
    staged_app_dir: PathBuf,
    asset_manifest_path: PathBuf,
    artifacts_removed: bool,
}

fn run_self_test(args: &SelfTestArgs) -> Result<SelfTestReport, AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
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
    let staged_app_dir = artifact.app_dir.clone();
    let asset_manifest_path = artifact.asset_manifest_path.clone();
    let artifacts_removed = if args.keep_artifacts {
        false
    } else {
        std::fs::remove_dir_all(&output_dir)?;
        true
    };

    Ok(SelfTestReport {
        manifest_path: args.manifest_path.clone(),
        app_name: launch_config.app_name,
        window_count: launch_config.windows.len(),
        frontend_dist: launch_config.frontend_dist,
        entry: launch_config.packaged_entry,
        host_events,
        staged_app_dir,
        asset_manifest_path,
        artifacts_removed,
    })
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

    use super::{default_output_dir, run_self_test};
    use crate::cli::SelfTestArgs;

    fn temp_dir() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("axion-self-test-{unique}"))
    }

    fn write_project(root: &Path) -> PathBuf {
        let frontend = root.join("frontend");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(frontend.join("index.html"), "<!doctype html><html></html>").unwrap();
        fs::write(frontend.join("app.js"), "console.log('axion');").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(
            &manifest,
            r#"
[app]
name = "self-test-app"

[window]
id = "main"
title = "Self Test"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = ["app.ping"]
events = ["app.log"]
protocols = ["axion"]
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
            keep_artifacts: false,
        })
        .expect("self-test should pass");

        assert_eq!(report.app_name, "self-test-app");
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
            keep_artifacts: true,
        })
        .expect("self-test should pass");

        assert!(!report.artifacts_removed);
        assert!(report.staged_app_dir.exists());
        assert!(report.asset_manifest_path.exists());
    }
}
