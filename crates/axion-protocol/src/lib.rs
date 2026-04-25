use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;
use url::Url;

pub const AXION_SCHEME: &str = "axion";
pub const AXION_APP_AUTHORITY: &str = "app";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub file_path: PathBuf,
    pub request_path: String,
    pub content_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePolicy {
    pub headers: BTreeMap<String, String>,
}

impl ResourcePolicy {
    pub fn for_asset(asset: &ResolvedAsset) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_owned(), asset.content_type.to_owned());
        headers.insert("x-content-type-options".to_owned(), "nosniff".to_owned());
        headers.insert("referrer-policy".to_owned(), "no-referrer".to_owned());
        headers.insert(
            "cross-origin-resource-policy".to_owned(),
            "same-origin".to_owned(),
        );
        headers.insert(
            "cache-control".to_owned(),
            cache_control_for_asset(asset).to_owned(),
        );
        Self { headers }
    }

    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

pub fn default_resource_policy_summary() -> &'static str {
    "html/json=no-cache; static-assets=public,max-age=31536000,immutable; nosniff=true; referrer=no-referrer; corp=same-origin"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppAssetResolver {
    frontend_dist: PathBuf,
    default_document: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("build entry '{entry}' must stay within frontend_dist '{frontend_dist}'")]
    EntryOutsideFrontendDist {
        entry: PathBuf,
        frontend_dist: PathBuf,
    },
    #[error("invalid Axion URL '{value}'")]
    InvalidUrl { value: String },
    #[error("unsupported scheme '{scheme}', expected '{AXION_SCHEME}'")]
    UnsupportedScheme { scheme: String },
    #[error("unsupported authority '{authority}', expected '{AXION_APP_AUTHORITY}'")]
    UnsupportedAuthority { authority: String },
    #[error("requested path '{path}' escapes the application root")]
    PathTraversal { path: String },
    #[error("requested asset '{request_path}' does not exist at '{file_path}'")]
    MissingAsset {
        request_path: String,
        file_path: PathBuf,
    },
    #[error("requested asset '{request_path}' resolves to a directory at '{file_path}'")]
    AssetIsDirectory {
        request_path: String,
        file_path: PathBuf,
    },
    #[error("requested asset path must not contain symlinks: '{path}'")]
    SymlinkNotAllowed { path: PathBuf },
}

pub trait ProtocolHandler {
    fn scheme(&self) -> &str;
}

impl AppAssetResolver {
    pub fn new(frontend_dist: PathBuf, entry: PathBuf) -> Result<Self, ProtocolError> {
        let relative_entry = entry.strip_prefix(&frontend_dist).map_err(|_| {
            ProtocolError::EntryOutsideFrontendDist {
                entry: entry.clone(),
                frontend_dist: frontend_dist.clone(),
            }
        })?;

        let default_document = normalize_relative_path(relative_entry)?;

        Ok(Self {
            frontend_dist,
            default_document,
        })
    }

    pub fn frontend_dist(&self) -> &Path {
        &self.frontend_dist
    }

    pub fn default_document(&self) -> &str {
        &self.default_document
    }

    pub fn initial_url(&self) -> Url {
        self.url_for_path(self.default_document())
    }

    pub fn url_for_path(&self, request_path: &str) -> Url {
        Url::parse(&format!(
            "{AXION_SCHEME}://{AXION_APP_AUTHORITY}/{}",
            request_path.trim_start_matches('/')
        ))
        .expect("Axion protocol URLs must be well-formed")
    }

    pub fn parse_request(&self, url: &Url) -> Result<ResourceRequest, ProtocolError> {
        if url.scheme() != AXION_SCHEME {
            return Err(ProtocolError::UnsupportedScheme {
                scheme: url.scheme().to_owned(),
            });
        }

        let authority = url.host_str().unwrap_or_default();
        if authority != AXION_APP_AUTHORITY {
            return Err(ProtocolError::UnsupportedAuthority {
                authority: authority.to_owned(),
            });
        }

        let path = self.normalize_request_path(url.path())?;

        Ok(ResourceRequest {
            scheme: AXION_SCHEME.to_owned(),
            authority: AXION_APP_AUTHORITY.to_owned(),
            path,
            headers: BTreeMap::new(),
        })
    }

