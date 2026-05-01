use axion_core::{AppConfig, Builder, RunMode};
use axion_runtime::json_string_literal;

use crate::cli::{CheckArgs, DoctorArgs, SelfTestArgs};
use crate::commands::doctor::{doctor_gate_for_manifest, doctor_readiness_for_manifest};
use crate::error::AxionCliError;

pub fn run(args: CheckArgs) -> Result<(), AxionCliError> {
    let mut report = check_report(&args);

    if args.report_path.is_some() {
        report.mark_check_report_artifact_exists();
    }

    write_check_report_if_requested(&args, &report)?;

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

fn write_check_report_if_requested(
    args: &CheckArgs,
    report: &CheckReport,
) -> Result<(), AxionCliError> {
    let Some(path) = &args.report_path else {
        return Ok(());
    };

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(path, report.to_json())?;
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

fn check_artifacts(args: &CheckArgs) -> Vec<CheckArtifact> {
    vec![
        check_artifact(
            "check_report",
            args.report_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "target/axion/reports/check.json".to_owned()),
            args.report_path.is_some(),
        ),
        check_artifact(
            "dev_event_log_hint",
            "target/axion/reports/dev-events.jsonl",
            false,
        ),
        check_artifact(
            "dev_report_hint",
            "target/axion/reports/dev-report.json",
            false,
        ),
        check_artifact(
            "bundle_report_hint",
            "target/axion/reports/bundle.json",
            args.bundle,
        ),
        check_artifact(
            "release_report_hint",
            "target/axion/reports/release.json",
            false,
        ),
    ]
}

fn check_artifact(
    kind: impl Into<String>,
    path: impl Into<String>,
    required: bool,
) -> CheckArtifact {
    let path = path.into();
    CheckArtifact {
        exists: std::path::Path::new(&path).exists(),
        kind: kind.into(),
        path,
        required,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckReport {
    manifest_path: String,
    max_risk: String,
    bundle_requested: bool,
    report_path: Option<String>,
    capability_summary: CapabilitySummary,
    doctor_passed: bool,
    doctor_failures: Vec<String>,
    ready_for_dev: bool,
    ready_for_bundle: bool,
    ready_for_gui_smoke: bool,
    readiness_blockers: Vec<String>,
    readiness_warnings: Vec<String>,
    self_test_passed: bool,
    self_test_error: Option<String>,
    artifacts: Vec<CheckArtifact>,
    bundle_preflight_checked: bool,
    bundle_preflight_passed: Option<bool>,
    bundle_preflight_error: Option<String>,
    dev_preflight: Option<DevPreflightReport>,
    failure_phase: Option<String>,
    next_step: String,
    next_steps: Vec<String>,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckArtifact {
    kind: String,
    path: String,
    required: bool,
    exists: bool,
}

impl CheckReport {
    fn new(args: &CheckArgs) -> Self {
        Self {
            manifest_path: args.manifest_path.display().to_string(),
            max_risk: args.max_risk.as_str().to_owned(),
            bundle_requested: args.bundle,
            report_path: args
                .report_path
                .as_ref()
                .map(|path| path.display().to_string()),
            capability_summary: CapabilitySummary::from_manifest(&args.manifest_path),
            doctor_passed: false,
            doctor_failures: Vec::new(),
            ready_for_dev: false,
            ready_for_bundle: false,
            ready_for_gui_smoke: false,
            readiness_blockers: Vec::new(),
            readiness_warnings: Vec::new(),
            self_test_passed: false,
            self_test_error: None,
            artifacts: check_artifacts(args),
            bundle_preflight_checked: false,
            bundle_preflight_passed: None,
            bundle_preflight_error: None,
            dev_preflight: None,
            failure_phase: None,
            next_step: String::new(),
            next_steps: Vec::new(),
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
        self.failure_phase = self.failure_phase_for(dev_ok);
        self.next_steps = self.build_next_steps(dev_ok);
        self.next_step = self.next_steps.first().cloned().unwrap_or_else(|| {
            "run axion doctor --deny-warnings --max-risk medium for detailed diagnostics".to_owned()
        });
    }

    fn failure_phase_for(&self, dev_ok: bool) -> Option<String> {
        if !self.doctor_passed {
            Some("doctor".to_owned())
        } else if !self.ready_for_dev {
            Some("readiness".to_owned())
        } else if !self.self_test_passed {
            Some("self_test".to_owned())
        } else if self.bundle_requested && self.bundle_preflight_passed != Some(true) {
            Some("bundle_preflight".to_owned())
        } else if !dev_ok {
            Some("dev_preflight".to_owned())
        } else {
            None
        }
    }

    fn build_next_steps(&self, dev_ok: bool) -> Vec<String> {
        if !self.doctor_passed {
            let mut steps = vec![
                "run axion doctor --deny-warnings --max-risk medium and resolve gate failures"
                    .to_owned(),
            ];
            steps.extend(
                self.doctor_failures
                    .iter()
                    .map(|failure| format!("doctor failure: {failure}")),
            );
            return steps;
        }

        if !self.ready_for_dev {
            return self
                .readiness_blockers
                .iter()
                .map(|blocker| next_step_for_readiness_blocker(blocker))
                .collect();
        }

        if !self.self_test_passed {
            return vec![
                "run axion self-test --manifest-path <manifest> for full staging diagnostics"
                    .to_owned(),
            ];
        }

        if self.bundle_requested && self.bundle_preflight_passed != Some(true) {
            return vec![next_step_for_bundle_error(
                self.bundle_preflight_error.as_deref(),
            )];
        }

        if !dev_ok {
            return self
                .dev_preflight
                .as_ref()
                .map(|dev| {
                    dev.blockers
                        .iter()
                        .map(|blocker| next_step_for_dev_blocker(blocker))
                        .collect::<Vec<_>>()
                })
                .filter(|steps| !steps.is_empty())
                .unwrap_or_else(|| {
                    vec!["fix dev preflight blockers before using axion dev --launch".to_owned()]
                });
        }

        let mut steps = Vec::new();
        if self.ready_for_gui_smoke {
            steps.push(
                "run axion gui-smoke with --report-path target/axion/reports/gui-smoke.json"
                    .to_owned(),
            );
        } else if self.ready_for_bundle {
            steps.push("run axion bundle --build-executable; GUI smoke needs Servo checkout setup or a window.__AXION_GUI_SMOKE__ hook".to_owned());
        }
        if self.ready_for_bundle {
            steps.push("run axion bundle --build-executable, then axion release --archive for preview artifacts".to_owned());
        }
        if steps.is_empty() {
            steps.push(
                "run axion doctor --deny-warnings --max-risk medium for detailed diagnostics"
                    .to_owned(),
            );
        }
        steps
    }

    fn mark_check_report_artifact_exists(&mut self) {
        if let Some(artifact) = self
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == "check_report")
        {
            artifact.exists = true;
        }
    }

    fn print_human(&self) {
        for line in self.human_lines() {
            println!("{line}");
        }
    }

    fn human_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("Axion check".to_owned());
        lines.push(format!("manifest: {}", self.manifest_path));
        lines.push(format!("result: {}", self.result));
        lines.push(format!(
            "failure_phase: {}",
            self.failure_phase.as_deref().unwrap_or("none")
        ));
        lines.push(format!("next_step: {}", self.next_step));
        for step in self.next_steps.iter().skip(1) {
            lines.push(format!("next_step.detail: {step}"));
        }
        lines.push(String::new());
        lines.push("[gate]".to_owned());
        lines.push(format!(
            "doctor: {}",
            if self.doctor_passed { "ok" } else { "failed" }
        ));
        for reason in &self.doctor_failures {
            lines.push(format!("doctor.failure: {reason}"));
        }
        lines.push(String::new());
        lines.push("[capabilities]".to_owned());
        lines.extend(self.capability_summary.human_lines());
        lines.push(String::new());
        lines.push("[readiness]".to_owned());
        lines.push(format!(
            "readiness: dev={}, bundle={}, gui_smoke={}",
            self.ready_for_dev, self.ready_for_bundle, self.ready_for_gui_smoke
        ));
        for blocker in &self.readiness_blockers {
            lines.push(format!("readiness.blocker: {blocker}"));
        }
        for warning in &self.readiness_warnings {
            lines.push(format!("readiness.warning: {warning}"));
        }
        lines.push(String::new());
        lines.push("[self_test]".to_owned());
        lines.push(format!(
            "self_test: {}",
            if self.self_test_passed {
                "ok"
            } else if self.doctor_passed && self.ready_for_dev {
                "failed"
            } else {
                "skipped"
            }
        ));
        if let Some(error) = &self.self_test_error {
            lines.push(format!("self_test.error: {error}"));
        }
        lines.push(String::new());
        lines.push("[artifacts]".to_owned());
        for artifact in &self.artifacts {
            lines.push(format!(
                "artifact: kind={}, required={}, exists={}, path={}",
                artifact.kind, artifact.required, artifact.exists, artifact.path
            ));
        }
        lines.push(String::new());
        lines.push("[bundle_preflight]".to_owned());
        if self.bundle_preflight_checked {
            match self.bundle_preflight_passed {
                Some(true) => lines.push(format!(
                    "bundle.preflight: ok (target={})",
                    axion_packager::current_bundle_target().as_str()
                )),
                Some(false) => lines.push(format!(
                    "bundle.preflight: failed ({})",
                    self.bundle_preflight_error.as_deref().unwrap_or("unknown")
                )),
                None => lines.push("bundle.preflight: skipped".to_owned()),
            }
        } else {
            lines.push("bundle.preflight: skipped (pass --bundle to enable)".to_owned());
        }
        lines.push(String::new());
        lines.push("[dev_preflight]".to_owned());
        if let Some(dev) = &self.dev_preflight {
            lines.extend(dev.human_lines());
        } else {
            lines.push("dev.preflight: skipped (pass --dev to enable)".to_owned());
        }
        lines
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"axion.check-report.v1\",\"manifest_path\":{},\"max_risk\":{},\"bundle_requested\":{},\"dev_requested\":{},\"report_path\":{},\"doctor\":{{\"passed\":{},\"failed_reasons\":{}}},\"capabilities\":{},\"readiness\":{{\"ready_for_dev\":{},\"ready_for_bundle\":{},\"ready_for_gui_smoke\":{},\"blockers\":{},\"warnings\":{}}},\"self_test\":{{\"passed\":{},\"error\":{}}},\"artifacts\":{},\"bundle_preflight\":{{\"checked\":{},\"passed\":{},\"error\":{}}},\"dev_preflight\":{},\"failure_phase\":{},\"next_step\":{},\"next_steps\":{},\"next_actions\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            json_string_literal(&self.max_risk),
            self.bundle_requested,
            self.dev_preflight.is_some(),
            optional_json_string_literal(self.report_path.as_deref()),
            self.doctor_passed,
            json_string_array_literal(&self.doctor_failures),
            self.capability_summary.to_json(),
            self.ready_for_dev,
            self.ready_for_bundle,
            self.ready_for_gui_smoke,
            json_string_array_literal(&self.readiness_blockers),
            json_string_array_literal(&self.readiness_warnings),
            self.self_test_passed,
            optional_json_string_literal(self.self_test_error.as_deref()),
            check_artifact_array_json(&self.artifacts),
            self.bundle_preflight_checked,
            optional_json_bool(self.bundle_preflight_passed),
            optional_json_string_literal(self.bundle_preflight_error.as_deref()),
            self.dev_preflight
                .as_ref()
                .map(DevPreflightReport::to_json)
                .unwrap_or_else(|| "{\"checked\":false}".to_owned()),
            optional_json_string_literal(self.failure_phase.as_deref()),
            json_string_literal(&self.next_step),
            json_string_array_literal(&self.next_steps),
            check_next_actions_json(&self.next_steps, self.failure_phase.is_some()),
            json_string_literal(&self.result),
        )
    }
}

fn check_next_actions_json(steps: &[String], required: bool) -> String {
    let actions = steps
        .iter()
        .map(|step| {
            format!(
                "{{\"kind\":{},\"required\":{},\"step\":{}}}",
                json_string_literal(check_next_action_kind(step)),
                required,
                json_string_literal(step)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{actions}]")
}

fn check_next_action_kind(step: &str) -> &'static str {
    if step.contains("gui-smoke")
        || step.contains("GUI smoke")
        || step.contains("__AXION_GUI_SMOKE__")
    {
        "gui_smoke"
    } else if step.contains("release --archive") || step.contains("release artifacts") {
        "release"
    } else if step.contains("bundle")
        || step.contains("[bundle]")
        || step.contains("frontend asset")
    {
        "bundle"
    } else if step.contains("self-test") {
        "self_test"
    } else if step.contains("doctor") || step.contains("security") || step.contains("risk") {
        "doctor"
    } else if step.contains("[dev]")
        || step.contains("dev preflight")
        || step.contains("--fallback-packaged")
    {
        "dev_preflight"
    } else if step.contains("readiness") {
        "readiness"
    } else {
        "general"
    }
}

fn next_step_for_readiness_blocker(blocker: &str) -> String {
    if blocker.contains("bundle icon") {
        "configure [bundle] icon with a valid .icns/.ico/.png path before bundle/release".to_owned()
    } else if blocker.contains("window.__AXION_GUI_SMOKE__") {
        "add window.__AXION_GUI_SMOKE__ to frontend assets or skip GUI smoke for this app"
            .to_owned()
    } else if blocker.contains("servo source") {
        "run GUI smoke from the Axion checkout or pass --cargo-target-dir target so Servo sources are discoverable".to_owned()
    } else if blocker.contains("security warnings") || blocker.contains("security risk") {
        "run axion doctor and narrow capabilities, navigation origins, or max risk before release"
            .to_owned()
    } else if blocker.contains("build assets") {
        "fix [build].frontend_dist and [build].entry so packaged frontend assets can be validated"
            .to_owned()
    } else {
        format!("resolve readiness blocker: {blocker}")
    }
}

fn next_step_for_bundle_error(error: Option<&str>) -> String {
    let Some(error) = error else {
        return "fix bundle preflight issues before running axion bundle --build-executable"
            .to_owned();
    };
    if error.contains("icon") {
        "fix [bundle] icon, then rerun axion check --bundle".to_owned()
    } else if error.contains("frontend") || error.contains("entry") {
        "fix frontend asset paths, then rerun axion check --bundle".to_owned()
    } else {
        format!("fix bundle preflight issue: {error}")
    }
}

fn next_step_for_dev_blocker(blocker: &str) -> String {
    if blocker.contains("[dev] cwd") {
        "fix [dev].cwd or pass --frontend-cwd to an existing frontend directory".to_owned()
    } else if blocker.contains("[dev] command") {
        "set a non-empty [dev] command or remove it and start the dev server separately".to_owned()
    } else if blocker.contains("packaged fallback") {
        "fix packaged frontend assets before using --fallback-packaged".to_owned()
    } else {
        format!("fix dev preflight blocker: {blocker}")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CapabilitySummary {
    manifest_loaded: bool,
    error: Option<String>,
    windows: Vec<CapabilityWindowSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapabilityWindowSummary {
    id: String,
    bridge_enabled: bool,
    profiles: Vec<String>,
    profile_expansions: Vec<axion_core::CapabilityProfileConfig>,
    explicit_commands: Vec<String>,
    explicit_events: Vec<String>,
    explicit_protocols: Vec<String>,
    commands: Vec<String>,
    events: Vec<String>,
    protocols: Vec<String>,
    allowed_navigation_origins: Vec<String>,
    allow_remote_navigation: bool,
    risk: String,
}

impl CapabilitySummary {
    fn from_manifest(manifest_path: &std::path::Path) -> Self {
        match axion_manifest::load_app_config_from_path(manifest_path) {
            Ok(config) => Self::from_config(&config),
            Err(error) => Self {
                manifest_loaded: false,
                error: Some(error.to_string()),
                windows: Vec::new(),
            },
        }
    }

    fn from_config(config: &AppConfig) -> Self {
        let windows = config
            .windows
            .iter()
            .map(|window| {
                let capability = config.capabilities.get(window.id.as_str());
                let bridge_enabled = capability.is_some_and(|capability| {
                    capability
                        .protocols
                        .iter()
                        .any(|protocol| protocol == "axion")
                });
                let commands = capability
                    .map(|capability| capability.commands.clone())
                    .unwrap_or_default();
                let allowed_navigation_origins = capability
                    .map(|capability| capability.allowed_navigation_origins.clone())
                    .unwrap_or_default();
                let allow_remote_navigation = capability
                    .map(|capability| capability.allow_remote_navigation)
                    .unwrap_or_default();
                let risk = capability_summary_risk(
                    bridge_enabled,
                    allow_remote_navigation,
                    &allowed_navigation_origins,
                    &commands,
                )
                .to_owned();

                CapabilityWindowSummary {
                    id: window.id.as_str().to_owned(),
                    bridge_enabled,
                    profiles: capability
                        .map(|capability| capability.profiles.clone())
                        .unwrap_or_default(),
                    profile_expansions: capability
                        .map(|capability| capability.profile_expansions.clone())
                        .unwrap_or_default(),
                    explicit_commands: capability
                        .map(|capability| capability.explicit_commands.clone())
                        .unwrap_or_default(),
                    explicit_events: capability
                        .map(|capability| capability.explicit_events.clone())
                        .unwrap_or_default(),
                    explicit_protocols: capability
                        .map(|capability| capability.explicit_protocols.clone())
                        .unwrap_or_default(),
                    commands,
                    events: capability
                        .map(|capability| capability.events.clone())
                        .unwrap_or_default(),
                    protocols: capability
                        .map(|capability| capability.protocols.clone())
                        .unwrap_or_default(),
                    allowed_navigation_origins,
                    allow_remote_navigation,
                    risk,
                }
            })
            .collect();

        Self {
            manifest_loaded: true,
            error: None,
            windows,
        }
    }

    fn human_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "capabilities.summary: manifest_loaded={}, windows={}",
            self.manifest_loaded,
            self.windows.len()
        )];
        if let Some(error) = &self.error {
            lines.push(format!("capabilities.error: {error}"));
        }
        for window in &self.windows {
            lines.push(format!(
                "capabilities.window.{}: bridge={}, risk={}, profiles={}, commands={}, events={}, protocols={}, navigation_origins={}, remote_navigation={}",
                window.id,
                if window.bridge_enabled { "enabled" } else { "disabled" },
                window.risk,
                list_or_none(&window.profiles),
                list_or_none(&window.commands),
                list_or_none(&window.events),
                list_or_none(&window.protocols),
                list_or_none(&window.allowed_navigation_origins),
                window.allow_remote_navigation,
            ));
            for expansion in &window.profile_expansions {
                lines.push(format!(
                    "capabilities.window.{}.profile.{}: commands={}, events={}, protocols={}",
                    window.id,
                    expansion.profile,
                    list_or_none(&expansion.commands),
                    list_or_none(&expansion.events),
                    list_or_none(&expansion.protocols),
                ));
            }
        }
        lines
    }

    fn to_json(&self) -> String {
        let windows = self
            .windows
            .iter()
            .map(CapabilityWindowSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"manifest_loaded\":{},\"error\":{},\"windows\":[{}]}}",
            self.manifest_loaded,
            optional_json_string_literal(self.error.as_deref()),
            windows,
        )
    }
}

impl CapabilityWindowSummary {
    fn to_json(&self) -> String {
        let profile_expansions = self
            .profile_expansions
            .iter()
            .map(capability_profile_expansion_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"id\":{},\"bridge_enabled\":{},\"risk\":{},\"profiles\":{},\"profile_expansions\":[{}],\"explicit_commands\":{},\"explicit_events\":{},\"explicit_protocols\":{},\"commands\":{},\"events\":{},\"protocols\":{},\"allowed_navigation_origins\":{},\"allow_remote_navigation\":{}}}",
            json_string_literal(&self.id),
            self.bridge_enabled,
            json_string_literal(&self.risk),
            json_string_array_literal(&self.profiles),
            profile_expansions,
            json_string_array_literal(&self.explicit_commands),
            json_string_array_literal(&self.explicit_events),
            json_string_array_literal(&self.explicit_protocols),
            json_string_array_literal(&self.commands),
            json_string_array_literal(&self.events),
            json_string_array_literal(&self.protocols),
            json_string_array_literal(&self.allowed_navigation_origins),
            self.allow_remote_navigation,
        )
    }
}

fn capability_profile_expansion_json(expansion: &axion_core::CapabilityProfileConfig) -> String {
    format!(
        "{{\"profile\":{},\"commands\":{},\"events\":{},\"protocols\":{}}}",
        json_string_literal(&expansion.profile),
        json_string_array_literal(&expansion.commands),
        json_string_array_literal(&expansion.events),
        json_string_array_literal(&expansion.protocols),
    )
}

fn capability_summary_risk(
    bridge_enabled: bool,
    allow_remote_navigation: bool,
    allowed_navigation_origins: &[String],
    commands: &[String],
) -> &'static str {
    if allow_remote_navigation {
        "high"
    } else if !bridge_enabled {
        "low"
    } else if !allowed_navigation_origins.is_empty()
        || commands.iter().any(|command| {
            command.starts_with("fs.")
                || command.starts_with("clipboard.")
                || command.starts_with("dialog.")
                || command == "app.exit"
                || command == "window.close"
                || command == "window.reload"
        })
    {
        "medium"
    } else {
        "low"
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
    warnings: Vec<String>,
    recommendations: Vec<String>,
    recommended_commands: Vec<String>,
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
            warnings: Vec::new(),
            recommendations: Vec::new(),
            recommended_commands: Vec::new(),
        }
    }

    fn passed(&self) -> bool {
        self.blockers.is_empty()
    }

    fn human_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "dev.preflight: {}",
            if self.passed() { "ok" } else { "failed" }
        ));
        lines.push(format!("dev.server: {}", self.dev_server_status));
        if let Some(url) = &self.dev_server_url {
            lines.push(format!("dev.server.url: {url}"));
        }
        lines.push(format!("dev.watch_root: {}", self.watch_root));
        lines.push(format!("dev.packaged_fallback: {}", self.packaged_fallback));
        lines.push(format!(
            "dev.frontend_command: {}",
            if self.frontend_command_configured {
                "configured"
            } else {
                "not configured"
            }
        ));
        if let Some(cwd) = &self.frontend_cwd {
            lines.push(format!("dev.frontend_cwd: {cwd}"));
        }
        if let Some(timeout_ms) = self.frontend_timeout_ms {
            lines.push(format!("dev.frontend_timeout_ms: {timeout_ms}"));
        }
        lines.push(format!("dev.event_log_hint: {}", self.event_log_hint));
        lines.push(format!("dev.report_path_hint: {}", self.report_path_hint));
        for blocker in &self.blockers {
            lines.push(format!("dev.blocker: {blocker}"));
        }
        for warning in &self.warnings {
            lines.push(format!("dev.warning: {warning}"));
        }
        for recommendation in &self.recommendations {
            lines.push(format!("dev.recommendation: {recommendation}"));
        }
        for command in &self.recommended_commands {
            lines.push(format!("dev.recommended_command: {command}"));
        }
        lines
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"checked\":true,\"passed\":{},\"manifest_loaded\":{},\"dev_server\":{{\"status\":{},\"url\":{}}},\"frontend_command\":{{\"configured\":{},\"cwd\":{},\"timeout_ms\":{}}},\"watch_root\":{},\"packaged_fallback\":{},\"event_log_hint\":{},\"report_path_hint\":{},\"blockers\":{},\"warnings\":{},\"recommendations\":{},\"recommended_commands\":{}}}",
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
            json_string_array_literal(&self.warnings),
            json_string_array_literal(&self.recommendations),
            json_string_array_literal(&self.recommended_commands),
        )
    }
}

