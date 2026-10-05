use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use axion_core::{Builder, RunMode};
use axion_packager::{
    BundleArtifact, BundleMetadata, BundlePlan, BundleVerificationReport, current_bundle_target,
    stage_bundle_from_web_assets_with_deployment_manifest, verify_bundle_artifact,
};
use axion_runtime::json_string_literal;

use crate::cli::BundleArgs;
use crate::commands::doctor::doctor_readiness_for_manifest;
use crate::error::AxionCliError;

pub fn run(args: BundleArgs) -> Result<(), AxionCliError> {
    let report = bundle_report(&args)?;
    write_report_if_requested(args.report_path.as_deref(), &report)?;
    if args.json {
        println!("{}", report.to_json());
    } else {
        report.print_human();
    }

    if report.result() == "failed" {
        return Err(std::io::Error::other(format!(
            "manifest is not ready for bundle; run `cargo run -p axion-cli -- check --manifest-path {} --bundle`",
            args.manifest_path.display()
        ))
        .into());
    }

    Ok(())
}

pub(crate) fn bundle_report(args: &BundleArgs) -> Result<BundleReport, AxionCliError> {
    let readiness = doctor_readiness_for_manifest(&args.manifest_path)?;
    if !readiness.ready_for_bundle() {
        return Ok(BundleReport::readiness_failed(args, readiness.blockers()));
    }

    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let target = current_bundle_target();
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| default_output_dir(&args.manifest_path, &launch_config.app_name));
    let executable_path = resolve_executable_path(
        &args.manifest_path,
        &launch_config.app_name,
        args.executable.clone(),
        args.build_executable,
        args.bin.as_deref(),
        args.json,
    )?;

    let deployment_manifest = axion_manifest::deployment_manifest_source(&args.manifest_path)?;
    let artifact = stage_bundle_from_web_assets_with_deployment_manifest(
        launch_config.frontend_dist,
        launch_config.packaged_entry,
        BundlePlan {
            target,
            output_dir: output_dir.clone(),
            executable_path,
        },
        &BundleMetadata {
            app_name: launch_config.app_name.clone(),
            identifier: launch_config.identifier.clone(),
            version: launch_config.version.clone(),
            description: launch_config.description.clone(),
            authors: launch_config.authors.clone(),
            homepage: launch_config.homepage.clone(),
            icon: app.config().bundle.icon.clone(),
        },
        &args.manifest_path,
        &deployment_manifest,
    )?;
    let verification = verify_bundle_artifact(&artifact)?;
    let report = BundleReport::success(BundleReportSuccessInput {
        manifest_path: &args.manifest_path,
        app_name: &launch_config.app_name,
        identifier: launch_config.identifier.as_deref(),
        app_version: launch_config.version.as_deref(),
        artifact: &artifact,
        verification: &verification,
        build_executable: args.build_executable,
        report_path: args.report_path.as_deref(),
    });

    Ok(report)
}

fn resolve_executable_path(
    manifest_path: &Path,
    app_name: &str,
    explicit_executable: Option<PathBuf>,
    build_executable: bool,
    bin: Option<&str>,
    quiet: bool,
) -> Result<Option<PathBuf>, AxionCliError> {
    if let Some(executable) = explicit_executable {
        return Ok(Some(executable));
    }
    if build_executable {
        return build_release_executable(manifest_path, bin, quiet).map(Some);
    }
    Ok(default_executable_path(manifest_path, app_name))
}

