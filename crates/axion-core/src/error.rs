use thiserror::Error;

#[derive(Debug, Error)]
pub enum AxionError {
    #[error("app name must not be empty")]
    MissingAppName,
    #[error("build configuration is required")]
    MissingBuildConfig,
    #[error("at least one window must be configured")]
    MissingWindow,
    #[error("window id must not be empty")]
    InvalidWindowId,
    #[error("duplicate window id '{window_id}'")]
    DuplicateWindowId { window_id: String },
    #[error("window '{window_id}' must have a non-empty title")]
    InvalidWindowTitle { window_id: String },
    #[error("window '{window_id}' must have a non-zero width and height")]
    InvalidWindowSize { window_id: String },
    #[error("capabilities reference unknown window id '{window_id}'")]
    UnknownCapabilityWindow { window_id: String },
    #[error("build frontend_dist must not be empty")]
    MissingFrontendDist,
    #[error("build entry must not be empty")]
    MissingBuildEntry,
}
