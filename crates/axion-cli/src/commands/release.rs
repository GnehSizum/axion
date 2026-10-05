use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use axion_runtime::json_string_literal;

use crate::cli::{BundleArgs, DoctorArgs, ReleaseArgs, SelfTestArgs};
use crate::commands::bundle::{bundle_report, write_report_if_requested};
use crate::commands::doctor::{doctor_gate_for_manifest, doctor_readiness_for_manifest};
use crate::commands::report_util::{json_string_array_literal, optional_json_string_literal};
use crate::error::AxionCliError;
use serde::Deserialize;

pub fn run(args: ReleaseArgs) -> Result<(), AxionCliError> {
    let mut report = release_report(&args);
    write_release_report_if_requested(args.report_path.as_deref(), &report)?;
    report.refresh_artifacts();
    write_release_report_if_requested(args.report_path.as_deref(), &report)?;

    if args.json {
        println!("{}", report.to_json());
    } else {
        report.print_human();
    }

    if report.result == "failed" {
        return Err(std::io::Error::other("release failed").into());
    }

    Ok(())
}

fn release_report(args: &ReleaseArgs) -> ReleaseReport {
    let mut report = ReleaseReport::new(args);

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
            report.readiness_blockers.push(error.to_string());
        }
    }
    if report.doctor_passed && report.ready_for_dev {
        if let Some(check_report_path) = &args.check_report_path {
            match load_check_report_reuse(
                check_report_path,
                &args.manifest_path,
                args.max_risk.as_str(),
            ) {
                Ok(()) => {
                    report.check_report_reused = true;
                    report.self_test_passed = true;
                }
                Err(error) => {
                    report.check_report_error = Some(error.to_string());
                    report.self_test_error = Some(error.to_string());
                }
            }
        } else {
            match crate::commands::self_test::run(SelfTestArgs {
                manifest_path: args.manifest_path.clone(),
                output_dir: None,
                report_path: None,
                json: false,
                quiet: true,
                keep_artifacts: args.keep_artifacts,
            }) {
                Ok(()) => report.self_test_passed = true,
                Err(error) => report.self_test_error = Some(error.to_string()),
            }
        }
    }

    if report.doctor_passed && report.ready_for_bundle && report.self_test_passed {
        let bundle_args = BundleArgs {
            manifest_path: args.manifest_path.clone(),
            output_dir: args.output_dir.clone(),
            executable: args.executable.clone(),
            bin: args.bin.clone(),
            report_path: args.bundle_report_path.clone(),
            build_executable: !args.skip_build_executable,
            json: true,
        };
        match bundle_report(&bundle_args) {
            Ok(bundle) => {
                if let Err(error) =
                    write_report_if_requested(args.bundle_report_path.as_deref(), &bundle)
                {
                    report.bundle_error = Some(error.to_string());
                }
                report.bundle_passed = bundle.result() == "ok";
                report.bundle_report = Some(bundle.to_json());
                report.bundle_dir = bundle.bundle_dir().map(str::to_owned);
                report.bundle_manifest = bundle.bundle_manifest().map(str::to_owned);
                report.bundle_bytes = Some(bundle.bundle_bytes());

                if args.archive && report.bundle_passed {
                    match create_archive(
                        bundle.bundle_dir().map(PathBuf::from),
                        args.archive_path.clone(),
                    ) {
                        Ok(archive) => report.archive = archive,
                        Err(error) => {
                            report.archive.requested = true;
                            report.archive.error = Some(error.to_string());
                        }
                    }
                }
            }
            Err(error) => report.bundle_error = Some(error.to_string()),
        }
    }

    report.finalize();
    report.refresh_artifacts();
    report
}

