use crate::error::{Result, RuntimeError};
use crate::schema::SchemaValidator;
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "parse" | "parse_as" => stdlib_parse(args),
        "stringify" => stdlib_stringify(args),
        "stringify_pretty" => stdlib_stringify_pretty(args),
        "is_valid" => stdlib_is_valid(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::json::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::json::{} expected String, got {}",
            fn_name,
            other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::json::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn stdlib_parse(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "parse")?;
    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(json_val) => {
            let value = SchemaValidator::json_to_value(&json_val);
            Ok(Value::Result {
                is_ok: true,
                value: Box::new(value),
            })
        }
        Err(e) => Ok(Value::Result {
            is_ok: false,
            value: Box::new(Value::String(e.to_string())),
        }),
    }
}

fn stdlib_stringify(args: Vec<Value>) -> Result<Value> {
    let val = args
        .into_iter()
        .next()
        .ok_or_else(|| RuntimeError::TypeError("std::json::stringify missing argument".to_string()))?;
    let json = val.to_json();
    match serde_json::to_string(&json) {
        Ok(s) => Ok(Value::String(s)),
        Err(e) => Err(RuntimeError::CallError(format!(
            "std::json::stringify error: {}",
            e
        ))),
    }
}

fn stdlib_stringify_pretty(args: Vec<Value>) -> Result<Value> {
    let val = args
        .into_iter()
        .next()
        .ok_or_else(|| {
            RuntimeError::TypeError("std::json::stringify_pretty missing argument".to_string())
        })?;
    let json = val.to_json();
    match serde_json::to_string_pretty(&json) {
        Ok(s) => Ok(Value::String(s)),
        Err(e) => Err(RuntimeError::CallError(format!(
            "std::json::stringify_pretty error: {}",
            e
        ))),
    }
}

fn stdlib_is_valid(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "is_valid")?;
    Ok(Value::Bool(
        serde_json::from_str::<serde_json::Value>(&s).is_ok(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_object() {
        let result = call("parse", vec![Value::String(r#"{"a": 1, "b": "hello"}"#.into())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => match *value {
                Value::Map(pairs) => {
                    assert_eq!(pairs.len(), 2);
                }
                _ => panic!("expected Map"),
            },
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn parse_array() {
        let result = call("parse", vec![Value::String("[1, 2, 3]".into())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => match *value {
                Value::Array(arr) => assert_eq!(arr.len(), 3),
                _ => panic!("expected Array"),
            },
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn parse_error() {
        let result = call("parse", vec![Value::String("not json".into())]).unwrap();
        match result {
            Value::Result { is_ok: false, .. } => {}
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn stringify_basic() {
        let result = call("stringify", vec![Value::Int(42)]).unwrap();
        assert_eq!(result, Value::String("42".into()));
    }

    #[test]
    fn stringify_map() {
        let val = Value::Map(vec![("a".into(), Value::Int(1))]);
        let result = call("stringify", vec![val]).unwrap();
        assert_eq!(result, Value::String(r#"{"a":1}"#.into()));
    }

    #[test]
    fn stringify_pretty_format() {
        let result = call("stringify_pretty", vec![Value::Int(42)]).unwrap();
        assert_eq!(result, Value::String("42".into()));
    }

    #[test]
    fn is_valid_checks() {
        assert_eq!(
            call("is_valid", vec![Value::String(r#"{"a": 1}"#.into())]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call("is_valid", vec![Value::String("not json".into())]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