fn build_release_executable(
    manifest_path: &Path,
    bin: Option<&str>,
    quiet: bool,
) -> Result<PathBuf, AxionCliError> {
    let cargo_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml")
        .canonicalize()?;
    let cargo_project_dir = cargo_manifest_path
        .parent()
        .expect("Cargo manifest has a parent");
    if !quiet {
        println!(
            "building executable: cargo build --release --features servo-runtime --manifest-path {}",
            cargo_manifest_path.display()
        );
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(cargo_project_dir)
        .args([
            "build",
            "--release",
            "--features",
            "servo-runtime",
            "--message-format=json-render-diagnostics",
            "--manifest-path",
        ])
        .arg(&cargo_manifest_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(bin) = bin {
        command.args(["--bin", bin]);
    }
    let mut child = command.spawn()?;
    let mut executables = Vec::new();
    let mut read_error = None;
    for line in BufReader::new(child.stdout.take().expect("Cargo stdout is piped")).lines() {
        match line {
            Ok(line) => match serde_json::from_str::<CargoMessage>(&line) {
                Ok(message) => {
                    if message.reason == "compiler-artifact"
                        && message.target.as_ref().is_some_and(|target| {
                            target.kind.iter().any(|kind| kind == "bin")
                                && bin.is_none_or(|bin| target.name == bin)
                        })
                    {
                        if let Some(executable) = message.executable {
                            if !executables.contains(&executable) {
                                executables.push(executable);
                            }
                        }
                    }
                    if let Some(rendered) = message.message.and_then(|message| message.rendered) {
                        eprint!("{rendered}");
                    }
                }
                Err(error) => {
                    read_error.get_or_insert_with(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("invalid Cargo JSON message: {error}"),
                        )
                    });
                }
            },
            Err(error) => {
                read_error.get_or_insert(error);
            }
        }
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "cargo build --release --features servo-runtime failed with status {status}"
        ))
        .into());
    }
    if let Some(error) = read_error {
        return Err(error.into());
    }
    select_cargo_executable(executables).map_err(Into::into)
}

#[derive(serde::Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    target: Option<CargoTarget>,
    #[serde(default)]
    executable: Option<PathBuf>,
    #[serde(default)]
    message: Option<CargoDiagnostic>,
}

#[derive(serde::Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(serde::Deserialize)]
struct CargoDiagnostic {
    rendered: Option<String>,
}

fn select_cargo_executable(executables: Vec<PathBuf>) -> Result<PathBuf, std::io::Error> {
    match executables.as_slice() {
        [path] if path.is_file() => Ok(path.clone()),
        [_] => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cargo executable artifact does not exist",
        )),
        [] => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cargo succeeded without an executable binary artifact",
        )),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Cargo produced several executable binaries; select one with --bin <name>",
        )),
    }
}

