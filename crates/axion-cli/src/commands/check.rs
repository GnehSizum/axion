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
            next_step: String::new(),
            result: "failed".to_owned(),
        }
    }

    fn finalize(&mut self) {
        let bundle_ok = !self.bundle_requested || self.bundle_preflight_passed == Some(true);
        let passed = self.doctor_passed && self.ready_for_dev && self.self_test_passed && bundle_ok;

        self.result = if passed { "ok" } else { "failed" }.to_owned();
        self.next_step = if !self.doctor_passed {
            "run axion doctor and resolve gate failures".to_owned()
        } else if !self.ready_for_dev {
            "resolve readiness.blocker entries before running heavier checks".to_owned()
        } else if !self.self_test_passed {
            "run axion self-test for full staging diagnostics".to_owned()
        } else if self.bundle_requested && self.bundle_preflight_passed != Some(true) {
            "fix bundle preflight issues before running axion bundle --build-executable".to_owned()
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
        println!("next_step: {}", self.next_step);
        println!("result: {}", self.result);
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"axion.check-report.v1\",\"manifest_path\":{},\"max_risk\":{},\"bundle_requested\":{},\"doctor\":{{\"passed\":{},\"failed_reasons\":{}}},\"readiness\":{{\"ready_for_dev\":{},\"ready_for_bundle\":{},\"ready_for_gui_smoke\":{},\"blockers\":{},\"warnings\":{}}},\"self_test\":{{\"passed\":{},\"error\":{}}},\"bundle_preflight\":{{\"checked\":{},\"passed\":{},\"error\":{}}},\"next_step\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            json_string_literal(&self.max_risk),
            self.bundle_requested,
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
            json_string_literal(&self.next_step),
            json_string_literal(&self.result),
        )
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
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "ok");
        assert!(json.contains("\"schema\":\"axion.check-report.v1\""));
        assert!(json.contains("\"doctor\":{\"passed\":true"));
        assert!(json.contains("\"ready_for_dev\":true"));
        assert!(json.contains("\"bundle_preflight\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains(
            "\"next_step\":\"run axion gui-smoke, then axion bundle --build-executable\""
        ));
        assert!(json.contains("\"result\":\"ok\""));
    }
}
