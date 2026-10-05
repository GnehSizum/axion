use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(super) struct Sdk {
    pub root: PathBuf,
    pub version: String,
    pub servo_path: PathBuf,
    pub rust_version: Option<String>,
    pub toolchain_file: PathBuf,
}

#[derive(Debug, Serialize)]
pub(super) struct ApplicationSdk {
    pub sdk: Sdk,
    pub cargo_manifest: PathBuf,
    pub servo_runtime_feature: bool,
    pub application_toolchain_file: Option<PathBuf>,
}

#[derive(Deserialize)]
struct CargoMetadata {
    workspace_root: PathBuf,
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    manifest_path: PathBuf,
    rust_version: Option<String>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Deserialize)]
struct CargoDependency {
    name: String,
    path: Option<PathBuf>,
    rename: Option<String>,
    kind: Option<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    optional: bool,
}

fn metadata(manifest: &Path) -> Result<CargoMetadata, String> {
    let manifest = manifest.canonicalize().map_err(|error| {
        format!(
            "cannot read Cargo manifest '{}': {error}",
            manifest.display()
        )
    })?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(manifest.parent().expect("Cargo manifest has a parent"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .map_err(|error| {
            format!(
                "cannot run Cargo metadata for '{}': {error}",
                manifest.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Cargo metadata failed for '{}': {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "invalid Cargo metadata for '{}': {error}",
            manifest.display()
        )
    })
}

fn sdk_from_metadata(metadata: CargoMetadata) -> Result<Sdk, String> {
    let root = metadata.workspace_root;
    let package = |name: &str| {
        metadata
            .packages
            .iter()
            .find(|package| package.name == name)
            .ok_or_else(|| format!("SDK '{}' has no {name} workspace package", root.display()))
    };
    for name in [
        "axion-core",
        "axion-manifest",
        "axion-runtime",
        "axion-window-winit",
    ] {
        let package = package(name)?;
        let expected = root.join("crates").join(name).join("Cargo.toml");
        if expected.canonicalize().ok().as_ref()
            != package.manifest_path.canonicalize().ok().as_ref()
        {
            return Err(format!(
                "SDK {name} must be located at '{}'",
                expected.display()
            ));
        }
        if package.version != env!("CARGO_PKG_VERSION") {
            return Err(format!(
                "SDK '{}' uses {name} {}, but this CLI requires {}; select a matching SDK or CLI",
                root.display(),
                package.version,
                env!("CARGO_PKG_VERSION")
            ));
        }
    }
    let runtime = package("axion-runtime")?;
    if !runtime.features.contains_key("servo-runtime") {
        return Err(format!(
            "SDK '{}' has no axion-runtime/servo-runtime feature",
            root.display()
        ));
    }
    let window = package("axion-window-winit")?;
    let servo = window
        .dependencies
        .iter()
        .find(|dependency| dependency.name == "servo" && dependency.kind.is_none())
        .and_then(|dependency| dependency.path.as_ref())
        .ok_or_else(|| format!("SDK '{}' has no local Servo dependency", root.display()))?;
    if !servo.join("Cargo.toml").is_file() {
        return Err(format!(
            "SDK Servo manifest is missing at '{}'",
            servo.display()
        ));
    }
    let servo_path = servo
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("invalid SDK Servo dependency path '{}'", servo.display()))?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let toolchain_file = root.join("rust-toolchain.toml");
    if !toolchain_file.is_file() {
        return Err(format!(
            "SDK toolchain file is missing at '{}'",
            toolchain_file.display()
        ));
    }
    Ok(Sdk {
        root,
        version: runtime.version.clone(),
        servo_path,
        rust_version: runtime.rust_version.clone(),
        toolchain_file,
    })
}

pub(super) fn validate_sdk_root(path: &Path) -> Result<Sdk, String> {
    let path = path.canonicalize().map_err(|error| {
        format!(
            "SDK '{}' is unavailable: {error}; pass --sdk-path <Axion checkout>",
            path.display()
        )
    })?;
    let sdk = sdk_from_metadata(metadata(&path.join("Cargo.toml"))?)?;
    if sdk.root != path {
        return Err(format!(
            "SDK root must be the workspace root '{}'; pass --sdk-path with that directory",
            sdk.root.display()
        ));
    }
    let config_name = if cfg!(target_os = "macos") {
        "config.macos.example.toml"
    } else {
        "config.example.toml"
    };
    if !sdk.root.join(".cargo").join(config_name).is_file() {
        return Err(format!(
            "SDK '{}' is missing .cargo/{config_name}",
            sdk.root.display()
        ));
    }
    Ok(sdk)
}

pub(super) fn application_sdk(manifest_path: &Path) -> Result<ApplicationSdk, String> {
    let cargo_manifest = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("Cargo.toml")
        .canonicalize()
        .map_err(|error| {
            format!(
                "Cargo.toml next to '{}' is unavailable: {error}",
                manifest_path.display()
            )
        })?;
    let application = metadata(&cargo_manifest)?;
    let package = application
        .packages
        .iter()
        .find(|package| package.manifest_path == cargo_manifest)
        .ok_or_else(|| "application package was not found in Cargo metadata".to_owned())?;
    let runtime = package
        .dependencies
        .iter()
        .find(|dependency| dependency.name == "axion-runtime" && dependency.kind.is_none())
        .ok_or_else(|| {
            "application has no direct axion-runtime dependency; bind it to a local Axion SDK"
                .to_owned()
        })?;
    let runtime_path = runtime.path.as_ref().ok_or_else(|| {
        "application axion-runtime dependency is not a local source SDK".to_owned()
    })?;
    let sdk_metadata = metadata(&runtime_path.join("Cargo.toml"))?;
    let sdk = sdk_from_metadata(sdk_metadata)?;
    for dependency in package.dependencies.iter().filter(|dependency| {
        dependency.kind.is_none()
            && matches!(
                dependency.name.as_str(),
                "axion-core" | "axion-manifest" | "axion-runtime"
            )
    }) {
        let expected = sdk
            .root
            .join("crates")
            .join(&dependency.name)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let actual = dependency
            .path
            .as_ref()
            .and_then(|path| path.canonicalize().ok());
        if actual.as_ref() != Some(&expected) {
            return Err(format!(
                "application {} is not bound to the same SDK '{}'; update the Axion dependency paths together",
                dependency.name,
                sdk.root.display()
            ));
        }
    }
    let application_toolchain_file = cargo_manifest.parent().unwrap().join("rust-toolchain.toml");
    Ok(ApplicationSdk {
        servo_runtime_feature: runtime_feature_enabled(package, runtime),
        application_toolchain_file: application_toolchain_file
            .is_file()
            .then_some(application_toolchain_file),
        cargo_manifest,
        sdk,
    })
}

fn runtime_feature_enabled(package: &CargoPackage, runtime: &CargoDependency) -> bool {
    if !package.features.contains_key("servo-runtime") {
        return false;
    }
    let name = runtime.rename.as_deref().unwrap_or(&runtime.name);
    let mut pending = vec!["servo-runtime".to_owned(), "default".to_owned()];
    let mut enabled = BTreeSet::new();
    while let Some(feature) = pending.pop() {
        if enabled.insert(feature.clone()) {
            if let Some(features) = package.features.get(&feature) {
                pending.extend(features.iter().cloned());
            }
        }
    }
    let direct = format!("{name}/servo-runtime");
    let conditional = format!("{name}?/servo-runtime");
    let dependency_enabled = !runtime.optional
        || enabled.contains(&format!("dep:{name}"))
        || enabled.contains(name)
        || enabled.contains(&direct);
    dependency_enabled
        && (runtime
            .features
            .iter()
            .any(|feature| feature == "servo-runtime")
            || enabled.contains(&direct)
            || enabled.contains(&conditional))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_feature_follows_aliases_and_local_feature_forwarding() {
        let mut package: CargoPackage = serde_json::from_value(serde_json::json!({
            "name":"app", "version":"0.1.0", "manifest_path":"/tmp/Cargo.toml",
            "features":{"servo-runtime":["desktop"],"desktop":["engine/servo-runtime"]},
            "dependencies":[{"name":"axion-runtime","rename":"engine","path":"/tmp/sdk/crates/axion-runtime"}]
        })).unwrap();
        assert!(runtime_feature_enabled(&package, &package.dependencies[0]));
        package
            .features
            .insert("servo-runtime".to_owned(), Vec::new());
        assert!(!runtime_feature_enabled(&package, &package.dependencies[0]));
    }

    #[test]
    fn conditional_runtime_feature_requires_the_optional_dependency() {
        let mut package: CargoPackage = serde_json::from_value(serde_json::json!({
            "name":"app", "version":"0.1.0", "manifest_path":"/tmp/Cargo.toml",
            "features":{"servo-runtime":["engine?/servo-runtime"]},
            "dependencies":[{"name":"axion-runtime","rename":"engine","optional":true,"path":"/tmp/sdk/crates/axion-runtime"}]
        })).unwrap();
        assert!(!runtime_feature_enabled(&package, &package.dependencies[0]));
        package
            .features
            .get_mut("servo-runtime")
            .unwrap()
            .push("dep:engine".to_owned());
        assert!(runtime_feature_enabled(&package, &package.dependencies[0]));
    }
}

#[cfg(test)]
pub(super) fn fixture_sdk(path: &Path, version: &str) {
    std::fs::create_dir_all(path.join(".cargo")).unwrap();
    std::fs::write(path.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/axion-core\", \"crates/axion-manifest\", \"crates/axion-runtime\", \"crates/axion-window-winit\"]\nexclude = [\"servo\"]\nresolver = \"3\"\n").unwrap();
    std::fs::write(
        path.join("rust-toolchain.toml"),
        include_str!("../../../../rust-toolchain.toml"),
    )
    .unwrap();
    for name in ["config.example.toml", "config.macos.example.toml"] {
        std::fs::write(
            path.join(".cargo").join(name),
            "# SDK fixture configuration\n",
        )
        .unwrap();
    }
    for name in [
        "axion-core",
        "axion-manifest",
        "axion-runtime",
        "axion-window-winit",
    ] {
        let crate_path = path.join("crates").join(name);
        std::fs::create_dir_all(crate_path.join("src")).unwrap();
        let extra = match name {
            "axion-runtime" => {
                "\n[features]\nservo-runtime = [\"axion-window-winit/servo-runtime\"]\n[dependencies]\naxion-window-winit = { path = \"../axion-window-winit\" }\n"
            }
            "axion-window-winit" => {
                "\n[features]\nservo-runtime = [\"dep:servo\"]\n[dependencies]\nservo = { path = \"../../servo/components/servo\", optional = true }\n"
            }
            _ => "",
        };
        std::fs::write(crate_path.join("Cargo.toml"), format!("[package]\nname = {name:?}\nversion = {version:?}\nedition = \"2024\"\nrust-version = \"1.88.0\"\n{extra}")).unwrap();
        std::fs::write(crate_path.join("src/lib.rs"), "").unwrap();
    }
    let servo = path.join("servo/components/servo");
    std::fs::create_dir_all(servo.join("src")).unwrap();
    std::fs::write(
        servo.join("Cargo.toml"),
        "[package]\nname = \"servo\"\nversion = \"0.6.0\"\nedition = \"2024\"\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(servo.join("src/lib.rs"), "").unwrap();
}

#[cfg(test)]
pub(super) fn fixture_application(path: &Path, sdk_root: &Path) {
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::write(path.join("src/main.rs"), "fn main() {}\n").unwrap();
    let mut source = String::from(
        "[package]\nname = \"sdk-fixture-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nservo-runtime = [\"axion-runtime/servo-runtime\"]\n\n[dependencies]\n",
    );
    for name in ["axion-core", "axion-manifest", "axion-runtime"] {
        let dependency_path = sdk_root.join("crates").join(name);
        source.push_str(&format!("{name} = {{ path = {dependency_path:?} }}\n"));
    }
    std::fs::write(path.join("Cargo.toml"), source).unwrap();
}
