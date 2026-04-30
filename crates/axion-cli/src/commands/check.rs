use axion_core::{Builder, RunMode};
use axion_runtime::json_string_literal;

use crate::cli::{CheckArgs, DoctorArgs, SelfTestArgs};
use crate::commands::doctor::{doctor_gate_for_manifest, doctor_readiness_for_manifest};
use crate::error::AxionCliError;

pub fn run(args: CheckArgs) -> Result<(), AxionCliError> {
    let report = check_report(&args);

    if args.json {
        println!("{}", report.to_json());
    } else {
        report.print_human();
    }

    if report.result == "failed" {
        return Err(std::io::Error::other("check failed").into());
    }

    Ok(())
}

fn check_report(args: &CheckArgs) -> CheckReport {
    let mut report = CheckReport::new(args);

    let doctor_args = DoctorArgs {
        manifest_path: args.manifest_path.clone(),
        json: false,
        deny_warnings: true,
        max_risk: Some(args.max_risk),
    };

    match doctor_gate_for_manifest(&doctor_args) {
        Ok(gate) => {
            report.doctor_passed = gate.passed_status();
            report.doctor_failures = gate.failed_reasons().to_vec();
        }
        Err(error) => {
            report.doctor_passed = false;
            report.doctor_failures.push(error.to_string());
        }
    }

    match doctor_readiness_for_manifest(&args.manifest_path) {
        Ok(readiness) => {
            report.ready_for_dev = readiness.ready_for_dev();
            report.ready_for_bundle = readiness.ready_for_bundle();
            report.ready_for_gui_smoke = readiness.ready_for_gui_smoke();
            report.readiness_blockers = readiness.blockers().to_vec();
            report.readiness_warnings = readiness.warnings().to_vec();
        }
        Err(error) => {
            report.ready_for_dev = false;
            report.ready_for_bundle = false;
            report.ready_for_gui_smoke = false;
            report.readiness_blockers.push(error.to_string());
        }
    }

    if report.doctor_passed && report.ready_for_dev {
        match crate::commands::self_test::run(SelfTestArgs {
            manifest_path: args.manifest_path.clone(),
            output_dir: None,
            report_path: None,
            json: false,
            quiet: true,
            keep_artifacts: args.keep_artifacts,
        }) {
            Ok(()) => report.self_test_passed = true,
            Err(error) => {
                report.self_test_passed = false;
                report.self_test_error = Some(error.to_string());
            }
        }
    }

    if args.bundle {
        report.bundle_preflight_checked = true;
        if report.ready_for_bundle {
            match bundle_preflight(&args.manifest_path) {
                Ok(()) => report.bundle_preflight_passed = Some(true),
                Err(error) => {
                    report.bundle_preflight_passed = Some(false);
                    report.bundle_preflight_error = Some(error.to_string());
                }
            }
        } else {
            report.bundle_preflight_passed = Some(false);
            report.bundle_preflight_error =
                Some("manifest is not ready for bundle checks".to_owned());
        }
    }

    if args.dev {
        report.dev_preflight = Some(dev_preflight(&args.manifest_path));
    }

    report.finalize();
    report
}

fn bundle_preflight(manifest_path: &std::path::Path) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(manifest_path)?;
    axion_packager::validate_web_assets(&config.build.frontend_dist, &config.build.entry)?;
    axion_packager::validate_bundle_icon(config.bundle.icon.as_deref())?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckReport {
    manifest_path: String,
    max_risk: String,
    bundle_requested: bool,
    doctor_passed: bool,
    doctor_failures: Vec<String>,
    ready_for_dev: bool,
    ready_for_bundle: bool,
    ready_for_gui_smoke: bool,
    readiness_blockers: Vec<String>,
    readiness_warnings: Vec<String>,
    self_test_passed: bool,
    self_test_error: Option<String>,
    bundle_preflight_checked: bool,
    bundle_preflight_passed: Option<bool>,
    bundle_preflight_error: Option<String>,
    dev_preflight: Option<DevPreflightReport>,
    next_step: String,
    result: String,
}

