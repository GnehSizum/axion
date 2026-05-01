use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use axion_core::{
    AppConfig, AppIdentity, BuildConfig, BundleConfig, CapabilityConfig, CapabilityProfileConfig,
    ClipboardBackendConfig, ClipboardConfig, DevServerConfig, DialogBackendConfig, DialogConfig,
    LifecycleConfig, NativeConfig, WindowConfig, WindowId,
};
use url::Url;

use crate::model::{ManifestDocument, WindowSection};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read manifest at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse TOML manifest at {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("manifest at {path} must define a non-empty app.name")]
    MissingAppName { path: PathBuf },
    #[error("manifest at {path} defines an invalid app.name '{value}'")]
    InvalidAppName { path: PathBuf, value: String },
    #[error("manifest at {path} has no parent directory")]
    MissingManifestDirectory { path: PathBuf },
    #[error("manifest at {path} contains an invalid dev.url '{value}'")]
    InvalidDevUrl {
        path: PathBuf,
        value: String,
        #[source]
        source: url::ParseError,
    },
    #[error("manifest at {path} defines duplicate window id '{window_id}'")]
    DuplicateWindowId { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines an empty window id")]
    InvalidWindowId { path: PathBuf },
    #[error("manifest at {path} defines an empty title for window id '{window_id}'")]
    InvalidWindowTitle { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines an invalid size for window id '{window_id}'")]
    InvalidWindowSize { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines an empty command for window id '{window_id}'")]
    InvalidCapabilityCommand { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines an empty capability profile for window id '{window_id}'")]
    InvalidCapabilityProfile { path: PathBuf, window_id: String },
    #[error(
        "manifest at {path} defines unknown capability profile '{value}' for window id '{window_id}'"
    )]
    UnknownCapabilityProfile {
        path: PathBuf,
        window_id: String,
        value: String,
    },
    #[error("manifest at {path} defines invalid command '{value}' for window id '{window_id}'")]
    InvalidCapabilityCommandName {
        path: PathBuf,
        window_id: String,
        value: String,
    },
    #[error("manifest at {path} defines an empty event for window id '{window_id}'")]
    InvalidCapabilityEvent { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines invalid event '{value}' for window id '{window_id}'")]
    InvalidCapabilityEventName {
        path: PathBuf,
        window_id: String,
        value: String,
    },
    #[error("manifest at {path} defines an empty protocol for window id '{window_id}'")]
    InvalidCapabilityProtocol { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines invalid protocol '{value}' for window id '{window_id}'")]
    InvalidCapabilityProtocolName {
        path: PathBuf,
        window_id: String,
        value: String,
    },
    #[error("manifest at {path} defines an empty navigation origin for window id '{window_id}'")]
    InvalidNavigationOrigin { path: PathBuf, window_id: String },
    #[error(
        "manifest at {path} defines invalid navigation origin '{value}' for window id '{window_id}'"
    )]
    InvalidNavigationOriginValue {
        path: PathBuf,
        window_id: String,
        value: String,
    },
    #[error(
        "manifest at {path} defines bridge commands or events for window id '{window_id}' but does not allow the axion protocol"
    )]
    BridgeRequiresAxionProtocol { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines capabilities for unknown window id '{window_id}'")]
    UnknownCapabilityWindow { path: PathBuf, window_id: String },
    #[error("manifest at {path} defines an invalid bundle.icon path '{value}'")]
    InvalidBundleIconPath { path: PathBuf, value: PathBuf },
    #[error("manifest at {path} defines invalid native.dialog.backend '{value}'")]
    InvalidNativeDialogBackend { path: PathBuf, value: String },
    #[error("manifest at {path} defines invalid native.clipboard.backend '{value}'")]
    InvalidNativeClipboardBackend { path: PathBuf, value: String },
    #[error("manifest at {path} defines invalid native.lifecycle.close_timeout_ms '{value}'")]
    InvalidNativeLifecycleCloseTimeout { path: PathBuf, value: u64 },
}

pub fn load_from_path(path: impl AsRef<Path>) -> Result<ManifestDocument, ManifestError> {
    let path = path.as_ref().to_path_buf();
    let source = fs::read_to_string(&path).map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;

    toml::from_str(&source).map_err(|source| ManifestError::Parse { path, source })
}