fn write_release_report_if_requested(
    report_path: Option<&Path>,
    report: &ReleaseReport,
) -> Result<(), AxionCliError> {
    let Some(report_path) = report_path else {
        return Ok(());
    };

    if let Some(parent) = report_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(report_path, format!("{}\n", report.to_json()))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseReport {
    manifest_path: String,
    max_risk: String,
    report_path: Option<String>,
    bundle_report_path: Option<String>,
    check_report_path: Option<String>,
    check_report_reused: bool,
    check_report_error: Option<String>,
    doctor_passed: bool,
    doctor_failures: Vec<String>,
    ready_for_dev: bool,
    ready_for_bundle: bool,
    ready_for_gui_smoke: bool,
    readiness_blockers: Vec<String>,
    readiness_warnings: Vec<String>,
    self_test_passed: bool,
    self_test_error: Option<String>,
    bundle_passed: bool,
    bundle_error: Option<String>,
    bundle_report: Option<String>,
    bundle_dir: Option<String>,
    bundle_manifest: Option<String>,
    bundle_bytes: Option<u64>,
    build_executable: bool,
    archive: ArchiveReport,
    artifacts: Vec<ArtifactReport>,
    failure_phase: Option<String>,
    failed_reasons: Vec<String>,
    next_step: String,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchiveReport {
    requested: bool,
    passed: bool,
    path: Option<String>,
    bytes: Option<u64>,
    fnv1a64: Option<String>,
    error: Option<String>,
    verification: ArchiveVerification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchiveVerification {
    checked: bool,
    passed: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactReport {
    kind: String,
    path: String,
    exists: bool,
    bytes: Option<u64>,
    fnv1a64: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseSummary {
    artifacts_total: usize,
    artifacts_missing: usize,
    artifacts_with_errors: usize,
    check_report_reused: bool,
    archive_requested: bool,
    archive_passed: bool,
}

impl ReleaseReport {
    fn new(args: &ReleaseArgs) -> Self {
        Self {
            manifest_path: args.manifest_path.display().to_string(),
            max_risk: args.max_risk.as_str().to_owned(),
            report_path: args
                .report_path
                .as_ref()
                .map(|path| path.display().to_string()),
            bundle_report_path: args
                .bundle_report_path
                .as_ref()
                .map(|path| path.display().to_string()),
            check_report_path: args
                .check_report_path
                .as_ref()
                .map(|path| path.display().to_string()),
            check_report_reused: false,
            check_report_error: None,
            doctor_passed: false,
            doctor_failures: Vec::new(),
            ready_for_dev: false,
            ready_for_bundle: false,
            ready_for_gui_smoke: false,
            readiness_blockers: Vec::new(),
            readiness_warnings: Vec::new(),
            self_test_passed: false,
            self_test_error: None,
            bundle_passed: false,
            bundle_error: None,
            bundle_report: None,
            bundle_dir: None,
            bundle_manifest: None,
            bundle_bytes: None,
            build_executable: !args.skip_build_executable,
            archive: ArchiveReport {
                requested: args.archive,
                passed: !args.archive,
                path: None,
                bytes: None,
                fnv1a64: None,
                error: None,
                verification: ArchiveVerification {
                    checked: false,
                    passed: !args.archive,
                    error: None,
                },
            },
            artifacts: Vec::new(),
            failure_phase: None,
            failed_reasons: Vec::new(),
            next_step: String::new(),
            result: "failed".to_owned(),
        }
    }

    fn finalize(&mut self) {
        self.failure_phase = None;
        self.failed_reasons.clear();

        let archive_ok = !self.archive.requested || self.archive.passed;
        let passed = self.doctor_passed
            && self.ready_for_bundle
            && self.self_test_passed
            && self.bundle_passed
            && archive_ok;
        self.result = if passed { "ok" } else { "failed" }.to_owned();
        self.next_step = if !self.doctor_passed {
            self.failure_phase = Some("doctor".to_owned());
            self.failed_reasons.extend(self.doctor_failures.clone());
            if self.failed_reasons.is_empty() {
                self.failed_reasons
                    .push("doctor release gate did not pass".to_owned());
            }
            "run axion doctor and resolve release gate failures".to_owned()
        } else if !self.ready_for_dev || !self.ready_for_bundle {
            self.failure_phase = Some("readiness".to_owned());
            self.failed_reasons
                .extend(self.readiness_blockers.iter().cloned());
            if self.failed_reasons.is_empty() {
                self.failed_reasons
                    .push("release readiness checks did not pass".to_owned());
            }
            "resolve readiness.blocker entries before release".to_owned()
        } else if !self.self_test_passed {
            self.failure_phase = Some("self_test".to_owned());
            self.failed_reasons.push(
                self.self_test_error
                    .clone()
                    .unwrap_or_else(|| "quiet self-test did not pass".to_owned()),
            );
            "run axion self-test for full staging diagnostics".to_owned()
        } else if !self.bundle_passed {
            self.failure_phase = Some("bundle".to_owned());
            self.failed_reasons.push(
                self.bundle_error
                    .clone()
                    .unwrap_or_else(|| "bundle staging did not pass".to_owned()),
            );
            "run axion bundle --build-executable for bundle diagnostics".to_owned()
        } else if self.archive.requested && !self.archive.passed {
            self.failure_phase = Some("archive".to_owned());
            self.failed_reasons.push(
                self.archive
                    .error
                    .clone()
                    .or_else(|| self.archive.verification.error.clone())
                    .unwrap_or_else(|| {
                        "archive generation or verification did not pass".to_owned()
                    }),
            );
            "fix archive generation before sharing release artifacts".to_owned()
        } else if self.ready_for_gui_smoke {
            "optional: run axion gui-smoke before publishing the preview artifact".to_owned()
        } else {
            "release artifact is ready; GUI smoke still needs Servo checkout setup".to_owned()
        };
    }

    fn refresh_artifacts(&mut self) {
        let mut artifacts = Vec::new();

        if let Some(path) = &self.report_path {
            let mut artifact = artifact_for_file("release_report", Path::new(path), false);
            artifact.bytes = None;
            artifacts.push(artifact);
        }
        if let Some(path) = &self.bundle_report_path {
            artifacts.push(artifact_for_file("bundle_report", Path::new(path), true));
        }
        if let Some(path) = &self.bundle_manifest {
            artifacts.push(artifact_for_file("bundle_manifest", Path::new(path), true));
        }
        if let Some(path) = &self.archive.path {
            artifacts.push(artifact_for_file("archive", Path::new(path), true));
        }

        self.artifacts = artifacts;
    }

    fn summary(&self) -> ReleaseSummary {
        ReleaseSummary {
            artifacts_total: self.artifacts.len(),
            artifacts_missing: self
                .artifacts
                .iter()
                .filter(|artifact| !artifact.exists)
                .count(),
            artifacts_with_errors: self
                .artifacts
                .iter()
                .filter(|artifact| artifact.error.is_some())
                .count(),
            check_report_reused: self.check_report_reused,
            archive_requested: self.archive.requested,
            archive_passed: self.archive.passed,
        }
    }

    fn print_human(&self) {
        let summary = self.summary();
        println!("Axion release");
        println!("manifest: {}", self.manifest_path);
        println!(
            "summary: result={}, phase={}, next_step={}",
            self.result,
            self.failure_phase.as_deref().unwrap_or("none"),
            self.next_step
        );
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
        println!(
            "bundle: {}",
            if self.bundle_passed { "ok" } else { "failed" }
        );
        if let Some(error) = &self.bundle_error {
            println!("bundle.error: {error}");
        }
        if let Some(bundle_dir) = &self.bundle_dir {
            println!("bundle_dir: {bundle_dir}");
        }
        if let Some(bundle_manifest) = &self.bundle_manifest {
            println!("bundle_manifest: {bundle_manifest}");
        }
        if let Some(bundle_bytes) = self.bundle_bytes {
            println!("bundle_bytes: {bundle_bytes}");
        }
        if self.archive.requested {
            println!(
                "archive: {}",
                if self.archive.passed { "ok" } else { "failed" }
            );
            if let Some(path) = &self.archive.path {
                println!("archive_path: {path}");
            }
            if let Some(bytes) = self.archive.bytes {
                println!("archive_bytes: {bytes}");
            }
            if let Some(fingerprint) = &self.archive.fnv1a64 {
                println!("archive_fnv1a64: {fingerprint}");
            }
            if let Some(error) = &self.archive.error {
                println!("archive.error: {error}");
            }
            println!(
                "archive.verification: {}",
                if self.archive.verification.checked {
                    if self.archive.verification.passed {
                        "ok"
                    } else {
                        "failed"
                    }
                } else {
                    "not_checked"
                }
            );
            if let Some(error) = &self.archive.verification.error {
                println!("archive.verification.error: {error}");
            }
        } else {
            println!("archive: skipped (pass --archive to create a tar artifact)");
        }
        if let Some(report_path) = &self.report_path {
            println!("report: {report_path}");
        }
        if let Some(bundle_report_path) = &self.bundle_report_path {
            println!("bundle_report: {bundle_report_path}");
        }
        if let Some(check_report_path) = &self.check_report_path {
            println!("check_report: {check_report_path}");
            println!(
                "check_report.reused: {}",
                if self.check_report_reused {
                    "true"
                } else {
                    "false"
                }
            );
            if let Some(error) = &self.check_report_error {
                println!("check_report.error: {error}");
            }
        }
        println!(
            "artifacts: total={}, missing={}, errors={}",
            summary.artifacts_total, summary.artifacts_missing, summary.artifacts_with_errors
        );
        for artifact in &self.artifacts {
            println!(
                "artifact: kind={}, exists={}, path={}",
                artifact.kind, artifact.exists, artifact.path
            );
            if let Some(bytes) = artifact.bytes {
                println!("artifact.bytes: kind={}, bytes={}", artifact.kind, bytes);
            }
            if let Some(fingerprint) = &artifact.fnv1a64 {
                println!(
                    "artifact.fnv1a64: kind={}, fnv1a64={}",
                    artifact.kind, fingerprint
                );
            }
            if let Some(error) = &artifact.error {
                println!("artifact.error: kind={}, error={}", artifact.kind, error);
            }
        }
        if let Some(phase) = &self.failure_phase {
            println!("failure_phase: {phase}");
        }
        for reason in &self.failed_reasons {
            println!("failed_reason: {reason}");
        }
        println!("next_step: {}", self.next_step);
        println!("result: {}", self.result);
    }

    fn to_json(&self) -> String {
        let summary = self.summary();
        format!(
            "{{\"schema\":\"axion.release-report.v1\",\"manifest_path\":{},\"max_risk\":{},\"report_path\":{},\"bundle_report_path\":{},\"check_report\":{{\"path\":{},\"reused\":{},\"error\":{}}},\"doctor\":{{\"passed\":{},\"failed_reasons\":{}}},\"readiness\":{{\"ready_for_dev\":{},\"ready_for_bundle\":{},\"ready_for_gui_smoke\":{},\"blockers\":{},\"warnings\":{}}},\"self_test\":{{\"passed\":{},\"error\":{}}},\"bundle\":{{\"passed\":{},\"error\":{},\"build_executable\":{},\"bundle_dir\":{},\"bundle_manifest\":{},\"bundle_bytes\":{},\"report\":{}}},\"archive\":{},\"artifacts\":{},\"summary\":{},\"failure_phase\":{},\"failed_reasons\":{},\"next_step\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            json_string_literal(&self.max_risk),
            optional_json_string_literal(self.report_path.as_deref()),
            optional_json_string_literal(self.bundle_report_path.as_deref()),
            optional_json_string_literal(self.check_report_path.as_deref()),
            self.check_report_reused,
            optional_json_string_literal(self.check_report_error.as_deref()),
            self.doctor_passed,
            json_string_array_literal(&self.doctor_failures),
            self.ready_for_dev,
            self.ready_for_bundle,
            self.ready_for_gui_smoke,
            json_string_array_literal(&self.readiness_blockers),
            json_string_array_literal(&self.readiness_warnings),
            self.self_test_passed,
            optional_json_string_literal(self.self_test_error.as_deref()),
            self.bundle_passed,
            optional_json_string_literal(self.bundle_error.as_deref()),
            self.build_executable,
            optional_json_string_literal(self.bundle_dir.as_deref()),
            optional_json_string_literal(self.bundle_manifest.as_deref()),
            optional_json_u64(self.bundle_bytes),
            self.bundle_report.as_deref().unwrap_or("null"),
            self.archive.to_json(),
            artifact_array_json(&self.artifacts),
            summary.json(),
            optional_json_string_literal(self.failure_phase.as_deref()),
            json_string_array_literal(&self.failed_reasons),
            json_string_literal(&self.next_step),
            json_string_literal(&self.result),
        )
    }
}

impl ReleaseSummary {
    fn json(&self) -> String {
        format!(
            "{{\"artifacts_total\":{},\"artifacts_missing\":{},\"artifacts_with_errors\":{},\"check_report_reused\":{},\"archive_requested\":{},\"archive_passed\":{}}}",
            self.artifacts_total,
            self.artifacts_missing,
            self.artifacts_with_errors,
            self.check_report_reused,
            self.archive_requested,
            self.archive_passed,
        )
    }
}

impl ArchiveReport {
    fn to_json(&self) -> String {
        format!(
            "{{\"requested\":{},\"passed\":{},\"path\":{},\"bytes\":{},\"fnv1a64\":{},\"error\":{},\"verification\":{}}}",
            self.requested,
            self.passed,
            optional_json_string_literal(self.path.as_deref()),
            optional_json_u64(self.bytes),
            optional_json_string_literal(self.fnv1a64.as_deref()),
            optional_json_string_literal(self.error.as_deref()),
            self.verification.to_json(),
        )
    }
}

impl ArchiveVerification {
    fn to_json(&self) -> String {
        format!(
            "{{\"checked\":{},\"passed\":{},\"error\":{}}}",
            self.checked,
            self.passed,
            optional_json_string_literal(self.error.as_deref()),
        )
    }
}

impl ArtifactReport {
    fn to_json(&self) -> String {
        format!(
            "{{\"kind\":{},\"path\":{},\"exists\":{},\"bytes\":{},\"fnv1a64\":{},\"error\":{}}}",
            json_string_literal(&self.kind),
            json_string_literal(&self.path),
            self.exists,
            optional_json_u64(self.bytes),
            optional_json_string_literal(self.fnv1a64.as_deref()),
            optional_json_string_literal(self.error.as_deref()),
        )
    }
}

#[derive(Deserialize)]
struct ReusableCheckReport {
    schema: String,
    manifest_path: PathBuf,
    check_identity: Option<String>,
    max_risk: String,
    result: String,
    doctor: PassedCheck,
    self_test: PassedCheck,
    bundle_preflight: PassedCheck,
    readiness: CheckedReadiness,
}

#[derive(Deserialize)]
struct PassedCheck {
    passed: bool,
}

#[derive(Deserialize)]
struct CheckedReadiness {
    ready_for_dev: bool,
    ready_for_bundle: bool,
}

fn load_check_report_reuse(
    check_report_path: &Path,
    manifest_path: &Path,
    max_risk: &str,
) -> Result<(), std::io::Error> {
    let body = fs::read_to_string(check_report_path)?;
    let check: ReusableCheckReport = serde_json::from_str(&body).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid check report JSON: {error}"),
        )
    })?;
    let invalid = |message| std::io::Error::new(std::io::ErrorKind::InvalidData, message);
    if check.schema != "axion.check-report.v1" {
        return Err(invalid("check report schema must be axion.check-report.v1"));
    }
    if check.manifest_path.canonicalize()? != manifest_path.canonicalize()? {
        return Err(invalid(
            "check report manifest_path does not match release manifest",
        ));
    }
    if check.result != "ok" || !check.doctor.passed || !check.self_test.passed {
        return Err(invalid(
            "check report result, doctor and self_test must pass",
        ));
    }
    if !check.bundle_preflight.passed {
        return Err(invalid("check report bundle_preflight must pass"));
    }
    if !check.readiness.ready_for_dev || !check.readiness.ready_for_bundle {
        return Err(invalid(
            "check report readiness must be ready for dev and bundle",
        ));
    }
    if check.max_risk != max_risk
        || check.check_identity.as_deref()
            != Some(super::check_identity::check_identity(manifest_path, max_risk)?.as_str())
    {
        return Err(invalid(
            "check report content identity or check parameters changed; run check again",
        ));
    }
    Ok(())
}

fn create_archive(
    bundle_dir: Option<PathBuf>,
    archive_path: Option<PathBuf>,
) -> Result<ArchiveReport, AxionCliError> {
    let bundle_dir =
        bundle_dir.ok_or_else(|| std::io::Error::other("bundle_dir is unavailable"))?;
    let archive_path = archive_path.unwrap_or_else(|| default_archive_path(&bundle_dir));
    if resolved_output_path(&archive_path)?.starts_with(&bundle_dir.canonicalize()?) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "archive output must be outside the bundle directory",
        )
        .into());
    }
    if let Some(parent) = archive_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    write_tar_archive(&bundle_dir, &archive_path)?;
    let bytes = fs::metadata(&archive_path)?.len();
    let fingerprint = fnv1a64_file_hex(&archive_path)?;
    let verification = verify_archive(&archive_path, &bundle_dir, bytes, &fingerprint);
    let passed = verification.passed;
    let error = verification.error.clone();

    Ok(ArchiveReport {
        requested: true,
        passed,
        path: Some(archive_path.display().to_string()),
        bytes: Some(bytes),
        fnv1a64: Some(fingerprint),
        error,
        verification,
    })
}

