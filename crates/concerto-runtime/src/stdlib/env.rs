use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "get" => stdlib_get(args),
        "require" => stdlib_require(args),
        "all" => stdlib_all(),
        "has" => stdlib_has(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::env::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::env::{} expected String, got {}",
            fn_name,
            other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::env::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn stdlib_get(args: Vec<Value>) -> Result<Value> {
    let name = expect_string(&args, 0, "get")?;
    match std::env::var(&name) {
        Ok(val) => Ok(Value::Option(Some(Box::new(Value::String(val))))),
        Err(_) => Ok(Value::Option(None)),
    }
}

fn stdlib_require(args: Vec<Value>) -> Result<Value> {
    let name = expect_string(&args, 0, "require")?;
    match std::env::var(&name) {
        Ok(val) => Ok(Value::Result {
            is_ok: true,
            value: Box::new(Value::String(val)),
        }),
        Err(_) => Ok(Value::Result {
            is_ok: false,
            value: Box::new(Value::String(format!(
                "missing environment variable: {}",
                name
            ))),
        }),
    }
}

fn stdlib_all() -> Result<Value> {
    let pairs: Vec<(String, Value)> = std::env::vars()
        .map(|(k, v)| (k, Value::String(v)))
        .collect();
    Ok(Value::Map(pairs))
}

fn stdlib_has(args: Vec<Value>) -> Result<Value> {
    let name = expect_string(&args, 0, "has")?;
    Ok(Value::Bool(std::env::var(&name).is_ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_existing() {
        std::env::set_var("CONCERTO_TEST_VAR", "hello");
        let result = call("get", vec![Value::String("CONCERTO_TEST_VAR".into())]).unwrap();
        match result {
            Value::Option(Some(v)) => assert_eq!(*v, Value::String("hello".into())),
            _ => panic!("expected Some"),
        }
        std::env::remove_var("CONCERTO_TEST_VAR");
    }

    #[test]
    fn get_missing() {
        let result = call(
            "get",
            vec![Value::String("CONCERTO_NONEXISTENT_12345".into())],
        )
        .unwrap();
        assert_eq!(result, Value::Option(None));
    }

    #[test]
    fn require_missing() {
        let result = call(
            "require",
            vec![Value::String("CONCERTO_NONEXISTENT_12345".into())],
        )
        .unwrap();
        match result {
            Value::Result { is_ok: false, .. } => {}
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn has_check() {
        std::env::set_var("CONCERTO_TEST_HAS", "1");
        assert_eq!(
            call("has", vec![Value::String("CONCERTO_TEST_HAS".into())]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call(
                "has",
                vec![Value::String("CONCERTO_NONEXISTENT_12345".into())]
            )
            .unwrap(),
            Value::Bool(false)
        );
        std::env::remove_var("CONCERTO_TEST_HAS");
    }

    #[test]
    fn all_returns_map() {
        let result = call("all", vec![]).unwrap();
        match result {
            Value::Map(_) => {}
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