pub fn load_app_config_from_path(path: impl AsRef<Path>) -> Result<AppConfig, ManifestError> {
    let path = path.as_ref().to_path_buf();
    let manifest = load_from_path(&path)?;
    let manifest_dir = path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ManifestError::MissingManifestDirectory { path: path.clone() })?;

    let app_name = manifest.app.name.trim();
    if app_name.is_empty() {
        return Err(ManifestError::MissingAppName { path });
    }
    reject_invalid_app_name(&path, app_name)?;

    let mut identity = AppIdentity::new(app_name);
    if let Some(identifier) = clean_optional_string(manifest.app.identifier) {
        identity = identity.with_identifier(identifier);
    }
    if let Some(version) = clean_optional_string(manifest.app.version) {
        identity = identity.with_version(version);
    }
    if let Some(description) = clean_optional_string(manifest.app.description) {
        identity = identity.with_description(description);
    }
    let authors = clean_string_list(manifest.app.authors);
    if !authors.is_empty() {
        identity = identity.with_authors(authors);
    }
    if let Some(homepage) = clean_optional_string(manifest.app.homepage) {
        identity = identity.with_homepage(homepage);
    }

    let dev = manifest
        .dev
        .map(|dev| {
            Url::parse(&dev.url)
                .map(|url| DevServerConfig {
                    url,
                    command: clean_optional_string(dev.command),
                    cwd: dev.cwd.map(|cwd| resolve_path(&manifest_dir, cwd)),
                    timeout_ms: dev.timeout_ms,
                })
                .map_err(|source| ManifestError::InvalidDevUrl {
                    path: path.clone(),
                    value: dev.url,
                    source,
                })
        })
        .transpose()?;

    let build = BuildConfig::new(
        resolve_path(&manifest_dir, manifest.build.frontend_dist),
        resolve_path(&manifest_dir, manifest.build.entry),
    );
    let bundle = bundle_config_from_manifest(&path, &manifest_dir, manifest.bundle)?;
    let native = native_config_from_manifest(&path, manifest.native)?;

    let windows = manifest_windows(manifest.window, manifest.windows)
        .into_iter()
        .enumerate()
        .map(|(index, window)| window_config_from_manifest(window, index, &identity.name))
        .collect::<Vec<_>>();
    reject_invalid_windows(&path, &windows)?;
    reject_duplicate_window_ids(&path, &windows)?;

    let capabilities = manifest
        .capabilities
        .into_iter()
        .map(|(window_id, capability)| {
            capability_config_from_manifest(&path, &window_id, capability)
                .map(|capability| (window_id, capability))
        })
        .collect::<Result<_, _>>()?;
    reject_unknown_capability_windows(&path, &windows, &capabilities)?;

    Ok(AppConfig {
        identity,
        windows,
        dev,
        build,
        bundle,
        native,
        capabilities,
    })
}

fn native_config_from_manifest(
    path: &Path,
    native: Option<crate::model::NativeSection>,
) -> Result<NativeConfig, ManifestError> {
    let Some(native) = native else {
        return Ok(NativeConfig::new());
    };
    let mut config = NativeConfig::new();

    if let Some(dialog) = native.dialog {
        if let Some(backend) = clean_optional_string(dialog.backend) {
            let backend = match backend.as_str() {
                "headless" => DialogBackendConfig::Headless,
                "system" => DialogBackendConfig::System,
                _ => {
                    return Err(ManifestError::InvalidNativeDialogBackend {
                        path: path.to_path_buf(),
                        value: backend,
                    });
                }
            };
            config = config.with_dialog(DialogConfig { backend });
        }
    }

    if let Some(clipboard) = native.clipboard {
        if let Some(backend) = clean_optional_string(clipboard.backend) {
            let backend = match backend.as_str() {
                "memory" => ClipboardBackendConfig::Memory,
                "system" => ClipboardBackendConfig::System,
                _ => {
                    return Err(ManifestError::InvalidNativeClipboardBackend {
                        path: path.to_path_buf(),
                        value: backend,
                    });
                }
            };
            config = config.with_clipboard(ClipboardConfig { backend });
        }
    }

    if let Some(lifecycle) = native.lifecycle {
        if let Some(close_timeout_ms) = lifecycle.close_timeout_ms {
            if close_timeout_ms == 0 {
                return Err(ManifestError::InvalidNativeLifecycleCloseTimeout {
                    path: path.to_path_buf(),
                    value: close_timeout_ms,
                });
            }
            config = config.with_lifecycle(LifecycleConfig { close_timeout_ms });
        }
    }

    Ok(config)
}