fn verify_archive(
    path: &Path,
    source_dir: &Path,
    expected_bytes: u64,
    expected_fingerprint: &str,
) -> ArchiveVerification {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() == 0 => ArchiveVerification {
            checked: true,
            passed: false,
            error: Some("archive file is empty".to_owned()),
        },
        Ok(metadata) if metadata.len() != expected_bytes => ArchiveVerification {
            checked: true,
            passed: false,
            error: Some(format!(
                "archive byte count changed: expected {expected_bytes}, found {}",
                metadata.len()
            )),
        },
        Ok(_) => match fnv1a64_file_hex(path) {
            Ok(actual) if actual == expected_fingerprint => {
                let result = verify_archive_members(path, source_dir);
                ArchiveVerification {
                    checked: true,
                    passed: result.is_ok(),
                    error: result.err().map(|error| error.to_string()),
                }
            }
            Ok(actual) => ArchiveVerification {
                checked: true,
                passed: false,
                error: Some(format!(
                    "archive fingerprint changed: expected {expected_fingerprint}, found {actual}"
                )),
            },
            Err(error) => ArchiveVerification {
                checked: true,
                passed: false,
                error: Some(error.to_string()),
            },
        },
        Err(error) => ArchiveVerification {
            checked: true,
            passed: false,
            error: Some(error.to_string()),
        },
    }
}

