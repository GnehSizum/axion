use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use axion_runtime::json_string_literal;

use crate::cli::{BundleArgs, DoctorArgs, ReleaseArgs, SelfTestArgs};
use crate::commands::bundle::{bundle_report, write_report_if_requested};
use crate::commands::doctor::{doctor_gate_for_manifest, doctor_readiness_for_manifest};
use crate::error::AxionCliError;

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
            Err(error) => report.self_test_error = Some(error.to_string()),
        }
    }

    if report.doctor_passed && report.ready_for_bundle && report.self_test_passed {
        let bundle_args = BundleArgs {
            manifest_path: args.manifest_path.clone(),
            output_dir: args.output_dir.clone(),
            executable: args.executable.clone(),
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

    fn print_human(&self) {
        println!("Axion release");
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
        format!(
            "{{\"schema\":\"axion.release-report.v1\",\"manifest_path\":{},\"max_risk\":{},\"report_path\":{},\"bundle_report_path\":{},\"doctor\":{{\"passed\":{},\"failed_reasons\":{}}},\"readiness\":{{\"ready_for_dev\":{},\"ready_for_bundle\":{},\"ready_for_gui_smoke\":{},\"blockers\":{},\"warnings\":{}}},\"self_test\":{{\"passed\":{},\"error\":{}}},\"bundle\":{{\"passed\":{},\"error\":{},\"build_executable\":{},\"bundle_dir\":{},\"bundle_manifest\":{},\"bundle_bytes\":{},\"report\":{}}},\"archive\":{},\"artifacts\":{},\"failure_phase\":{},\"failed_reasons\":{},\"next_step\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            json_string_literal(&self.max_risk),
            optional_json_string_literal(self.report_path.as_deref()),
            optional_json_string_literal(self.bundle_report_path.as_deref()),
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
            optional_json_string_literal(self.failure_phase.as_deref()),
            json_string_array_literal(&self.failed_reasons),
            json_string_literal(&self.next_step),
            json_string_literal(&self.result),
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

fn create_archive(
    bundle_dir: Option<PathBuf>,
    archive_path: Option<PathBuf>,
) -> Result<ArchiveReport, AxionCliError> {
    let bundle_dir =
        bundle_dir.ok_or_else(|| std::io::Error::other("bundle_dir is unavailable"))?;
    let archive_path = archive_path.unwrap_or_else(|| default_archive_path(&bundle_dir));
    if let Some(parent) = archive_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    write_tar_archive(&bundle_dir, &archive_path)?;
    let bytes = fs::metadata(&archive_path)?.len();
    let fingerprint = fnv1a64_file_hex(&archive_path)?;
    let verification = verify_archive(&archive_path, bytes, &fingerprint);
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
            Ok(actual) if actual == expected_fingerprint => ArchiveVerification {
                checked: true,
                passed: true,
                error: None,
            },
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
    let root_name = source_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".to_owned());
    let mut output = fs::File::create(archive_path)?;
    write_tar_entries(&mut output, &source_dir, &source_dir, &root_name)?;
    output.write_all(&[0_u8; 1024])?;
    Ok(())
}

fn write_tar_entries(
    output: &mut fs::File,
    root: &Path,
    current: &Path,
    root_name: &str,
) -> Result<(), std::io::Error> {
    let relative = current.strip_prefix(root).map_err(std::io::Error::other)?;
    let entry_name = if relative.as_os_str().is_empty() {
        root_name.to_owned()
    } else {
        format!("{root_name}/{}", relative_path_string(relative))
    };
    let metadata = fs::metadata(current)?;
    if metadata.is_dir() {
        write_tar_header(output, &format!("{entry_name}/"), 0, b'5', 0o755)?;
        let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            write_tar_entries(output, root, &entry.path(), root_name)?;
        }
    } else if metadata.is_file() {
        write_tar_header(output, &entry_name, metadata.len(), b'0', 0o644)?;
        let mut file = fs::File::open(current)?;
        std::io::copy(&mut file, output)?;
        pad_tar_entry(output, metadata.len())?;
    }
    Ok(())
}

fn write_tar_header(
    output: &mut fs::File,
    name: &str,
    size: u64,
    entry_type: u8,
    mode: u32,
) -> Result<(), std::io::Error> {
    if name.len() > 100 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("tar path is too long: {name}"),
        ));
    }

    let mut header = [0_u8; 512];
    write_tar_field(&mut header[0..100], name.as_bytes());
    write_tar_octal(&mut header[100..108], u64::from(mode));
    write_tar_octal(&mut header[108..116], 0);
    write_tar_octal(&mut header[116..124], 0);
    write_tar_octal(&mut header[124..136], size);
    write_tar_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = entry_type;
    write_tar_field(&mut header[257..263], b"ustar\0");
    write_tar_field(&mut header[263..265], b"00");

    let checksum = header.iter().map(|byte| u64::from(*byte)).sum();
    write_tar_octal(&mut header[148..156], checksum);
    output.write_all(&header)
}

fn write_tar_field(field: &mut [u8], value: &[u8]) {
    let length = value.len().min(field.len());
    field[..length].copy_from_slice(&value[..length]);
}

fn write_tar_octal(field: &mut [u8], value: u64) {
    let width = field.len();
    let encoded = format!("{value:0width$o}\0", width = width - 1);
    field.copy_from_slice(encoded.as_bytes());
}

fn pad_tar_entry(output: &mut fs::File, size: u64) -> Result<(), std::io::Error> {
    let remainder = size % 512;
    if remainder != 0 {
        let padding = 512 - remainder;
        output.write_all(&vec![0_u8; padding as usize])?;
    }
    Ok(())
}

fn fnv1a64_file_hex(path: &Path) -> Result<String, std::io::Error> {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut file = fs::File::open(path)?;
    let mut hash = FNV_OFFSET_BASIS;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer)?;
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

fn relative_path_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn optional_json_u64(value: Option<u64>) -> String {
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

    use super::{create_archive, release_report};

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
            report_path: Some(PathBuf::from("target/axion/reports/release.json")),
            bundle_report_path: Some(PathBuf::from("target/axion/reports/bundle.json")),
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
        assert!(json.contains("\"failure_phase\":"));
        assert!(json.contains("\"failed_reasons\":["));
        assert!(json.contains("\"result\":\"failed\""));
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
}
