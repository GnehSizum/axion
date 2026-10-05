use axion_core::DialogBackendConfig;

use crate::{
    dialog_error, dialog_filters_field, json_bool_field, json_string_field, json_string_literal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogBackendKind {
    Headless,
    System,
    SystemUnavailable,
}

impl DialogBackendKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Headless => "headless",
            Self::System => "system",
            Self::SystemUnavailable => "system-unavailable",
        }
    }

    pub const fn resolve_for_current_platform(self) -> Self {
        match self {
            Self::System => {
                #[cfg(target_os = "macos")]
                {
                    Self::System
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Self::SystemUnavailable
                }
            }
            other => other,
        }
    }
}

impl From<DialogBackendConfig> for DialogBackendKind {
    fn from(value: DialogBackendConfig) -> Self {
        match value {
            DialogBackendConfig::Headless => Self::Headless,
            DialogBackendConfig::System => Self::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRequestKind {
    Open,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogRequest {
    pub kind: DialogRequestKind,
    pub title: Option<String>,
    pub default_path: Option<std::path::PathBuf>,
    pub directory: bool,
    pub multiple: bool,
    pub filters: Vec<DialogFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogResponse {
    pub canceled: bool,
    pub path: Option<std::path::PathBuf>,
    pub paths: Option<Vec<std::path::PathBuf>>,
    pub backend: DialogBackendKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogRequestError {
    InvalidPayload { message: String },
}

impl std::fmt::Display for DialogRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPayload { message } => {
                write!(formatter, "{}", dialog_error("invalid-payload", message))
            }
        }
    }
}

impl std::error::Error for DialogRequestError {}

impl DialogRequest {
    pub(crate) fn from_payload(
        kind: DialogRequestKind,
        payload: &str,
    ) -> Result<Self, DialogRequestError> {
        let request = Self {
            kind,
            title: json_string_field(payload, "title"),
            default_path: json_string_field(payload, "defaultPath").map(std::path::PathBuf::from),
            directory: json_bool_field(payload, "directory").unwrap_or(false),
            multiple: json_bool_field(payload, "multiple").unwrap_or(false),
            filters: dialog_filters_field(payload, "filters")?,
        };
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), DialogRequestError> {
        if matches!(self.kind, DialogRequestKind::Save) && self.directory {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog.save does not support 'directory=true'".to_owned(),
            });
        }
        if matches!(self.kind, DialogRequestKind::Save) && self.multiple {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog.save does not support 'multiple=true'".to_owned(),
            });
        }
        if self
            .filters
            .iter()
            .any(|filter| filter.name.trim().is_empty())
        {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog filters require a non-empty 'name'".to_owned(),
            });
        }
        if self.filters.iter().any(|filter| {
            filter.extensions.is_empty()
                || filter.extensions.iter().any(|ext| ext.trim().is_empty())
        }) {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog filters require at least one non-empty extension".to_owned(),
            });
        }
        Ok(())
    }
}

impl DialogResponse {
    fn canceled(backend: DialogBackendKind) -> Self {
        Self {
            canceled: true,
            path: None,
            paths: None,
            backend,
        }
    }

    #[cfg(target_os = "macos")]
    fn selected(path: impl Into<std::path::PathBuf>, backend: DialogBackendKind) -> Self {
        let path = path.into();
        Self {
            canceled: false,
            path: Some(path),
            paths: None,
            backend,
        }
    }

    #[cfg(target_os = "macos")]
    fn selected_multiple(paths: Vec<std::path::PathBuf>, backend: DialogBackendKind) -> Self {
        let path = paths.first().cloned();
        Self {
            canceled: false,
            path,
            paths: Some(paths),
            backend,
        }
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"canceled\":{},\"path\":{},\"paths\":{},\"backend\":{}}}",
            self.canceled,
            self.path
                .as_ref()
                .and_then(|path| path.to_str())
                .map(json_string_literal)
                .unwrap_or_else(|| "null".to_owned()),
            self.paths
                .as_ref()
                .map(|paths| {
                    let entries = paths
                        .iter()
                        .filter_map(|path| path.to_str())
                        .map(json_string_literal)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("[{entries}]")
                })
                .unwrap_or_else(|| "null".to_owned()),
            json_string_literal(self.backend.as_str()),
        )
    }
}

