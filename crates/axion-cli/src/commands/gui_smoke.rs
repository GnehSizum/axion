use std::path::{Path, PathBuf};
use std::process::Command;

use axion_core::{Builder, RunMode};
use axion_runtime::{DiagnosticsReport, json_string_literal};

use crate::cli::GuiSmokeArgs;
use crate::error::AxionCliError;

const PASSED_PREFIX: &str = "Axion GUI smoke passed: ";

pub fn run(args: GuiSmokeArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let cargo_manifest_path = cargo_manifest_path_for(&args.manifest_path)?;
    let build_env = parse_build_env(&args.build_env)?;
    let output = run_gui_smoke_process(&cargo_manifest_path, &args, &build_env)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report = extract_gui_smoke_report(&stdout);

    if !args.quiet {
        print!("{stdout}");
        eprint!("{stderr}");
    }

    match (output.status.success(), report) {
        (true, Some(report)) if report_result_ok(report) => {
            if let Some(path) = &args.report_path {
                write_report_json(path, report)?;
            }
            if !args.quiet {
                println!("Axion GUI smoke");
                println!("manifest: {}", args.manifest_path.display());
                println!("cargo_manifest: {}", cargo_manifest_path.display());
                if let Some(path) = &args.report_path {
                    println!("diagnostics_report: {}", path.display());
                }
                println!("result: ok");
            }
            Ok(())
        }
        (true, Some(report)) => {
            if let Some(path) = &args.report_path {
                write_report_json(path, report)?;
            }
            Err(std::io::Error::other("GUI smoke report result was not ok").into())
        }
        _ => {
            let failure = gui_smoke_failure(output.status, report.is_some(), &stderr);
            if let Some(path) = &args.report_path {
                if let Some(report) = report {
                    write_report_json(path, report)?;
                } else {
                    let report = failed_report(FailedReportInput {
                        manifest_path: &args.manifest_path,
                        cargo_manifest_path: &cargo_manifest_path,
                        launch_config: &launch_config,
                        message: &failure.message,
                        failure_phase: failure.phase.as_str(),
                        help: failure.help,
                        stdout: &stdout,
                        stderr: &stderr,
                        status: output.status,
                        report_found: false,
                        timeout_ms: args.timeout_ms,
                        cargo_target_dir: args.cargo_target_dir.as_deref(),
                        build_env_keys: build_env
                            .iter()
                            .map(|(key, _)| key.as_str())
                            .collect::<Vec<_>>(),
                        serial_build: args.serial_build,
                    });
                    write_report_json(path, &report)?;
                }
            }
            Err(std::io::Error::other(failure.message).into())
        }
    }
}

fn run_gui_smoke_process(
    cargo_manifest_path: &Path,
    args: &GuiSmokeArgs,
    build_env: &[(String, String)],
) -> Result<std::process::Output, AxionCliError> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .arg("run")
        .arg("--manifest-path")
        .arg(cargo_manifest_path)
        .arg("--features")
        .arg("servo-runtime")
        .env("AXION_GUI_SMOKE", "1");

    if let Some(target_dir) = &args.cargo_target_dir {
        command.env("CARGO_TARGET_DIR", target_dir);
    }

    if args.serial_build {
        command.env("CARGO_BUILD_JOBS", "1").env("MAKEFLAGS", "-j1");
    }

    for (key, value) in build_env {
        command.env(key, value);
    }

    if let Some(timeout_ms) = args.timeout_ms {
        command.env("AXION_GUI_SMOKE_TIMEOUT_MS", timeout_ms.to_string());
    }

    Ok(command.output()?)
}

fn cargo_manifest_path_for(manifest_path: &Path) -> Result<PathBuf, AxionCliError> {
    let cargo_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml");
    if cargo_manifest_path.is_file() {
        return Ok(cargo_manifest_path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "cannot run GUI smoke because Cargo.toml was not found next to manifest '{}'",
            manifest_path.display()
        ),
    )
    .into())
}

fn extract_gui_smoke_report(stdout: &str) -> Option<&str> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(PASSED_PREFIX))
        .map(str::trim)
        .filter(|report| {
            report.starts_with('{')
                && report.ends_with('}')
                && report.contains("\"schema\":\"axion.diagnostics-report.v1\"")
        })
}

fn report_result_ok(report: &str) -> bool {
    report.contains("\"result\":\"ok\"")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailurePhase {
    Build,
    Runtime,
    Report,
}

impl FailurePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Runtime => "runtime",
            Self::Report => "report",
        }
    }
}

struct GuiSmokeFailure {
    message: String,
    phase: FailurePhase,
    help: &'static str,
}

