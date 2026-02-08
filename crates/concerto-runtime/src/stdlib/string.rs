use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "split" => stdlib_split(args),
        "join" => stdlib_join(args),
        "trim" => stdlib_trim(args),
        "trim_start" => stdlib_trim_start(args),
        "trim_end" => stdlib_trim_end(args),
        "replace" => stdlib_replace(args),
        "to_upper" => stdlib_to_upper(args),
        "to_lower" => stdlib_to_lower(args),
        "contains" => stdlib_contains(args),
        "starts_with" => stdlib_starts_with(args),
        "ends_with" => stdlib_ends_with(args),
        "substring" => stdlib_substring(args),
        "len" => stdlib_len(args),
        "repeat" => stdlib_repeat(args),
        "reverse" => stdlib_reverse(args),
        "parse_int" => stdlib_parse_int(args),
        "parse_float" => stdlib_parse_float(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::string::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::string::{} expected String at arg {}, got {}",
            fn_name, idx, other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::string::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn expect_int(args: &[Value], idx: usize, fn_name: &str) -> Result<i64> {
    match args.get(idx) {
        Some(Value::Int(n)) => Ok(*n),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::string::{} expected Int at arg {}, got {}",
            fn_name, idx, other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::string::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn stdlib_split(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "split")?;
    let delim = expect_string(&args, 1, "split")?;
    let parts: Vec<Value> = s.split(&delim).map(|p| Value::String(p.to_string())).collect();
    Ok(Value::Array(parts))
}

fn stdlib_join(args: Vec<Value>) -> Result<Value> {
    let arr = match args.first() {
        Some(Value::Array(a)) => a,
        _ => {
            return Err(RuntimeError::TypeError(
                "std::string::join expected Array as first argument".to_string(),
            ))
        }
    };
    let sep = expect_string(&args, 1, "join")?;
    let strings: Vec<String> = arr.iter().map(|v| v.display_string()).collect();
    Ok(Value::String(strings.join(&sep)))
}

fn stdlib_trim(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "trim")?;
    Ok(Value::String(s.trim().to_string()))
}

fn stdlib_trim_start(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "trim_start")?;
    Ok(Value::String(s.trim_start().to_string()))
}

fn stdlib_trim_end(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "trim_end")?;
    Ok(Value::String(s.trim_end().to_string()))
}

fn stdlib_replace(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "replace")?;
    let from = expect_string(&args, 1, "replace")?;
    let to = expect_string(&args, 2, "replace")?;
    Ok(Value::String(s.replace(&from, &to)))
}

fn stdlib_to_upper(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "to_upper")?;
    Ok(Value::String(s.to_uppercase()))
}

fn stdlib_to_lower(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "to_lower")?;
    Ok(Value::String(s.to_lowercase()))
}

fn stdlib_contains(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "contains")?;
    let sub = expect_string(&args, 1, "contains")?;
    Ok(Value::Bool(s.contains(&sub)))
}

fn stdlib_starts_with(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "starts_with")?;
    let prefix = expect_string(&args, 1, "starts_with")?;
    Ok(Value::Bool(s.starts_with(&prefix)))
}

fn stdlib_ends_with(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "ends_with")?;
    let suffix = expect_string(&args, 1, "ends_with")?;
    Ok(Value::Bool(s.ends_with(&suffix)))
}

fn stdlib_substring(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "substring")?;
    let start = expect_int(&args, 1, "substring")? as usize;
    let end = expect_int(&args, 2, "substring")? as usize;
    let result: String = s.chars().skip(start).take(end.saturating_sub(start)).collect();
    Ok(Value::String(result))
}

fn stdlib_len(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "len")?;
    Ok(Value::Int(s.chars().count() as i64))
}

fn stdlib_repeat(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "repeat")?;
    let n = expect_int(&args, 1, "repeat")?;
    Ok(Value::String(s.repeat(n.max(0) as usize)))
}

fn stdlib_reverse(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "reverse")?;
    Ok(Value::String(s.chars().rev().collect()))
}

