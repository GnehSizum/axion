use std::path::{Path, PathBuf};

use axion_core::{Builder, RunMode};

use crate::cli::BuildArgs;
use crate::error::AxionCliError;

pub fn run(args: BuildArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let launch_config = app.runtime_launch_config(RunMode::Production);
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| default_output_dir(&args.manifest_path, &launch_config.app_name));

    let artifact = axion_packager::stage_web_assets(
        launch_config.frontend_dist,
        launch_config.packaged_entry,
        &output_dir,
    )?;

    println!("Axion build artifact");
    println!("manifest: {}", args.manifest_path.display());
    println!("app: {}", launch_config.app_name);
    println!("output_dir: {}", artifact.output_dir.display());
    println!("app_dir: {}", artifact.app_dir.display());
    println!("entry_path: {}", artifact.entry_path.display());
    println!("asset_manifest: {}", artifact.asset_manifest_path.display());

    Ok(())
}

fn default_output_dir(manifest_path: &Path, app_name: &str) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("target")
        .join("axion")
        .join(app_name)
        .join("build")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::default_output_dir;

    #[test]
    fn default_output_dir_is_workspace_local() {
        let path = default_output_dir(Path::new("/tmp/demo/axion.toml"), "hello-axion");
        assert_eq!(
            path,
            PathBuf::from("/tmp/demo/target/axion/hello-axion/build")
        );
    }
}