fn dev_preflight(manifest_path: &std::path::Path) -> DevPreflightReport {
    let config = match axion_manifest::load_app_config_from_path(manifest_path) {
        Ok(config) => config,
        Err(error) => return DevPreflightReport::failed(error.to_string()),
    };

    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
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
            warnings.push(
                "dev server is not reachable; start it or pass --fallback-packaged".to_owned(),
            );
            ("unreachable".to_owned(), Some(dev_server.url.to_string()))
        }
        None => {
            warnings.push(
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
    if let Some(dev) = &config.dev {
        if let Some(cwd) = &dev.cwd {
            if !cwd.exists() {
                blockers.push(format!("[dev] cwd does not exist: {}", cwd.display()));
            } else if !cwd.is_dir() {
                blockers.push(format!("[dev] cwd is not a directory: {}", cwd.display()));
            }
        }
        if frontend_command_configured && dev.timeout_ms.is_none() {
            warnings.push(
                "[dev] command is configured without timeout_ms; default timeout will be used"
                    .to_owned(),
            );
        }
    }
    if let Some(timeout_ms) = frontend_timeout_ms {
        if timeout_ms < 1000 {
            warnings.push("[dev] timeout_ms is very short; consider at least 5000ms".to_owned());
        }
    }

    recommendations.push(
        "archive dev sessions with --event-log target/axion/reports/dev-events.jsonl --report-path target/axion/reports/dev-report.json".to_owned(),
    );
    let manifest = manifest_path.display().to_string();
    let recommended_commands = vec![
        format!(
            "axion check --manifest-path {manifest} --dev --bundle --report-path target/axion/reports/check.json"
        ),
        format!(
            "axion dev --manifest-path {manifest} --launch --fallback-packaged --watch --reload --restart-on-change --event-log target/axion/reports/dev-events.jsonl --report-path target/axion/reports/dev-report.json"
        ),
    ];

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
        warnings,
        recommendations,
        recommended_commands,
    }
}