impl CheckReport {
    fn new(args: &CheckArgs) -> Self {
        Self {
            manifest_path: args.manifest_path.display().to_string(),
            max_risk: args.max_risk.as_str().to_owned(),
            bundle_requested: args.bundle,
            doctor_passed: false,
            doctor_failures: Vec::new(),
            ready_for_dev: false,
            ready_for_bundle: false,
            ready_for_gui_smoke: false,
            readiness_blockers: Vec::new(),
            readiness_warnings: Vec::new(),
            self_test_passed: false,
            self_test_error: None,
            bundle_preflight_checked: false,
            bundle_preflight_passed: None,
            bundle_preflight_error: None,
            dev_preflight: None,
            next_step: String::new(),
            result: "failed".to_owned(),
        }
    }

    fn finalize(&mut self) {
        let bundle_ok = !self.bundle_requested || self.bundle_preflight_passed == Some(true);
        let dev_ok = self
            .dev_preflight
            .as_ref()
            .is_none_or(DevPreflightReport::passed);
        let passed = self.doctor_passed
            && self.ready_for_dev
            && self.self_test_passed
            && bundle_ok
            && dev_ok;

        self.result = if passed { "ok" } else { "failed" }.to_owned();
        self.next_step = if !self.doctor_passed {
            "run axion doctor and resolve gate failures".to_owned()
        } else if !self.ready_for_dev {
            "resolve readiness.blocker entries before running heavier checks".to_owned()
        } else if !self.self_test_passed {
            "run axion self-test for full staging diagnostics".to_owned()
        } else if self.bundle_requested && self.bundle_preflight_passed != Some(true) {
            "fix bundle preflight issues before running axion bundle --build-executable".to_owned()
        } else if !dev_ok {
            "fix dev preflight blockers before using axion dev --launch".to_owned()
        } else if self.ready_for_gui_smoke {
            "run axion gui-smoke, then axion bundle --build-executable".to_owned()
        } else if self.ready_for_bundle {
            "run axion bundle --build-executable; GUI smoke needs additional setup".to_owned()
        } else {
            "run axion doctor --deny-warnings --max-risk medium for detailed diagnostics".to_owned()
        };
    }

