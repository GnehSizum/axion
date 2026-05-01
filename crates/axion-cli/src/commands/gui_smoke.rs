use std::path::{Path, PathBuf};
use std::process::Command;

use axion_core::{Builder, RunMode};
use axion_runtime::{DiagnosticsReport, json_string_literal};

use crate::cli::GuiSmokeArgs;
use crate::commands::report_util::{
    json_array_section, json_string_array_values, json_string_field, json_string_fields,
    matching_json_delimiter, next_json_object,
};
use crate::error::AxionCliError;

const PASSED_PREFIX: &str = "Axion GUI smoke passed: ";

pub fn run(args: GuiSmokeArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let cargo_manifest_path = cargo_manifest_path_for(&args.manifest_path)?;
    let build_env = parse_build_env(&args.build_env)?;
    validate_required_smoke_checks(&args.require_check)?;
    validate_required_report_items(&args.require_command, "--require-command")?;
    validate_required_report_items(&args.require_host_event, "--require-host-event")?;
    validate_required_report_items(&args.require_window, "--require-window")?;
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
            let summary = smoke_check_summary(report);
            if let Some(path) = &args.report_path {
                write_report_json(path, report)?;
            }
            let required = RequiredSmokeChecks::from_args(&args.require_check, &summary);
            let report_requirements = RequiredReportItems::from_args(
                &args.require_command,
                &args.require_host_event,
                &args.require_window,
                report,
            );
            if !required.is_satisfied() || !report_requirements.is_satisfied() {
                if let Some(path) = &args.report_path {
                    let report = runtime_policy_failed_report(RuntimePolicyFailedReportInput {
                        manifest_path: &args.manifest_path,
                        cargo_manifest_path: &cargo_manifest_path,
                        launch_config: &launch_config,
                        summary: &summary,
                        required: &required,
                        report_requirements: &report_requirements,
                        source_report: report,
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
                return Err(std::io::Error::other(format!(
                    "GUI smoke runtime requirements were not satisfied; {}; {}; {}; next_step: {}",
                    summary.to_line(),
                    required.to_line(),
                    report_requirements.to_line(),
                    runtime_policy_next_step(&required, &report_requirements)
                ))
                .into());
            }
            if !args.quiet {
                println!("Axion GUI smoke");
                println!("manifest: {}", args.manifest_path.display());
                println!("cargo_manifest: {}", cargo_manifest_path.display());
                if let Some(path) = &args.report_path {
                    println!("diagnostics_report: {}", path.display());
                }
                println!("{}", summary.to_line());
                if !required.required_ids.is_empty() {
                    println!("{}", required.to_line());
                }
                if !report_requirements.is_empty() {
                    println!("{}", report_requirements.to_line());
                }
                println!("result: ok");
            }
            Ok(())
        }
        (true, Some(report)) => {
            let summary = smoke_check_summary(report);
            if let Some(path) = &args.report_path {
                write_report_json(path, report)?;
            }
            Err(std::io::Error::other(format!(
                "GUI smoke report result was not ok; {}; next_step: {}",
                summary.to_line(),
                summary.next_step()
            ))
            .into())
        }
        _ => {
            let failure = gui_smoke_failure(output.status, report.is_some(), &stderr);
            let report_summary = report.map(smoke_check_summary);
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
                        next_step: failure.phase.next_step(),
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
            let message = gui_smoke_failure_message(&failure, report_summary.as_ref());
            Err(std::io::Error::other(message).into())
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmokeCheckSummary {
    total: usize,
    passed_ids: Vec<String>,
    failed_ids: Vec<String>,
    skipped_ids: Vec<String>,
    failed_error_codes: Vec<String>,
}

impl SmokeCheckSummary {
    fn to_line(&self) -> String {
        let failed = if self.failed_ids.is_empty() {
            "none".to_owned()
        } else {
            self.failed_ids.join(",")
        };
        let error_codes = if self.failed_error_codes.is_empty() {
            "none".to_owned()
        } else {
            self.failed_error_codes.join(",")
        };
        format!(
            "smoke_checks: total={}, failed={failed}, error_codes={error_codes}",
            self.total
        )
    }

    fn next_step(&self) -> String {
        if self.failed_ids.is_empty() {
            "inspect the returned diagnostics report and frontend console output".to_owned()
        } else {
            format!(
                "fix failing smoke checks: {}; inspect diagnostics.smoke_checks[].detail for command payloads and error codes",
                self.failed_ids.join(",")
            )
        }
    }
}

fn smoke_check_summary(report: &str) -> SmokeCheckSummary {
    let Some(checks) = json_array_section(report, "\"smoke_checks\"") else {
        return SmokeCheckSummary {
            total: 0,
            passed_ids: Vec::new(),
            failed_ids: Vec::new(),
            skipped_ids: Vec::new(),
            failed_error_codes: Vec::new(),
        };
    };

    let mut total = 0;
    let mut passed_ids = Vec::new();
    let mut failed_ids = Vec::new();
    let mut skipped_ids = Vec::new();
    let mut failed_error_codes = Vec::new();
    let mut cursor = 0;
    while let Some((object, next_cursor)) = next_json_object(checks, cursor) {
        total += 1;
        let id = json_string_field(object, "id").unwrap_or_else(|| format!("smoke-check-{total}"));
        match json_string_field(object, "status").as_deref() {
            Some("pass") => {
                if !passed_ids.contains(&id) {
                    passed_ids.push(id);
                }
            }
            Some("fail") => {
                if !failed_ids.contains(&id) {
                    failed_ids.push(id);
                }
                for code in json_string_fields(object, "code") {
                    if !failed_error_codes.contains(&code) {
                        failed_error_codes.push(code);
                    }
                }
            }
            Some("skip") => {
                if !skipped_ids.contains(&id) {
                    skipped_ids.push(id);
                }
            }
            _ => {}
        }
        cursor = next_cursor;
    }

    SmokeCheckSummary {
        total,
        passed_ids,
        failed_ids,
        skipped_ids,
        failed_error_codes,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredSmokeChecks {
    required_ids: Vec<String>,
    missing_ids: Vec<String>,
    failed_ids: Vec<String>,
    skipped_ids: Vec<String>,
}

impl RequiredSmokeChecks {
    fn from_args(required_ids: &[String], summary: &SmokeCheckSummary) -> Self {
        let mut normalized_required_ids = Vec::new();
        for id in required_ids {
            if !normalized_required_ids.contains(id) {
                normalized_required_ids.push(id.clone());
            }
        }

        let mut missing_ids = Vec::new();
        let mut failed_ids = Vec::new();
        let mut skipped_ids = Vec::new();

        for id in &normalized_required_ids {
            if summary.failed_ids.contains(id) {
                failed_ids.push(id.clone());
            } else if summary.skipped_ids.contains(id) {
                skipped_ids.push(id.clone());
            } else if !summary.passed_ids.contains(id) {
                missing_ids.push(id.clone());
            }
        }

        Self {
            required_ids: normalized_required_ids,
            missing_ids,
            failed_ids,
            skipped_ids,
        }
    }

    fn is_satisfied(&self) -> bool {
        self.missing_ids.is_empty() && self.failed_ids.is_empty() && self.skipped_ids.is_empty()
    }

    fn to_line(&self) -> String {
        format!(
            "required_checks: total={}, missing={}, failed={}, skipped={}",
            self.required_ids.len(),
            list_or_none(&self.missing_ids),
            list_or_none(&self.failed_ids),
            list_or_none(&self.skipped_ids)
        )
    }
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

fn validate_required_smoke_checks(values: &[String]) -> Result<(), AxionCliError> {
    for value in values {
        if value.is_empty()
            || !value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '_' | '-')
            })
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "invalid --require-check value '{value}'; use stable lower-case smoke check ids such as bridge.bootstrap or input.snapshot"
                ),
            )
            .into());
        }
    }

    Ok(())
}

