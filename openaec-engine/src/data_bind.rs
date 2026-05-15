//! Data binding — resolve dot-notation paths and format values.

use serde_json::Value;

/// Resolve a dot-notation bind path against JSON data.
///
/// Examples:
/// - "client.name" → data["client"]["name"]
/// - "_static.Label Text" → returns "Label Text" literally
/// - "_page_number" → returns None (handled separately by zone renderer)
pub fn resolve_bind(data: &Value, path: &str) -> Option<String> {
    // Static text: "_static.My Label"
    if let Some(label) = path.strip_prefix("_static.") {
        return Some(label.to_string());
    }

    // Page number placeholder — handled at render time
    if path == "_page_number" {
        return None;
    }

    // Dot-notation traversal
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;
    for part in &parts {
        match current {
            Value::Object(map) => {
                current = map.get(*part)?;
            }
            Value::Array(arr) => {
                let idx: usize = part.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }

    Some(value_to_string(current))
}

/// Resolve a bind path and return the raw Value (for arrays/objects).
pub fn resolve_bind_value<'a>(data: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;
    for part in &parts {
        match current {
            Value::Object(map) => {
                current = map.get(*part)?;
            }
            Value::Array(arr) => {
                let idx: usize = part.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Convert a JSON value to display string.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

/// Format a value with optional format specifier.
///
/// Formats:
/// - "currency_nl": Dutch currency format (€1.234,56)
/// - None: default string conversion
pub fn format_value(value: &str, format: Option<&str>) -> String {
    match format {
        Some("currency_nl") => format_currency_nl(value),
        _ => value.to_string(),
    }
}

/// Dutch currency formatting: "1234.56" → "€ 1.234,56"
fn format_currency_nl(value: &str) -> String {
    let num: f64 = match value.replace(',', ".").parse() {
        Ok(n) => n,
        Err(_) => return value.to_string(),
    };

    let is_negative = num < 0.0;
    let abs = num.abs();
    let cents = ((abs * 100.0).round() as i64) % 100;
    let euros = (abs * 100.0).round() as i64 / 100;

    // Format with dots as thousands separator
    let euro_str = {
        let s = euros.to_string();
        let mut result = String::new();
        for (i, ch) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push('.');
            }
            result.push(ch);
        }
        result.chars().rev().collect::<String>()
    };

    let sign = if is_negative { "-" } else { "" };
    format!("{}€ {},{:02}", sign, euro_str, cents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_simple() {
        let data = json!({"project": "Test Project"});
        assert_eq!(resolve_bind(&data, "project"), Some("Test Project".into()));
    }

    #[test]
    fn test_resolve_nested() {
        let data = json!({"client": {"name": "Acme", "address": "Straat 1"}});
        assert_eq!(resolve_bind(&data, "client.name"), Some("Acme".into()));
        assert_eq!(resolve_bind(&data, "client.address"), Some("Straat 1".into()));
    }

    #[test]
    fn test_resolve_missing() {
        let data = json!({"project": "Test"});
        assert_eq!(resolve_bind(&data, "nonexistent"), None);
        assert_eq!(resolve_bind(&data, "project.sub"), None);
    }

    #[test]
    fn test_resolve_static() {
        let data = json!({});
        assert_eq!(resolve_bind(&data, "_static.Label"), Some("Label".into()));
        assert_eq!(resolve_bind(&data, "_static.Locatie:"), Some("Locatie:".into()));
    }

    #[test]
    fn test_resolve_page_number() {
        let data = json!({});
        assert_eq!(resolve_bind(&data, "_page_number"), None);
    }

    #[test]
    fn test_resolve_number() {
        let data = json!({"count": 42});
        assert_eq!(resolve_bind(&data, "count"), Some("42".into()));
    }

    #[test]
    fn test_format_currency_nl() {
        assert_eq!(format_currency_nl("1234.56"), "€ 1.234,56");
        assert_eq!(format_currency_nl("0"), "€ 0,00");
        assert_eq!(format_currency_nl("999999.99"), "€ 999.999,99");
        assert_eq!(format_currency_nl("-50"), "-€ 50,00");
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value("1234.56", Some("currency_nl")), "€ 1.234,56");
        assert_eq!(format_value("hello", None), "hello");
    }

    #[test]
    fn test_resolve_array_index() {
        let data = json!({"items": ["a", "b", "c"]});
        assert_eq!(resolve_bind(&data, "items.1"), Some("b".into()));
    }
}