    fn print_human(&self) {
        println!("Axion check");
        println!("manifest: {}", self.manifest_path);
        println!(
            "doctor: {}",
            if self.doctor_passed { "ok" } else { "failed" }
        );
        for reason in &self.doctor_failures {
            println!("doctor.failure: {reason}");
        }
        println!(
            "readiness: dev={}, bundle={}, gui_smoke={}",
            self.ready_for_dev, self.ready_for_bundle, self.ready_for_gui_smoke
        );
        for blocker in &self.readiness_blockers {
            println!("readiness.blocker: {blocker}");
        }
        for warning in &self.readiness_warnings {
            println!("readiness.warning: {warning}");
        }
        println!(
            "self_test: {}",
            if self.self_test_passed {
                "ok"
            } else if self.doctor_passed && self.ready_for_dev {
                "failed"
            } else {
                "skipped"
            }
        );
        if let Some(error) = &self.self_test_error {
            println!("self_test.error: {error}");
        }
        if self.bundle_preflight_checked {
            match self.bundle_preflight_passed {
                Some(true) => println!(
                    "bundle.preflight: ok (target={})",
                    axion_packager::current_bundle_target().as_str()
                ),
                Some(false) => println!(
                    "bundle.preflight: failed ({})",
                    self.bundle_preflight_error.as_deref().unwrap_or("unknown")
                ),
                None => println!("bundle.preflight: skipped"),
            }
        } else {
            println!("bundle.preflight: skipped (pass --bundle to enable)");
        }
        if let Some(dev) = &self.dev_preflight {
            dev.print_human();
        } else {
            println!("dev.preflight: skipped (pass --dev to enable)");
        }
        println!("next_step: {}", self.next_step);
        println!("result: {}", self.result);
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"axion.check-report.v1\",\"manifest_path\":{},\"max_risk\":{},\"bundle_requested\":{},\"dev_requested\":{},\"doctor\":{{\"passed\":{},\"failed_reasons\":{}}},\"readiness\":{{\"ready_for_dev\":{},\"ready_for_bundle\":{},\"ready_for_gui_smoke\":{},\"blockers\":{},\"warnings\":{}}},\"self_test\":{{\"passed\":{},\"error\":{}}},\"bundle_preflight\":{{\"checked\":{},\"passed\":{},\"error\":{}}},\"dev_preflight\":{},\"next_step\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            json_string_literal(&self.max_risk),
            self.bundle_requested,
            self.dev_preflight.is_some(),
            self.doctor_passed,
            json_string_array_literal(&self.doctor_failures),
            self.ready_for_dev,
            self.ready_for_bundle,
            self.ready_for_gui_smoke,
            json_string_array_literal(&self.readiness_blockers),
            json_string_array_literal(&self.readiness_warnings),
            self.self_test_passed,
            optional_json_string_literal(self.self_test_error.as_deref()),
            self.bundle_preflight_checked,
            optional_json_bool(self.bundle_preflight_passed),
            optional_json_string_literal(self.bundle_preflight_error.as_deref()),
            self.dev_preflight
                .as_ref()
                .map(DevPreflightReport::to_json)
                .unwrap_or_else(|| "{\"checked\":false}".to_owned()),
            json_string_literal(&self.next_step),
            json_string_literal(&self.result),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DevPreflightReport {
    manifest_loaded: bool,
    dev_server_status: String,
    dev_server_url: Option<String>,
    frontend_command_configured: bool,
    frontend_cwd: Option<String>,
    frontend_timeout_ms: Option<u64>,
    watch_root: String,
    packaged_fallback: String,
    event_log_hint: String,
    report_path_hint: String,
    blockers: Vec<String>,
    recommendations: Vec<String>,
}

impl DevPreflightReport {
    fn failed(error: impl Into<String>) -> Self {
        Self {
            manifest_loaded: false,
            dev_server_status: "unknown".to_owned(),
            dev_server_url: None,
            frontend_command_configured: false,
            frontend_cwd: None,
            frontend_timeout_ms: None,
            watch_root: "unknown".to_owned(),
            packaged_fallback: "unknown".to_owned(),
            event_log_hint: "target/axion/reports/dev-events.jsonl".to_owned(),
            report_path_hint: "target/axion/reports/dev-report.json".to_owned(),
            blockers: vec![error.into()],
            recommendations: Vec::new(),
        }
    }

    fn passed(&self) -> bool {
        self.blockers.is_empty()
    }

    fn print_human(&self) {
        println!(
            "dev.preflight: {}",
            if self.passed() { "ok" } else { "failed" }
        );
        println!("dev.server: {}", self.dev_server_status);
        if let Some(url) = &self.dev_server_url {
            println!("dev.server.url: {url}");
        }
        println!("dev.watch_root: {}", self.watch_root);
        println!("dev.packaged_fallback: {}", self.packaged_fallback);
        println!(
            "dev.frontend_command: {}",
            if self.frontend_command_configured {
                "configured"
            } else {
                "not configured"
            }
        );
        if let Some(cwd) = &self.frontend_cwd {
            println!("dev.frontend_cwd: {cwd}");
        }
        if let Some(timeout_ms) = self.frontend_timeout_ms {
            println!("dev.frontend_timeout_ms: {timeout_ms}");
        }
        println!("dev.event_log_hint: {}", self.event_log_hint);
        println!("dev.report_path_hint: {}", self.report_path_hint);
        for blocker in &self.blockers {
            println!("dev.blocker: {blocker}");
        }
        for recommendation in &self.recommendations {
            println!("dev.recommendation: {recommendation}");
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"checked\":true,\"passed\":{},\"manifest_loaded\":{},\"dev_server\":{{\"status\":{},\"url\":{}}},\"frontend_command\":{{\"configured\":{},\"cwd\":{},\"timeout_ms\":{}}},\"watch_root\":{},\"packaged_fallback\":{},\"event_log_hint\":{},\"report_path_hint\":{},\"blockers\":{},\"recommendations\":{}}}",
            self.passed(),
            self.manifest_loaded,
            json_string_literal(&self.dev_server_status),
            optional_json_string_literal(self.dev_server_url.as_deref()),
            self.frontend_command_configured,
            optional_json_string_literal(self.frontend_cwd.as_deref()),
            self.frontend_timeout_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_owned()),
            json_string_literal(&self.watch_root),
            json_string_literal(&self.packaged_fallback),
            json_string_literal(&self.event_log_hint),
            json_string_literal(&self.report_path_hint),
            json_string_array_literal(&self.blockers),
            json_string_array_literal(&self.recommendations),
        )
    }
}

fn dev_preflight(manifest_path: &std::path::Path) -> DevPreflightReport {
    let config = match axion_manifest::load_app_config_from_path(manifest_path) {
        Ok(config) => config,
        Err(error) => return DevPreflightReport::failed(error.to_string()),
    };

    let mut blockers = Vec::new();
    let mut recommendations = Vec::new();

    let watch_root =
        match axion_packager::validate_web_assets(&config.build.frontend_dist, &config.build.entry)
        {
            Ok(_) => format!("ok ({})", config.build.frontend_dist.display()),
            Err(error) => {
                blockers.push(format!(
                    "frontend assets are not valid for watch/reload: {error}"
                ));
                format!("invalid ({})", config.build.frontend_dist.display())
            }
        };

    let packaged_fallback = match Builder::new().apply_config(config.clone()).build() {
        Ok(app) => match axion_runtime::launch_request(&app, RunMode::Production) {
            Ok(request) => match request.target {
                axion_runtime::RuntimeLaunchTarget::AppProtocol(target) => {
                    format!("available ({})", target.initial_url)
                }
                axion_runtime::RuntimeLaunchTarget::DevServer(url) => {
                    let message = format!("production launch unexpectedly resolved to {url}");
                    blockers.push(message.clone());
                    format!("invalid ({message})")
                }
            },
            Err(error) => {
                blockers.push(format!("packaged fallback is unavailable: {error}"));
                format!("unavailable ({error})")
            }
        },
        Err(error) => {
            blockers.push(format!("app configuration is invalid: {error}"));
            format!("unavailable ({error})")
        }
    };

    let (dev_server_status, dev_server_url) = match &config.dev {
        Some(dev_server) if crate::commands::dev::dev_server_is_reachable(&config) => {
            recommendations.push(
                "dev server is reachable; use axion dev --launch for live frontend assets"
                    .to_owned(),
            );
            ("reachable".to_owned(), Some(dev_server.url.to_string()))
        }
        Some(dev_server) => {
            recommendations.push(
                "dev server is not reachable; start it or pass --fallback-packaged".to_owned(),
            );
            ("unreachable".to_owned(), Some(dev_server.url.to_string()))
        }
        None => {
            recommendations.push(
                "no [dev] server is configured; use --fallback-packaged or add [dev] url"
                    .to_owned(),
            );
            ("not configured".to_owned(), None)
        }
    };

    let (frontend_command_configured, frontend_cwd, frontend_timeout_ms) = config
        .dev
        .as_ref()
        .map(|dev| {
            (
                dev.command
                    .as_ref()
                    .is_some_and(|command| !command.trim().is_empty()),
                dev.cwd.as_ref().map(|cwd| cwd.display().to_string()),
                dev.timeout_ms,
            )
        })
        .unwrap_or((false, None, None));

    if config.dev.as_ref().is_some_and(|dev| {
        dev.command
            .as_ref()
            .is_some_and(|command| command.trim().is_empty())
    }) {
        blockers.push("[dev] command is configured but empty".to_owned());
    }
    if let Some(timeout_ms) = frontend_timeout_ms {
        if timeout_ms < 1000 {
            recommendations
                .push("[dev] timeout_ms is very short; consider at least 5000ms".to_owned());
        }
    }

    recommendations.push(
        "archive dev sessions with --event-log target/axion/reports/dev-events.jsonl --report-path target/axion/reports/dev-report.json".to_owned(),
    );

    DevPreflightReport {
        manifest_loaded: true,
        dev_server_status,
        dev_server_url,
        frontend_command_configured,
        frontend_cwd,
        frontend_timeout_ms,
        watch_root,
        packaged_fallback,
        event_log_hint: "target/axion/reports/dev-events.jsonl".to_owned(),
        report_path_hint: "target/axion/reports/dev-report.json".to_owned(),
        blockers,
        recommendations,
    }
}

fn optional_json_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

fn json_string_array_literal(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",");

