use std::path::{Path, PathBuf};
use std::process::Command;

use axion_core::{Builder, RunMode};
use axion_packager::{
    BundleMetadata, BundlePlan, current_bundle_target, stage_bundle_from_web_assets_with_metadata,
    verify_bundle_artifact,
};

use crate::cli::BundleArgs;
use crate::error::AxionCliError;

pub fn run(args: BundleArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let target = current_bundle_target();
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| default_output_dir(&args.manifest_path, &launch_config.app_name));
    let executable_path = resolve_executable_path(
        &args.manifest_path,
        &launch_config.app_name,
        args.executable,
        args.build_executable,
    )?;

    let artifact = stage_bundle_from_web_assets_with_metadata(
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
    )?;
    let verification = verify_bundle_artifact(&artifact)?;

    println!("Axion bundle");
    println!("manifest: {}", args.manifest_path.display());
    println!("app: {}", launch_config.app_name);
    if let Some(identifier) = &launch_config.identifier {
        println!("identifier: {identifier}");
    }
    if let Some(version) = &launch_config.version {
        println!("version: {version}");
    }
    println!("target: {}", artifact.target.as_str());
    println!("layout: {}", artifact.target.layout_summary());
    println!("output_dir: {}", artifact.output_dir.display());
    println!("bundle_dir: {}", artifact.bundle_dir.display());
    println!(
        "resources_app_dir: {}",
        artifact.resources_app_dir.display()
    );
    println!("entry_path: {}", artifact.entry_path.display());
    println!("asset_manifest: {}", artifact.asset_manifest_path.display());
    println!("metadata: {}", artifact.metadata_path.display());
    println!(
        "bundle_manifest: {}",
        artifact.bundle_manifest_path.display()
    );
    println!("verification: ok");
    println!("bundle_files: {}", verification.bundle_file_count);
    println!("fingerprinted_files: {}", verification.fingerprinted_files);
    println!("bundle_bytes: {}", verification.total_bytes);
    println!("checked_dirs: {}", verification.checked_dirs);
    println!("checked_files: {}", verification.checked_files);
    println!("checked_paths: {}", verification.checked_paths.len());
    match &artifact.icon_path {
        Some(path) => println!("icon: {}", path.display()),
        None => println!("icon: not configured"),
    }
    match &artifact.executable_path {
        Some(path) => println!("executable: {}", path.display()),
        None => println!(
            "executable: not bundled (pass --executable or --build-executable to include one)"
        ),
    }

    Ok(())
}

fn resolve_executable_path(
    manifest_path: &Path,
    app_name: &str,
    explicit_executable: Option<PathBuf>,
    build_executable: bool,
) -> Result<Option<PathBuf>, AxionCliError> {
    if let Some(executable) = explicit_executable {
        return Ok(Some(executable));
    }

    if build_executable {
        build_release_executable(manifest_path)?;
    }

    Ok(default_executable_path(manifest_path, app_name))
}

fn build_release_executable(manifest_path: &Path) -> Result<(), AxionCliError> {
    let cargo_manifest_path = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("Cargo.toml");
    if !cargo_manifest_path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "cannot build executable because Cargo.toml was not found next to manifest '{}'",
                manifest_path.display()
            ),
        )
        .into());
    }

    println!(
        "building executable: cargo build --release --manifest-path {}",
        cargo_manifest_path.display()
    );
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&cargo_manifest_path)
        .status()?;

    if !status.success() {
        return Err(std::io::Error::other(format!(
            "cargo build --release failed with status {status}"
        ))
        .into());
    }

    Ok(())
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

    use super::{default_executable_path, default_output_dir, executable_file_name};
    use axion_packager::BundleTarget;

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
}