fn bundle_config_from_manifest(
    path: &Path,
    manifest_dir: &Path,
    bundle: Option<crate::model::BundleSection>,
) -> Result<BundleConfig, ManifestError> {
    let Some(bundle) = bundle else {
        return Ok(BundleConfig::new());
    };
    let Some(icon) = bundle.icon else {
        return Ok(BundleConfig::new());
    };

    reject_invalid_project_relative_path(path, &icon)?;
    Ok(BundleConfig::new().with_icon(resolve_path(manifest_dir, icon)))
}

fn reject_invalid_project_relative_path(path: &Path, value: &Path) -> Result<(), ManifestError> {
    if value.as_os_str().is_empty()
        || value.is_absolute()
        || value.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(ManifestError::InvalidBundleIconPath {
            path: path.to_path_buf(),
            value: value.to_path_buf(),
        });
    }

    Ok(())
}

fn reject_invalid_app_name(path: &Path, app_name: &str) -> Result<(), ManifestError> {
    if app_name == "."
        || app_name == ".."
        || app_name.contains('/')
        || app_name.contains('\\')
        || app_name.contains('\0')
    {
        return Err(ManifestError::InvalidAppName {
            path: path.to_path_buf(),
            value: app_name.to_owned(),
        });
    }

    Ok(())
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn clean_string_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_owned();
        if !value.is_empty() && !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    normalized
}

fn reject_invalid_windows(path: &Path, windows: &[WindowConfig]) -> Result<(), ManifestError> {
    for window in windows {
        let window_id = window.id.as_str();
        if window_id.trim().is_empty() {
            return Err(ManifestError::InvalidWindowId {
                path: path.to_path_buf(),
            });
        }
        if window.title.trim().is_empty() {
            return Err(ManifestError::InvalidWindowTitle {
                path: path.to_path_buf(),
                window_id: window_id.to_owned(),
            });
        }
        if window.width == 0 || window.height == 0 {
            return Err(ManifestError::InvalidWindowSize {
                path: path.to_path_buf(),
                window_id: window_id.to_owned(),
            });
        }
    }

    Ok(())
}