fn gui_smoke_failure(
    status: std::process::ExitStatus,
    report_found: bool,
    stderr: &str,
) -> GuiSmokeFailure {
    if report_found {
        return GuiSmokeFailure {
            message: format!("GUI smoke runtime exited unsuccessfully with status {status}"),
            phase: FailurePhase::Runtime,
            help: RUNTIME_FAILURE_HELP,
        };
    }

    let detail = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no diagnostics report was printed");
    let phase = classify_failure_phase(status, stderr);
    let help = match phase {
        FailurePhase::Build => BUILD_FAILURE_HELP,
        FailurePhase::Runtime => RUNTIME_FAILURE_HELP,
        FailurePhase::Report => REPORT_FAILURE_HELP,
    };
    GuiSmokeFailure {
        message: format!(
            "GUI smoke {} failed with status {status}: {detail}",
            phase.as_str()
        ),
        phase,
        help,
    }
}

struct FailedReportInput<'a> {
    manifest_path: &'a Path,
    cargo_manifest_path: &'a Path,
    launch_config: &'a axion_core::RuntimeLaunchConfig,
    message: &'a str,
    failure_phase: &'a str,
    help: &'a str,
    stdout: &'a str,
    stderr: &'a str,
    status: std::process::ExitStatus,
    report_found: bool,
    timeout_ms: Option<u64>,
    cargo_target_dir: Option<&'a Path>,
    build_env_keys: Vec<&'a str>,
    serial_build: bool,
}

fn failed_report(input: FailedReportInput<'_>) -> String {
    DiagnosticsReport {
        source: "axion-cli gui-smoke".to_owned(),
        exported_at_unix_seconds: Some(current_unix_timestamp_secs()),
        manifest_path: Some(input.manifest_path.to_path_buf()),
        app_name: input.launch_config.app_name.clone(),
        identifier: input.launch_config.identifier.clone(),
        version: input.launch_config.version.clone(),
        description: input.launch_config.description.clone(),
        authors: input.launch_config.authors.clone(),
        homepage: input.launch_config.homepage.clone(),
        mode: Some("production".to_owned()),
        window_count: input.launch_config.windows.len(),
        windows: Vec::new(),
        frontend_dist: Some(input.launch_config.frontend_dist.clone()),
        entry: Some(input.launch_config.packaged_entry.clone()),
        configured_dialog_backend: None,
        dialog_backend: None,
        icon: None,
        host_events: Vec::new(),
        staged_app_dir: None,
        asset_manifest_path: None,
        artifacts_removed: None,
        diagnostics: Some(format!(
            "{{\"error\":{},\"failure_phase\":{},\"help\":{},\"status_code\":{},\"success\":{},\"report_found\":{},\"timeout_ms\":{},\"cargo_manifest_path\":{},\"cargo_target_dir\":{},\"serial_build\":{},\"build_env_keys\":{},\"stdout\":{},\"stderr\":{}}}",
            json_string_literal(input.message),
            json_string_literal(input.failure_phase),
            json_string_literal(input.help),
            optional_status_code(input.status),
            input.status.success(),
            input.report_found,
            optional_timeout_ms(input.timeout_ms),
            json_string_literal(&input.cargo_manifest_path.display().to_string()),
            optional_path_json_string(input.cargo_target_dir),
            input.serial_build,
            string_array_json(&input.build_env_keys),
            json_string_literal(input.stdout),
            json_string_literal(input.stderr),
        )),
        result: "failed".to_owned(),
    }
    .to_json()
}

fn optional_status_code(status: std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_timeout_ms(timeout_ms: Option<u64>) -> String {
    timeout_ms
        .map(|timeout_ms| timeout_ms.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_path_json_string(path: Option<&Path>) -> String {
    path.map(|path| json_string_literal(&path.display().to_string()))
        .unwrap_or_else(|| "null".to_owned())
}

fn string_array_json(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn classify_failure_phase(status: std::process::ExitStatus, stderr: &str) -> FailurePhase {
    if status.success() {
        return FailurePhase::Report;
    }

    let stderr = stderr.to_ascii_lowercase();
    let build_markers = [
        "compiling ",
        "could not compile",
        "build failed",
        "failed to run custom build command",
        "failed to build",
        "error: failed to",
        "importerror",
    ];
    if build_markers.iter().any(|marker| stderr.contains(marker)) {
        FailurePhase::Build
    } else {
        FailurePhase::Runtime
    }
}

fn parse_build_env(values: &[String]) -> Result<Vec<(String, String)>, AxionCliError> {
    values
        .iter()
        .map(|value| {
            let (key, value) = value.split_once('=').ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid --build-env value '{value}'; expected KEY=VALUE"),
                )
            })?;
            if key.is_empty() || key.contains('\0') || value.contains('\0') {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid --build-env value '{key}=...'; keys and values must be non-NUL and keys must be non-empty"),
                )
                .into());
            }
            Ok((key.to_owned(), value.to_owned()))
        })
        .collect()
}

const BUILD_FAILURE_HELP: &str = "Cargo or Servo failed before the GUI smoke hook ran. Check stderr for Rust MSRV, Python >=3.11 or uv availability, LLVM/lld, macOS SDK, and dependency build errors. For generated apps, try --cargo-target-dir <axion-checkout>/target and --serial-build.";
const RUNTIME_FAILURE_HELP: &str = "The app started but exited before returning a valid GUI smoke report. Check frontend errors, bridge capability allowlists, and the window.__AXION_GUI_SMOKE__ hook.";
const REPORT_FAILURE_HELP: &str = "The process exited successfully but did not print a valid axion.diagnostics-report.v1 report. Ensure the frontend defines window.__AXION_GUI_SMOKE__ and returns a JSON report.";

