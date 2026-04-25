use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub const AXION_ASSET_MANIFEST_FILE_NAME: &str = "axion-assets.json";
pub const AXION_BUNDLE_MANIFEST_FILE_NAME: &str = "axion-bundle-manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleTarget {
    MacOsApp,
    LinuxDir,
    WindowsDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundlePlan {
    pub target: BundleTarget,
    pub output_dir: PathBuf,
    pub executable_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleMetadata {
    pub app_name: String,
    pub identifier: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub icon: Option<PathBuf>,
}

impl BundleMetadata {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            identifier: None,
            version: None,
            description: None,
            authors: Vec::new(),
            homepage: None,
            icon: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAssetsValidation {
    pub relative_entry: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifact {
    pub output_dir: PathBuf,
    pub app_dir: PathBuf,
    pub entry_path: PathBuf,
    pub asset_manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleArtifact {
    pub target: BundleTarget,
    pub output_dir: PathBuf,
    pub bundle_dir: PathBuf,
    pub resources_app_dir: PathBuf,
    pub executable_path: Option<PathBuf>,
    pub entry_path: PathBuf,
    pub asset_manifest_path: PathBuf,
    pub metadata_path: PathBuf,
    pub bundle_manifest_path: PathBuf,
    pub icon_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleVerificationReport {
    pub bundle_dir: PathBuf,
    pub checked_paths: Vec<PathBuf>,
    pub bundle_file_count: usize,
}

#[derive(Debug, Error)]
pub enum PackagerError {
    #[error("packaged entry '{entry}' must stay within frontend_dist '{frontend_dist}'")]
    EntryOutsideFrontendDist {
        entry: PathBuf,
        frontend_dist: PathBuf,
    },
    #[error("frontend_dist '{path}' must exist and be a directory")]
    MissingFrontendDist { path: PathBuf },
    #[error("packaged entry '{path}' must exist and be a file")]
    MissingEntry { path: PathBuf },
    #[error("frontend_dist must not contain symlinks: '{path}'")]
    SymlinkNotAllowed { path: PathBuf },
    #[error(
        "output app directory '{output_dir}' must not be inside frontend_dist '{frontend_dist}'"
    )]
    OutputInsideFrontendDist {
        output_dir: PathBuf,
        frontend_dist: PathBuf,
    },
    #[error("frontend_dist must not contain reserved generated asset path '{path}'")]
    ReservedAssetPath { path: PathBuf },
    #[error("bundle executable '{path}' must exist and be a file")]
    MissingExecutable { path: PathBuf },
    #[error("bundle icon '{path}' must exist and be a file")]
    MissingIcon { path: PathBuf },
    #[error("bundle path '{path}' is missing or has an unexpected type")]
    MissingBundlePath { path: PathBuf },
    #[error("bundle manifest '{path}' is invalid: {message}")]
    InvalidBundleManifest { path: PathBuf, message: String },
    #[error("failed to prepare build artifact: {0}")]
    Io(#[from] std::io::Error),
}

pub fn current_bundle_target() -> BundleTarget {
    #[cfg(target_os = "macos")]
    {
        BundleTarget::MacOsApp
    }
    #[cfg(target_os = "windows")]
    {
        BundleTarget::WindowsDir
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        BundleTarget::LinuxDir
    }
}

pub fn stage_web_assets(
    frontend_dist: impl Into<PathBuf>,
    entry: impl Into<PathBuf>,
    output_dir: impl Into<PathBuf>,
) -> Result<BuildArtifact, PackagerError> {
    let frontend_dist = frontend_dist.into();
    let entry = entry.into();
    let output_dir = output_dir.into();
    let validation = validate_web_assets(&frontend_dist, &entry)?;

    let app_dir = output_dir.join("app");
    reject_output_inside_frontend_dist(&frontend_dist, &app_dir)?;
    let asset_files = collect_asset_files(&frontend_dist)?;

    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)?;
    }
    fs::create_dir_all(&app_dir)?;
    copy_dir_recursive(&frontend_dist, &app_dir)?;
    let asset_manifest_path =
        write_asset_manifest(&app_dir, &validation.relative_entry, &asset_files)?;

    Ok(BuildArtifact {
        output_dir,
        app_dir: app_dir.clone(),
        entry_path: app_dir.join(validation.relative_entry),
        asset_manifest_path,
    })
}

pub fn stage_bundle_from_web_assets(
    frontend_dist: impl Into<PathBuf>,
    entry: impl Into<PathBuf>,
    bundle_plan: BundlePlan,
    app_name: &str,
) -> Result<BundleArtifact, PackagerError> {
    let metadata = BundleMetadata::new(app_name);
    stage_bundle_from_web_assets_with_metadata(frontend_dist, entry, bundle_plan, &metadata)
}

pub fn stage_bundle_from_web_assets_with_metadata(
    frontend_dist: impl Into<PathBuf>,
    entry: impl Into<PathBuf>,
    bundle_plan: BundlePlan,
    metadata: &BundleMetadata,
) -> Result<BundleArtifact, PackagerError> {
    let frontend_dist = frontend_dist.into();
    let entry = entry.into();
    let validation = validate_web_assets(&frontend_dist, &entry)?;
    let executable_path = validate_bundle_executable(bundle_plan.executable_path.as_deref())?;
    let icon_path = validate_bundle_icon(metadata.icon.as_deref())?;

    let bundle_dir = bundle_root_dir(
        &bundle_plan.output_dir,
        bundle_plan.target,
        &metadata.app_name,
    );
    let resources_app_dir = bundle_resources_dir(&bundle_dir, bundle_plan.target);
    reject_output_inside_frontend_dist(&frontend_dist, &resources_app_dir)?;
    let asset_files = collect_asset_files(&frontend_dist)?;

    if bundle_dir.exists() {
        fs::remove_dir_all(&bundle_dir)?;
    }
    fs::create_dir_all(&resources_app_dir)?;
    copy_dir_recursive(&frontend_dist, &resources_app_dir)?;
    let asset_manifest_path =
        write_asset_manifest(&resources_app_dir, &validation.relative_entry, &asset_files)?;
    let copied_executable_path = copy_bundle_executable(
        executable_path.as_deref(),
        &bundle_dir,
        bundle_plan.target,
        &metadata.app_name,
    )?;
    let copied_icon_path = copy_bundle_icon(icon_path.as_deref(), &bundle_dir, bundle_plan.target)?;
    let metadata_path = write_bundle_metadata(
        &bundle_dir,
        bundle_plan.target,
        metadata,
        copied_executable_path.as_deref(),
        copied_icon_path.as_deref(),
    )?;
    let entry_path = resources_app_dir.join(validation.relative_entry);
    let bundle_manifest_path = write_bundle_manifest(
        &bundle_dir,
        bundle_plan.target,
        metadata,
        &resources_app_dir,
        &entry_path,
        &asset_manifest_path,
        &metadata_path,
        copied_executable_path.as_deref(),
        copied_icon_path.as_deref(),
    )?;

    Ok(BundleArtifact {
        target: bundle_plan.target,
        output_dir: bundle_plan.output_dir,
        bundle_dir,
        resources_app_dir: resources_app_dir.clone(),
        executable_path: copied_executable_path,
        entry_path,
        asset_manifest_path,
        metadata_path,
        bundle_manifest_path,
        icon_path: copied_icon_path,
    })
}

pub fn verify_bundle_artifact(
    artifact: &BundleArtifact,
) -> Result<BundleVerificationReport, PackagerError> {
    let mut checked_paths = Vec::new();
    require_bundle_dir(&artifact.bundle_dir, &mut checked_paths)?;
    require_bundle_dir(&artifact.resources_app_dir, &mut checked_paths)?;
    require_bundle_file(&artifact.entry_path, &mut checked_paths)?;
    require_bundle_file(&artifact.asset_manifest_path, &mut checked_paths)?;
    require_bundle_file(&artifact.metadata_path, &mut checked_paths)?;
    require_bundle_file(&artifact.bundle_manifest_path, &mut checked_paths)?;

    if let Some(path) = &artifact.executable_path {
        require_bundle_file(path, &mut checked_paths)?;
    }
    if let Some(path) = &artifact.icon_path {
        require_bundle_file(path, &mut checked_paths)?;
    }

    verify_bundle_manifest_references(artifact)?;
    let bundle_file_count = collect_bundle_manifest_files(&artifact.bundle_dir)?.len();

    Ok(BundleVerificationReport {
        bundle_dir: artifact.bundle_dir.clone(),
        checked_paths,
        bundle_file_count,
    })
}

fn bundle_root_dir(output_dir: &Path, target: BundleTarget, app_name: &str) -> PathBuf {
    match target {
        BundleTarget::MacOsApp => output_dir.join(format!("{app_name}.app")),
        BundleTarget::LinuxDir | BundleTarget::WindowsDir => output_dir.join(app_name),
    }
}

fn bundle_resources_dir(bundle_dir: &Path, target: BundleTarget) -> PathBuf {
    match target {
        BundleTarget::MacOsApp => bundle_dir.join("Contents").join("Resources").join("app"),
        BundleTarget::LinuxDir | BundleTarget::WindowsDir => {
            bundle_dir.join("resources").join("app")
        }
    }
}

fn validate_bundle_executable(path: Option<&Path>) -> Result<Option<PathBuf>, PackagerError> {
    let Some(path) = path else {
        return Ok(None);
    };

    if !path.is_file() {
        return Err(PackagerError::MissingExecutable {
            path: path.to_path_buf(),
        });
    }

    Ok(Some(path.to_path_buf()))
}

pub fn validate_bundle_icon(path: Option<&Path>) -> Result<Option<PathBuf>, PackagerError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(path).map_err(|_| PackagerError::MissingIcon {
        path: path.to_path_buf(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(PackagerError::SymlinkNotAllowed {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_file() {
        return Err(PackagerError::MissingIcon {
            path: path.to_path_buf(),
        });
    }

    Ok(Some(path.to_path_buf()))
}

fn require_bundle_dir(path: &Path, checked_paths: &mut Vec<PathBuf>) -> Result<(), PackagerError> {
    if !path.is_dir() {
        return Err(PackagerError::MissingBundlePath {
            path: path.to_path_buf(),
        });
    }
    checked_paths.push(path.to_path_buf());
    Ok(())
}

fn require_bundle_file(path: &Path, checked_paths: &mut Vec<PathBuf>) -> Result<(), PackagerError> {
    if !path.is_file() {
        return Err(PackagerError::MissingBundlePath {
            path: path.to_path_buf(),
        });
    }
    checked_paths.push(path.to_path_buf());
    Ok(())
}

fn verify_bundle_manifest_references(artifact: &BundleArtifact) -> Result<(), PackagerError> {
    let manifest = fs::read_to_string(&artifact.bundle_manifest_path)?;
    require_bundle_manifest_field(
        artifact,
        &manifest,
        "entry",
        &bundle_relative_path(&artifact.bundle_dir, &artifact.entry_path),
    )?;
    require_bundle_manifest_field(
        artifact,
        &manifest,
        "asset_manifest",
        &bundle_relative_path(&artifact.bundle_dir, &artifact.asset_manifest_path),
    )?;
    require_bundle_manifest_field(
        artifact,
        &manifest,
        "metadata",
        &bundle_relative_path(&artifact.bundle_dir, &artifact.metadata_path),
    )?;

    match &artifact.executable_path {
        Some(path) => require_bundle_manifest_field(
            artifact,
            &manifest,
            "executable",
            &bundle_relative_path(&artifact.bundle_dir, path),
        )?,
        None => require_bundle_manifest_null_field(artifact, &manifest, "executable")?,
    }
    match &artifact.icon_path {
        Some(path) => require_bundle_manifest_field(
            artifact,
            &manifest,
            "icon",
            &bundle_relative_path(&artifact.bundle_dir, path),
        )?,
        None => require_bundle_manifest_null_field(artifact, &manifest, "icon")?,
    }

    let files = collect_bundle_manifest_files(&artifact.bundle_dir)?;
    for file in files
        .into_iter()
        .filter(|file| file.path != AXION_BUNDLE_MANIFEST_FILE_NAME)
    {
        let expected = format!(
            "{{ \"path\": {}, \"bytes\": {}, \"fnv1a64\": {} }}",
            json_string_literal(&file.path),
            file.bytes,
            json_string_literal(&file.fnv1a64)
        );
        if !manifest.contains(&expected) {
            return Err(PackagerError::InvalidBundleManifest {
                path: artifact.bundle_manifest_path.clone(),
                message: format!("missing file entry for '{}'", file.path),
            });
        }
    }

    Ok(())
}

fn require_bundle_manifest_field(
    artifact: &BundleArtifact,
    manifest: &str,
    field: &str,
    value: &str,
) -> Result<(), PackagerError> {
    let expected = format!("\"{field}\": {}", json_string_literal(value));
    if !manifest.contains(&expected) {
        return Err(PackagerError::InvalidBundleManifest {
            path: artifact.bundle_manifest_path.clone(),
            message: format!("missing {field} reference '{}'", value),
        });
    }
    Ok(())
}

fn require_bundle_manifest_null_field(
    artifact: &BundleArtifact,
    manifest: &str,
    field: &str,
) -> Result<(), PackagerError> {
    let expected = format!("\"{field}\": null");
    if !manifest.contains(&expected) {
        return Err(PackagerError::InvalidBundleManifest {
            path: artifact.bundle_manifest_path.clone(),
            message: format!("missing null {field} reference"),
        });
    }
    Ok(())
}

fn copy_bundle_executable(
    source: Option<&Path>,
    bundle_dir: &Path,
    target: BundleTarget,
    app_name: &str,
) -> Result<Option<PathBuf>, PackagerError> {
    let Some(source) = source else {
        return Ok(None);
    };

    let destination = bundle_executable_path(bundle_dir, target, app_name);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, &destination)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&destination)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions)?;
    }

    Ok(Some(destination))
}

fn copy_bundle_icon(
    source: Option<&Path>,
    bundle_dir: &Path,
    target: BundleTarget,
) -> Result<Option<PathBuf>, PackagerError> {
    let Some(source) = source else {
        return Ok(None);
    };
    let Some(file_name) = source.file_name() else {
        return Err(PackagerError::MissingIcon {
            path: source.to_path_buf(),
        });
    };
    let destination = bundle_icon_path(bundle_dir, target, file_name);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, &destination)?;
    Ok(Some(destination))
}

fn bundle_icon_path(
    bundle_dir: &Path,
    target: BundleTarget,
    file_name: &std::ffi::OsStr,
) -> PathBuf {
    match target {
        BundleTarget::MacOsApp => bundle_dir
            .join("Contents")
            .join("Resources")
            .join(file_name),
        BundleTarget::LinuxDir | BundleTarget::WindowsDir => {
            bundle_dir.join("resources").join(file_name)
        }
    }
}

fn bundle_executable_path(bundle_dir: &Path, target: BundleTarget, app_name: &str) -> PathBuf {
    match target {
        BundleTarget::MacOsApp => bundle_dir.join("Contents").join("MacOS").join(app_name),
        BundleTarget::LinuxDir => bundle_dir.join("bin").join(app_name),
        BundleTarget::WindowsDir => bundle_dir.join("bin").join(format!("{app_name}.exe")),
    }
}

fn write_bundle_metadata(
    bundle_dir: &Path,
    target: BundleTarget,
    metadata: &BundleMetadata,
    executable_path: Option<&Path>,
    icon_path: Option<&Path>,
) -> Result<PathBuf, PackagerError> {
    match target {
        BundleTarget::MacOsApp => {
            write_macos_metadata(bundle_dir, metadata, executable_path, icon_path)
        }
        BundleTarget::LinuxDir | BundleTarget::WindowsDir => write_directory_bundle_metadata(
            bundle_dir,
            target,
            metadata,
            executable_path,
            icon_path,
        ),
    }
}

fn write_macos_metadata(
    bundle_dir: &Path,
    metadata: &BundleMetadata,
    executable_path: Option<&Path>,
    icon_path: Option<&Path>,
) -> Result<PathBuf, PackagerError> {
    let contents_dir = bundle_dir.join("Contents");
    fs::create_dir_all(&contents_dir)?;
    fs::write(contents_dir.join("PkgInfo"), "APPL????\n")?;

    let executable_name = executable_path
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.app_name.clone());
    let identifier = metadata
        .identifier
        .clone()
        .unwrap_or_else(|| format!("dev.axion.{}", metadata.app_name));
    let version = metadata
        .version
        .as_deref()
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let description = metadata.description.as_deref().unwrap_or("");
    let homepage = metadata.homepage.as_deref().unwrap_or("");
    let authors = metadata
        .authors
        .iter()
        .map(|author| format!("    <string>{}</string>\n", xml_escape(author)))
        .collect::<String>();
    let icon_key = icon_path
        .and_then(Path::file_name)
        .map(|file_name| {
            format!(
                "  <key>CFBundleIconFile</key>\n  <string>{}</string>\n",
                xml_escape(&file_name.to_string_lossy())
            )
        })
        .unwrap_or_default();
    let info_plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>CFBundleExecutable</key>\n\
  <string>{}</string>\n\
  <key>CFBundleIdentifier</key>\n\
  <string>{}</string>\n\
  <key>CFBundleName</key>\n\
  <string>{}</string>\n\
  <key>CFBundlePackageType</key>\n\
  <string>APPL</string>\n\
  <key>CFBundleShortVersionString</key>\n\
  <string>{}</string>\n\
{}\
  <key>AxionDescription</key>\n\
  <string>{}</string>\n\
  <key>AxionHomepage</key>\n\
  <string>{}</string>\n\
  <key>AxionAuthors</key>\n\
  <array>\n\
{}  </array>\n\
</dict>\n\
</plist>\n",
        xml_escape(&executable_name),
        xml_escape(&identifier),
        xml_escape(&metadata.app_name),
        xml_escape(version),
        icon_key,
        xml_escape(description),
        xml_escape(homepage),
        authors,
    );
    let metadata_path = contents_dir.join("Info.plist");
    fs::write(&metadata_path, info_plist)?;
    Ok(metadata_path)
}

