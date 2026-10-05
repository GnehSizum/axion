use std::fs;
use std::path::{Component, Path, PathBuf};

use axion_core::{NativeConfig, RunMode};

use super::{fs_error, fs_io_error, json_string_literal};

pub(super) fn data_dir_for_identity(
    name: &str,
    identifier: Option<&str>,
    native: &NativeConfig,
    mode: RunMode,
    frontend: &Path,
) -> Result<PathBuf, String> {
    if let Some(path) = &native.app_data_dir {
        if path.as_os_str().is_empty() {
            return Err(fs_error(
                "invalid-path",
                "app data directory must not be empty",
            ));
        }
        return directory_location(path, true);
    }
    if mode == RunMode::Development {
        let root = frontend
            .parent()
            .unwrap_or(frontend)
            .join("target/axion-data");
        return directory_location(&root, false).map(|root| root.join(name_segment(name)));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let root = resolve_base(
        std::env::consts::OS,
        home.as_deref(),
        xdg.as_deref(),
        local.as_deref(),
    )?;
    production_directory(&root, name, identifier)
}

fn production_directory(
    root: &Path,
    name: &str,
    identifier: Option<&str>,
) -> Result<PathBuf, String> {
    let segment = match identifier {
        Some(identifier) => {
            if identifier.is_empty()
                || matches!(identifier, "." | "..")
                || !identifier.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
                })
            {
                return Err(fs_error(
                    "invalid-identity",
                    "app identifier must be a safe directory name",
                ));
            }
            identifier.to_owned()
        }
        None => name_segment(name),
    };
    directory_location(root, false).map(|root| root.join(segment))
}

fn name_segment(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    if value.is_empty() {
        "app".to_owned()
    } else {
        value
    }
}

fn resolve_base(
    platform: &str,
    home: Option<&Path>,
    xdg: Option<&Path>,
    local: Option<&Path>,
) -> Result<PathBuf, String> {
    fn absolute(path: Option<&Path>) -> Option<&Path> {
        path.filter(|path| path.is_absolute())
    }
    let base = match platform {
        "macos" => absolute(home).map(|home| home.join("Library/Application Support")),
        "linux" => absolute(xdg)
            .map(Path::to_path_buf)
            .or_else(|| absolute(home).map(|home| home.join(".local/share"))),
        "windows" => absolute(local).map(Path::to_path_buf),
        _ => None,
    };
    base.ok_or_else(|| {
        fs_error(
            "data-directory-unavailable",
            "no absolute operating-system user data directory is available",
        )
    })
}

// Resolve trusted ancestors (including system aliases), but reject a symlink at the
// configured sandbox base itself. No directory is created by this function.
fn directory_location(path: &Path, reject_base_symlink: bool) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| fs_io_error("resolve app data directory", &error))?
            .join(path)
    };
    if reject_base_symlink {
        reject_symlink(&absolute)?;
    }
    let mut current = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(current) {
            Ok(_) => {
                let root = current
                    .canonicalize()
                    .map_err(|error| fs_io_error("resolve app data directory", &error))?;
                return Ok(missing
                    .into_iter()
                    .rev()
                    .fold(root, |root, segment| root.join(segment)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = current.file_name() else {
                    return Err(fs_error(
                        "invalid-path",
                        "app data directory must have normal path components",
                    ));
                };
                missing.push(name.to_owned());
                current = current.parent().ok_or_else(|| {
                    fs_error(
                        "invalid-path",
                        "app data directory has no existing ancestor",
                    )
                })?;
            }
            Err(error) => return Err(fs_io_error("inspect app data directory", &error)),
        }
    }
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(fs_error(
            "symlink-rejected",
            "app data path must not be a symlink",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(fs_io_error("inspect app data path", &error)),
    }
}

fn check_existing_components(base: &Path, relative: &Path) -> Result<(), String> {
    reject_symlink(base)?;
    let canonical_base = match base.canonicalize() {
        Ok(base) => Some(base),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(fs_io_error("access app data directory", &error)),
    };
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        reject_symlink(&current)?;
        if let Some(base) = &canonical_base {
            match current.canonicalize() {
                Ok(path) if !path.starts_with(base) => {
                    return Err(fs_error(
                        "path-escape",
                        "app data path escapes the app data directory",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(fs_io_error("access app data path", &error)),
            }
        }
    }
    Ok(())
}

pub(super) fn resolve_app_data_path(
    app_data_dir: &Path,
    relative_path: &str,
    create_parent: bool,
) -> Result<PathBuf, String> {
    let relative = validate_app_data_relative_path(relative_path)?;
    let base = directory_location(app_data_dir, true)?;
    let path = base.join(relative);
    check_existing_components(&base, relative)?;
    if create_parent {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| fs_io_error("create app data parent directory", &error))?;
        }
        check_existing_components(&base, relative)?;
    } else {
        let canonical_path = path
            .canonicalize()
            .map_err(|error| fs_io_error("access app data path", &error))?;
        if !canonical_path.starts_with(&base) {
            return Err(fs_error(
                "path-escape",
                "app data path escapes the app data directory",
            ));
        }
    }
    Ok(path)
}

pub(super) fn validate_app_data_relative_path(relative_path: &str) -> Result<&Path, String> {
    let relative = Path::new(relative_path);
    if relative_path.trim().is_empty()
        || relative.is_absolute()
        || relative_path.contains(['\\', '\0'])
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(fs_error(
            "invalid-path",
            "app data path must be a non-empty relative path without parent or root components",
        ));
    }
    Ok(relative)
}