    pub fn resolve_url(&self, url: &Url) -> Result<ResolvedAsset, ProtocolError> {
        let request = self.parse_request(url)?;
        self.resolve_request_path(&request.path)
    }

    pub fn resolve_request_path(&self, request_path: &str) -> Result<ResolvedAsset, ProtocolError> {
        let normalized = self.normalize_request_path(request_path)?;
        let file_path = self.frontend_dist.join(&normalized);

        Ok(ResolvedAsset {
            content_type: content_type_for_path(&file_path),
            file_path,
            request_path: normalized,
        })
    }

    pub fn resolve_existing_request_path(
        &self,
        request_path: &str,
    ) -> Result<ResolvedAsset, ProtocolError> {
        let resolved = self.resolve_request_path(request_path)?;
        reject_symlinked_path(&self.frontend_dist, &resolved.request_path)?;

        let metadata =
            fs::metadata(&resolved.file_path).map_err(|_| ProtocolError::MissingAsset {
                request_path: resolved.request_path.clone(),
                file_path: resolved.file_path.clone(),
            })?;

        if metadata.is_dir() {
            return Err(ProtocolError::AssetIsDirectory {
                request_path: resolved.request_path.clone(),
                file_path: resolved.file_path.clone(),
            });
        }

        Ok(resolved)
    }

    fn normalize_request_path(&self, request_path: &str) -> Result<String, ProtocolError> {
        let trimmed = request_path.trim();
        if trimmed.is_empty() || trimmed == "/" {
            return Ok(self.default_document.clone());
        }

        normalize_relative_path(Path::new(trimmed.trim_start_matches('/')))
    }
}

