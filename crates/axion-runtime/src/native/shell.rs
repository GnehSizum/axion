use crate::{json_string_field, json_string_literal, shell_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellOpenRequest {
    pub(crate) target: String,
    pub(crate) scheme: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShellOpenRequestError {
    InvalidPayload { message: String },
    InvalidTarget { message: String },
    UnsupportedTarget { message: String },
    OpenFailed { message: String },
}

impl std::fmt::Display for ShellOpenRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPayload { message } => {
                write!(formatter, "{}", shell_error("invalid-payload", message))
            }
            Self::InvalidTarget { message } => {
                write!(formatter, "{}", shell_error("invalid-target", message))
            }
            Self::UnsupportedTarget { message } => {
                write!(formatter, "{}", shell_error("unsupported-target", message))
            }
            Self::OpenFailed { message } => {
                write!(formatter, "{}", shell_error("open-failed", message))
            }
        }
    }
}

impl std::error::Error for ShellOpenRequestError {}

impl ShellOpenRequest {
    pub(crate) fn from_payload(payload: &str) -> Result<Self, ShellOpenRequestError> {
        let target = json_string_field(payload, "target").ok_or_else(|| {
            ShellOpenRequestError::InvalidPayload {
                message: "shell.open requires a JSON string field named 'target'".to_owned(),
            }
        })?;
        Self::from_target(&target)
    }

    pub(crate) fn from_target(target: &str) -> Result<Self, ShellOpenRequestError> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return Err(ShellOpenRequestError::InvalidTarget {
                message: "shell.open target must not be empty".to_owned(),
            });
        }
        if trimmed != target || trimmed.chars().any(char::is_control) {
            return Err(ShellOpenRequestError::InvalidTarget {
                message: "shell.open target must be a clean URL string".to_owned(),
            });
        }
        if trimmed.len() > 2048 {
            return Err(ShellOpenRequestError::InvalidTarget {
                message: "shell.open target is too long".to_owned(),
            });
        }

        let url =
            url::Url::parse(trimmed).map_err(|error| ShellOpenRequestError::InvalidTarget {
                message: format!("shell.open target must be an absolute URL: {error}"),
            })?;
        let scheme = url.scheme().to_ascii_lowercase();
        match scheme.as_str() {
            "http" | "https" => {
                if url.host_str().is_none() {
                    return Err(ShellOpenRequestError::InvalidTarget {
                        message: "shell.open http(s) URLs require a host".to_owned(),
                    });
                }
            }
            "mailto" => {
                if url.path().trim().is_empty() {
                    return Err(ShellOpenRequestError::InvalidTarget {
                        message: "shell.open mailto URLs require a recipient".to_owned(),
                    });
                }
            }
            _ => {
                return Err(ShellOpenRequestError::UnsupportedTarget {
                    message: "shell.open only supports http, https, and mailto URLs".to_owned(),
                });
            }
        }

        Ok(Self {
            target: url.as_str().to_owned(),
            scheme,
        })
    }
}

pub(crate) fn execute_shell_open_request(request: ShellOpenRequest) -> Result<String, String> {
    let command = platform_shell_open_command(&request.target).ok_or_else(|| {
        ShellOpenRequestError::OpenFailed {
            message: "shell.open is not available on this platform".to_owned(),
        }
        .to_string()
    })?;

    let child = std::process::Command::new(&command.program)
        .args(&command.args)
        .spawn()
        .map_err(|error| {
            ShellOpenRequestError::OpenFailed {
                message: format!("failed to launch platform opener: {error}"),
            }
            .to_string()
        })?;

    let pid = child.id();
    drop(reap_child_in_background(child));
    Ok(format!(
        "{{\"opened\":true,\"target\":{},\"scheme\":{},\"backend\":{},\"pid\":{}}}",
        json_string_literal(&request.target),
        json_string_literal(&request.scheme),
        json_string_literal(&command.backend),
        pid,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellOpenCommand {
    program: String,
    args: Vec<String>,
    backend: String,
}

fn platform_shell_open_command(target: &str) -> Option<ShellOpenCommand> {
    #[cfg(target_os = "macos")]
    {
        Some(ShellOpenCommand {
            program: "open".to_owned(),
            args: vec![target.to_owned()],
            backend: "open".to_owned(),
        })
    }

    #[cfg(target_os = "windows")]
    {
        Some(ShellOpenCommand {
            program: "rundll32".to_owned(),
            args: vec!["url.dll,FileProtocolHandler".to_owned(), target.to_owned()],
            backend: "rundll32".to_owned(),
        })
    }

    #[cfg(all(
        not(target_os = "macos"),
        not(target_os = "windows"),
        any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")
    ))]
    {
        Some(ShellOpenCommand {
            program: "xdg-open".to_owned(),
            args: vec![target.to_owned()],
            backend: "xdg-open".to_owned(),
        })
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd"
    )))]
    {
        let _ = target;
        None
    }
}

fn reap_child_in_background(
    mut child: std::process::Child,
) -> std::thread::JoinHandle<std::io::Result<std::process::ExitStatus>> {
    std::thread::spawn(move || child.wait())
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::reap_child_in_background;
    use std::process::Command;

    #[test]
    fn background_reaper_waits_for_successful_and_failed_children() {
        for code in [0, 7] {
            #[cfg(unix)]
            let child = Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("exit {code}"))
                .spawn()
                .unwrap();
            #[cfg(windows)]
            let child = Command::new("cmd")
                .args(["/C", &format!("exit {code}")])
                .spawn()
                .unwrap();
            let pid = child.id();
            let status = reap_child_in_background(child).join().unwrap().unwrap();
            assert_eq!(status.code(), Some(code));
            #[cfg(unix)]
            {
                let state = Command::new("/bin/ps")
                    .args(["-p", &pid.to_string(), "-o", "stat="])
                    .output()
                    .unwrap();
                assert!(state.status.success() || state.status.code() == Some(1));
                assert!(
                    state.stderr.is_empty(),
                    "{}",
                    String::from_utf8_lossy(&state.stderr)
                );
                assert!(
                    state.stdout.is_empty(),
                    "child {pid} remains after wait: {}",
                    String::from_utf8_lossy(&state.stdout)
                );
            }
            #[cfg(windows)]
            let _ = pid;
        }
    }
}
