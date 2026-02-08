use crate::error::{Result, RuntimeError};
use crate::value::Value;

use std::io::Write;

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "read_file" => stdlib_read_file(args),
        "write_file" => stdlib_write_file(args),
        "append_file" => stdlib_append_file(args),
        "exists" => stdlib_exists(args),
        "list_dir" => stdlib_list_dir(args),
        "remove_file" => stdlib_remove_file(args),
        "file_size" => stdlib_file_size(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::fs::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::fs::{} expected String at arg {}, got {}",
            fn_name,
            idx,
            other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::fs::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn wrap_io_ok(value: Value) -> Value {
    Value::Result {
        is_ok: true,
        value: Box::new(value),
    }
}

fn wrap_io_err(e: std::io::Error) -> Value {
    Value::Result {
        is_ok: false,
        value: Box::new(Value::String(e.to_string())),
    }
}

fn stdlib_read_file(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "read_file")?;
    Ok(match std::fs::read_to_string(&path) {
        Ok(content) => wrap_io_ok(Value::String(content)),
        Err(e) => wrap_io_err(e),
    })
}

fn stdlib_write_file(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "write_file")?;
    let content = expect_string(&args, 1, "write_file")?;
    Ok(match std::fs::write(&path, &content) {
        Ok(()) => wrap_io_ok(Value::Nil),
        Err(e) => wrap_io_err(e),
    })
}

fn stdlib_append_file(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "append_file")?;
    let content = expect_string(&args, 1, "append_file")?;
    let result = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .and_then(|mut f| f.write_all(content.as_bytes()));
    Ok(match result {
        Ok(()) => wrap_io_ok(Value::Nil),
        Err(e) => wrap_io_err(e),
    })
}

fn stdlib_exists(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "exists")?;
    Ok(Value::Bool(std::path::Path::new(&path).exists()))
}

fn stdlib_list_dir(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "list_dir")?;
    match std::fs::read_dir(&path) {
        Ok(entries) => {
            let mut files = Vec::new();
            for entry in entries {
                match entry {
                    Ok(e) => {
                        files.push(Value::String(e.file_name().to_string_lossy().to_string()));
                    }
                    Err(e) => return Ok(wrap_io_err(e)),
                }
            }
            Ok(wrap_io_ok(Value::Array(files)))
        }
        Err(e) => Ok(wrap_io_err(e)),
    }
}

fn stdlib_remove_file(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "remove_file")?;
    Ok(match std::fs::remove_file(&path) {
        Ok(()) => wrap_io_ok(Value::Nil),
        Err(e) => wrap_io_err(e),
    })
}

fn stdlib_file_size(args: Vec<Value>) -> Result<Value> {
    let path = expect_string(&args, 0, "file_size")?;
    Ok(match std::fs::metadata(&path) {
        Ok(meta) => wrap_io_ok(Value::Int(meta.len() as i64)),
        Err(e) => wrap_io_err(e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> String {
        format!("/tmp/concerto_test_{}", name)
    }

    #[test]
    fn write_and_read_file() {
        let path = temp_path("write_read");
        call(
            "write_file",
            vec![Value::String(path.clone()), Value::String("hello".into())],
        )
        .unwrap();
        let result = call("read_file", vec![Value::String(path.clone())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => {
                assert_eq!(*value, Value::String("hello".into()))
            }
            _ => panic!("expected Ok"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_file_not_found() {
        let result = call(
            "read_file",
            vec![Value::String("/tmp/concerto_nonexistent_12345".into())],
        )
        .unwrap();
        match result {
            Value::Result { is_ok: false, .. } => {}
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn append_file_test() {
        let path = temp_path("append");
        call(
            "write_file",
            vec![Value::String(path.clone()), Value::String("hello".into())],
        )
        .unwrap();
        call(
            "append_file",
            vec![Value::String(path.clone()), Value::String(" world".into())],
        )
        .unwrap();
        let result = call("read_file", vec![Value::String(path.clone())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => {
                assert_eq!(*value, Value::String("hello world".into()))
            }
            _ => panic!("expected Ok"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn exists_check() {
        let path = temp_path("exists");
        // Create file
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"test").unwrap();
        assert_eq!(
            call("exists", vec![Value::String(path.clone())]).unwrap(),
            Value::Bool(true)
        );
        std::fs::remove_file(&path).ok();
        assert_eq!(
            call(
                "exists",
                vec![Value::String("/tmp/concerto_nonexistent_12345".into())]
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn list_dir_test() {
        let result = call("list_dir", vec![Value::String("/tmp".into())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => match *value {
                Value::Array(_) => {}
                _ => panic!("expected Array"),
            },
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn remove_file_test() {
        let path = temp_path("remove");
        std::fs::write(&path, "test").unwrap();
        let result = call("remove_file", vec![Value::String(path.clone())]).unwrap();
        match result {
            Value::Result { is_ok: true, .. } => {}
            _ => panic!("expected Ok"),
        }
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn file_size_test() {
        let path = temp_path("size");
        std::fs::write(&path, "hello").unwrap();
        let result = call("file_size", vec![Value::String(path.clone())]).unwrap();
        match result {
            Value::Result { is_ok: true, value } => assert_eq!(*value, Value::Int(5)),
            _ => panic!("expected Ok"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