pub(crate) fn write_report_if_requested(
    report_path: Option<&Path>,
    report: &BundleReport,
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
pub(crate) struct BundleReport {
    manifest_path: String,
    app: Option<String>,
    identifier: Option<String>,
    app_version: Option<String>,
    target: String,
    layout: String,
    output_dir: Option<String>,
    bundle_dir: Option<String>,
    resources_app_dir: Option<String>,
    entry_path: Option<String>,
    asset_manifest: Option<String>,
    metadata: Option<String>,
    platform_metadata: Vec<String>,
    bundle_manifest: Option<String>,
    icon: Option<String>,
    executable: Option<String>,
    build_executable: bool,
    report_path: Option<String>,
    verification: BundleReportVerification,
    blockers: Vec<String>,
    warnings: Vec<String>,
    result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BundleReportVerification {
    checked: bool,
    passed: bool,
    error: Option<String>,
    checked_dirs: usize,
    checked_files: usize,
    checked_paths: Vec<String>,
    bundle_files: usize,
    fingerprinted_files: usize,
    bundle_bytes: u64,
}

struct BundleReportSuccessInput<'a> {
    manifest_path: &'a Path,
    app_name: &'a str,
    identifier: Option<&'a str>,
    app_version: Option<&'a str>,
    artifact: &'a BundleArtifact,
    verification: &'a BundleVerificationReport,
    build_executable: bool,
    report_path: Option<&'a Path>,
}

impl BundleReport {
    fn readiness_failed(args: &BundleArgs, blockers: &[String]) -> Self {
        let target = current_bundle_target();
        Self {
            manifest_path: args.manifest_path.display().to_string(),
            app: None,
            identifier: None,
            app_version: None,
            target: target.as_str().to_owned(),
            layout: target.layout_summary().to_owned(),
            output_dir: args
                .output_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            bundle_dir: None,
            resources_app_dir: None,
            entry_path: None,
            asset_manifest: None,
            metadata: None,
            platform_metadata: Vec::new(),
            bundle_manifest: None,
            icon: None,
            executable: args
                .executable
                .as_ref()
                .map(|path| path.display().to_string()),
            build_executable: args.build_executable,
            report_path: args
                .report_path
                .as_ref()
                .map(|path| path.display().to_string()),
            verification: BundleReportVerification::failed(
                "manifest is not ready for bundle checks",
            ),
            blockers: blockers.to_vec(),
            warnings: Vec::new(),
            result: "failed".to_owned(),
        }
    }

    fn success(input: BundleReportSuccessInput<'_>) -> Self {
        Self {
            manifest_path: input.manifest_path.display().to_string(),
            app: Some(input.app_name.to_owned()),
            identifier: input.identifier.map(str::to_owned),
            app_version: input.app_version.map(str::to_owned),
            target: input.artifact.target.as_str().to_owned(),
            layout: input.artifact.target.layout_summary().to_owned(),
            output_dir: Some(input.artifact.output_dir.display().to_string()),
            bundle_dir: Some(input.artifact.bundle_dir.display().to_string()),
            resources_app_dir: Some(input.artifact.resources_app_dir.display().to_string()),
            entry_path: Some(input.artifact.entry_path.display().to_string()),
            asset_manifest: Some(input.artifact.asset_manifest_path.display().to_string()),
            metadata: Some(input.artifact.metadata_path.display().to_string()),
            platform_metadata: input
                .artifact
                .platform_metadata_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            bundle_manifest: Some(input.artifact.bundle_manifest_path.display().to_string()),
            icon: input
                .artifact
                .icon_path
                .as_ref()
                .map(|path| path.display().to_string()),
            executable: input
                .artifact
                .executable_path
                .as_ref()
                .map(|path| path.display().to_string()),
            build_executable: input.build_executable,
            report_path: input.report_path.map(|path| path.display().to_string()),
            verification: BundleReportVerification::passed(input.verification),
            blockers: Vec::new(),
            warnings: Vec::new(),
            result: "ok".to_owned(),
        }
    }

    pub(crate) fn result(&self) -> &str {
        &self.result
    }

    pub(crate) fn bundle_dir(&self) -> Option<&str> {
        self.bundle_dir.as_deref()
    }

    pub(crate) fn bundle_manifest(&self) -> Option<&str> {
        self.bundle_manifest.as_deref()
    }

    pub(crate) fn bundle_bytes(&self) -> u64 {
        self.verification.bundle_bytes
    }

    pub(crate) fn print_human(&self) {
        println!("Axion bundle");
        println!("manifest: {}", self.manifest_path);
        if let Some(app) = &self.app {
            println!("app: {app}");
        }
        if let Some(identifier) = &self.identifier {
            println!("identifier: {identifier}");
        }
        if let Some(version) = &self.app_version {
            println!("version: {version}");
        }
        println!("target: {}", self.target);
        println!("layout: {}", self.layout);
        if let Some(output_dir) = &self.output_dir {
            println!("output_dir: {output_dir}");
        }
        if let Some(bundle_dir) = &self.bundle_dir {
            println!("bundle_dir: {bundle_dir}");
        }
        if let Some(resources_app_dir) = &self.resources_app_dir {
            println!("resources_app_dir: {resources_app_dir}");
        }
        if let Some(entry_path) = &self.entry_path {
            println!("entry_path: {entry_path}");
        }
        if let Some(asset_manifest) = &self.asset_manifest {
            println!("asset_manifest: {asset_manifest}");
        }
        if let Some(metadata) = &self.metadata {
            println!("metadata: {metadata}");
        }
        for platform_metadata in &self.platform_metadata {
            println!("platform_metadata: {platform_metadata}");
        }
        if let Some(bundle_manifest) = &self.bundle_manifest {
            println!("bundle_manifest: {bundle_manifest}");
        }
        if self.verification.passed {
            println!("verification: ok");
        } else {
            println!(
                "verification: failed ({})",
                self.verification.error.as_deref().unwrap_or("unknown")
            );
        }
        println!("bundle_files: {}", self.verification.bundle_files);
        println!(
            "fingerprinted_files: {}",
            self.verification.fingerprinted_files
        );
        println!("bundle_bytes: {}", self.verification.bundle_bytes);
        println!("checked_dirs: {}", self.verification.checked_dirs);
        println!("checked_files: {}", self.verification.checked_files);
        println!("checked_paths: {}", self.verification.checked_paths.len());
        match &self.icon {
            Some(path) => println!("icon: {path}"),
            None => println!("icon: not configured"),
        }
        match &self.executable {
            Some(path) => println!("executable: {path}"),
            None => println!(
                "executable: not bundled (pass --executable or --build-executable to include one)"
            ),
        }
        if let Some(report_path) = &self.report_path {
            println!("report: {report_path}");
        }
        for blocker in &self.blockers {
            println!("readiness.blocker: {blocker}");
        }
        for warning in &self.warnings {
            println!("warning: {warning}");
        }
        println!("result: {}", self.result);
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"axion.bundle-report.v1\",\"manifest_path\":{},\"app\":{},\"identifier\":{},\"app_version\":{},\"target\":{},\"layout\":{},\"output_dir\":{},\"bundle_dir\":{},\"resources_app_dir\":{},\"entry_path\":{},\"asset_manifest\":{},\"metadata\":{},\"platform_metadata\":{},\"bundle_manifest\":{},\"icon\":{},\"executable\":{},\"build_executable\":{},\"report_path\":{},\"verification\":{},\"blockers\":{},\"warnings\":{},\"result\":{}}}",
            json_string_literal(&self.manifest_path),
            optional_json_string_literal(self.app.as_deref()),
            optional_json_string_literal(self.identifier.as_deref()),
            optional_json_string_literal(self.app_version.as_deref()),
            json_string_literal(&self.target),
            json_string_literal(&self.layout),
            optional_json_string_literal(self.output_dir.as_deref()),
            optional_json_string_literal(self.bundle_dir.as_deref()),
            optional_json_string_literal(self.resources_app_dir.as_deref()),
            optional_json_string_literal(self.entry_path.as_deref()),
            optional_json_string_literal(self.asset_manifest.as_deref()),
            optional_json_string_literal(self.metadata.as_deref()),
            json_string_array_literal(&self.platform_metadata),
            optional_json_string_literal(self.bundle_manifest.as_deref()),
            optional_json_string_literal(self.icon.as_deref()),
            optional_json_string_literal(self.executable.as_deref()),
            self.build_executable,
            optional_json_string_literal(self.report_path.as_deref()),
            self.verification.to_json(),
            json_string_array_literal(&self.blockers),
            json_string_array_literal(&self.warnings),
            json_string_literal(&self.result),
        )
    }
}