fn stdlib_parse_int(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "parse_int")?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(Value::Result {
            is_ok: true,
            value: Box::new(Value::Int(n)),
        }),
        Err(e) => Ok(Value::Result {
            is_ok: false,
            value: Box::new(Value::String(e.to_string())),
        }),
    }
}

fn stdlib_parse_float(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "parse_float")?;
    match s.trim().parse::<f64>() {
        Ok(f) => Ok(Value::Result {
            is_ok: true,
            value: Box::new(Value::Float(f)),
        }),
        Err(e) => Ok(Value::Result {
            is_ok: false,
            value: Box::new(Value::String(e.to_string())),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic() {
        let result = call("split", vec![Value::String("a,b,c".into()), Value::String(",".into())]).unwrap();
        assert_eq!(
            result,
            Value::Array(vec![
                Value::String("a".into()),
                Value::String("b".into()),
                Value::String("c".into()),
            ])
        );
    }

    #[test]
    fn join_basic() {
        let arr = Value::Array(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]);
        let result = call("join", vec![arr, Value::String(",".into())]).unwrap();
        assert_eq!(result, Value::String("a,b,c".into()));
    }

    #[test]
    fn trim_basic() {
        assert_eq!(
            call("trim", vec![Value::String("  hello  ".into())]).unwrap(),
            Value::String("hello".into())
        );
    }

    #[test]
    fn replace_basic() {
        let result = call(
            "replace",
            vec![
                Value::String("hello world".into()),
                Value::String("world".into()),
                Value::String("Concerto".into()),
            ],
        )
        .unwrap();
        assert_eq!(result, Value::String("hello Concerto".into()));
    }

    #[test]
    fn to_upper_lower() {
        assert_eq!(
            call("to_upper", vec![Value::String("hello".into())]).unwrap(),
            Value::String("HELLO".into())
        );
        assert_eq!(
            call("to_lower", vec![Value::String("HELLO".into())]).unwrap(),
            Value::String("hello".into())
        );
    }

    #[test]
    fn contains_check() {
        assert_eq!(
            call("contains", vec![Value::String("hello".into()), Value::String("ell".into())]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call("contains", vec![Value::String("hello".into()), Value::String("xyz".into())]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn starts_ends_with() {
        assert_eq!(
            call("starts_with", vec![Value::String("hello".into()), Value::String("hel".into())]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call("ends_with", vec![Value::String("hello".into()), Value::String("llo".into())]).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn substring_basic() {
        assert_eq!(
            call("substring", vec![Value::String("hello".into()), Value::Int(1), Value::Int(4)]).unwrap(),
            Value::String("ell".into())
        );
    }

    #[test]
    fn len_chars() {
        assert_eq!(
            call("len", vec![Value::String("hello".into())]).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn repeat_basic() {
        assert_eq!(
            call("repeat", vec![Value::String("ab".into()), Value::Int(3)]).unwrap(),
            Value::String("ababab".into())
        );
    }

    #[test]
    fn reverse_basic() {
        assert_eq!(
            call("reverse", vec![Value::String("hello".into())]).unwrap(),
            Value::String("olleh".into())
        );
    }

    #[test]
    fn parse_int_ok() {
        let result = call("parse_int", vec![Value::String("42".into())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => assert_eq!(*value, Value::Int(42)),
            _ => panic!("expected Ok(42)"),
        }
    }

    #[test]
    fn parse_int_err() {
        let result = call("parse_int", vec![Value::String("abc".into())]).unwrap();
        match result {
            Value::Result { is_ok: false, .. } => {}
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn parse_float_ok() {
        let result = call("parse_float", vec![Value::String("3.14".into())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => assert_eq!(*value, Value::Float(3.14)),
            _ => panic!("expected Ok(3.14)"),
        }
    }

    #[test]
    fn trim_start_end() {
        assert_eq!(
            call("trim_start", vec![Value::String("  hello  ".into())]).unwrap(),
            Value::String("hello  ".into())
        );
        assert_eq!(
            call("trim_end", vec![Value::String("  hello  ".into())]).unwrap(),
            Value::String("  hello".into())
        );
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