fn validate_required_report_items(values: &[String], flag: &str) -> Result<(), AxionCliError> {
    for value in values {
        if value.is_empty()
            || !value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '_' | '-' | ':')
            })
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "invalid {flag} value '{value}'; use stable lower-case ids such as app.ping, window.ready, or main"
                ),
            )
            .into());
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredReportItems {
    required_commands: Vec<String>,
    required_host_events: Vec<String>,
    required_windows: Vec<String>,
    missing_commands: Vec<String>,
    missing_host_events: Vec<String>,
    missing_windows: Vec<String>,
}

impl RequiredReportItems {
    fn from_args(
        required_commands: &[String],
        required_host_events: &[String],
        required_windows: &[String],
        report: &str,
    ) -> Self {
        let required_commands = dedupe_strings(required_commands);
        let required_host_events = dedupe_strings(required_host_events);
        let required_windows = dedupe_strings(required_windows);
        let available_commands = report_array_values(report, "configured_commands");
        let mut available_host_events = report_array_values(report, "host_events");
        available_host_events.extend(report_array_values(report, "hostEvents"));
        available_host_events = dedupe_strings(&available_host_events);
        let available_windows = report_window_ids(report);

        let missing_commands = missing_values(&required_commands, &available_commands);
        let missing_host_events = missing_values(&required_host_events, &available_host_events);
        let missing_windows = missing_values(&required_windows, &available_windows);

        Self {
            required_commands,
            required_host_events,
            required_windows,
            missing_commands,
            missing_host_events,
            missing_windows,
        }
    }