impl BundleReportVerification {
    fn passed(verification: &BundleVerificationReport) -> Self {
        Self {
            checked: true,
            passed: true,
            error: None,
            checked_dirs: verification.checked_dirs,
            checked_files: verification.checked_files,
            checked_paths: verification
                .checked_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            bundle_files: verification.bundle_file_count,
            fingerprinted_files: verification.fingerprinted_files,
            bundle_bytes: verification.total_bytes,
        }
    }

    fn failed(error: &str) -> Self {
        Self {
            checked: false,
            passed: false,
            error: Some(error.to_owned()),
            checked_dirs: 0,
            checked_files: 0,
            checked_paths: Vec::new(),
            bundle_files: 0,
            fingerprinted_files: 0,
            bundle_bytes: 0,
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"checked\":{},\"passed\":{},\"error\":{},\"checked_dirs\":{},\"checked_files\":{},\"checked_paths\":{},\"bundle_files\":{},\"fingerprinted_files\":{},\"bundle_bytes\":{}}}",
            self.checked,
            self.passed,
            optional_json_string_literal(self.error.as_deref()),
            self.checked_dirs,
            self.checked_files,
            json_string_array_literal(&self.checked_paths),
            self.bundle_files,
            self.fingerprinted_files,
            self.bundle_bytes,
        )
    }
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

fn default_output_dir(manifest_path: &Path, app_name: &str) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("target")
        .join("axion")
        .join(app_name)
        .join("bundle")
}

