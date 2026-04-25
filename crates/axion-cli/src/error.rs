#[derive(Debug, thiserror::Error)]
pub enum AxionCliError {
    #[error(transparent)]
    Core(#[from] axion_core::AxionError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Manifest(#[from] axion_manifest::ManifestError),
    #[error(transparent)]
    Packager(#[from] axion_packager::PackagerError),
    #[error(transparent)]
    Runtime(#[from] axion_runtime::RuntimeError),
}