fn capability_config_from_manifest(
    path: &Path,
    window_id: &str,
    capability: crate::model::CapabilitySection,
) -> Result<CapabilityConfig, ManifestError> {
    let profiles = normalize_capability_values(
        path,
        window_id,
        capability.profiles,
        is_known_capability_profile,
        |path, window_id| ManifestError::InvalidCapabilityProfile { path, window_id },
        |path, window_id, value| ManifestError::UnknownCapabilityProfile {
            path,
            window_id,
            value,
        },
    )?;
    let profile_expansions = capability_profile_expansions(&profiles);
    let profile_capability = merge_profile_expansions(&profile_expansions);
    let explicit_commands = normalize_capability_values(
        path,
        window_id,
        capability.commands,
        is_valid_command_name,
        |path, window_id| ManifestError::InvalidCapabilityCommand { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityCommandName {
            path,
            window_id,
            value,
        },
    )?;
    let commands = normalize_capability_values(
        path,
        window_id,
        merge_capability_values(profile_capability.commands, explicit_commands.clone()),
        is_valid_command_name,
        |path, window_id| ManifestError::InvalidCapabilityCommand { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityCommandName {
            path,
            window_id,
            value,
        },
    )?;
    let explicit_events = normalize_capability_values(
        path,
        window_id,
        capability.events,
        is_valid_event_name,
        |path, window_id| ManifestError::InvalidCapabilityEvent { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityEventName {
            path,
            window_id,
            value,
        },
    )?;
    let events = normalize_capability_values(
        path,
        window_id,
        merge_capability_values(profile_capability.events, explicit_events.clone()),
        is_valid_event_name,
        |path, window_id| ManifestError::InvalidCapabilityEvent { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityEventName {
            path,
            window_id,
            value,
        },
    )?;
    let explicit_protocols = normalize_capability_values(
        path,
        window_id,
        capability.protocols,
        is_valid_protocol_name,
        |path, window_id| ManifestError::InvalidCapabilityProtocol { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityProtocolName {
            path,
            window_id,
            value,
        },
    )?;
    let protocols = normalize_capability_values(
        path,
        window_id,
        merge_capability_values(profile_capability.protocols, explicit_protocols.clone()),
        is_valid_protocol_name,
        |path, window_id| ManifestError::InvalidCapabilityProtocol { path, window_id },
        |path, window_id, value| ManifestError::InvalidCapabilityProtocolName {
            path,
            window_id,
            value,
        },
    )?;
    let allowed_navigation_origins =
        normalize_navigation_origins(path, window_id, capability.allowed_navigation_origins)?;

    if (!commands.is_empty() || !events.is_empty())
        && !protocols.iter().any(|protocol| protocol == "axion")
    {
        return Err(ManifestError::BridgeRequiresAxionProtocol {
            path: path.to_path_buf(),
            window_id: window_id.to_owned(),
        });
    }

    Ok(CapabilityConfig {
        profiles,
        profile_expansions,
        explicit_commands,
        explicit_events,
        explicit_protocols,
        commands,
        events,
        protocols,
        allowed_navigation_origins,
        allow_remote_navigation: capability.allow_remote_navigation,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CapabilityProfileExpansion {
    commands: Vec<String>,
    events: Vec<String>,
    protocols: Vec<String>,
}

fn merge_profile_expansions(profiles: &[CapabilityProfileConfig]) -> CapabilityProfileExpansion {
    let mut expansion = CapabilityProfileExpansion::default();
    for profile in profiles {
        expansion.commands.extend(profile.commands.iter().cloned());
        expansion.events.extend(profile.events.iter().cloned());
        expansion
            .protocols
            .extend(profile.protocols.iter().cloned());
    }

    expansion
}

fn capability_profile_expansions(profiles: &[String]) -> Vec<CapabilityProfileConfig> {
    profiles
        .iter()
        .map(|profile| {
            let mut commands = profile_commands(profile)
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>();
            commands.sort();
            commands.dedup();
            let mut events = profile_events(profile)
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>();
            events.sort();
            events.dedup();
            let mut protocols = profile_protocols(profile)
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>();
            protocols.sort();
            protocols.dedup();

            CapabilityProfileConfig {
                profile: profile.clone(),
                commands,
                events,
                protocols,
            }
        })
        .collect()
}

fn profile_commands(profile: &str) -> &'static [&'static str] {
    match profile {
        "app-info" => &["app.ping", "app.info", "app.version", "app.echo"],
        "app-control" => &["app.exit"],
        "window-control" => &[
            "window.info",
            "window.reload",
            "window.focus",
            "window.set_title",
            "window.set_size",
            "window.show",
            "window.hide",
            "window.close",
            "window.confirm_close",
            "window.prevent_close",
        ],
        "multi-window" => &[
            "window.list",
            "window.info",
            "window.reload",
            "window.focus",
            "window.set_title",
            "window.close",
            "window.confirm_close",
            "window.prevent_close",
        ],
        "clipboard-access" => &["clipboard.read_text", "clipboard.write_text"],
        "file-access" => &[
            "fs.create_dir",
            "fs.exists",
            "fs.list_dir",
            "fs.read_text",
            "fs.remove",
            "fs.write_text",
        ],
        "dialog-access" => &["dialog.open", "dialog.save"],
        _ => &[],
    }
}

fn profile_events(profile: &str) -> &'static [&'static str] {
    match profile {
        "app-events" => &["app.log"],
        _ => &[],
    }
}

fn profile_protocols(profile: &str) -> &'static [&'static str] {
    match profile {
        "minimal" | "app-info" | "app-control" | "app-events" | "window-control"
        | "multi-window" | "clipboard-access" | "file-access" | "dialog-access" => &["axion"],
        _ => &[],
    }
}

fn is_known_capability_profile(value: &str) -> bool {
    matches!(
        value,
        "minimal"
            | "app-info"
            | "app-control"
            | "app-events"
            | "window-control"
            | "multi-window"
            | "clipboard-access"
            | "file-access"
            | "dialog-access"
    )
}

fn merge_capability_values(mut first: Vec<String>, second: Vec<String>) -> Vec<String> {
    first.extend(second);
    first
}

fn normalize_navigation_origins(
    path: &Path,
    window_id: &str,
    values: Vec<String>,
) -> Result<Vec<String>, ManifestError> {
    let mut normalized = Vec::new();

    for value in values {
        let value = value.trim();
        if value.is_empty() {
            return Err(ManifestError::InvalidNavigationOrigin {
                path: path.to_path_buf(),
                window_id: window_id.to_owned(),
            });
        }

        let Ok(url) = Url::parse(value) else {
            return Err(ManifestError::InvalidNavigationOriginValue {
                path: path.to_path_buf(),
                window_id: window_id.to_owned(),
                value: value.to_owned(),
            });
        };

        if !url_has_origin_only(&url) {
            return Err(ManifestError::InvalidNavigationOriginValue {
                path: path.to_path_buf(),
                window_id: window_id.to_owned(),
                value: value.to_owned(),
            });
        }

        normalized.push(origin_string(&url));
    }

    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn url_has_origin_only(url: &Url) -> bool {
    url.host_str().is_some()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && !url.cannot_be_a_base()
}

fn origin_string(url: &Url) -> String {
    format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default())
        + &url
            .port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
}

fn normalize_capability_values(
    path: &Path,
    window_id: &str,
    values: Vec<String>,
    is_valid: impl Fn(&str) -> bool,
    empty_error: impl Fn(PathBuf, String) -> ManifestError,
    invalid_error: impl Fn(PathBuf, String, String) -> ManifestError,
) -> Result<Vec<String>, ManifestError> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            return Err(empty_error(path.to_path_buf(), window_id.to_owned()));
        }
        if !is_valid(value) {
            return Err(invalid_error(
                path.to_path_buf(),
                window_id.to_owned(),
                value.to_owned(),
            ));
        }
        normalized.push(value.to_owned());
    }

    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn is_valid_command_name(value: &str) -> bool {
    value.split('.').all(|segment| {
        !segment.is_empty()
            && segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
    })
}

fn is_valid_event_name(value: &str) -> bool {
    is_valid_command_name(value)
}

fn is_valid_protocol_name(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    first.is_ascii_lowercase()
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn manifest_windows(
    legacy_window: Option<WindowSection>,
    windows: Vec<WindowSection>,
) -> Vec<WindowSection> {
    let mut resolved = Vec::new();

    if let Some(window) = legacy_window {
        resolved.push(window);
    }

    resolved.extend(windows);

    if resolved.is_empty() {
        resolved.push(WindowSection::default());
    }

    resolved
}

fn window_config_from_manifest(
    window: WindowSection,
    index: usize,
    app_name: &str,
) -> WindowConfig {
    let id = window.id.map(WindowId::new).unwrap_or_else(|| {
        if index == 0 {
            WindowId::main()
        } else {
            WindowId::new(format!("window-{}", index + 1))
        }
    });

    let title = window.title.unwrap_or_else(|| {
        if index == 0 {
            app_name.to_owned()
        } else {
            format!("{app_name} {}", index + 1)
        }
    });

    WindowConfig {
        id,
        title,
        width: window.width.unwrap_or(960),
        height: window.height.unwrap_or(720),
        resizable: window.resizable,
        visible: window.visible,
    }
}

fn reject_duplicate_window_ids(path: &Path, windows: &[WindowConfig]) -> Result<(), ManifestError> {
    let mut seen = BTreeSet::new();

    for window in windows {
        let window_id = window.id.as_str().to_owned();
        if !seen.insert(window_id.clone()) {
            return Err(ManifestError::DuplicateWindowId {
                path: path.to_path_buf(),
                window_id,
            });
        }
    }

    Ok(())
}

fn reject_unknown_capability_windows(
    path: &Path,
    windows: &[WindowConfig],
    capabilities: &std::collections::BTreeMap<String, CapabilityConfig>,
) -> Result<(), ManifestError> {
    let window_ids = windows
        .iter()
        .map(|window| window.id.as_str())
        .collect::<BTreeSet<_>>();

    for window_id in capabilities.keys() {
        if !window_ids.contains(window_id.as_str()) {
            return Err(ManifestError::UnknownCapabilityWindow {
                path: path.to_path_buf(),
                window_id: window_id.clone(),
            });
        }
    }

    Ok(())
}

fn resolve_path(base: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{ClipboardBackendConfig, DialogBackendConfig};

    use super::{ManifestError, load_app_config_from_path};

    static TEST_MANIFEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn write_manifest(source: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("axion-manifest-test-{unique}-{serial}"));
        fs::create_dir_all(base.join("frontend")).expect("test manifest directory must be created");
        let path = base.join("axion.toml");
        fs::write(&path, source).expect("test manifest must be written");
        path
    }

    #[test]
    fn manifest_loader_resolves_relative_paths_and_capabilities() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"
identifier = "dev.axion.hello"
version = "1.2.3"
description = " Hello from Axion "
authors = [" Alice ", "Bob", "Alice", ""]
homepage = " https://example.dev "

[window]
id = "main"
title = "Hello"
width = 800
height = 600

[dev]
url = "http://127.0.0.1:3000"
command = "python3 -m http.server 3000"
cwd = "frontend"
timeout_ms = 2500

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[bundle]
icon = "icons/app.icns"

[native.dialog]
backend = "headless"

[native.clipboard]
backend = "memory"

[capabilities.main]
commands = ["app.ping"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = [" https://docs.example ", "https://docs.example"]
allow_remote_navigation = false
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(config.identity.name, "hello");
        assert_eq!(
            config.identity.identifier.as_deref(),
            Some("dev.axion.hello")
        );
        assert_eq!(config.identity.version.as_deref(), Some("1.2.3"));
        assert_eq!(
            config.identity.description.as_deref(),
            Some("Hello from Axion")
        );
        assert_eq!(config.identity.authors, vec!["Alice", "Bob"]);
        assert_eq!(
            config.identity.homepage.as_deref(),
            Some("https://example.dev")
        );
        assert_eq!(config.windows[0].title, "Hello");
        let dev = config.dev.as_ref().expect("dev config should load");
        assert_eq!(dev.command.as_deref(), Some("python3 -m http.server 3000"));
        assert_eq!(
            dev.cwd.as_deref(),
            Some(manifest_path.parent().unwrap().join("frontend").as_path())
        );
        assert_eq!(dev.timeout_ms, Some(2500));
        assert!(config.build.frontend_dist.is_absolute());
        assert!(config.build.entry.is_absolute());
        assert_eq!(
            config.bundle.icon,
            Some(manifest_path.parent().unwrap().join("icons/app.icns"))
        );
        assert_eq!(config.native.dialog.backend, DialogBackendConfig::Headless);
        assert_eq!(
            config.native.clipboard.backend,
            ClipboardBackendConfig::Memory
        );
        assert_eq!(config.native.lifecycle.close_timeout_ms, 3000);
        assert_eq!(config.capabilities["main"].commands, vec!["app.ping"]);
        assert_eq!(config.capabilities["main"].events, vec!["app.log"]);
        assert_eq!(
            config.capabilities["main"].allowed_navigation_origins,
            vec!["https://docs.example"]
        );
    }

    #[test]
    fn manifest_loader_accepts_system_dialog_backend() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.dialog]
backend = "system"
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(config.native.dialog.backend, DialogBackendConfig::System);
    }

    #[test]
    fn manifest_loader_accepts_system_clipboard_backend() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.clipboard]
backend = "system"
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(
            config.native.clipboard.backend,
            ClipboardBackendConfig::System
        );
    }

    #[test]
    fn manifest_loader_accepts_lifecycle_close_timeout() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.lifecycle]