fn default_executable_path(manifest_path: &Path, app_name: &str) -> Option<PathBuf> {
    let executable_name = executable_file_name(app_name);
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut target_dirs = vec![manifest_dir.join("target")];

    for ancestor in manifest_dir.ancestors() {
        target_dirs.push(ancestor.join("target"));
    }

    target_dirs
        .into_iter()
        .flat_map(|target_dir| {
            let executable_name = executable_name.clone();
            ["release", "debug"]
                .into_iter()
                .map(move |profile| target_dir.join(profile).join(&executable_name))
        })
        .find(|path| path.is_file())
}

fn executable_file_name(app_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{app_name}.exe")
    } else {
        app_name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        BundleReport, BundleReportSuccessInput, default_executable_path, default_output_dir,
        executable_file_name, write_report_if_requested,
    };
    use axion_packager::{
        BundleArtifact, BundleTarget, BundleVerificationReport, current_bundle_target,
    };

    #[test]
    fn default_output_dir_is_workspace_local() {
        let path = default_output_dir(Path::new("/tmp/demo/axion.toml"), "hello-axion");
        assert_eq!(
            path,
            PathBuf::from("/tmp/demo/target/axion/hello-axion/bundle")
        );
    }

    #[test]
    fn bundle_layout_summary_describes_platform_structure() {
        assert!(
            BundleTarget::MacOsApp
                .layout_summary()
                .contains("Contents/MacOS")
        );
        assert!(
            BundleTarget::LinuxDir
                .layout_summary()
                .contains("resources/app")
        );
        assert!(BundleTarget::WindowsDir.layout_summary().contains("*.exe"));
    }

    #[test]
    fn default_executable_path_searches_manifest_ancestors() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("axion-bundle-cli-{unique}"));
        let app_dir = root.join("examples").join("hello-axion");
        let executable = root
            .join("target")
            .join("release")
            .join(executable_file_name("hello-axion"));
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(&executable, "binary").unwrap();

        assert_eq!(
            default_executable_path(&app_dir.join("axion.toml"), "hello-axion"),
            Some(executable)
        );
    }

    #[test]
    fn default_executable_path_prefers_release_over_debug() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("axion-bundle-cli-prefer-{unique}"));
        let app_dir = root.join("examples").join("hello-axion");
        let debug_executable = root
            .join("target")
            .join("debug")
            .join(executable_file_name("hello-axion"));
        let release_executable = root
            .join("target")
            .join("release")
            .join(executable_file_name("hello-axion"));
        fs::create_dir_all(debug_executable.parent().unwrap()).unwrap();
        fs::create_dir_all(release_executable.parent().unwrap()).unwrap();
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(&debug_executable, "debug").unwrap();
        fs::write(&release_executable, "release").unwrap();

        assert_eq!(
            default_executable_path(&app_dir.join("axion.toml"), "hello-axion"),
            Some(release_executable)
        );
    }

    #[test]
    fn default_executable_path_falls_back_to_debug() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("axion-bundle-cli-debug-{unique}"));
        let app_dir = root.join("examples").join("hello-axion");
        let debug_executable = root
            .join("target")
            .join("debug")
            .join(executable_file_name("hello-axion"));
        fs::create_dir_all(debug_executable.parent().unwrap()).unwrap();
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(&debug_executable, "debug").unwrap();

        assert_eq!(
            default_executable_path(&app_dir.join("axion.toml"), "hello-axion"),
            Some(debug_executable)
        );
    }

    #[test]
    fn bundle_report_serializes_stable_json_schema() {
        let target = current_bundle_target();
        let artifact = BundleArtifact {
            target,
            output_dir: PathBuf::from("/tmp/out"),
            bundle_dir: PathBuf::from("/tmp/out/demo"),
            resources_app_dir: PathBuf::from("/tmp/out/demo/resources/app"),
            executable_path: Some(PathBuf::from("/tmp/out/demo/bin/demo")),
            entry_path: PathBuf::from("/tmp/out/demo/resources/app/index.html"),
            asset_manifest_path: PathBuf::from("/tmp/out/demo/resources/app/axion-assets.json"),
            metadata_path: PathBuf::from("/tmp/out/demo/axion-bundle.txt"),
            platform_metadata_paths: vec![PathBuf::from("/tmp/out/demo/demo.desktop")],
            bundle_manifest_path: PathBuf::from("/tmp/out/demo/axion-bundle-manifest.json"),
            icon_path: Some(PathBuf::from("/tmp/out/demo/resources/app.icns")),
        };
        let verification = BundleVerificationReport {
            bundle_dir: artifact.bundle_dir.clone(),
            checked_paths: vec![artifact.bundle_dir.clone(), artifact.entry_path.clone()],
            checked_dirs: 1,
            checked_files: 1,
            bundle_file_count: 5,
            fingerprinted_files: 5,
            total_bytes: 42,
        };
        let report = BundleReport::success(BundleReportSuccessInput {
            manifest_path: Path::new("examples/demo/axion.toml"),
            app_name: "demo",
            identifier: Some("dev.axion.demo"),
            app_version: Some("1.0.0"),
            artifact: &artifact,
            verification: &verification,
            build_executable: true,
            report_path: Some(Path::new("target/axion/reports/demo-bundle.json")),
        });
        let json = report.to_json();

        assert!(json.contains("\"schema\":\"axion.bundle-report.v1\""));
        assert!(json.contains("\"verification\":{\"checked\":true,\"passed\":true"));
        assert!(json.contains("\"platform_metadata\":[\"/tmp/out/demo/demo.desktop\"]"));
        assert!(json.contains("\"report_path\":\"target/axion/reports/demo-bundle.json\""));
        assert!(json.contains("\"bundle_files\":5"));
        assert!(json.contains("\"build_executable\":true"));
        assert!(json.contains("\"result\":\"ok\""));
    }

    #[test]
    fn bundle_report_serializes_readiness_blockers() {
        let report = BundleReport::readiness_failed(
            &crate::cli::BundleArgs {
                manifest_path: PathBuf::from("axion.toml"),
                output_dir: None,
                executable: None,
                bin: None,
                report_path: None,
                build_executable: false,
                json: true,
            },
            &["bundle: icon is not configured".to_owned()],
        );
        let json = report.to_json();

        assert!(json.contains("\"schema\":\"axion.bundle-report.v1\""));
        assert!(json.contains("\"verification\":{\"checked\":false,\"passed\":false"));
        assert!(json.contains("\"blockers\":[\"bundle: icon is not configured\"]"));
        assert!(json.contains("\"result\":\"failed\""));
    }

    #[test]
    fn bundle_report_can_be_written_to_report_path() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let report_path = std::env::temp_dir()
            .join(format!("axion-bundle-report-{unique}"))
            .join("bundle.json");
        let report = BundleReport::readiness_failed(
            &crate::cli::BundleArgs {
                manifest_path: PathBuf::from("axion.toml"),
                output_dir: None,
                executable: None,
                bin: None,
                report_path: Some(report_path.clone()),
                build_executable: false,
                json: true,
            },
            &["bundle: icon is not configured".to_owned()],
        );

        write_report_if_requested(Some(&report_path), &report).expect("report should be written");
        let written = fs::read_to_string(&report_path).expect("report file should exist");

        assert!(written.contains("\"schema\":\"axion.bundle-report.v1\""));
        assert!(written.contains(&format!(
            "\"report_path\":{}",
            axion_runtime::json_string_literal(&report_path.display().to_string())
        )));
    }
    #[test]
    fn cargo_artifacts_select_custom_paths_and_reject_missing_or_ambiguous_binaries() {
        let root = std::env::temp_dir().join(format!(
            "axion-cargo-artifacts-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("shared-target/aarch64/release")).unwrap();
        let current = root.join("shared-target/aarch64/release/different-bin");
        fs::write(&current, "binary").unwrap();
        let message: super::CargoMessage = serde_json::from_str(&serde_json::json!({"reason":"compiler-artifact", "target":{"name":"different-bin","kind":["bin"]}, "executable":current}).to_string()).unwrap();
        assert_eq!(
            super::select_cargo_executable(vec![message.executable.unwrap()]).unwrap(),
            current
        );
        assert!(super::select_cargo_executable(vec![]).is_err());
        assert!(
            super::select_cargo_executable(vec![current.clone(), root.join("other")])
                .unwrap_err()
                .to_string()
                .contains("--bin")
        );
        assert!(super::select_cargo_executable(vec![root.join("old-debug-missing")]).is_err());
    }
}
