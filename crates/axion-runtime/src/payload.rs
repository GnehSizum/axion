use super::{DialogFilter, DialogRequestError};
use serde::Deserialize;
use serde_json::Value;

fn field(payload: &str, name: &str) -> Option<Value> {
    serde_json::from_str::<serde_json::Map<String, Value>>(payload)
        .ok()?
        .remove(name)
}
pub(super) fn json_string_field(payload: &str, name: &str) -> Option<String> {
    field(payload, name)?.as_str().map(str::to_owned)
}
pub(super) fn json_bool_field(payload: &str, name: &str) -> Option<bool> {
    field(payload, name)?.as_bool()
}
pub(super) fn json_u32_field(payload: &str, name: &str) -> Option<u32> {
    field(payload, name)?.as_u64()?.try_into().ok()
}
#[derive(Deserialize)]
struct Filter {
    name: String,
    extensions: Vec<String>,
}
pub(super) fn dialog_filters_field(
    payload: &str,
    name: &str,
) -> Result<Vec<DialogFilter>, DialogRequestError> {
    let Some(value) = field(payload, name).filter(|v| !v.is_null()) else {
        return Ok(Vec::new());
    };
    let filters: Vec<Filter> = serde_json::from_value(value).map_err(|error| DialogRequestError::InvalidPayload {
        message: format!("dialog filters require objects with string name and string-array extensions: {error}"),
    })?;
    Ok(filters
        .into_iter()
        .map(|f| DialogFilter {
            name: f.name,
            extensions: f.extensions,
        })
        .collect())
}

#[derive(Default, Deserialize)]
#[allow(dead_code)]
struct FsPayload {
    path: Option<String>,
    contents: Option<String>,
    recursive: Option<bool>,
}
#[derive(Default, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct WindowPayload {
    target: Option<String>,
    title: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    request_id: Option<String>,
}
#[derive(Default, Deserialize)]
#[allow(dead_code)]
struct ClipboardPayload {
    text: Option<String>,
}
#[derive(Default, Deserialize)]
#[allow(dead_code)]
struct ShellPayload {
    target: Option<String>,
}
#[derive(Default, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct DialogPayload {
    title: Option<String>,
    default_path: Option<String>,
    directory: Option<bool>,
    multiple: Option<bool>,
    filters: Option<Vec<Filter>>,
}

/// Validate native field types before any host side effects. Unknown fields remain compatible.
pub(super) fn validate_native_payload(command: &str, payload: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| format!("bridge.invalid-payload: {error}"))?;
    let value = if value.is_null() {
        Value::Object(Default::default())
    } else {
        value
    };
    let family = command.split('.').next().unwrap_or("bridge");
    let result = match family {
        "fs" => serde_json::from_value::<FsPayload>(value).map(|_| ()),
        "window" => serde_json::from_value::<WindowPayload>(value).map(|_| ()),
        "clipboard" => serde_json::from_value::<ClipboardPayload>(value).map(|_| ()),
        "shell" => serde_json::from_value::<ShellPayload>(value).map(|_| ()),
        "dialog" => serde_json::from_value::<DialogPayload>(value).map(|_| ()),
        _ => return Ok(()),
    };
    result.map_err(|error| format!("{family}.invalid-payload: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fields_are_top_level_and_strings_decode_unicode() {
        assert_eq!(
            json_string_field(
                r#"{"nested":{"path":"wrong.txt"},"path":"right.txt"}"#,
                "path"
            )
            .as_deref(),
            Some("right.txt")
        );
        assert_eq!(
            json_string_field(r#"{"path":"\u0061-\uD83D\uDE00.txt"}"#, "path").as_deref(),
            Some("a-😀.txt")
        );
        assert_eq!(
            json_bool_field(
                r#"{"nested":{"recursive":true},"recursive":false}"#,
                "recursive"
            ),
            Some(false)
        );
    }
    #[test]
    fn integers_do_not_accept_prefixes_fractions_or_overflow() {
        for payload in [
            r#"{"width":1.5}"#,
            r#"{"width":-1}"#,
            r#"{"width":4294967296}"#,
            r#"{"width":1e1}"#,
            r#"{"width":"1"}"#,
        ] {
            assert_eq!(json_u32_field(payload, "width"), None);
            assert!(validate_native_payload("window.set_size", payload).is_err());
        }
        assert_eq!(json_u32_field(r#"{"width":100}"#, "width"), Some(100));
    }
    #[test]
    fn invalid_types_fail_before_native_defaults_are_applied() {
        assert!(
            validate_native_payload("fs.remove", r#"{"path":"a","recursive":"false"}"#).is_err()
        );
        assert!(validate_native_payload("dialog.open", r#"{"multiple":"true"}"#).is_err());
        assert!(
            validate_native_payload("fs.write_text", r#"{"path":"a" "contents":"b"}"#).is_err()
        );
        assert!(validate_native_payload("window.info", "null").is_ok());
        assert!(
            validate_native_payload(
                "dialog.open",
                r#"{"filters":[{"name":"Text","extensions":["txt"]}]}"#
            )
            .is_ok()
        );
    }
    #[test]
    fn duplicate_fields_have_explicit_last_value_semantics() {
        assert_eq!(
            json_string_field(r#"{"path":"a","path":"b"}"#, "path").as_deref(),
            Some("b")
        );
        assert_eq!(
            json_bool_field(r#"{"recursive":true,"recursive":false}"#, "recursive"),
            Some(false)
        );
    }
}
