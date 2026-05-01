use axion_runtime::json_string_literal;

pub fn json_array_section<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let key_index = source.find(key)?;
    let array_start = source[key_index..].find('[')? + key_index;
    let array_end = matching_json_delimiter(source, array_start, '[', ']')?;
    source.get(array_start + 1..array_end)
}

pub fn json_object_section<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let key_index = source.find(key)?;
    let object_start = source[key_index..].find('{')? + key_index;
    let object_end = matching_json_delimiter(source, object_start, '{', '}')?;
    source.get(object_start..=object_end)
}

pub fn next_json_object(source: &str, start: usize) -> Option<(&str, usize)> {
    let object_start = source.get(start..)?.find('{')? + start;
    let object_end = matching_json_delimiter(source, object_start, '{', '}')?;
    Some((source.get(object_start..=object_end)?, object_end + 1))
}

pub fn matching_json_delimiter(
    source: &str,
    start: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in source
        .char_indices()
        .skip_while(|(index, _)| *index < start)
    {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        if character == '"' {
            in_string = true;
        } else if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }

    None
}

pub fn optional_json_string_field(source: &str, field: &str) -> Option<String> {
    if source.contains(&format!("\"{field}\":null")) {
        None
    } else {
        json_string_field(source, field)
    }
}

pub fn json_string_field(source: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\":\"");
    let start = source.find(&key)? + key.len();
    let mut value = String::new();
    let mut escaped = false;
    for character in source[start..].chars() {
        if escaped {
            value.push(match character {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(value);
        } else {
            value.push(character);
        }
    }
    None
}

pub fn json_string_fields(source: &str, field: &str) -> Vec<String> {
    let key = format!("\"{field}\":\"");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find(&key) {
        let start = cursor + relative_start + key.len();
        let mut value = String::new();
        let mut escaped = false;
        let mut end = start;
        for (offset, character) in source[start..].char_indices() {
            end = start + offset + character.len_utf8();
            if escaped {
                value.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                if !values.contains(&value) {
                    values.push(value);
                }
                break;
            } else {
                value.push(character);
            }
        }
        cursor = end;
    }
    values
}

pub fn json_string_array_values(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find('"') {
        let start = cursor + relative_start + 1;
        let mut value = String::new();
        let mut escaped = false;
        let mut end = start;
        for (offset, character) in source[start..].char_indices() {
            end = start + offset + character.len_utf8();
            if escaped {
                value.push(match character {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                values.push(value);
                break;
            } else {
                value.push(character);
            }
        }
        cursor = end;
    }
    values
}

pub fn json_bool_field(source: &str, field: &str) -> Option<bool> {
    let key = format!("\"{field}\":");
    let start = source.find(&key)? + key.len();
    if source[start..].starts_with("true") {
        Some(true)
    } else if source[start..].starts_with("false") {
        Some(false)
    } else {
        None
    }
}

pub fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

pub fn json_string_array_literal(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

#[cfg(test)]
mod tests {
    use super::{
        json_array_section, json_object_section, json_string_array_values, json_string_field,
        json_string_fields, next_json_object,
    };

    #[test]
    fn sections_ignore_delimiters_inside_strings() {
        let source = r#"{"outer":{"message":"not } done","items":[{"name":"a]"}]}}"#;

        assert_eq!(
            json_object_section(source, "\"outer\""),
            Some(r#"{"message":"not } done","items":[{"name":"a]"}]}"#)
        );
        assert_eq!(
            json_array_section(source, "\"items\""),
            Some(r#"{"name":"a]"}"#)
        );
    }

    #[test]
    fn string_helpers_decode_escaped_values() {
        let source = r#"{"message":"line\nnext","items":["one","two\"quoted"]}"#;

        assert_eq!(
            json_string_field(source, "message"),
            Some("line\nnext".to_owned())
        );
        assert_eq!(
            json_string_array_values(json_array_section(source, "\"items\"").unwrap()),
            vec!["one".to_owned(), "two\"quoted".to_owned()]
        );
    }

    #[test]
    fn next_json_object_walks_arrays() {
        let source = r#"{"id":"one"},{"id":"two","detail":{"code":"x"}}"#;

        let (first, cursor) = next_json_object(source, 0).unwrap();
        let (second, _) = next_json_object(source, cursor).unwrap();

        assert_eq!(json_string_fields(first, "id"), vec!["one".to_owned()]);
        assert_eq!(json_string_fields(second, "id"), vec!["two".to_owned()]);
        assert_eq!(json_string_fields(second, "code"), vec!["x".to_owned()]);
    }
}
