use axion_runtime::json_string_literal;
use serde_json::Value;

pub fn json_string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(str::to_owned)
}

pub fn json_string_fields(value: &Value, field: &str) -> Vec<String> {
    let mut values = Vec::new();
    collect_string_fields(value, field, &mut values);
    values
}

fn collect_string_fields(value: &Value, field: &str, values: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(value) = object.get(field).and_then(Value::as_str) {
                if !values.iter().any(|candidate| candidate == value) {
                    values.push(value.to_owned());
                }
            }
            for child in object.values() {
                collect_string_fields(child, field, values);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_string_fields(child, field, values);
            }
        }
        _ => {}
    }
}

pub fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

pub fn json_string_array_literal(values: &[String]) -> String {
    serde_json::to_string(values).expect("strings serialize to JSON")
}
