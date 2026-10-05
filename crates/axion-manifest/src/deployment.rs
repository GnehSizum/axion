use crate::{ManifestError, load_app_config_from_path};
use axion_core::AppConfig;
use std::path::{Path, PathBuf};

/// Load packaged resources relative to the executable, with a development fallback.
pub fn load_app_config_for_executable(
    development_manifest: &Path,
) -> Result<AppConfig, ManifestError> {
    let executable = std::env::current_exe().map_err(|source| ManifestError::Read {
        path: development_manifest.to_path_buf(),
        source,
    })?;
    load_app_config_from_path(manifest_for_executable(&executable, development_manifest))
}

fn manifest_for_executable(executable: &Path, development_manifest: &Path) -> PathBuf {
    let parent = executable.parent().unwrap_or(Path::new("."));
    let bundle_parent = parent.parent().unwrap_or(parent);
    let macos_bundle = parent.file_name().is_some_and(|name| name == "MacOS");
    let adjacent = parent.join("resources").join("axion.toml");
    let packaged = bundle_parent
        .join(if macos_bundle {
            "Resources"
        } else {
            "resources"
        })
        .join("axion.toml");
    let standard_layout = macos_bundle || parent.file_name().is_some_and(|name| name == "bin");
    let candidates = if standard_layout {
        [packaged, adjacent]
    } else {
        [adjacent, packaged]
    };
    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| development_manifest.to_path_buf())
}

/// Preserve application policy while replacing development-only paths for deployment.
pub fn deployment_manifest_source(manifest_path: &Path) -> Result<String, ManifestError> {
    let config = load_app_config_from_path(manifest_path)?;
    let failure = |message: String| ManifestError::Deployment {
        path: manifest_path.to_path_buf(),
        message,
    };
    let frontend = config
        .build
        .frontend_dist
        .canonicalize()
        .map_err(|error| failure(error.to_string()))?;
    let entry = config
        .build
        .entry
        .canonicalize()
        .map_err(|error| failure(error.to_string()))?;
    let relative = entry
        .strip_prefix(&frontend)
        .map_err(|_| failure("entry is outside frontend_dist".to_owned()))?;
    let relative = relative
        .to_str()
        .ok_or_else(|| failure("entry must be valid UTF-8".to_owned()))?
        .replace('\\', "/");
    let source = std::fs::read_to_string(manifest_path).map_err(|source| ManifestError::Read {
        path: manifest_path.to_path_buf(),
        source,
    })?;
    let mut value: toml::Value =
        toml::from_str(&source).map_err(|source| ManifestError::Parse {
            path: manifest_path.to_path_buf(),
            source,
        })?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| failure("manifest must be a table".to_owned()))?;
    table.remove("dev");
    table.remove("bundle"); // Icons belong to bundle metadata, not runtime resource lookup.
    let build = table
        .get_mut("build")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| failure("build must be a table".to_owned()))?;
    build.insert(
        "frontend_dist".to_owned(),
        toml::Value::String("app".to_owned()),
    );
    build.insert(
        "entry".to_owned(),
        toml::Value::String(format!("app/{relative}")),
    );
    // A development data directory must not become an absolute path in a distributable.
    if let Some(fs) = table
        .get_mut("native")
        .and_then(toml::Value::as_table_mut)
        .and_then(|native| native.get_mut("fs"))
        .and_then(toml::Value::as_table_mut)
    {
        fs.remove("app_data_dir");
    }
    Ok(toml::to_string(&value)?)
}

pub fn manifest_warnings(path: &Path) -> Result<Vec<String>, ManifestError> {
    let source = std::fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut ignored = std::collections::BTreeSet::new();
    let _: crate::ManifestDocument =
        serde_ignored::deserialize(toml::Deserializer::new(&source), |field| {
            ignored.insert(field.to_string());
        })
        .map_err(|source| ManifestError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(ignored
        .into_iter()
        .map(|field| format!("unknown manifest field '{field}'"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "axion-deployment-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(path.join("frontend")).unwrap();
        std::fs::write(path.join("frontend/index.html"), "hello").unwrap();
        std::fs::write(
            path.join("axion.toml"),
            r#"
[app]
name = "deployment-test"
identifier = "com.axion.deployment-test"
[window]
visibile = false
[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
[dev]
url = "http://localhost:1234"
[native.fs]
app_data_dir = "test-data"
[capabilities.main]
profiles = ["minimal"]
"#,
        )
        .unwrap();
        path
    }

    #[test]
    fn deployment_preserves_policy_and_removes_development_paths() {
        let root = fixture();
        let source = deployment_manifest_source(&root.join("axion.toml")).unwrap();
        let value: toml::Value = toml::from_str(&source).unwrap();
        assert_eq!(value["build"]["frontend_dist"].as_str(), Some("app"));
        assert_eq!(value["build"]["entry"].as_str(), Some("app/index.html"));
        assert!(value.get("dev").is_none());
        assert!(value["native"]["fs"].get("app_data_dir").is_none());
        assert_eq!(
            value["capabilities"]["main"]["profiles"][0].as_str(),
            Some("minimal")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unknown_fields_warn_without_rejecting_forward_compatible_loading() {
        let root = fixture();
        assert!(
            crate::load_from_path(root.join("axion.toml"))
                .unwrap()
                .window
                .unwrap()
                .visible
        );
        let warnings = manifest_warnings(&root.join("axion.toml")).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("visibile"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn standard_bundle_resources_take_priority_over_adjacent_shadow_config() {
        let root = fixture();
        for (executable, resources) in [
            (
                root.join("Demo.app/Contents/MacOS/demo"),
                root.join("Demo.app/Contents/Resources"),
            ),
            (root.join("linux/bin/demo"), root.join("linux/resources")),
        ] {
            std::fs::create_dir_all(&resources).unwrap();
            std::fs::create_dir_all(executable.parent().unwrap().join("resources")).unwrap();
            std::fs::write(resources.join("axion.toml"), "canonical").unwrap();
            std::fs::write(
                executable.parent().unwrap().join("resources/axion.toml"),
                "shadow",
            )
            .unwrap();
            assert_eq!(
                manifest_for_executable(&executable, &root.join("missing.toml")),
                resources.join("axion.toml")
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_manifest_is_selected_without_development_source() {
        let root = fixture();
        let source = deployment_manifest_source(&root.join("axion.toml")).unwrap();
        for executable in [
            root.join("Demo.app/Contents/MacOS/demo"),
            root.join("linux/bin/demo"),
        ] {
            let parent = executable.parent().unwrap();
            let resources = if parent.file_name().unwrap() == "MacOS" {
                parent.parent().unwrap().join("Resources")
            } else {
                parent.parent().unwrap().join("resources")
            };
            std::fs::create_dir_all(resources.join("app")).unwrap();
            std::fs::write(resources.join("app/index.html"), "packaged").unwrap();
            std::fs::write(resources.join("axion.toml"), &source).unwrap();
            let manifest = manifest_for_executable(&executable, &root.join("missing.toml"));
            let config = load_app_config_from_path(&manifest).unwrap();
            assert_eq!(config.build.frontend_dist, resources.join("app"));
            assert!(config.build.entry.is_file());
            assert!(config.dev.is_none());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