fn artifact_for_file(kind: &str, path: &Path, include_fingerprint: bool) -> ArtifactReport {
    let mut artifact = ArtifactReport {
        kind: kind.to_owned(),
        path: path.display().to_string(),
        exists: false,
        bytes: None,
        fnv1a64: None,
        error: None,
    };

    match fs::metadata(path) {
        Ok(metadata) => {
            artifact.exists = true;
            artifact.bytes = Some(metadata.len());
            if include_fingerprint {
                match fnv1a64_file_hex(path) {
                    Ok(fingerprint) => artifact.fnv1a64 = Some(fingerprint),
                    Err(error) => artifact.error = Some(error.to_string()),
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => artifact.error = Some(error.to_string()),
    }

    artifact
}

fn default_archive_path(bundle_dir: &Path) -> PathBuf {
    let file_name = bundle_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "axion-bundle".to_owned());
    bundle_dir.with_file_name(format!("{file_name}.tar"))
}

fn write_tar_archive(source_dir: &Path, archive_path: &Path) -> Result<(), std::io::Error> {
    let source_dir = source_dir.canonicalize()?;
    if resolved_output_path(archive_path)?.starts_with(&source_dir) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "archive output must be outside the bundle directory",
        ));
    }
    let root_name = source_dir
        .file_name()
        .ok_or_else(|| std::io::Error::other("bundle has no directory name"))?;
    let mut output = tar::Builder::new(fs::File::create(archive_path)?);
    append_tar_entries(&mut output, &source_dir, &source_dir, Path::new(root_name))?;
    output.finish()
}

