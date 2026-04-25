use std::path::{Path, PathBuf};

use axion_core::{Builder, RunMode};
use axion_packager::{BundlePlan, current_bundle_target, stage_bundle_from_web_assets};

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
    let executable_path = args
        .executable
        .or_else(|| default_executable_path(&args.manifest_path, &launch_config.app_name));

    let artifact = stage_bundle_from_web_assets(
        launch_config.frontend_dist,
        launch_config.packaged_entry,
        BundlePlan {
            target,
            output_dir: output_dir.clone(),
            executable_path,
        },
        &launch_config.app_name,
    )?;

    println!("Axion bundle");
    println!("manifest: {}", args.manifest_path.display());
    println!("app: {}", launch_config.app_name);
    println!("target: {:?}", artifact.target);
    println!("output_dir: {}", artifact.output_dir.display());
    println!("bundle_dir: {}", artifact.bundle_dir.display());
    println!(
        "resources_app_dir: {}",
        artifact.resources_app_dir.display()
    );
    println!("entry_path: {}", artifact.entry_path.display());
    println!("asset_manifest: {}", artifact.asset_manifest_path.display());
    println!("metadata: {}", artifact.metadata_path.display());
    match &artifact.executable_path {
        Some(path) => println!("executable: {}", path.display()),
        None => println!("executable: not bundled (pass --executable to include one)"),
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
    let mut candidates = vec![
        manifest_dir
            .join("target")
            .join("release")
            .join(&executable_name),
    ];

    for ancestor in manifest_dir.ancestors() {
        candidates.push(
            ancestor
                .join("target")
                .join("release")
                .join(&executable_name),
        );
    }

    candidates.into_iter().find(|path| path.is_file())
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

    #[test]
    fn default_output_dir_is_workspace_local() {
        let path = default_output_dir(Path::new("/tmp/demo/axion.toml"), "hello-axion");
        assert_eq!(
            path,
            PathBuf::from("/tmp/demo/target/axion/hello-axion/bundle")
        );
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
}