    fn is_empty(&self) -> bool {
        self.required_commands.is_empty()
            && self.required_host_events.is_empty()
            && self.required_windows.is_empty()
    }

    fn is_satisfied(&self) -> bool {
        self.missing_commands.is_empty()
            && self.missing_host_events.is_empty()
            && self.missing_windows.is_empty()
    }

    fn to_line(&self) -> String {
        format!(
            "required_runtime: commands_missing={}, host_events_missing={}, windows_missing={}",
            list_or_none(&self.missing_commands),
            list_or_none(&self.missing_host_events),
            list_or_none(&self.missing_windows)
        )
    }

    fn missing_items(&self) -> Vec<String> {
        let mut values = Vec::new();
        values.extend(
            self.missing_commands
                .iter()
                .map(|value| format!("command:{value}")),
        );
        values.extend(
            self.missing_host_events
                .iter()
                .map(|value| format!("host_event:{value}")),
        );
        values.extend(
            self.missing_windows
                .iter()
                .map(|value| format!("window:{value}")),
        );
        values
    }
}

fn dedupe_strings(values: &[String]) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(value) {
            deduped.push(value.clone());
        }
    }
    deduped
}

fn missing_values(required: &[String], available: &[String]) -> Vec<String> {
    required
        .iter()
        .filter(|value| !available.contains(value))
        .cloned()
        .collect()
}

