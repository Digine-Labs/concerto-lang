use std::collections::HashMap;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Dispatch a built-in function call.
///
/// Built-in functions are registered in the global scope with `$builtin_` prefixed names.
/// The VM calls this when CALL targets a Function with that prefix.
pub fn call_builtin(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "$builtin_ok" => {
            let inner = args.into_iter().next().unwrap_or(Value::Nil);
            Ok(Value::Result {
                is_ok: true,
                value: Box::new(inner),
            })
        }
        "$builtin_err" => {
            let inner = args.into_iter().next().unwrap_or(Value::Nil);
            Ok(Value::Result {
                is_ok: false,
                value: Box::new(inner),
            })
        }
        "$builtin_some" => {
            let inner = args.into_iter().next().unwrap_or(Value::Nil);
            Ok(Value::Option(Some(Box::new(inner))))
        }
        "$builtin_env" => {
            let key = args
                .into_iter()
                .next()
                .and_then(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .ok_or_else(|| {
                    RuntimeError::TypeError("env() requires a string argument".to_string())
                })?;
            match std::env::var(&key) {
                Ok(val) => Ok(Value::String(val)),
                Err(_) => Ok(Value::Nil),
            }
        }
        "$builtin_print" => {
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{}", arg);
            }
            Ok(Value::Nil)
        }
        "$builtin_println" => {
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{}", arg);
            }
            println!();
            Ok(Value::Nil)
        }
        "$builtin_len" => {
            let val = args.into_iter().next().unwrap_or(Value::Nil);
            match &val {
                Value::String(s) => Ok(Value::Int(s.len() as i64)),
                Value::Array(a) => Ok(Value::Int(a.len() as i64)),
                Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                _ => Err(RuntimeError::TypeError(format!(
                    "len() not supported on {}",
                    val.type_name()
                ))),
            }
        }
        "$builtin_typeof" => {
            let val = args.into_iter().next().unwrap_or(Value::Nil);
            Ok(Value::String(val.type_name().to_string()))
        }
        "$builtin_tool_error_new" => {
            let msg = args
                .into_iter()
                .next()
                .map(|v| v.display_string())
                .unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("message".to_string(), Value::String(msg));
            Ok(Value::Struct {
                type_name: "ToolError".to_string(),
                fields,
            })
        }
        "$builtin_panic" => {
            let msg = args
                .into_iter()
                .next()
                .map(|v| v.display_string())
                .unwrap_or_else(|| "panic!".to_string());
            Err(RuntimeError::UnhandledThrow(format!("panic: {}", msg)))
        }
        _ => Err(RuntimeError::CallError(format!(
            "unknown builtin: {}",
            name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ok() {
        let result = call_builtin("$builtin_ok", vec![Value::Int(42)]).unwrap();
        assert_eq!(
            result,
            Value::Result {
                is_ok: true,
                value: Box::new(Value::Int(42))
            }
        );
    }

    #[test]
    fn builtin_err() {
        let result = call_builtin("$builtin_err", vec![Value::String("oops".to_string())]).unwrap();
        match result {
            Value::Result { is_ok, value } => {
                assert!(!is_ok);
                assert_eq!(*value, Value::String("oops".to_string()));
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn builtin_some() {
        let result = call_builtin("$builtin_some", vec![Value::Int(10)]).unwrap();
        assert_eq!(result, Value::Option(Some(Box::new(Value::Int(10)))));
    }

    #[test]
    fn builtin_env_missing() {
        let result = call_builtin(
            "$builtin_env",
            vec![Value::String("__NONEXISTENT_VAR__".to_string())],
        )
        .unwrap();
        assert_eq!(result, Value::Nil);
    }

    #[test]
    fn builtin_len() {
        let result = call_builtin(
            "$builtin_len",
            vec![Value::Array(vec![Value::Int(1), Value::Int(2)])],
        )
        .unwrap();
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn builtin_typeof() {
        let result = call_builtin("$builtin_typeof", vec![Value::Int(42)]).unwrap();
        assert_eq!(result, Value::String("Int".to_string()));
    }

    #[test]
    fn builtin_tool_error_new() {
        let result = call_builtin(
            "$builtin_tool_error_new",
            vec![Value::String("file not found".to_string())],
        )
        .unwrap();
        match result {
            Value::Struct { type_name, fields } => {
                assert_eq!(type_name, "ToolError");
                assert_eq!(
                    fields.get("message"),
                    Some(&Value::String("file not found".to_string()))
                );
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn unknown_builtin() {
        assert!(call_builtin("$builtin_nonexistent", vec![]).is_err());
    }
}