fn write_report_json(path: &Path, report: &str) -> Result<(), AxionCliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(path, report)?;
    Ok(())
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        FailedReportInput, FailurePhase, PASSED_PREFIX, classify_failure_phase,
        extract_gui_smoke_report, failed_report, gui_smoke_failure, parse_build_env,
        report_result_ok,
    };

    #[cfg(unix)]
    fn failing_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        std::process::ExitStatus::from_raw(1 << 8)
    }

    #[cfg(windows)]
    fn failing_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;

        std::process::ExitStatus::from_raw(1)
    }

    #[test]
    fn extracts_gui_smoke_report_from_stdout() {
        let report = "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\"}";
        let stdout = format!("build output\n{PASSED_PREFIX}{report}\n");

        assert_eq!(extract_gui_smoke_report(&stdout), Some(report));
    }

    #[test]
    fn rejects_non_schema_output() {
        let stdout = format!("{PASSED_PREFIX}{{\"result\":\"ok\"}}\n");

        assert_eq!(extract_gui_smoke_report(&stdout), None);
    }

    #[test]
    fn report_result_must_be_ok() {
        assert!(report_result_ok(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\"}",
        ));
        assert!(!report_result_ok(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"failed\"}",
        ));
    }

    #[test]
    fn failure_message_uses_last_stderr_line() {
        let failure = gui_smoke_failure(failing_status(), false, "warning\nerror detail\n");

        assert!(failure.message.contains("error detail"));
        assert_eq!(failure.phase, FailurePhase::Runtime);
    }

    #[test]
    fn failure_phase_distinguishes_build_and_report_failures() {
        assert_eq!(
            classify_failure_phase(
                failing_status(),
                "error: failed to run custom build command"
            ),
            FailurePhase::Build,
        );
        assert_eq!(
            classify_failure_phase(failing_status(), "frontend crashed"),
            FailurePhase::Runtime,
        );
        assert_eq!(
            classify_failure_phase(success_status(), ""),
            FailurePhase::Report,
        );
    }

    #[cfg(unix)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;

        std::process::ExitStatus::from_raw(0)
    }

    #[test]
    fn parses_build_env_values() {
        let values = vec!["CARGO_BUILD_JOBS=1".to_owned(), "MAKEFLAGS=-j1".to_owned()];
        let parsed = parse_build_env(&values).expect("build env should parse");

        assert_eq!(
            parsed,
            vec![
                ("CARGO_BUILD_JOBS".to_owned(), "1".to_owned()),
                ("MAKEFLAGS".to_owned(), "-j1".to_owned()),
            ],
        );
        assert!(parse_build_env(&["INVALID".to_owned()]).is_err());
    }

    #[test]
    fn failed_report_includes_cli_context() {
        let launch_config = axion_core::RuntimeLaunchConfig {
            app_name: "gui-smoke-test".to_owned(),
            identifier: Some("dev.axion.gui-smoke-test".to_owned()),
            version: Some("0.1.0".to_owned()),
            description: None,
            authors: Vec::new(),
            homepage: None,
            mode: axion_core::RunMode::Production,
            entrypoint: axion_core::LaunchEntrypoint::Packaged(std::path::PathBuf::from(
                "frontend/index.html",
            )),
            frontend_dist: std::path::PathBuf::from("frontend"),
            packaged_entry: std::path::PathBuf::from("frontend/index.html"),
            native: axion_core::NativeConfig::default(),
            windows: Vec::new(),
        };
        let report = failed_report(FailedReportInput {
            manifest_path: std::path::Path::new("axion.toml"),
            cargo_manifest_path: std::path::Path::new("Cargo.toml"),
            launch_config: &launch_config,
            message: "failed",
            failure_phase: "build",
            help: "help text",
            stdout: "out",
            stderr: "err",
            status: failing_status(),
            report_found: false,
            timeout_ms: Some(30000),
            cargo_target_dir: Some(std::path::Path::new("target")),
            build_env_keys: vec!["CARGO_BUILD_JOBS"],
            serial_build: true,
        });

        assert!(report.contains("\"result\":\"failed\""));
        assert!(report.contains("\"status_code\":1"));
        assert!(report.contains("\"success\":false"));
        assert!(report.contains("\"report_found\":false"));
        assert!(report.contains("\"timeout_ms\":30000"));
        assert!(report.contains("\"cargo_manifest_path\":\"Cargo.toml\""));
        assert!(report.contains("\"failure_phase\":\"build\""));
        assert!(report.contains("\"help\":\"help text\""));
        assert!(report.contains("\"cargo_target_dir\":\"target\""));
        assert!(report.contains("\"serial_build\":true"));
        assert!(report.contains("\"build_env_keys\":[\"CARGO_BUILD_JOBS\"]"));
    }
}