fn report_array_values(report: &str, field: &str) -> Vec<String> {
    let key = format!("\"{field}\"");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative_index) = report[cursor..].find(&key) {
        let key_index = cursor + relative_index;
        let Some(array_start) = report[key_index..].find('[').map(|start| key_index + start) else {
            break;
        };
        let Some(array_end) = matching_json_delimiter(report, array_start, '[', ']') else {
            break;
        };
        if let Some(section) = report.get(array_start + 1..array_end) {
            for value in json_string_array_values(section) {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        cursor = array_end + 1;
    }
    values
}

fn report_window_ids(report: &str) -> Vec<String> {
    let Some(windows) = json_array_section(report, "\"windows\"") else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    let mut cursor = 0;
    while let Some((window, next_cursor)) = next_json_object(windows, cursor) {
        if let Some(id) = json_string_field(window, "id") {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        cursor = next_cursor;
    }
    ids
}

fn runtime_policy_next_step(
    checks: &RequiredSmokeChecks,
    report_items: &RequiredReportItems,
) -> String {
    let mut blockers = Vec::new();
    blockers.extend(checks.missing_ids.iter().cloned());
    blockers.extend(checks.failed_ids.iter().cloned());
    blockers.extend(checks.skipped_ids.iter().cloned());
    blockers.extend(report_items.missing_items());
    if blockers.is_empty() {
        "inspect the returned diagnostics report".to_owned()
    } else {
        format!(
            "make required GUI smoke runtime coverage pass: {}; update window.__AXION_GUI_SMOKE__ or manifest capabilities if coverage is missing",
            blockers.join(",")
        )
    }
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

    fn next_step(self) -> &'static str {
        match self {
            Self::Build => {
                "fix Cargo/Servo build prerequisites, then rerun gui-smoke with --serial-build if the machine is resource constrained"
            }
            Self::Runtime => {
                "inspect frontend exceptions, bridge capability allowlists, and window.__AXION_GUI_SMOKE__ checks"
            }
            Self::Report => {
                "ensure window.__AXION_GUI_SMOKE__ prints a valid axion.diagnostics-report.v1 report"
            }
        }
    }
}

struct GuiSmokeFailure {
    message: String,
    phase: FailurePhase,
    help: &'static str,
}

fn gui_smoke_failure_message(
    failure: &GuiSmokeFailure,
    summary: Option<&SmokeCheckSummary>,
) -> String {
    if let Some(summary) = summary {
        format!(
            "{}; {}; next_step: {}",
            failure.message,
            summary.to_line(),
            summary.next_step()
        )
    } else {
        failure.message.clone()
    }
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
    next_step: &'a str,
    stdout: &'a str,
    stderr: &'a str,
    status: std::process::ExitStatus,
    report_found: bool,
    timeout_ms: Option<u64>,
    cargo_target_dir: Option<&'a Path>,
    build_env_keys: Vec<&'a str>,
    serial_build: bool,
}

struct RuntimePolicyFailedReportInput<'a> {
    manifest_path: &'a Path,
    cargo_manifest_path: &'a Path,
    launch_config: &'a axion_core::RuntimeLaunchConfig,
    summary: &'a SmokeCheckSummary,
    required: &'a RequiredSmokeChecks,
    report_requirements: &'a RequiredReportItems,
    source_report: &'a str,
    timeout_ms: Option<u64>,
    cargo_target_dir: Option<&'a Path>,
    build_env_keys: Vec<&'a str>,
    serial_build: bool,
}

