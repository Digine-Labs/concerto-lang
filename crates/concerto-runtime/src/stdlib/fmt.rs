use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "format" => stdlib_format(args),
        "pad_left" => stdlib_pad_left(args),
        "pad_right" => stdlib_pad_right(args),
        "truncate" => stdlib_truncate(args),
        "indent" => stdlib_indent(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::fmt::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::fmt::{} expected String at arg {}, got {}",
            fn_name, idx, other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::fmt::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn expect_int(args: &[Value], idx: usize, fn_name: &str) -> Result<i64> {
    match args.get(idx) {
        Some(Value::Int(n)) => Ok(*n),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::fmt::{} expected Int at arg {}, got {}",
            fn_name, idx, other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::fmt::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn stdlib_format(args: Vec<Value>) -> Result<Value> {
    let template = expect_string(&args, 0, "format")?;
    let format_args = match args.get(1) {
        Some(Value::Array(a)) => a.clone(),
        Some(other) => {
            return Err(RuntimeError::TypeError(format!(
                "std::fmt::format expected Array as second arg, got {}",
                other.type_name()
            )))
        }
        None => vec![],
    };

    let mut result = String::new();
    let mut arg_idx = 0;
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'}') {
            chars.next(); // consume '}'
            if let Some(arg) = format_args.get(arg_idx) {
                result.push_str(&arg.display_string());
            } else {
                result.push_str("{}");
            }
            arg_idx += 1;
        } else {
            result.push(ch);
        }
    }
    Ok(Value::String(result))
}

fn stdlib_pad_left(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "pad_left")?;
    let width = expect_int(&args, 1, "pad_left")? as usize;
    let pad_char = expect_string(&args, 2, "pad_left")?
        .chars()
        .next()
        .unwrap_or(' ');
    let char_count = s.chars().count();
    if char_count >= width {
        Ok(Value::String(s))
    } else {
        let padding: String = std::iter::repeat_n(pad_char, width - char_count).collect();
        Ok(Value::String(format!("{}{}", padding, s)))
    }
}

fn stdlib_pad_right(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "pad_right")?;
    let width = expect_int(&args, 1, "pad_right")? as usize;
    let pad_char = expect_string(&args, 2, "pad_right")?
        .chars()
        .next()
        .unwrap_or(' ');
    let char_count = s.chars().count();
    if char_count >= width {
        Ok(Value::String(s))
    } else {
        let padding: String = std::iter::repeat_n(pad_char, width - char_count).collect();
        Ok(Value::String(format!("{}{}", s, padding)))
    }
}

fn stdlib_truncate(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "truncate")?;
    let max_len = expect_int(&args, 1, "truncate")? as usize;
    let result: String = s.chars().take(max_len).collect();
    Ok(Value::String(result))
}

fn stdlib_indent(args: Vec<Value>) -> Result<Value> {
    let s = expect_string(&args, 0, "indent")?;
    let spaces = expect_int(&args, 1, "indent")? as usize;
    let prefix: String = " ".repeat(spaces);
    let result: String = s
        .lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Value::String(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_basic() {
        let result = call(
            "format",
            vec![
                Value::String("Hello, {}!".into()),
                Value::Array(vec![Value::String("world".into())]),
            ],
        )
        .unwrap();
        assert_eq!(result, Value::String("Hello, world!".into()));
    }

    #[test]
    fn format_multiple() {
        let result = call(
            "format",
            vec![
                Value::String("{} has {} items".into()),
                Value::Array(vec![Value::String("Alice".into()), Value::Int(5)]),
            ],
        )
        .unwrap();
        assert_eq!(result, Value::String("Alice has 5 items".into()));
    }

    #[test]
    fn pad_left_basic() {
        let result = call(
            "pad_left",
            vec![Value::String("42".into()), Value::Int(6), Value::String("0".into())],
        )
        .unwrap();
        assert_eq!(result, Value::String("000042".into()));
    }

    #[test]
    fn pad_right_basic() {
        let result = call(
            "pad_right",
            vec![Value::String("hi".into()), Value::Int(5), Value::String(".".into())],
        )
        .unwrap();
        assert_eq!(result, Value::String("hi...".into()));
    }

    #[test]
    fn truncate_basic() {
        let result = call(
            "truncate",
            vec![Value::String("Long text here".into()), Value::Int(8)],
        )
        .unwrap();
        assert_eq!(result, Value::String("Long tex".into()));
    }

    #[test]
    fn truncate_short_string() {
        let result = call("truncate", vec![Value::String("hi".into()), Value::Int(10)]).unwrap();
        assert_eq!(result, Value::String("hi".into()));
    }

    #[test]
    fn indent_basic() {
        let result = call(
            "indent",
            vec![Value::String("line1\nline2".into()), Value::Int(4)],
        )
        .unwrap();
        assert_eq!(result, Value::String("    line1\n    line2".into()));
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