close_timeout_ms = 1500
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(config.native.lifecycle.close_timeout_ms, 1500);
    }

    #[test]
    fn manifest_loader_rejects_invalid_lifecycle_close_timeout() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.lifecycle]
close_timeout_ms = 0
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");

        assert!(matches!(
            error,
            ManifestError::InvalidNativeLifecycleCloseTimeout { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_dialog_backend() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.dialog]
backend = "portal"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");

        assert!(matches!(
            error,
            ManifestError::InvalidNativeDialogBackend { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_clipboard_backend() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[native.clipboard]
backend = "portal"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");

        assert!(matches!(
            error,
            ManifestError::InvalidNativeClipboardBackend { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_bundle_icon_paths() {
        for icon in ["", "/tmp/app.icns", "../app.icns"] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[bundle]
icon = {icon:?}
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(error, ManifestError::InvalidBundleIconPath { .. }));
        }
    }

    #[test]
    fn manifest_loader_accepts_windows_array() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[[windows]]
id = "main"
title = "Main"
width = 800
height = 600

[[windows]]
id = "settings"
title = "Settings"
width = 480
height = 360
visible = false

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = ["app.ping"]
protocols = ["axion"]

[capabilities.settings]
commands = ["window.info"]
events = ["settings.changed"]
protocols = ["axion"]
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(config.windows.len(), 2);
        assert_eq!(config.windows[0].id.as_str(), "main");
        assert_eq!(config.windows[0].title, "Main");
        assert_eq!(config.windows[1].id.as_str(), "settings");
        assert_eq!(config.windows[1].title, "Settings");
        assert!(!config.windows[1].visible);
        assert_eq!(
            config.capabilities["settings"].commands,
            vec!["window.info"]
        );
        assert_eq!(
            config.capabilities["settings"].events,
            vec!["settings.changed"]
        );
    }

    #[test]
    fn manifest_loader_defaults_window_when_absent() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(config.windows.len(), 1);
        assert_eq!(config.windows[0].id.as_str(), "main");
        assert_eq!(config.windows[0].title, "hello");
    }

    #[test]
    fn manifest_loader_rejects_path_like_app_names() {
        for name in ["../evil", "nested/app", r"nested\app", ".", ".."] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = {name:?}

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(error, ManifestError::InvalidAppName { .. }));
        }
    }

    #[test]
    fn manifest_loader_rejects_duplicate_window_ids() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[[windows]]
id = "main"
title = "Duplicate"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(error, ManifestError::DuplicateWindowId { .. }));
    }

    #[test]
    fn manifest_loader_rejects_empty_window_id() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = ""

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(error, ManifestError::InvalidWindowId { .. }));
    }

    #[test]
    fn manifest_loader_rejects_empty_window_title() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"