fn resolved_output_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut existing = absolute.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        suffix.push(
            existing
                .file_name()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "archive path cannot be resolved",
                    )
                })?
                .to_owned(),
        );
        existing = existing
            .parent()
            .ok_or_else(|| std::io::Error::other("archive path has no existing ancestor"))?;
    }
    let mut resolved = existing.canonicalize()?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn archive_mode(metadata: &fs::Metadata) -> u32 {
    if metadata.is_dir() {
        return 0o755;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            return 0o755;
        }
    }
    0o644
}

fn append_tar_entries(
    output: &mut tar::Builder<fs::File>,
    root: &Path,
    current: &Path,
    root_name: &Path,
) -> Result<(), std::io::Error> {
    let relative = current.strip_prefix(root).map_err(std::io::Error::other)?;
    let name = root_name.join(relative);
    let metadata = fs::symlink_metadata(current)?;
    let mut header = tar::Header::new_gnu();
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_mode(archive_mode(&metadata));
    if metadata.is_dir() {
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        output.append_data(&mut header, name, std::io::empty())?;
        let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            append_tar_entries(output, root, &entry.path(), root_name)?;
        }
    } else if metadata.is_file() {
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(metadata.len());
        header.set_cksum();
        output.append_data(&mut header, name, fs::File::open(current)?)?;
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "bundle archive contains a symlink or unsupported file type",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct ArchiveMember {
    directory: bool,
    mode: u32,
    bytes: u64,
    fingerprint: Option<String>,
}

fn expected_archive_members(
    root: &Path,
    current: &Path,
    root_name: &Path,
    members: &mut BTreeMap<PathBuf, ArchiveMember>,
) -> Result<(), std::io::Error> {
    let metadata = fs::symlink_metadata(current)?;
    if !metadata.is_dir() && !metadata.is_file() {
        return Err(std::io::Error::other(
            "bundle contains an unsupported archive member",
        ));
    }
    let name = root_name.join(current.strip_prefix(root).map_err(std::io::Error::other)?);
    members.insert(
        name,
        ArchiveMember {
            directory: metadata.is_dir(),
            mode: archive_mode(&metadata),
            bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            fingerprint: if metadata.is_file() {
                Some(fnv1a64_file_hex(current)?)
            } else {
                None
            },
        },
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(current)? {
            expected_archive_members(root, &entry?.path(), root_name, members)?;
        }
    }
    Ok(())
}

fn verify_archive_members(archive_path: &Path, source_dir: &Path) -> Result<(), std::io::Error> {
    let source_dir = source_dir.canonicalize()?;
    let root_name = source_dir
        .file_name()
        .ok_or_else(|| std::io::Error::other("bundle has no directory name"))?;
    let mut expected = BTreeMap::new();
    expected_archive_members(
        &source_dir,
        &source_dir,
        Path::new(root_name),
        &mut expected,
    )?;
    let mut archive = tar::Archive::new(fs::File::open(archive_path)?);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(std::io::Error::other("archive member has an unsafe path"));
        }
        let member = expected.remove(&path).ok_or_else(|| {
            std::io::Error::other(format!(
                "unexpected or duplicate archive member: {}",
                path.display()
            ))
        })?;
        let kind = entry.header().entry_type();
        if (member.directory && !kind.is_dir())
            || (!member.directory && !kind.is_file())
            || entry.header().mode()? != member.mode
            || entry.header().size()? != member.bytes
        {
            return Err(std::io::Error::other(format!(
                "archive member type, mode or size differs: {}",
                path.display()
            )));
        }
        if let Some(fingerprint) = member.fingerprint {
            if fnv1a64_reader_hex(&mut entry)? != fingerprint {
                return Err(std::io::Error::other(format!(
                    "archive member content differs: {}",
                    path.display()
                )));
            }
        }
    }
    if !expected.is_empty() {
        return Err(std::io::Error::other("archive is missing bundle members"));
    }
    Ok(())
}