fn reject_symlinked_path(root: &Path, relative_path: &str) -> Result<(), ProtocolError> {
    let mut current = root.to_path_buf();
    reject_symlink(&current)?;

    for segment in relative_path.split('/') {
        current.push(segment);
        reject_symlink(&current)?;
    }

    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), ProtocolError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ProtocolError::MissingAsset {
        request_path: path.display().to_string(),
        file_path: path.to_path_buf(),
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ProtocolError::SymlinkNotAllowed {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<String, ProtocolError> {
    let mut segments = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(segment) => segments.push(segment.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ProtocolError::PathTraversal {
                    path: path.display().to_string(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ProtocolError::PathTraversal {
                    path: path.display().to_string(),
                });
            }
        }
    }

    let normalized = segments.join("/");
    if normalized.is_empty() {
        return Err(ProtocolError::InvalidUrl {
            value: path.display().to_string(),
        });
    }

    Ok(normalized)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("mjs") => "text/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn cache_control_for_asset(asset: &ResolvedAsset) -> &'static str {
    if asset.content_type.starts_with("text/html")
        || asset.content_type.starts_with("application/json")
        || asset.request_path == AXION_ASSET_MANIFEST_FILE_NAME
    {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    }
}

const AXION_ASSET_MANIFEST_FILE_NAME: &str = "axion-assets.json";

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        AXION_APP_AUTHORITY, AXION_SCHEME, AppAssetResolver, ProtocolError, ResourcePolicy,
        content_type_for_path, default_resource_policy_summary,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-protocol-{name}-{unique}-{serial}"))
    }

    #[test]
    fn resolver_uses_relative_default_document() {
        let resolver = AppAssetResolver::new(
            PathBuf::from("/tmp/frontend"),
            PathBuf::from("/tmp/frontend/index.html"),
        )
        .expect("resolver should build");

        assert_eq!(resolver.default_document(), "index.html");
        assert_eq!(resolver.initial_url().as_str(), "axion://app/index.html");
    }

    #[test]
    fn resolver_rejects_entry_outside_frontend_root() {
        let error = AppAssetResolver::new(
            PathBuf::from("/tmp/frontend"),
            PathBuf::from("/tmp/other/index.html"),
        )
        .expect_err("resolver should reject entry outside frontend root");

        assert!(matches!(
            error,
            ProtocolError::EntryOutsideFrontendDist { .. }
        ));
    }

    #[test]
    fn resolver_blocks_parent_directory_access() {
        let resolver = AppAssetResolver::new(
            PathBuf::from("/tmp/frontend"),
            PathBuf::from("/tmp/frontend/index.html"),
        )
        .expect("resolver should build");

        let error = resolver
            .resolve_request_path("../secrets.txt")
            .expect_err("path traversal must fail");

        assert!(matches!(error, ProtocolError::PathTraversal { .. }));
    }

    #[test]
    fn resolver_requires_existing_file_assets() {
        let frontend = temp_dir("existing-assets");
        fs::create_dir_all(frontend.join("nested")).unwrap();
        fs::write(frontend.join("index.html"), "<html>Hello</html>").unwrap();

        let resolver = AppAssetResolver::new(frontend.clone(), frontend.join("index.html"))
            .expect("resolver should build");

        let resolved = resolver
            .resolve_existing_request_path("/")
            .expect("default document should resolve");
        assert_eq!(resolved.request_path, "index.html");

        let missing = resolver
            .resolve_existing_request_path("/missing.js")
            .expect_err("missing asset should fail");
        assert!(matches!(missing, ProtocolError::MissingAsset { .. }));

        let directory = resolver
            .resolve_existing_request_path("/nested")
            .expect_err("directory asset should fail");
        assert!(matches!(directory, ProtocolError::AssetIsDirectory { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn resolver_rejects_symlinked_assets() {
        use std::os::unix::fs::symlink;

        let frontend = temp_dir("symlink-assets");
        let external = temp_dir("symlink-external");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(frontend.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(external.join("secret.txt"), "secret").unwrap();
        symlink(external.join("secret.txt"), frontend.join("secret.txt")).unwrap();

        let resolver = AppAssetResolver::new(frontend.clone(), frontend.join("index.html"))
            .expect("resolver should build");

        let error = resolver
            .resolve_existing_request_path("/secret.txt")
            .expect_err("symlinked asset should fail");

        assert!(matches!(error, ProtocolError::SymlinkNotAllowed { .. }));
    }

    #[test]
    fn resolver_parses_axion_urls() {
        let resolver = AppAssetResolver::new(
            PathBuf::from("/tmp/frontend"),
            PathBuf::from("/tmp/frontend/index.html"),
        )
        .expect("resolver should build");

        let request = resolver
            .parse_request(&resolver.initial_url())
            .expect("request should parse");

        assert_eq!(request.scheme, AXION_SCHEME);
        assert_eq!(request.authority, AXION_APP_AUTHORITY);
        assert_eq!(request.path, "index.html");
    }

    #[test]
    fn content_type_detection_covers_html_and_js() {
        assert_eq!(
            content_type_for_path(&PathBuf::from("index.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            content_type_for_path(&PathBuf::from("main.js")),
            "text/javascript; charset=utf-8"
        );
    }

    #[test]
    fn resource_policy_adds_security_and_cache_headers() {
        let frontend = temp_dir("resource-policy");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(frontend.join("index.html"), "<html>Hello</html>").unwrap();
        fs::write(frontend.join("app.js"), "console.log('hello');").unwrap();
        let resolver = AppAssetResolver::new(frontend.clone(), frontend.join("index.html"))
            .expect("resolver should build");
        let html = resolver
            .resolve_request_path("index.html")
            .expect("html should resolve");
        let script = resolver
            .resolve_request_path("app.js")
            .expect("script should resolve");

        let html_policy = ResourcePolicy::for_asset(&html);
        let script_policy = ResourcePolicy::for_asset(&script);

        assert_eq!(
            html_policy.header_value("content-type"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(html_policy.header_value("cache-control"), Some("no-cache"));
        assert_eq!(
            script_policy.header_value("cache-control"),
            Some("public, max-age=31536000, immutable")
        );
        assert_eq!(
            script_policy.header_value("x-content-type-options"),
            Some("nosniff")
        );
        assert_eq!(
            script_policy.header_value("referrer-policy"),
            Some("no-referrer")
        );
        assert_eq!(
            script_policy.header_value("cross-origin-resource-policy"),
            Some("same-origin")
        );
        assert!(default_resource_policy_summary().contains("static-assets"));
    }
}