title = ""

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(error, ManifestError::InvalidWindowTitle { .. }));
    }

    #[test]
    fn manifest_loader_rejects_zero_window_size() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"
width = 0
height = 600

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(error, ManifestError::InvalidWindowSize { .. }));
    }

    #[test]
    fn manifest_loader_rejects_capabilities_for_unknown_window() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.missing]
commands = ["app.ping"]
protocols = ["axion"]
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::UnknownCapabilityWindow { .. }
        ));
    }

    #[test]
    fn manifest_loader_trims_and_deduplicates_capability_values() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = [" app.ping ", "app.ping", " window.info "]
events = [" app.log ", "app.log", " window.resized "]
protocols = [" axion ", "axion"]
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");

        assert_eq!(
            config.capabilities["main"].commands,
            vec!["app.ping", "window.info"]
        );
        assert_eq!(
            config.capabilities["main"].explicit_commands,
            vec!["app.ping", "window.info"]
        );
        assert_eq!(
            config.capabilities["main"].events,
            vec!["app.log", "window.resized"]
        );
        assert_eq!(
            config.capabilities["main"].explicit_events,
            vec!["app.log", "window.resized"]
        );
        assert_eq!(config.capabilities["main"].protocols, vec!["axion"]);
        assert_eq!(
            config.capabilities["main"].explicit_protocols,
            vec!["axion"]
        );
    }

    #[test]
    fn manifest_loader_expands_capability_profiles() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