fn write_directory_bundle_metadata(
    bundle_dir: &Path,
    target: BundleTarget,
    metadata: &BundleMetadata,
    executable_path: Option<&Path>,
    icon_path: Option<&Path>,
) -> Result<PathBuf, PackagerError> {
    fs::create_dir_all(bundle_dir)?;
    let executable = executable_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "none".to_owned());
    let metadata_path = bundle_dir.join("axion-bundle.txt");
    let icon = icon_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "none".to_owned());
    fs::write(
        &metadata_path,
        format!(
            "app={}\nidentifier={}\nversion={}\ndescription={}\nauthors={}\nhomepage={}\ntarget={target:?}\nexecutable={executable}\nicon={icon}\nresources=resources/app\n",
            metadata.app_name,
            metadata.identifier.as_deref().unwrap_or(""),
            metadata
                .version
                .as_deref()
                .unwrap_or(env!("CARGO_PKG_VERSION")),
            metadata.description.as_deref().unwrap_or(""),
            metadata.authors.join(","),
            metadata.homepage.as_deref().unwrap_or(""),
        ),
    )?;
    Ok(metadata_path)
}

fn write_bundle_manifest(
    bundle_dir: &Path,
    target: BundleTarget,
    metadata: &BundleMetadata,
    resources_app_dir: &Path,
    entry_path: &Path,
    asset_manifest_path: &Path,
    metadata_path: &Path,
    executable_path: Option<&Path>,
    icon_path: Option<&Path>,
) -> Result<PathBuf, PackagerError> {
    let manifest_path = bundle_dir.join(AXION_BUNDLE_MANIFEST_FILE_NAME);
    let files = collect_bundle_manifest_files(bundle_dir)?
        .into_iter()
        .map(|file| {
            format!(
                "    {{ \"path\": {}, \"bytes\": {}, \"fnv1a64\": {} }}",
                json_string_literal(&file.path),
                file.bytes,
                json_string_literal(&file.fnv1a64)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let source = format!(
        "{{\n  \"version\": 1,\n  \"target\": {},\n  \"app\": {},\n  \"identifier\": {},\n  \"app_version\": {},\n  \"resources\": {},\n  \"entry\": {},\n  \"asset_manifest\": {},\n  \"metadata\": {},\n  \"executable\": {},\n  \"icon\": {},\n  \"files\": [\n{}\n  ]\n}}\n",
        json_string_literal(bundle_target_name(target)),
        json_string_literal(&metadata.app_name),
        json_optional_string_literal(metadata.identifier.as_deref()),
        json_optional_string_literal(metadata.version.as_deref()),
        json_string_literal(&bundle_relative_path(bundle_dir, resources_app_dir)),
        json_string_literal(&bundle_relative_path(bundle_dir, entry_path)),
        json_string_literal(&bundle_relative_path(bundle_dir, asset_manifest_path)),
        json_string_literal(&bundle_relative_path(bundle_dir, metadata_path)),
        json_optional_string_literal(
            executable_path
                .map(|path| bundle_relative_path(bundle_dir, path))
                .as_deref(),
        ),
        json_optional_string_literal(
            icon_path
                .map(|path| bundle_relative_path(bundle_dir, path))
                .as_deref(),
        ),
        files
    );
    fs::write(&manifest_path, source)?;
    Ok(manifest_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BundleManifestFile {
    path: String,
    bytes: u64,
    fnv1a64: String,
}

fn collect_bundle_manifest_files(
    bundle_dir: &Path,
) -> Result<Vec<BundleManifestFile>, PackagerError> {
    let mut files = Vec::new();
    collect_bundle_manifest_files_recursive(bundle_dir, bundle_dir, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_bundle_manifest_files_recursive(
    bundle_dir: &Path,
    current: &Path,
    files: &mut Vec<BundleManifestFile>,
) -> Result<(), PackagerError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            collect_bundle_manifest_files_recursive(bundle_dir, &path, files)?;
        } else if file_type.is_file() {
            files.push(BundleManifestFile {
                path: bundle_relative_path(bundle_dir, &path),
                bytes: entry.metadata()?.len(),
                fnv1a64: fnv1a64_file_hex(&path)?,
            });
        }
    }

    Ok(())
}

fn fnv1a64_file_hex(path: &Path) -> Result<String, PackagerError> {
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

fn bundle_target_name(target: BundleTarget) -> &'static str {
    match target {
        BundleTarget::MacOsApp => "macos-app",
        BundleTarget::LinuxDir => "linux-dir",
        BundleTarget::WindowsDir => "windows-dir",
    }
}

fn bundle_relative_path(bundle_dir: &Path, path: &Path) -> String {
    path.strip_prefix(bundle_dir)
        .map(relative_path_string)
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn validate_web_assets(
    frontend_dist: impl AsRef<Path>,
    entry: impl AsRef<Path>,
) -> Result<WebAssetsValidation, PackagerError> {
    let frontend_dist = frontend_dist.as_ref();
    let entry = entry.as_ref();

    if !frontend_dist.is_dir() {
        return Err(PackagerError::MissingFrontendDist {
            path: frontend_dist.to_path_buf(),
        });
    }

    reject_symlinks(frontend_dist)?;
    reject_reserved_asset_paths(frontend_dist)?;

    let relative_entry = entry
        .strip_prefix(frontend_dist)
        .map(Path::to_path_buf)
        .map_err(|_| PackagerError::EntryOutsideFrontendDist {
            entry: entry.to_path_buf(),
            frontend_dist: frontend_dist.to_path_buf(),
        })?;

    if !entry.is_file() {
        return Err(PackagerError::MissingEntry {
            path: entry.to_path_buf(),
        });
    }

    Ok(WebAssetsValidation { relative_entry })
}

fn reject_reserved_asset_paths(frontend_dist: &Path) -> Result<(), PackagerError> {
    let reserved_path = frontend_dist.join(AXION_ASSET_MANIFEST_FILE_NAME);
    if reserved_path.exists() {
        return Err(PackagerError::ReservedAssetPath {
            path: reserved_path,
        });
    }

    Ok(())
}

fn reject_output_inside_frontend_dist(
    frontend_dist: &Path,
    output_dir: &Path,
) -> Result<(), PackagerError> {
    let frontend_dist_compare = comparable_path(frontend_dist)?;
    let output_dir_compare = comparable_path(output_dir)?;

    if output_dir_compare.starts_with(&frontend_dist_compare) {
        return Err(PackagerError::OutputInsideFrontendDist {
            output_dir: output_dir.to_path_buf(),
            frontend_dist: frontend_dist.to_path_buf(),
        });
    }

    Ok(())
}

fn comparable_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.exists() {
        return path.canonicalize();
    }

    let mut missing_components = Vec::new();
    let mut current = path;
    while !current.exists() {
        let Some(file_name) = current.file_name() else {
            let base = if current.is_absolute() {
                PathBuf::from(current)
            } else {
                std::env::current_dir()?.join(current)
            };
            return Ok(missing_components
                .into_iter()
                .rev()
                .fold(base, |base, component| base.join(component)));
        };
        missing_components.push(file_name.to_owned());

        let Some(parent) = current.parent() else {
            let base = std::env::current_dir()?;
            return Ok(missing_components
                .into_iter()
                .rev()
                .fold(base, |base, component| base.join(component)));
        };
        current = parent;
    }

    let base = current.canonicalize()?;
    Ok(missing_components
        .into_iter()
        .rev()
        .fold(base, |base, component| base.join(component)))
}

fn reject_symlinks(path: &Path) -> Result<(), PackagerError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(PackagerError::SymlinkNotAllowed {
            path: path.to_path_buf(),
        });
    }

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            reject_symlinks(&entry?.path())?;
        }
    }

    Ok(())
}