    format!("[{values}]")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::cli::{CheckArgs, DoctorRisk};

    use super::{check_report, run};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-check-test-{unique}-{serial}"))
    }

    fn write_check_manifest() -> PathBuf {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let icons = root.join("icons");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&icons).unwrap();
        fs::create_dir_all(root.join("servo").join("components").join("servo")).unwrap();
        fs::write(
            frontend.join("index.html"),
            "<!doctype html><script src=\"app.js\"></script>",
        )
        .unwrap();
        fs::write(
            frontend.join("app.js"),
            "window.__AXION_GUI_SMOKE__ = async () => ({ result: 'ok' });",
        )
        .unwrap();
        fs::write(icons.join("app.icns"), "icon").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(
            &manifest,
            r#"
[app]
name = "check-test"
identifier = "dev.axion.check-test"
version = "1.0.0"

[window]
id = "main"
title = "Check Test"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[bundle]
icon = "icons/app.icns"

[capabilities.main]
profiles = ["app-info"]
"#,
        )
        .unwrap();
        manifest
    }

    #[test]
    fn check_runs_doctor_self_test_and_bundle_preflight() {
        run(CheckArgs {
            manifest_path: write_check_manifest(),
            max_risk: DoctorRisk::Medium,
            bundle: true,
            dev: false,
            json: false,
            keep_artifacts: false,
        })
        .expect("check should pass");
    }

    #[test]
    fn check_report_serializes_json_and_next_step() {
        let report = check_report(&CheckArgs {
            manifest_path: write_check_manifest(),
            max_risk: DoctorRisk::Medium,
            bundle: true,
            dev: true,
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "ok");
        assert!(json.contains("\"schema\":\"axion.check-report.v1\""));
        assert!(json.contains("\"dev_requested\":true"));
        assert!(json.contains("\"doctor\":{\"passed\":true"));
        assert!(json.contains("\"ready_for_dev\":true"));
        assert!(json.contains("\"bundle_preflight\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains("\"dev_preflight\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains("\"dev_server\":{\"status\":\"not configured\""));
        assert!(json.contains(
            "\"next_step\":\"run axion gui-smoke, then axion bundle --build-executable\""
        ));
        assert!(json.contains("\"result\":\"ok\""));
    }

    #[test]
    fn check_dev_preflight_reports_unreachable_dev_server_without_failing() {
        let manifest = write_check_manifest();
        let mut body = fs::read_to_string(&manifest).unwrap();
        body.push_str(
            r#"
[dev]
url = "http://127.0.0.1:9"
command = "python3 -m http.server 3000"
timeout_ms = 500
"#,
        );
        fs::write(&manifest, body).unwrap();

        let report = check_report(&CheckArgs {
            manifest_path: manifest,
            max_risk: DoctorRisk::Medium,
            bundle: false,
            dev: true,
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "ok");
        assert!(json.contains("\"status\":\"unreachable\""));
        assert!(json.contains("\"frontend_command\":{\"configured\":true"));
        assert!(json.contains("dev server is not reachable"));
        assert!(json.contains("timeout_ms is very short"));
    }
}