profiles = [" app-info ", "window-control", "clipboard-access", "file-access", "dialog-access", "app-events"]
commands = ["demo.greet", "app.ping"]
events = ["demo.ready"]
allowed_navigation_origins = ["https://docs.example"]
"#,
        );

        let config = load_app_config_from_path(&manifest_path).expect("manifest should load");
        let capability = &config.capabilities["main"];

        assert_eq!(
            capability.profiles,
            vec![
                "app-events",
                "app-info",
                "clipboard-access",
                "dialog-access",
                "file-access",
                "window-control"
            ]
        );
        assert_eq!(
            capability.commands,
            vec![
                "app.echo",
                "app.info",
                "app.ping",
                "app.version",
                "clipboard.read_text",
                "clipboard.write_text",
                "demo.greet",
                "dialog.open",
                "dialog.save",
                "fs.create_dir",
                "fs.exists",
                "fs.list_dir",
                "fs.read_text",
                "fs.remove",
                "fs.write_text",
                "window.close",
                "window.confirm_close",
                "window.focus",
                "window.hide",
                "window.info",
                "window.prevent_close",
                "window.reload",
                "window.set_size",
                "window.set_title",
                "window.show",
            ]
        );
        assert_eq!(capability.events, vec!["app.log", "demo.ready"]);
        assert_eq!(capability.protocols, vec!["axion"]);
        assert_eq!(capability.explicit_commands, vec!["app.ping", "demo.greet"]);
        assert_eq!(capability.explicit_events, vec!["demo.ready"]);
        assert!(capability.explicit_protocols.is_empty());
        assert_eq!(
            capability.profile_expansions[1].commands,
            vec!["app.echo", "app.info", "app.ping", "app.version"]
        );
        assert_eq!(capability.profile_expansions[0].events, vec!["app.log"]);
        assert_eq!(
            capability.allowed_navigation_origins,
            vec!["https://docs.example"]
        );
    }

    #[test]
    fn manifest_loader_rejects_invalid_capability_profiles() {
        for profile in ["", "unknown", "app info"] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
profiles = [{profile:?}]
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            if profile.is_empty() {
                assert!(matches!(
                    error,
                    ManifestError::InvalidCapabilityProfile { .. }
                ));
            } else {
                assert!(matches!(
                    error,
                    ManifestError::UnknownCapabilityProfile { .. }
                ));
            }
        }
    }

    #[test]
    fn manifest_loader_rejects_empty_capability_event() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
