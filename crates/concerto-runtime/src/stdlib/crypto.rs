use crate::error::{Result, RuntimeError};
use crate::value::Value;

use md5::Md5;
use sha2::{Digest, Sha256};

pub fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match name {
        "sha256" => stdlib_sha256(args),
        "md5" => stdlib_md5(args),
        "uuid" => stdlib_uuid(),
        "random_bytes" => stdlib_random_bytes(args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown function: std::crypto::{}",
            name
        ))),
    }
}

fn expect_string(args: &[Value], idx: usize, fn_name: &str) -> Result<String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(RuntimeError::TypeError(format!(
            "std::crypto::{} expected String, got {}",
            fn_name,
            other.type_name()
        ))),
        None => Err(RuntimeError::TypeError(format!(
            "std::crypto::{} missing argument {}",
            fn_name, idx
        ))),
    }
}

fn stdlib_sha256(args: Vec<Value>) -> Result<Value> {
    let input = expect_string(&args, 0, "sha256")?;
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    Ok(Value::String(format!("{:x}", result)))
}

fn stdlib_md5(args: Vec<Value>) -> Result<Value> {
    let input = expect_string(&args, 0, "md5")?;
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    Ok(Value::String(format!("{:x}", result)))
}

fn stdlib_uuid() -> Result<Value> {
    Ok(Value::String(uuid::Uuid::new_v4().to_string()))
}

fn stdlib_random_bytes(args: Vec<Value>) -> Result<Value> {
    let n = match args.first() {
        Some(Value::Int(n)) => *n as usize,
        _ => {
            return Err(RuntimeError::TypeError(
                "std::crypto::random_bytes expected Int".to_string(),
            ))
        }
    };
    // Use time-based entropy for random bytes
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    let mut state = seed;
    let hex: String = (0..n)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            format!("{:02x}", (state >> 33) as u8)
        })
        .collect();
    Ok(Value::String(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_hash() {
        // SHA-256 of "hello" = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let result = call("sha256", vec![Value::String("hello".into())]).unwrap();
        assert_eq!(
            result,
            Value::String(
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into()
            )
        );
    }

    #[test]
    fn md5_known_hash() {
        // MD5 of "hello" = 5d41402abc4b2a76b9719d911017c592
        let result = call("md5", vec![Value::String("hello".into())]).unwrap();
        assert_eq!(
            result,
            Value::String("5d41402abc4b2a76b9719d911017c592".into())
        );
    }

    #[test]
    fn uuid_format() {
        let result = call("uuid", vec![]).unwrap();
        match result {
            Value::String(s) => {
                assert_eq!(s.len(), 36); // UUID v4 format: 8-4-4-4-12
                assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
            }
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn random_bytes_length() {
        let result = call("random_bytes", vec![Value::Int(16)]).unwrap();
        match result {
            Value::String(s) => assert_eq!(s.len(), 32), // 16 bytes = 32 hex chars
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn unknown_function() {
        assert!(call("nonexistent", vec![]).is_err());
    }
}