fn fnv1a64_file_hex(path: &Path) -> Result<String, std::io::Error> {
    fnv1a64_reader_hex(&mut fs::File::open(path)?)
}

fn fnv1a64_reader_hex(reader: &mut impl Read) -> Result<String, std::io::Error> {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    Ok(format!("{hash:016x}"))
}

fn optional_json_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn artifact_array_json(values: &[ArtifactReport]) -> String {
    let values = values
        .iter()
        .map(ArtifactReport::to_json)
        .collect::<Vec<_>>()
        .join(",");

    format!("[{values}]")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::cli::{DoctorRisk, ReleaseArgs};

    use super::{create_archive, load_check_report_reuse, release_report};

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}"))
    }

    #[test]
    fn release_report_serializes_failed_gate() {
        let manifest = temp_dir("axion-release-missing").join("axion.toml");
        let report = release_report(&ReleaseArgs {
            manifest_path: manifest,
            output_dir: None,
            executable: None,
            bin: None,
            report_path: Some(PathBuf::from("target/axion/reports/release.json")),
            bundle_report_path: Some(PathBuf::from("target/axion/reports/bundle.json")),
            check_report_path: None,
            max_risk: DoctorRisk::Medium,
            skip_build_executable: true,
            archive: true,
            archive_path: None,
            keep_artifacts: false,
            json: true,
        });
        let json = report.to_json();

        assert_eq!(report.result, "failed");
        assert!(report.failure_phase.is_some());
        assert!(!report.failed_reasons.is_empty());
        assert!(json.contains("\"schema\":\"axion.release-report.v1\""));
        assert!(json.contains("\"bundle\":{\"passed\":false"));
        assert!(json.contains("\"archive\":{\"requested\":true,\"passed\":false"));
        assert!(json.contains("\"summary\":{\"artifacts_total\":"));
        assert!(json.contains("\"artifacts_missing\":"));
        assert!(json.contains("\"artifacts_with_errors\":"));
        assert!(json.contains("\"check_report_reused\":false"));
        assert!(json.contains("\"archive_requested\":true"));
        assert!(json.contains("\"archive_passed\":false"));
        assert!(json.contains("\"failure_phase\":"));
        assert!(json.contains("\"failed_reasons\":["));
        assert!(json.contains("\"result\":\"failed\""));
        assert_eq!(report.summary().artifacts_total, report.artifacts.len());
    }

    #[test]
    fn create_archive_writes_tar_and_fingerprint() {
        let root = temp_dir("axion-release-archive");
        let bundle = root.join("demo.app");
        let archive = root.join("demo.app.tar");
        fs::create_dir_all(bundle.join("Contents")).unwrap();
        fs::write(bundle.join("Contents").join("Info.plist"), "metadata").unwrap();

        let report = create_archive(Some(bundle), Some(archive.clone())).expect("archive succeeds");

        assert!(report.passed);
        assert_eq!(report.path, Some(archive.display().to_string()));
        assert!(report.bytes.unwrap_or_default() > 1024);
        assert_eq!(report.fnv1a64.as_deref().map(str::len), Some(16));
        assert!(report.verification.checked);
        assert!(report.verification.passed);
        assert!(report.verification.error.is_none());
    }

    fn check_fixture() -> (PathBuf, PathBuf) {
        let root = temp_dir("axion-release-check-report");
        fs::create_dir_all(root.join("frontend")).unwrap();
        fs::write(root.join("frontend/index.html"), "hello").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(&manifest, "[app]\nname = \"review-demo\"\n[window]\nid = \"main\"\ntitle = \"Review\"\n[build]\nfrontend_dist = \"frontend\"\nentry = \"frontend/index.html\"\n").unwrap();
        let report = root.join("check.json");
        let identity = super::super::check_identity::check_identity(&manifest, "medium").unwrap();
        fs::write(&report, serde_json::to_string_pretty(&serde_json::json!({
            "schema":"axion.check-report.v1", "manifest_path":manifest, "check_identity":identity, "max_risk":"medium", "result":"ok",
            "doctor":{"passed":true}, "self_test":{"passed":true}, "bundle_preflight":{"passed":true}, "readiness":{"ready_for_dev":true,"ready_for_bundle":true}
        })).unwrap()).unwrap();
        (manifest, report)
    }

    #[test]
    fn check_report_reuse_binds_resources_manifest_and_parameters() {
        let (manifest, report) = check_fixture();
        load_check_report_reuse(&report, &manifest, "medium").unwrap();
        assert!(load_check_report_reuse(&report, &manifest, "low").is_err());
        fs::write(
            manifest.parent().unwrap().join("frontend/index.html"),
            "changed",
        )
        .unwrap();
        assert!(
            load_check_report_reuse(&report, &manifest, "medium")
                .unwrap_err()
                .to_string()
                .contains("identity")
        );
        let (manifest, report) = check_fixture();
        let mut source = fs::read_to_string(&manifest).unwrap();
        source.push_str("\n# changed configuration\n");
        fs::write(&manifest, source).unwrap();
        assert!(load_check_report_reuse(&report, &manifest, "medium").is_err());
    }

    #[test]
    fn check_report_reuse_rejects_failed_bundle_preflight_and_old_reports() {
        let (manifest, report) = check_fixture();
        let mut body: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        body["bundle_preflight"]["passed"] = false.into();
        fs::write(&report, body.to_string()).unwrap();
        assert!(
            load_check_report_reuse(&report, &manifest, "medium")
                .unwrap_err()
                .to_string()
                .contains("bundle_preflight must pass")
        );
        body["bundle_preflight"]["passed"] = true.into();
        body.as_object_mut().unwrap().remove("check_identity");
        fs::write(&report, body.to_string()).unwrap();
        assert!(
            load_check_report_reuse(&report, &manifest, "medium")
                .unwrap_err()
                .to_string()
                .contains("identity")
        );
    }

    #[test]
    fn archive_output_cannot_overwrite_bundle_input() {
        let root = temp_dir("axion-release-overlap");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("keep.txt");
        fs::write(&file, "keep").unwrap();
        assert!(create_archive(Some(root), Some(file.clone())).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn archive_preserves_executable_permissions_after_unpack() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("axion-release-executable");
        let bundle = root.join("demo.app");
        fs::create_dir_all(&bundle).unwrap();
        let executable = bundle.join("demo");
        fs::write(&executable, "#!/bin/sh\nprintf working").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o751)).unwrap();
        let archive = root.join("demo.tar");
        create_archive(Some(bundle.clone()), Some(archive.clone())).unwrap();
        let extracted = root.join("unpacked");
        tar::Archive::new(fs::File::open(&archive).unwrap())
            .unpack(&extracted)
            .unwrap();
        let result = std::process::Command::new(extracted.join("demo.app/demo"))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"working");
        // A syntactically valid archive with the wrong mode must fail semantic verification.
        let mut writer = tar::Builder::new(fs::File::create(&archive).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        writer
            .append_data(&mut header, "demo.app", std::io::empty())
            .unwrap();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_size(fs::metadata(&executable).unwrap().len());
        header.set_cksum();
        writer
            .append_data(
                &mut header,
                "demo.app/demo",
                fs::File::open(&executable).unwrap(),
            )
            .unwrap();
        writer.finish().unwrap();
        drop(writer);
        let result = super::verify_archive(
            &archive,
            &bundle,
            fs::metadata(&archive).unwrap().len(),
            &super::fnv1a64_file_hex(&archive).unwrap(),
        );
        assert!(!result.passed);
        assert!(result.error.unwrap().contains("mode"));
    }
    #[test]
    fn cached_report_never_bypasses_the_current_doctor_gate() {
        let (manifest, check_report) = check_fixture();
        let mut source = fs::read_to_string(&manifest).unwrap();
        source.push_str("\n[capabilities.main]\nprofiles = [\"file-access\"]\n");
        fs::write(&manifest, source).unwrap();
        let report = release_report(&ReleaseArgs {
            manifest_path: manifest,
            output_dir: None,
            executable: None,
            bin: None,
            report_path: None,
            bundle_report_path: None,
            check_report_path: Some(check_report),
            max_risk: DoctorRisk::Low,
            skip_build_executable: true,
            archive: false,
            archive_path: None,
            keep_artifacts: false,
            json: true,
        });
        assert!(!report.doctor_passed);
        assert!(!report.check_report_reused);
        assert_eq!(report.result, "failed");
        assert!(
            report
                .doctor_failures
                .iter()
                .any(|reason| reason.contains("risk"))
        );
    }
}