events = [""]
protocols = ["axion"]
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::InvalidCapabilityEvent { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_capability_event_names() {
        for event in ["../secret", "app event", ".hidden", "app.", "app..event"] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
events = [{event:?}]
protocols = ["axion"]
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(
                error,
                ManifestError::InvalidCapabilityEventName { .. }
            ));
        }
    }

    #[test]
    fn manifest_loader_rejects_empty_capability_command() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = [""]
protocols = ["axion"]
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::InvalidCapabilityCommand { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_capability_command_names() {
        for command in ["../secret", "app ping", ".hidden", "app.", "app..ping"] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = [{command:?}]
protocols = ["axion"]
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(
                error,
                ManifestError::InvalidCapabilityCommandName { .. }
            ));
        }
    }

    #[test]
    fn manifest_loader_rejects_empty_capability_protocol() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = []
protocols = [""]
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::InvalidCapabilityProtocol { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_invalid_capability_protocol_names() {
        for protocol in ["Axion", "app protocol", "axion+app", "-axion"] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = []
protocols = [{protocol:?}]
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(
                error,
                ManifestError::InvalidCapabilityProtocolName { .. }
            ));
        }
    }

    #[test]
    fn manifest_loader_rejects_bridge_without_axion_protocol() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = ["app.ping"]
events = ["app.log"]
protocols = []
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::BridgeRequiresAxionProtocol { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_empty_navigation_origins() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
allowed_navigation_origins = [""]
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(
            error,
            ManifestError::InvalidNavigationOrigin { .. }
        ));
    }

    #[test]
    fn manifest_loader_rejects_non_origin_navigation_values() {
        for origin in [
            "not a url",
            "https://docs.example/path",
            "https://docs.example?query=1",
            "https://docs.example#fragment",
            "mailto:test@example.com",
        ] {
            let manifest_path = write_manifest(&format!(
                r#"
[app]
name = "hello"

[window]
id = "main"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
allowed_navigation_origins = [{origin:?}]
"#
            ));

            let error =
                load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
            assert!(matches!(
                error,
                ManifestError::InvalidNavigationOriginValue { .. }
            ));
        }
    }

    #[test]
    fn manifest_loader_rejects_invalid_dev_url() {
        let manifest_path = write_manifest(
            r#"
[app]
name = "hello"

[window]

[dev]
url = "not a url"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"
"#,
        );

        let error = load_app_config_from_path(&manifest_path).expect_err("manifest should fail");
        assert!(matches!(error, ManifestError::InvalidDevUrl { .. }));
    }
}