pub fn execute_dialog_request(
    backend: DialogBackendKind,
    request: DialogRequest,
) -> DialogResponse {
    match backend {
        DialogBackendKind::Headless => DialogResponse::canceled(DialogBackendKind::Headless),
        DialogBackendKind::SystemUnavailable => {
            DialogResponse::canceled(DialogBackendKind::SystemUnavailable)
        }
        DialogBackendKind::System => execute_system_dialog_request(request),
    }
}

fn execute_system_dialog_request(request: DialogRequest) -> DialogResponse {
    #[cfg(target_os = "macos")]
    {
        execute_macos_dialog_request(request)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        DialogResponse::canceled(DialogBackendKind::SystemUnavailable)
    }
}

#[cfg(target_os = "macos")]
fn execute_macos_dialog_request(request: DialogRequest) -> DialogResponse {
    let script = macos_dialog_script(&request);
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();
    let Ok(output) = output else {
        return DialogResponse::canceled(DialogBackendKind::SystemUnavailable);
    };

    if !output.status.success() {
        return DialogResponse::canceled(DialogBackendKind::System);
    }

    let paths = parse_macos_dialog_paths(&output.stdout);
    if paths.is_empty() {
        DialogResponse::canceled(DialogBackendKind::System)
    } else if request.multiple {
        DialogResponse::selected_multiple(paths, DialogBackendKind::System)
    } else {
        DialogResponse::selected(paths[0].clone(), DialogBackendKind::System)
    }
}

#[cfg(target_os = "macos")]
fn macos_dialog_script(request: &DialogRequest) -> String {
    let prompt = request
        .title
        .as_deref()
        .map(applescript_string_literal)
        .map(|title| format!(" with prompt {title}"))
        .unwrap_or_default();
    let default_location = request
        .default_path
        .as_ref()
        .and_then(|path| path.parent())
        .and_then(|path| path.to_str())
        .map(applescript_string_literal)
        .map(|path| format!(" default location POSIX file {path}"))
        .unwrap_or_default();

    let command = match request.kind {
        DialogRequestKind::Open if request.directory => {
            let multiple = if request.multiple {
                " with multiple selections allowed"
            } else {
                ""
            };
            format!("my axionJoinPaths(choose folder{prompt}{default_location}{multiple})")
        }
        DialogRequestKind::Open => {
            let multiple = if request.multiple {
                " with multiple selections allowed"
            } else {
                ""
            };
            format!("my axionJoinPaths(choose file{prompt}{default_location}{multiple})")
        }
        DialogRequestKind::Save => {
            let default_name = request
                .default_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|file_name| file_name.to_str())
                .map(applescript_string_literal)
                .map(|name| format!(" default name {name}"))
                .unwrap_or_default();
            format!("my axionJoinPaths(choose file name{prompt}{default_name}{default_location})")
        }
    };

    format!(
        "{command}\n\
        on axionJoinPaths(selectionResult)\n\
            if class of selectionResult is list then\n\
                set joinedPaths to \"\"\n\
                repeat with selectedItem in selectionResult\n\
                    set joinedPaths to joinedPaths & POSIX path of selectedItem & linefeed\n\
                end repeat\n\
                return joinedPaths\n\
            end if\n\
            return POSIX path of selectionResult\n\
        end axionJoinPaths"
    )
}

#[cfg(target_os = "macos")]
fn parse_macos_dialog_paths(stdout: &[u8]) -> Vec<std::path::PathBuf> {
    String::from_utf8_lossy(stdout)
        .trim()
        .split('\n')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(entry))
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn applescript_string_literal(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
