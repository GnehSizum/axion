use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeError {
    pub code: String,
    pub message: String,
}
impl BridgeError {
    pub fn new(code: impl Into<String>, detail: impl AsRef<str>) -> Self {
        let code = code.into();
        let message = format!("{code}: {}", detail.as_ref());
        Self { code, message }
    }
    /// Preserve the existing string message while adding a structured code.
    pub fn from_legacy(message: &str) -> Self {
        Self::from_legacy_with_fallback(message, "bridge.handler-error")
    }

    pub fn from_legacy_with_fallback(message: &str, fallback_code: &str) -> Self {
        let code = message
            .split_once(": ")
            .map(|(code, _)| code)
            .filter(|code| {
                code.contains('.')
                    && code.split('.').all(|part| {
                        !part.is_empty()
                            && part.as_bytes()[0].is_ascii_lowercase()
                            && part
                                .chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    })
            })
            .unwrap_or(fallback_code);
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for BridgeError {}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_does_not_override_a_valid_legacy_code() {
        assert_eq!(
            BridgeError::from_legacy_with_fallback(
                "window.control-timeout: late",
                "bridge.bad-request"
            )
            .code,
            "window.control-timeout"
        );
        for message in [
            "unclassified",
            "fs.not-found:missing",
            "bad..code: detail",
            "Bad.code: detail",
        ] {
            let error = BridgeError::from_legacy_with_fallback(message, "bridge.forbidden");
            assert_eq!(error.code, "bridge.forbidden");
            assert_eq!(error.message, message);
        }
    }

    #[test]
    fn legacy_error_keeps_message_and_stable_code() {
        let error = BridgeError::from_legacy("fs.not-found: missing");
        assert_eq!(error.code, "fs.not-found");
        assert_eq!(error.to_string(), "fs.not-found: missing");
        assert_eq!(
            BridgeError::from_legacy("unclassified").code,
            "bridge.handler-error"
        );
    }
}