fn runtime_policy_failed_report(input: RuntimePolicyFailedReportInput<'_>) -> String {
    let next_step = runtime_policy_next_step(input.required, input.report_requirements);
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
        configured_clipboard_backend: None,
        clipboard_backend: None,
        close_timeout_ms: Some(input.launch_config.native.lifecycle.close_timeout_ms),
        icon: None,
        host_events: Vec::new(),
        staged_app_dir: None,
        asset_manifest_path: None,
        artifacts_removed: None,
        diagnostics: Some(format!(
            "{{\"error\":\"required GUI smoke runtime coverage was not satisfied\",\"failure_phase\":\"runtime\",\"help\":{},\"next_step\":{},\"failed_check_ids\":{},\"error_codes\":{},\"required_checks\":{{\"required\":{},\"missing\":{},\"failed\":{},\"skipped\":{}}},\"required_runtime\":{{\"commands\":{},\"host_events\":{},\"windows\":{},\"missing_commands\":{},\"missing_host_events\":{},\"missing_windows\":{}}},\"smoke_check_summary\":{{\"total\":{},\"passed\":{},\"failed\":{},\"skipped\":{}}},\"status_code\":0,\"success\":false,\"report_found\":true,\"timeout_ms\":{},\"cargo_manifest_path\":{},\"cargo_target_dir\":{},\"serial_build\":{},\"build_env_keys\":{},\"source_report\":{}}}",
            json_string_literal(RUNTIME_FAILURE_HELP),
            json_string_literal(&next_step),
            string_vec_json(&input.summary.failed_ids),
            string_vec_json(&input.summary.failed_error_codes),
            string_vec_json(&input.required.required_ids),
            string_vec_json(&input.required.missing_ids),
            string_vec_json(&input.required.failed_ids),
            string_vec_json(&input.required.skipped_ids),
            string_vec_json(&input.report_requirements.required_commands),
            string_vec_json(&input.report_requirements.required_host_events),
            string_vec_json(&input.report_requirements.required_windows),
            string_vec_json(&input.report_requirements.missing_commands),
            string_vec_json(&input.report_requirements.missing_host_events),
            string_vec_json(&input.report_requirements.missing_windows),
            input.summary.total,
            string_vec_json(&input.summary.passed_ids),
            string_vec_json(&input.summary.failed_ids),
            string_vec_json(&input.summary.skipped_ids),
            optional_timeout_ms(input.timeout_ms),
            json_string_literal(&input.cargo_manifest_path.display().to_string()),
            optional_path_json_string(input.cargo_target_dir),
            input.serial_build,
            string_array_json(&input.build_env_keys),
            input.source_report,
        )),
        result: "failed".to_owned(),
    }
    .to_json()
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
        configured_clipboard_backend: None,
        clipboard_backend: None,
        close_timeout_ms: Some(input.launch_config.native.lifecycle.close_timeout_ms),
        icon: None,
        host_events: Vec::new(),
        staged_app_dir: None,
        asset_manifest_path: None,
        artifacts_removed: None,
        diagnostics: Some(format!(
            "{{\"error\":{},\"failure_phase\":{},\"help\":{},\"next_step\":{},\"failed_check_ids\":[],\"error_codes\":[],\"status_code\":{},\"success\":{},\"report_found\":{},\"timeout_ms\":{},\"cargo_manifest_path\":{},\"cargo_target_dir\":{},\"serial_build\":{},\"build_env_keys\":{},\"stdout\":{},\"stderr\":{}}}",
            json_string_literal(input.message),
            json_string_literal(input.failure_phase),
            json_string_literal(input.help),
            json_string_literal(input.next_step),
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

fn string_vec_json(values: &[String]) -> String {
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
    let runtime_markers = [
        "gui smoke failed",
        "winit(registerprotocol",
        "window.__axion_gui_smoke__",
        "axion_selftest",
    ];
    if runtime_markers.iter().any(|marker| stderr.contains(marker)) {
        return FailurePhase::Runtime;
    }

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
        FailedReportInput, FailurePhase, PASSED_PREFIX, RequiredReportItems, RequiredSmokeChecks,
        RuntimePolicyFailedReportInput, classify_failure_phase, extract_gui_smoke_report,
        failed_report, gui_smoke_failure, gui_smoke_failure_message, parse_build_env,
        report_array_values, report_result_ok, report_window_ids, runtime_policy_failed_report,
        runtime_policy_next_step, smoke_check_summary, validate_required_report_items,
        validate_required_smoke_checks,
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
    fn summarizes_gui_smoke_checks() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"failed\",",
            "\"diagnostics\":{\"smoke_checks\":[",
            "{\"id\":\"app.ping\",\"status\":\"pass\",\"detail\":\"ok\"},",
            "{\"id\":\"dialog.preview\",\"status\":\"skip\",\"detail\":\"unavailable\"},",
            "{\"id\":\"window.close\",\"status\":\"fail\",\"detail\":{\"error\":{\"code\":\"window.unavailable\"}}},",
            "{\"id\":\"window.prevent_close\",\"status\":\"fail\",\"detail\":{\"error\":{\"code\":\"window.duplicate\"}}}",
            "]}}"
        );
        let summary = smoke_check_summary(report);

        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed_ids, vec!["app.ping".to_owned()]);
        assert_eq!(summary.skipped_ids, vec!["dialog.preview".to_owned()]);
        assert_eq!(
            summary.failed_ids,
            vec!["window.close".to_owned(), "window.prevent_close".to_owned()]
        );
        assert_eq!(
            summary.failed_error_codes,
            vec![
                "window.unavailable".to_owned(),
                "window.duplicate".to_owned()
            ]
        );
        assert_eq!(
            summary.to_line(),
            "smoke_checks: total=4, failed=window.close,window.prevent_close, error_codes=window.unavailable,window.duplicate"
        );
    }

    #[test]
    fn summarizes_missing_gui_smoke_checks() {
        let summary =
            smoke_check_summary("{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\"}");

        assert_eq!(
            summary.to_line(),
            "smoke_checks: total=0, failed=none, error_codes=none"
        );
    }

    #[test]
    fn required_smoke_checks_report_missing_failed_and_skipped_ids() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\",",
            "\"diagnostics\":{\"smoke_checks\":[",
            "{\"id\":\"bridge.bootstrap\",\"status\":\"pass\"},",
            "{\"id\":\"input.snapshot\",\"status\":\"skip\"},",
            "{\"id\":\"window.close\",\"status\":\"fail\"}",
            "]}}"
        );
        let summary = smoke_check_summary(report);
        let required = RequiredSmokeChecks::from_args(
            &[
                "bridge.bootstrap".to_owned(),
                "input.snapshot".to_owned(),
                "window.close".to_owned(),
                "window.reload".to_owned(),
                "bridge.bootstrap".to_owned(),
            ],
            &summary,
        );

        assert!(!required.is_satisfied());
        assert_eq!(
            required.required_ids,
            vec![
                "bridge.bootstrap".to_owned(),
                "input.snapshot".to_owned(),
                "window.close".to_owned(),
                "window.reload".to_owned(),
            ]
        );
        assert_eq!(required.missing_ids, vec!["window.reload".to_owned()]);
        assert_eq!(required.failed_ids, vec!["window.close".to_owned()]);
        assert_eq!(required.skipped_ids, vec!["input.snapshot".to_owned()]);
        assert_eq!(
            required.to_line(),
            "required_checks: total=4, missing=window.reload, failed=window.close, skipped=input.snapshot"
        );
        assert!(!required.is_satisfied());
    }

    #[test]
    fn validates_required_smoke_check_ids() {
        assert!(
            validate_required_smoke_checks(&[
                "bridge.bootstrap".to_owned(),
                "input_snapshot".to_owned(),
                "window-close".to_owned()
            ])
            .is_ok()
        );
        assert!(validate_required_smoke_checks(&["".to_owned()]).is_err());
        assert!(validate_required_smoke_checks(&["Bridge.Bootstrap".to_owned()]).is_err());
    }

    #[test]
    fn required_report_items_find_missing_runtime_coverage() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\",",
            "\"windows\":[{\"id\":\"main\",\"configured_commands\":[\"app.ping\",\"window.info\"],",
            "\"host_events\":[\"window.ready\"]}],",
            "\"host_events\":[\"app.ready\",\"window.ready\"]}"
        );
        let required = RequiredReportItems::from_args(
            &["app.ping".to_owned(), "window.reload".to_owned()],
            &["window.ready".to_owned(), "window.closed".to_owned()],
            &["main".to_owned(), "settings".to_owned()],
            report,
        );

        assert!(!required.is_satisfied());
        assert_eq!(required.missing_commands, vec!["window.reload".to_owned()]);
        assert_eq!(
            required.missing_host_events,
            vec!["window.closed".to_owned()]
        );
        assert_eq!(required.missing_windows, vec!["settings".to_owned()]);
        assert_eq!(
            required.to_line(),
            "required_runtime: commands_missing=window.reload, host_events_missing=window.closed, windows_missing=settings"
        );
        assert!(
            runtime_policy_next_step(
                &RequiredSmokeChecks::from_args(&[], &smoke_check_summary(report)),
                &required
            )
            .contains("command:window.reload")
        );
    }

    #[test]
    fn report_runtime_extractors_collect_arrays_and_windows() {
        let report = concat!(
            "{\"windows\":[",
            "{\"id\":\"main\",\"configured_commands\":[\"app.ping\"],\"hostEvents\":[\"window.ready\"]},",
            "{\"id\":\"settings\",\"configured_commands\":[\"window.info\"],\"host_events\":[\"window.closed\"]}",
            "],\"host_events\":[\"app.ready\"]}"
        );

        assert_eq!(
            report_array_values(report, "configured_commands"),
            vec!["app.ping".to_owned(), "window.info".to_owned()]
        );
        assert_eq!(
            report_array_values(report, "host_events"),
            vec!["window.closed".to_owned(), "app.ready".to_owned()]
        );
        assert_eq!(
            report_array_values(report, "hostEvents"),
            vec!["window.ready".to_owned()]
        );
        assert_eq!(
            report_window_ids(report),
            vec!["main".to_owned(), "settings".to_owned()]
        );
    }

    #[test]
    fn validates_required_report_items() {
        assert!(
            validate_required_report_items(&["window.ready".to_owned()], "--require-host-event")
                .is_ok()
        );
        assert!(
            validate_required_report_items(&["Window.Ready".to_owned()], "--require-host-event")
                .is_err()
        );
    }

    #[test]
    fn failure_message_uses_last_stderr_line() {
        let failure = gui_smoke_failure(failing_status(), false, "warning\nerror detail\n");

        assert!(failure.message.contains("error detail"));
        assert_eq!(failure.phase, FailurePhase::Runtime);
    }

    #[test]
    fn failure_message_includes_report_summary_when_available() {
        let failure = gui_smoke_failure(failing_status(), true, "runtime failed");
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"failed\",",
            "\"diagnostics\":{\"smoke_checks\":[",
            "{\"id\":\"fs.roundtrip\",\"status\":\"fail\",\"detail\":{\"error\":{\"code\":\"fs.not-found\"}}}",
            "]}}"
        );
        let summary = smoke_check_summary(report);
        let message = gui_smoke_failure_message(&failure, Some(&summary));

        assert!(message.contains("smoke_checks: total=1, failed=fs.roundtrip"));
        assert!(message.contains("error_codes=fs.not-found"));
        assert!(message.contains("next_step: fix failing smoke checks: fs.roundtrip"));
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
            classify_failure_phase(
                failing_status(),
                "Compiling multi-window\nError: Winit(RegisterProtocol(\"GUI smoke failed: timed out after 30000ms\"))",
            ),
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
            next_step: "next action",
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
        assert!(report.contains("\"next_step\":\"next action\""));
        assert!(report.contains("\"failed_check_ids\":[]"));
        assert!(report.contains("\"error_codes\":[]"));
        assert!(report.contains("\"cargo_target_dir\":\"target\""));
        assert!(report.contains("\"serial_build\":true"));
        assert!(report.contains("\"build_env_keys\":[\"CARGO_BUILD_JOBS\"]"));
    }

    #[test]
    fn runtime_policy_failed_report_includes_policy_context() {
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
        let source_report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"ok\",",
            "\"diagnostics\":{\"smoke_checks\":[",
            "{\"id\":\"bridge.bootstrap\",\"status\":\"pass\"},",
            "{\"id\":\"input.snapshot\",\"status\":\"skip\"}",
            "]}}"
        );
        let summary = smoke_check_summary(source_report);
        let required = RequiredSmokeChecks::from_args(
            &["bridge.bootstrap".to_owned(), "input.snapshot".to_owned()],
            &summary,
        );
        let report_requirements = RequiredReportItems::from_args(
            &["app.ping".to_owned(), "window.reload".to_owned()],
            &["window.ready".to_owned()],
            &["main".to_owned()],
            source_report,
        );
        let report = runtime_policy_failed_report(RuntimePolicyFailedReportInput {
            manifest_path: std::path::Path::new("axion.toml"),
            cargo_manifest_path: std::path::Path::new("Cargo.toml"),
            launch_config: &launch_config,
            summary: &summary,
            required: &required,
            report_requirements: &report_requirements,
            source_report,
            timeout_ms: Some(30000),
            cargo_target_dir: Some(std::path::Path::new("target")),
            build_env_keys: vec!["CARGO_BUILD_JOBS"],
            serial_build: true,
        });

        assert!(report.contains("\"result\":\"failed\""));
        assert!(report.contains("\"failure_phase\":\"runtime\""));
        assert!(report.contains("\"required\":[\"bridge.bootstrap\",\"input.snapshot\"]"));
        assert!(report.contains("\"skipped\":[\"input.snapshot\"]"));
        assert!(report.contains("\"commands\":[\"app.ping\",\"window.reload\"]"));
        assert!(report.contains("\"missing_commands\":[\"app.ping\",\"window.reload\"]"));
        assert!(report.contains("\"missing_host_events\":[\"window.ready\"]"));
        assert!(report.contains("\"missing_windows\":[\"main\"]"));
        assert!(report.contains("\"source_report\":{\"schema\":\"axion.diagnostics-report.v1\""));
        assert!(report.contains("\"report_found\":true"));
    }
}