fn optional_json_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

fn check_artifact_array_json(values: &[CheckArtifact]) -> String {
    let values = values
        .iter()
        .map(|artifact| {
            format!(
                "{{\"kind\":{},\"path\":{},\"required\":{},\"exists\":{}}}",
                json_string_literal(&artifact.kind),
                json_string_literal(&artifact.path),
                artifact.required,
                artifact.exists,
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!("[{values}]")
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
            report_path: None,
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
            report_path: Some("target/axion/reports/check.json".into()),
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "ok");
        assert!(json.contains("\"schema\":\"axion.check-report.v1\""));
        assert!(json.contains("\"dev_requested\":true"));
        assert!(json.contains("\"report_path\":\"target/axion/reports/check.json\""));
        assert!(json.contains("\"artifacts\":["));
        assert!(json.contains("\"kind\":\"check_report\""));
        assert!(json.contains("\"kind\":\"dev_event_log_hint\""));
        assert!(json.contains("\"kind\":\"bundle_report_hint\""));
        assert!(json.contains("\"doctor\":{\"passed\":true"));
        assert!(json.contains("\"capabilities\":{\"manifest_loaded\":true"));
        assert!(json.contains("\"profile_expansions\":["));
        assert!(json.contains("\"profile\":\"app-info\""));
        assert!(
            json.contains("\"commands\":[\"app.echo\",\"app.info\",\"app.ping\",\"app.version\"]")
        );
        assert!(json.contains("\"risk\":\"low\""));
        assert!(json.contains("\"ready_for_dev\":true"));
        assert!(json.contains("\"bundle_preflight\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains("\"dev_preflight\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains("\"failure_phase\":null"));
        assert!(json.contains("\"dev_server\":{\"status\":\"not configured\""));
        assert!(json.contains("\"warnings\":[\"no [dev] server is configured"));
        assert!(json.contains("\"recommended_commands\":["));
        assert!(json.contains("axion check --manifest-path"));
        assert!(json.contains("axion dev --manifest-path"));
        assert!(json.contains(
            "\"next_step\":\"run axion gui-smoke with --report-path target/axion/reports/gui-smoke.json\""
        ));
        assert!(json.contains("\"next_steps\":[\"run axion gui-smoke with --report-path target/axion/reports/gui-smoke.json\",\"run axion bundle --build-executable, then axion release --archive for preview artifacts\"]"));
        assert!(json.contains("\"next_actions\":[{\"kind\":\"gui_smoke\",\"required\":false,\"step\":\"run axion gui-smoke with --report-path target/axion/reports/gui-smoke.json\""));
        assert!(json.contains("{\"kind\":\"release\",\"required\":false,\"step\":\"run axion bundle --build-executable, then axion release --archive for preview artifacts\"}"));
        assert!(json.contains("\"result\":\"ok\""));
    }

    #[test]
    fn check_human_output_is_grouped_but_keeps_stable_prefixes() {
        let report = check_report(&CheckArgs {
            manifest_path: write_check_manifest(),
            max_risk: DoctorRisk::Medium,
            bundle: true,
            dev: true,
            report_path: Some("target/axion/reports/check.json".into()),
            json: false,
            keep_artifacts: false,
        });
        let lines = report.human_lines();

        assert!(lines.contains(&"[gate]".to_owned()));
        assert!(lines.contains(&"[capabilities]".to_owned()));
        assert!(lines.contains(&"[readiness]".to_owned()));
        assert!(lines.contains(&"[self_test]".to_owned()));
        assert!(lines.contains(&"[artifacts]".to_owned()));
        assert!(lines.contains(&"[bundle_preflight]".to_owned()));
        assert!(lines.contains(&"[dev_preflight]".to_owned()));
        assert!(lines.iter().any(|line| line.starts_with("artifact: kind=")));
        assert!(lines.iter().any(|line| line == "failure_phase: none"));
        assert!(lines.iter().any(|line| line
            == "capabilities.window.main.profile.app-info: commands=app.echo,app.info,app.ping,app.version, events=none, protocols=axion"));
        assert!(lines.iter().any(|line| line.starts_with("dev.warning: ")));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("dev.recommended_command: axion check"))
        );
        assert!(lines.iter().any(|line| line
            == "next_step: run axion gui-smoke with --report-path target/axion/reports/gui-smoke.json"));
        assert!(lines.iter().any(|line| line
            == "next_step.detail: run axion bundle --build-executable, then axion release --archive for preview artifacts"));
        assert!(lines.iter().any(|line| line == "result: ok"));
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
            report_path: None,
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "ok");
        assert!(json.contains("\"status\":\"unreachable\""));
        assert!(json.contains("\"frontend_command\":{\"configured\":true"));
        assert!(json.contains("\"warnings\":[\"dev server is not reachable"));
        assert!(json.contains("[dev] timeout_ms is very short"));
        assert!(json.contains("\"blockers\":[]"));
    }

    #[test]
    fn check_dev_preflight_rejects_missing_frontend_cwd() {
        let manifest = write_check_manifest();
        let mut body = fs::read_to_string(&manifest).unwrap();
        body.push_str(
            r#"
[dev]
url = "http://127.0.0.1:9"
command = "python3 -m http.server 3000"
cwd = "missing-frontend"
"#,
        );
        fs::write(&manifest, body).unwrap();

        let report = check_report(&CheckArgs {
            manifest_path: manifest,
            max_risk: DoctorRisk::Medium,
            bundle: false,
            dev: true,
            report_path: None,
            json: true,
            keep_artifacts: false,
        });
        let json = report.to_json();

        assert_eq!(report.result, "failed");
        assert!(json.contains("[dev] cwd does not exist"));
        assert!(json.contains(
            "[dev] command is configured without timeout_ms; default timeout will be used"
        ));
        assert!(json.contains(
            "\"next_step\":\"fix [dev].cwd or pass --frontend-cwd to an existing frontend directory\""
        ));
        assert!(json.contains("\"failure_phase\":\"dev_preflight\""));
        assert!(json.contains("\"next_actions\":[{\"kind\":\"dev_preflight\",\"required\":true,\"step\":\"fix [dev].cwd or pass --frontend-cwd to an existing frontend directory\"}]"));
    }

    #[test]
    fn check_report_writes_json_to_report_path() {
        let root = temp_dir();
        let report_path = root.join("reports").join("check.json");

        run(CheckArgs {
            manifest_path: write_check_manifest(),
            max_risk: DoctorRisk::Medium,
            bundle: true,
            dev: true,
            report_path: Some(report_path.clone()),
            json: false,
            keep_artifacts: false,
        })
        .expect("check should write report");

        let body = fs::read_to_string(report_path).expect("check report should exist");
        assert!(body.contains("\"schema\":\"axion.check-report.v1\""));
        assert!(body.contains("\"artifacts\":["));
        assert!(body.contains("\"kind\":\"check_report\",\"path\":\""));
        assert!(body.contains("\"required\":true,\"exists\":true"));
        assert!(body.contains("\"dev_preflight\":{\"checked\":true"));
        assert!(body.contains("\"bundle_preflight\":{\"checked\":true"));
    }
}