fn collect_asset_files(frontend_dist: &Path) -> Result<Vec<PathBuf>, PackagerError> {
    let mut files = Vec::new();
    collect_asset_files_recursive(frontend_dist, frontend_dist, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_asset_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), PackagerError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            collect_asset_files_recursive(root, &path, files)?;
        } else if file_type.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .map_err(|_| PackagerError::EntryOutsideFrontendDist {
                    entry: path.clone(),
                    frontend_dist: root.to_path_buf(),
                })?;
            files.push(relative_path);
        }
    }

    Ok(())
}

fn write_asset_manifest(
    app_dir: &Path,
    relative_entry: &Path,
    asset_files: &[PathBuf],
) -> Result<PathBuf, PackagerError> {
    let manifest_path = app_dir.join(AXION_ASSET_MANIFEST_FILE_NAME);
    let files = asset_files
        .iter()
        .map(|path| format!("    {}", json_string_literal(&relative_path_string(path))))
        .collect::<Vec<_>>()
        .join(",\n");
    let source = format!(
        "{{\n  \"version\": 1,\n  \"entry\": {},\n  \"files\": [\n{}\n  ]\n}}\n",
        json_string_literal(&relative_path_string(relative_entry)),
        files
    );
    fs::write(&manifest_path, source)?;
    Ok(manifest_path)
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

fn json_string_literal(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    format!("\"{escaped}\"")
}

fn json_optional_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&destination_path)?;
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(source_path, destination_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        AXION_ASSET_MANIFEST_FILE_NAME, AXION_BUNDLE_MANIFEST_FILE_NAME, BundleMetadata,
        BundlePlan, BundleTarget, PackagerError, current_bundle_target,
        stage_bundle_from_web_assets, stage_bundle_from_web_assets_with_metadata, stage_web_assets,
        validate_web_assets, verify_bundle_artifact,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-packager-{name}-{unique}-{serial}"))
    }

    #[test]
    fn stage_web_assets_copies_frontend_dist() {
        let source = temp_dir("source");
        let output = temp_dir("output");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(
            source.join("nested").join("main.js"),
            "console.log('axion')",
        )
        .unwrap();

        let artifact =
            stage_web_assets(source.clone(), source.join("index.html"), output.clone()).unwrap();

        assert_eq!(artifact.output_dir, output);
        assert!(artifact.app_dir.join("index.html").exists());
        assert!(artifact.app_dir.join("nested").join("main.js").exists());
        assert_eq!(artifact.entry_path, artifact.app_dir.join("index.html"));
        assert_eq!(
            artifact.asset_manifest_path,
            artifact.app_dir.join("axion-assets.json")
        );
        assert!(artifact.asset_manifest_path.exists());

        let manifest = fs::read_to_string(&artifact.asset_manifest_path).unwrap();
        assert!(manifest.contains("\"version\": 1"));
        assert!(manifest.contains("\"entry\": \"index.html\""));
        assert!(manifest.contains("\"index.html\""));
        assert!(manifest.contains("\"nested/main.js\""));
    }

    #[test]
    fn validate_web_assets_reports_relative_entry() {
        let source = temp_dir("validate-source");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(
            source.join("nested").join("index.html"),
            "<html>Hello</html>",
        )
        .unwrap();

        let validation = validate_web_assets(&source, source.join("nested").join("index.html"))
            .expect("assets should validate");

        assert_eq!(
            validation.relative_entry,
            PathBuf::from("nested/index.html")
        );
    }

    #[test]
    fn validate_web_assets_rejects_reserved_asset_manifest_path() {
        let source = temp_dir("reserved-manifest-source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(source.join(AXION_ASSET_MANIFEST_FILE_NAME), "{}").unwrap();

        let error = validate_web_assets(&source, source.join("index.html"))
            .expect_err("reserved generated asset manifest path should fail");

        assert!(matches!(error, PackagerError::ReservedAssetPath { .. }));
    }

    #[test]
    fn stage_web_assets_rejects_entry_outside_frontend_dist() {
        let source = temp_dir("source-outside");
        let output = temp_dir("output-outside");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let error = stage_web_assets(
            source.clone(),
            std::env::temp_dir().join("external-index.html"),
            output,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            PackagerError::EntryOutsideFrontendDist { .. }
        ));
    }

    #[test]
    fn stage_web_assets_rejects_missing_frontend_dist() {
        let source = temp_dir("missing-source");
        let output = temp_dir("missing-output");

        let error =
            stage_web_assets(source.clone(), source.join("index.html"), output).unwrap_err();

        assert!(matches!(error, PackagerError::MissingFrontendDist { .. }));
    }

    #[test]
    fn stage_web_assets_rejects_missing_entry() {
        let source = temp_dir("missing-entry-source");
        let output = temp_dir("missing-entry-output");
        fs::create_dir_all(&source).unwrap();

        let error =
            stage_web_assets(source.clone(), source.join("index.html"), output).unwrap_err();

        assert!(matches!(error, PackagerError::MissingEntry { .. }));
    }

    #[test]
    fn stage_web_assets_rejects_output_inside_frontend_dist() {
        let source = temp_dir("nested-output-source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let output = source.join("target");
        let error = stage_web_assets(source.clone(), source.join("index.html"), output)
            .expect_err("output inside frontend_dist should fail");

        assert!(matches!(
            error,
            PackagerError::OutputInsideFrontendDist { .. }
        ));
        assert!(source.join("index.html").exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage_web_assets_rejects_symlinked_files() {
        use std::os::unix::fs::symlink;

        let source = temp_dir("symlink-source");
        let output = temp_dir("symlink-output");
        let external = temp_dir("symlink-external");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(external.join("secret.txt"), "secret").unwrap();
        symlink(external.join("secret.txt"), source.join("secret.txt")).unwrap();

        let error = stage_web_assets(source.clone(), source.join("index.html"), output)
            .expect_err("symlinked file should fail");

        assert!(matches!(error, PackagerError::SymlinkNotAllowed { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn stage_web_assets_rejects_symlinked_frontend_dist() {
        use std::os::unix::fs::symlink;

        let real_source = temp_dir("symlink-real-source");
        let source_link = temp_dir("symlink-source-link");
        let output = temp_dir("symlink-source-output");
        fs::create_dir_all(&real_source).unwrap();
        fs::write(real_source.join("index.html"), "<html>Hello</html>").unwrap();
        symlink(&real_source, &source_link).unwrap();

        let error = stage_web_assets(source_link.clone(), source_link.join("index.html"), output)
            .expect_err("symlinked frontend_dist should fail");

        assert!(matches!(error, PackagerError::SymlinkNotAllowed { .. }));
    }

    #[test]
    fn stage_bundle_from_web_assets_creates_platform_resources_dir() {
        let source = temp_dir("bundle-source");
        let output = temp_dir("bundle-output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output.clone(),
                executable_path: None,
            },
            "hello-axion",
        )
        .unwrap();

        assert_eq!(artifact.bundle_dir, output.join("hello-axion"));
        assert_eq!(
            artifact.resources_app_dir,
            output.join("hello-axion").join("resources").join("app")
        );
        assert_eq!(artifact.executable_path, None);
        assert!(artifact.entry_path.exists());
        assert!(artifact.asset_manifest_path.exists());
        assert!(artifact.metadata_path.exists());
        assert!(artifact.bundle_manifest_path.exists());
        assert_eq!(
            artifact.asset_manifest_path,
            artifact.resources_app_dir.join("axion-assets.json")
        );
        assert_eq!(
            artifact.metadata_path,
            output.join("hello-axion").join("axion-bundle.txt")
        );
        assert_eq!(
            artifact.bundle_manifest_path,
            output
                .join("hello-axion")
                .join(AXION_BUNDLE_MANIFEST_FILE_NAME)
        );
        let bundle_manifest = fs::read_to_string(&artifact.bundle_manifest_path).unwrap();
        assert!(bundle_manifest.contains("\"target\": \"linux-dir\""));
        assert!(bundle_manifest.contains("\"resources\": \"resources/app\""));
        assert!(bundle_manifest.contains("\"entry\": \"resources/app/index.html\""));
        assert!(bundle_manifest.contains("\"metadata\": \"axion-bundle.txt\""));
        assert!(bundle_manifest.contains("\"icon\": null"));
        assert!(bundle_manifest.contains("\"path\": \"resources/app/index.html\""));
        assert!(bundle_manifest.contains("\"fnv1a64\":"));

        let verification = verify_bundle_artifact(&artifact).unwrap();
        assert_eq!(verification.bundle_dir, artifact.bundle_dir);
        assert!(verification.bundle_file_count >= 3);
        assert!(
            verification
                .checked_paths
                .contains(&artifact.bundle_manifest_path)
        );
    }

    #[test]
    fn stage_bundle_from_web_assets_copies_executable() {
        let source = temp_dir("bundle-exe-source");
        let output = temp_dir("bundle-exe-output");
        let executable = temp_dir("bundle-exe-bin").join("hello-axion");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(&executable, "binary").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output.clone(),
                executable_path: Some(executable),
            },
            "hello-axion",
        )
        .unwrap();

        let bundled_executable = output.join("hello-axion").join("bin").join("hello-axion");
        assert_eq!(artifact.executable_path, Some(bundled_executable.clone()));
        assert!(bundled_executable.exists());
        assert!(artifact.metadata_path.exists());
    }

    #[test]
    fn stage_bundle_from_web_assets_rejects_missing_executable() {
        let source = temp_dir("bundle-missing-exe-source");
        let output = temp_dir("bundle-missing-exe-output");
        let executable = temp_dir("bundle-missing-exe-bin").join("hello-axion");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let error = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output,
                executable_path: Some(executable),
            },
            "hello-axion",
        )
        .expect_err("missing executable should fail");

        assert!(matches!(error, PackagerError::MissingExecutable { .. }));
    }

    #[test]
    fn stage_bundle_from_web_assets_creates_macos_metadata() {
        let source = temp_dir("bundle-macos-source");
        let output = temp_dir("bundle-macos-output");
        let executable = temp_dir("bundle-macos-bin").join("hello-axion");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(&executable, "binary").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::MacOsApp,
                output_dir: output.clone(),
                executable_path: Some(executable),
            },
            "hello-axion",
        )
        .unwrap();

        assert_eq!(artifact.bundle_dir, output.join("hello-axion.app"));
        assert_eq!(
            artifact.resources_app_dir,
            output
                .join("hello-axion.app")
                .join("Contents")
                .join("Resources")
                .join("app")
        );
        assert_eq!(
            artifact.executable_path,
            Some(
                output
                    .join("hello-axion.app")
                    .join("Contents")
                    .join("MacOS")
                    .join("hello-axion")
            )
        );
        assert_eq!(
            artifact.metadata_path,
            output
                .join("hello-axion.app")
                .join("Contents")
                .join("Info.plist")
        );
        assert!(artifact.metadata_path.exists());
        assert!(
            fs::read_to_string(&artifact.metadata_path)
                .unwrap()
                .contains("<key>CFBundleExecutable</key>")
        );
    }

    #[test]
    fn stage_bundle_from_web_assets_writes_app_metadata() {
        let source = temp_dir("bundle-metadata-source");
        let output = temp_dir("bundle-metadata-output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let artifact = stage_bundle_from_web_assets_with_metadata(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output,
                executable_path: None,
            },
            &BundleMetadata {
                app_name: "hello-axion".to_owned(),
                identifier: Some("dev.axion.hello".to_owned()),
                version: Some("1.2.3".to_owned()),
                description: Some("Hello metadata".to_owned()),
                authors: vec!["Axion Maintainers".to_owned()],
                homepage: Some("https://example.dev/hello".to_owned()),
                icon: None,
            },
        )
        .unwrap();

        let metadata = fs::read_to_string(&artifact.metadata_path).unwrap();
        assert!(metadata.contains("app=hello-axion"));
        assert!(metadata.contains("identifier=dev.axion.hello"));
        assert!(metadata.contains("version=1.2.3"));
        assert!(metadata.contains("description=Hello metadata"));
        assert!(metadata.contains("authors=Axion Maintainers"));
        assert!(metadata.contains("homepage=https://example.dev/hello"));
    }

    #[test]
    fn stage_bundle_from_web_assets_copies_icon_and_writes_metadata() {
        let source = temp_dir("bundle-icon-source");
        let output = temp_dir("bundle-icon-output");
        let icon = temp_dir("bundle-icon-file").join("app.icns");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(&icon, "icon").unwrap();

        let artifact = stage_bundle_from_web_assets_with_metadata(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::MacOsApp,
                output_dir: output,
                executable_path: None,
            },
            &BundleMetadata {
                app_name: "hello-axion".to_owned(),
                identifier: Some("dev.axion.hello".to_owned()),
                version: Some("1.2.3".to_owned()),
                description: None,
                authors: Vec::new(),
                homepage: None,
                icon: Some(icon),
            },
        )
        .unwrap();

        let copied_icon = artifact.icon_path.as_ref().expect("icon should be copied");
        assert_eq!(copied_icon.file_name().unwrap(), "app.icns");
        assert!(copied_icon.exists());
        let metadata = fs::read_to_string(&artifact.metadata_path).unwrap();
        assert!(metadata.contains("<key>CFBundleIconFile</key>"));
        assert!(metadata.contains("<string>app.icns</string>"));
        let bundle_manifest = fs::read_to_string(&artifact.bundle_manifest_path).unwrap();
        assert!(bundle_manifest.contains("\"target\": \"macos-app\""));
        assert!(bundle_manifest.contains("\"icon\": \"Contents/Resources/app.icns\""));
        assert!(bundle_manifest.contains("\"metadata\": \"Contents/Info.plist\""));
        assert!(bundle_manifest.contains("\"path\": \"Contents/Resources/app.icns\""));
        let verification = verify_bundle_artifact(&artifact).unwrap();
        assert!(verification.bundle_file_count >= 4);
    }

    #[test]
    fn verify_bundle_artifact_rejects_missing_manifest_references() {
        let source = temp_dir("bundle-verify-source");
        let output = temp_dir("bundle-verify-output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output,
                executable_path: None,
            },
            "hello-axion",
        )
        .unwrap();
        fs::write(&artifact.bundle_manifest_path, "{}").unwrap();

        let error = verify_bundle_artifact(&artifact).expect_err("invalid manifest should fail");

        assert!(matches!(error, PackagerError::InvalidBundleManifest { .. }));
    }

    #[test]
    fn verify_bundle_artifact_rejects_missing_files() {
        let source = temp_dir("bundle-verify-missing-source");
        let output = temp_dir("bundle-verify-missing-output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output,
                executable_path: None,
            },
            "hello-axion",
        )
        .unwrap();
        fs::remove_file(&artifact.entry_path).unwrap();

        let error = verify_bundle_artifact(&artifact).expect_err("missing entry should fail");

        assert!(matches!(error, PackagerError::MissingBundlePath { .. }));
    }

    #[test]
    fn verify_bundle_artifact_rejects_tampered_same_size_files() {
        let source = temp_dir("bundle-verify-tamper-source");
        let output = temp_dir("bundle-verify-tamper-output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "aaaa").unwrap();

        let artifact = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: output,
                executable_path: None,
            },
            "hello-axion",
        )
        .unwrap();
        fs::write(&artifact.entry_path, "bbbb").unwrap();

        let error = verify_bundle_artifact(&artifact).expect_err("tampered entry should fail");

        assert!(matches!(error, PackagerError::InvalidBundleManifest { .. }));
    }

    #[test]
    fn stage_bundle_from_web_assets_rejects_output_inside_frontend_dist() {
        let source = temp_dir("bundle-nested-output-source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "<html>Hello</html>").unwrap();

        let error = stage_bundle_from_web_assets(
            source.clone(),
            source.join("index.html"),
            BundlePlan {
                target: BundleTarget::LinuxDir,
                output_dir: source.join("bundle"),
                executable_path: None,
            },
            "hello-axion",
        )
        .expect_err("bundle output inside frontend_dist should fail");

        assert!(matches!(
            error,
            PackagerError::OutputInsideFrontendDist { .. }
        ));
        assert!(source.join("index.html").exists());
    }

    #[test]
    fn current_bundle_target_matches_host_platform() {
        let target = current_bundle_target();
        #[cfg(target_os = "macos")]
        assert_eq!(target, BundleTarget::MacOsApp);
        #[cfg(target_os = "windows")]
        assert_eq!(target, BundleTarget::WindowsDir);
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert_eq!(target, BundleTarget::LinuxDir);
    }
}
