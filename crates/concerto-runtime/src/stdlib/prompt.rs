use crate::error::{Result, RuntimeError};
use crate::value::Value;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "template" => stdlib_template(args),
        "from_file" => stdlib_from_file(args),
        "count_tokens" => stdlib_count_tokens(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::prompt::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::prompt::{} expected String, got {}",
            fn_name,
            other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::prompt::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

/// Replace `${name}` placeholders in text with values from a Map.
fn stdlib_template(args: Vec<Value>) -> Result<Value> {
    let text = expect_string(&args, 0, "template")?;
    let vars = match args.get(1) {
        Some(Value::Map(pairs)) => pairs.clone(),
        Some(other) => {
            return Err(RuntimeError::TypeError(format!(
                "std::prompt::template expected Map, got {}",
                other.type_name()
            )))
        }
        None => {
            return Err(RuntimeError::TypeError(
                "std::prompt::template missing vars argument".to_string(),
            ))
        }
    };

    let mut result = text;
    for (key, val) in &vars {
        let placeholder = format!("${{{}}}", key);
        let replacement = match val {
            Value::String(s) => s.clone(),
            other => format!("{}", other),
        };
        result = result.replace(&placeholder, &replacement);
    }
    Ok(Value::String(result))
}

/// Read a prompt template from a file and optionally apply variable substitution.
fn stdlib_from_file(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "from_file")?;
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Value::Result {
                is_ok: false,
                value: Box::new(Value::String(format!(
                    "failed to read prompt file '{}': {}",
                    path, e
                ))),
            })
        }
    };

    // If a vars Map is provided, apply template substitution
    if let Some(Value::Map(pairs)) = args.get(1) {
        let mut result = content;
        for (key, val) in pairs {
            let placeholder = format!("${{{}}}", key);
            let replacement = match val {
                Value::String(s) => s.clone(),
                other => format!("{}", other),
            };
            result = result.replace(&placeholder, &replacement);
        }
        Ok(Value::Result {
            is_ok: true,
            value: Box::new(Value::String(result)),
        })
    } else {
        Ok(Value::Result {
            is_ok: true,
            value: Box::new(Value::String(content)),
        })
    }
}

/// Approximate token count for a text string.
/// Uses word-count based heuristic: ~4 chars per token on average.
fn stdlib_count_tokens(args: Vec<Value>) -> Result<Value> {
    let text = expect_string(&args, 0, "count_tokens")?;
    // Approximation: split on whitespace, multiply by 4/3 for subword tokenization
    let word_count = text.split_whitespace().count();
    let approx_tokens = (word_count as f64 * 4.0 / 3.0).ceil() as i64;
    Ok(Value::Int(approx_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_basic() {
        let result = call(
            "template",
            vec![
                Value::String("Hello, ${name}! You are ${age} years old.".into()),
                Value::Map(vec![
                    ("name".into(), Value::String("Alice".into())),
                    ("age".into(), Value::Int(30)),
                ]),
            ],
        )
        .unwrap();
        assert_eq!(
            result,
            Value::String("Hello, Alice! You are 30 years old.".into())
        );
    }

    #[test]
    fn template_no_placeholders() {
        let result = call(
            "template",
            vec![
                Value::String("No placeholders here.".into()),
                Value::Map(vec![("key".into(), Value::String("val".into()))]),
            ],
        )
        .unwrap();
        assert_eq!(result, Value::String("No placeholders here.".into()));
    }

    #[test]
    fn from_file_missing() {
        let result = call(
            "from_file",
            vec![Value::String("/tmp/nonexistent_prompt_file_xyz.txt".into())],
        )
        .unwrap();
        match result {
            Value::Result { is_ok, .. } => assert!(!is_ok),
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn count_tokens_basic() {
        // "Hello world foo bar" = 4 words → ceil(4 * 4/3) = ceil(5.33) = 6
        let result = call(
            "count_tokens",
            vec![Value::String("Hello world foo bar".into())],
        )
        .unwrap();
        assert_eq!(result, Value::Int(6));
    }

    #[test]
    fn count_tokens_empty() {
        let result = call("count_tokens", vec![Value::String("".into())]).unwrap();
        assert_eq!(result, Value::Int(0));
    }
}
