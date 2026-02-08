use crate::error::{Result, RuntimeError};
use crate::value::Value;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "now" => stdlib_now(),
        "now_ms" => stdlib_now_ms(),
        "sleep" => stdlib_sleep(args),
        "measure" => Err(RuntimeError::CallError(
            "std::time::measure requires VM context for closure execution. \
             Use `let start = std::time::now_ms(); ... let elapsed = std::time::now_ms() - start;` instead."
                .to_string(),
        )),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::time::{}",
            name
        ))),
    }
}

fn stdlib_now() -> Result<Value> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let iso = epoch_to_iso8601(duration.as_secs(), duration.subsec_millis());
    Ok(Value::String(iso))
}

fn stdlib_now_ms() -> Result<Value> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok(Value::Int(duration.as_millis() as i64))
}

fn stdlib_sleep(args: Vec<Value>) -> Result<Value> {
    let ms = match args.first() {
        Some(Value::Int(n)) => *n,
        Some(other) => {
            return Err(RuntimeError::TypeError(format!(
                "std::time::sleep expected Int, got {}",
                other.type_name()
            )))
        }
        None => {
            return Err(RuntimeError::TypeError(
                "std::time::sleep missing argument".to_string(),
            ))
        }
    };
    std::thread::sleep(Duration::from_millis(ms.max(0) as u64));
    Ok(Value::Nil)
}

/// Convert Unix epoch seconds + millis to ISO 8601 UTC string.
/// Format: YYYY-MM-DDTHH:MM:SS.mmmZ
fn epoch_to_iso8601(epoch_secs: u64, millis: u32) -> String {
    // Days since 1970-01-01
    let total_secs = epoch_secs;
    let secs_in_day = 86400u64;
    let mut days = (total_secs / secs_in_day) as i64;
    let day_secs = (total_secs % secs_in_day) as u32;

    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Civil date from days since epoch (algorithm from Howard Hinnant)
    days += 719468; // shift to 0000-03-01
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let year = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, m, d, hours, minutes, seconds, millis
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_returns_string() {
        let result = call("now", vec![]).unwrap();
        match result {
            Value::String(s) => {
                assert!(s.ends_with('Z'));
                assert!(s.contains('T'));
            }
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn now_ms_returns_int() {
        let result = call("now_ms", vec![]).unwrap();
        match result {
            Value::Int(ms) => {
                // Should be a reasonable epoch time (after 2020)
                assert!(ms > 1_577_836_800_000);
            }
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn sleep_returns_nil() {
        let result = call("sleep", vec![Value::Int(1)]).unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn epoch_to_iso8601_known() {
        // 2024-01-01T00:00:00.000Z = 1704067200 seconds
        let result = epoch_to_iso8601(1704067200, 0);
        assert_eq!(result, "2024-01-01T00:00:00.000Z");
    }

    #[test]
    fn epoch_to_iso8601_with_millis() {
        let result = epoch_to_iso8601(0, 0);
        assert_eq!(result, "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
