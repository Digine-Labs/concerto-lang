use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    let msg = args.first().map(|v| v.display_string()).unwrap_or_default();

    match name {
        "info" => {
            eprintln!("[INFO]  {}", msg);
            Ok(Value::Nil)
        }
        "warn" => {
            eprintln!("[WARN]  {}", msg);
            Ok(Value::Nil)
        }
        "error" => {
            eprintln!("[ERROR] {}", msg);
            Ok(Value::Nil)
        }
        "debug" => {
            eprintln!("[DEBUG] {}", msg);
            Ok(Value::Nil)
        }
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::log::{}",
            name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_returns_nil() {
        assert_eq!(
            call("info", vec![Value::String("test".into())]).unwrap(),
            Value::Nil
        );
    }

    #[test]
    fn warn_returns_nil() {
        assert_eq!(
            call("warn", vec![Value::String("test".into())]).unwrap(),
            Value::Nil
        );
    }

    #[test]
    fn error_returns_nil() {
        assert_eq!(
            call("error", vec![Value::String("test".into())]).unwrap(),
            Value::Nil
        );
    }

    #[test]
    fn debug_returns_nil() {
        assert_eq!(
            call("debug", vec![Value::String("test".into())]).unwrap(),
            Value::Nil
        );
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
