use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;

pub fn utc_now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_rfc3339(&s.replace("Z", "+00:00")))
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn fmt_delta(seconds: f64) -> String {
    let secs = seconds as i64;
    if secs <= 0 {
        return "CLAIMABLE".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {:02}m", h, m)
    } else {
        format!("{}m {:02}s", m, s)
    }
}

pub fn fmt_clock(dt: &Option<DateTime<Utc>>) -> String {
    match dt {
        Some(d) => d.with_timezone(&chrono::Local).format("%H:%M:%S").to_string(),
        None => "?".to_string(),
    }
}

pub fn safe_int(s: &str) -> Option<i64> {
    s.parse::<i64>().ok()
}

pub fn read_csv(path: &Path) -> Vec<Vec<String>> {
    let data = fs::read_to_string(path).unwrap_or_default();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        if let Ok(record) = result {
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }
    }
    rows
}

pub fn parse_jsonish_list(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Null => vec![],
        serde_json::Value::Array(arr) => {
            arr.iter().filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            }).collect()
        }
        serde_json::Value::Number(n) => vec![n.to_string()],
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() { return vec![]; }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                match parsed {
                    serde_json::Value::Array(arr) => {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    }
                    other => vec![other.to_string()],
                }
            } else {
                vec![s.to_string()]
            }
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fmt_delta_claimable() {
        assert_eq!(fmt_delta(0.0), "CLAIMABLE");
        assert_eq!(fmt_delta(-5.0), "CLAIMABLE");
    }

    #[test]
    fn fmt_delta_minutes_and_seconds() {
        assert_eq!(fmt_delta(90.0), "1m 30s");
        assert_eq!(fmt_delta(65.0), "1m 05s");
    }

    #[test]
    fn fmt_delta_hours_and_minutes() {
        assert_eq!(fmt_delta(3661.0), "1h 01m");
        assert_eq!(fmt_delta(7200.0), "2h 00m");
    }

    #[test]
    fn safe_int_valid() {
        assert_eq!(safe_int("42"), Some(42));
        assert_eq!(safe_int("-1"), Some(-1));
        assert_eq!(safe_int("0"), Some(0));
    }

    #[test]
    fn safe_int_invalid() {
        assert_eq!(safe_int("abc"), None);
        assert_eq!(safe_int(""), None);
        assert_eq!(safe_int("3.14"), None);
    }

    #[test]
    fn parse_jsonish_list_null() {
        assert_eq!(parse_jsonish_list(&json!(null)), Vec::<String>::new());
    }

    #[test]
    fn parse_jsonish_list_array() {
        assert_eq!(parse_jsonish_list(&json!(["a", "b", "c"])), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_jsonish_list_number() {
        assert_eq!(parse_jsonish_list(&json!(42)), vec!["42"]);
    }

    #[test]
    fn parse_jsonish_list_string_plain() {
        assert_eq!(parse_jsonish_list(&json!("hello")), vec!["hello"]);
    }

    #[test]
    fn parse_jsonish_list_string_json_array() {
        assert_eq!(parse_jsonish_list(&json!("[\"x\",\"y\"]")), vec!["x", "y"]);
    }

    #[test]
    fn parse_jsonish_list_string_empty() {
        assert_eq!(parse_jsonish_list(&json!("")), Vec::<String>::new());
    }

    #[test]
    fn parse_jsonish_list_bool() {
        assert_eq!(parse_jsonish_list(&json!(true)), Vec::<String>::new());
    }

    #[test]
    fn parse_dt_valid() {
        let dt = parse_dt("2025-01-15T10:30:00Z");
        assert!(dt.is_some());
    }

    #[test]
    fn parse_dt_empty() {
        assert!(parse_dt("").is_none());
    }

    #[test]
    fn utc_now_iso_non_empty() {
        let s = utc_now_iso();
        assert!(!s.is_empty());
    }
}