pub(super) fn app_data_path_exists(
    app_data_dir: &Path,
    relative_path: &str,
) -> Result<bool, String> {
    let relative = validate_app_data_relative_path(relative_path)?;
    let base = directory_location(app_data_dir, true)?;
    check_existing_components(&base, relative)?;
    match base.join(relative).canonicalize() {
        Ok(path) if path.starts_with(&base) => Ok(true),
        Ok(_) => Err(fs_error(
            "path-escape",
            "app data path escapes the app data directory",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(fs_io_error("access app data path", &error)),
    }
}

pub(super) fn app_data_dir_entry_json(
    relative_dir: &str,
    entry: std::fs::DirEntry,
) -> Result<String, String> {
    let name = entry.file_name().to_string_lossy().into_owned();
    let metadata = std::fs::symlink_metadata(entry.path())
        .map_err(|error| fs_io_error("inspect app data directory entry", &error))?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        "symlink"
    } else if metadata.is_dir() {
        "directory"
    } else if metadata.is_file() {
        "file"
    } else {
        "other"
    };
    let entry_path = if relative_dir == "." {
        name.clone()
    } else {
        format!("{}/{}", relative_dir.trim_end_matches('/'), name)
    };

    Ok(format!(
        "{{\"name\":{},\"path\":{},\"kind\":{},\"bytes\":{}}}",
        json_string_literal(&name),
        json_string_literal(&entry_path),
        json_string_literal(kind),
        if metadata.is_file() {
            metadata.len().to_string()
        } else {
            "null".to_owned()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    fn fixture(name: &str) -> PathBuf {
        let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "axion-app-data-{name}-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn valid_paths_create_parents_and_missing_paths_do_not_report_exists() {
        let root = fixture("ordinary");
        let base = root.join("not-created/data");
        assert!(!app_data_path_exists(&base, "nested/file.txt").unwrap());
        assert!(!base.exists());
        let path = resolve_app_data_path(&base, "nested/file.txt", true).unwrap();
        assert_eq!(path, base.join("nested/file.txt"));
        assert!(path.parent().unwrap().is_dir());
        fs::write(&path, "text").unwrap();
        assert!(app_data_path_exists(&base, "nested/file.txt").unwrap());
        assert_eq!(
            resolve_app_data_path(&base, "nested/file.txt", false).unwrap(),
            path
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "text");
    }

    #[test]
    fn invalid_paths_have_no_directory_creation_side_effects() {
        let root = fixture("invalid");
        let base = root.join("data");
        for path in [
            "",
            " ",
            "..",
            "../outside/file",
            "/absolute",
            "notes\\file",
            "file\0name",
        ] {
            assert!(
                resolve_app_data_path(&base, path, true)
                    .unwrap_err()
                    .starts_with("fs.invalid-path:")
            );
            assert!(!base.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_at_every_relative_level_are_rejected_before_directory_creation() {
        use std::os::unix::fs::symlink;
        let root = fixture("symlinks");
        let base = root.join("data");
        let outside = root.join("outside");
        fs::create_dir_all(base.join("nested")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("marker"), "keep").unwrap();
        symlink(&outside, base.join("link")).unwrap();
        symlink(&outside, base.join("nested/link")).unwrap();
        symlink(outside.join("marker"), base.join("final-link")).unwrap();
        for path in [
            "link/created/sub/file.txt",
            "nested/link/created/sub/file.txt",
            "final-link",
        ] {
            let error = resolve_app_data_path(&base, path, true).unwrap_err();
            assert!(error.starts_with("fs.symlink-rejected:"), "{path}: {error}");
            assert!(
                app_data_path_exists(&base, path)
                    .unwrap_err()
                    .starts_with("fs.symlink-rejected:")
            );
            assert!(!outside.join("created").exists());
            assert_eq!(fs::read_to_string(outside.join("marker")).unwrap(), "keep");
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        }
        symlink(base.join("nested"), base.join("inside-link")).unwrap();
        assert!(
            resolve_app_data_path(&base, "inside-link/created/file", true)
                .unwrap_err()
                .starts_with("fs.symlink-rejected:")
        );
        assert!(!base.join("nested/created").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_base_and_dangling_components_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = fixture("symlink-base");
        let outside = root.join("outside");
        fs::create_dir_all(&outside).unwrap();
        let base = root.join("base-link");
        symlink(&outside, &base).unwrap();
        assert!(
            resolve_app_data_path(&base, "new/sub/file", true)
                .unwrap_err()
                .starts_with("fs.symlink-rejected:")
        );
        assert!(!outside.join("new").exists());
        assert!(
            app_data_path_exists(&base, "missing")
                .unwrap_err()
                .starts_with("fs.symlink-rejected:")
        );
        let real_base = root.join("real");
        fs::create_dir_all(&real_base).unwrap();
        symlink(root.join("missing-target"), real_base.join("dangling")).unwrap();
        assert!(
            resolve_app_data_path(&real_base, "dangling/new/file", true)
                .unwrap_err()
                .starts_with("fs.symlink-rejected:")
        );
        assert!(!root.join("missing-target").exists());
    }

    #[test]
    fn directory_selection_preserves_development_and_explicit_overrides() {
        let root = fixture("selection");
        let frontend = root.join("frontend");
        fs::create_dir_all(&frontend).unwrap();
        assert_eq!(
            data_dir_for_identity(
                "My App",
                Some("dev.example.app"),
                &NativeConfig::default(),
                RunMode::Development,
                &frontend
            )
            .unwrap(),
            root.join("target/axion-data/My-App")
        );
        let override_path = root.join("isolated/data");
        let native = NativeConfig::default().with_app_data_dir(&override_path);
        for mode in [RunMode::Development, RunMode::Production] {
            assert_eq!(
                data_dir_for_identity("My App", Some("dev.example.app"), &native, mode, &frontend)
                    .unwrap(),
                override_path
            );
        }
        assert!(!override_path.exists());
    }

    #[test]
    fn production_uses_stable_safe_identity_under_injected_root() {
        let root = fixture("production");
        assert_eq!(
            production_directory(&root, "Renamed App", Some("dev.example.app")).unwrap(),
            root.join("dev.example.app")
        );
        assert_eq!(
            production_directory(&root, "Original App", Some("dev.example.app")).unwrap(),
            root.join("dev.example.app")
        );
        assert_eq!(
            production_directory(&root, "My App", None).unwrap(),
            root.join("My-App")
        );
        for identifier in ["", ".", "..", "../outside", "a/b", "a\\b", "a:b"] {
            assert!(
                production_directory(&root, "My App", Some(identifier))
                    .unwrap_err()
                    .starts_with("fs.invalid-identity:")
            );
        }
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }

    #[test]
    fn platform_roots_are_injected_without_mutating_process_environment() {
        let root = fixture("roots");
        let home = root.join("home");
        let xdg = root.join("xdg");
        let local = root.join("local");
        assert_eq!(
            resolve_base("macos", Some(&home), None, None).unwrap(),
            home.join("Library/Application Support")
        );
        assert_eq!(
            resolve_base("linux", Some(&home), Some(&xdg), None).unwrap(),
            xdg
        );
        assert_eq!(
            resolve_base("linux", Some(&home), Some(Path::new("relative")), None).unwrap(),
            home.join(".local/share")
        );
        assert_eq!(
            resolve_base("windows", None, None, Some(&local)).unwrap(),
            local
        );
        for platform in ["macos", "linux", "windows", "unsupported"] {
            assert!(
                resolve_base(platform, None, None, None)
                    .unwrap_err()
                    .starts_with("fs.data-directory-unavailable:")
            );
        }
        assert!(!home.exists());
        assert!(!xdg.exists());
        assert!(!local.exists());
    }
}
