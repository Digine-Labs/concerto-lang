pub mod collections;
pub mod crypto;
pub mod env;
pub mod fmt;
pub mod fs;
pub mod http;
pub mod json;
pub mod log;
pub mod math;
pub mod prompt;
pub mod string;
pub mod time;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Dispatch a std:: library call by full path name.
/// Called from VM exec_call when function name starts with "std::".
pub fn call_stdlib(name: &str, args: Vec<Value>) -> Result<Value> {
    let path = name.strip_prefix("std::").unwrap_or(name);

    // Handle nested paths like "collections::Set::new"
    let (module, function) = path
        .split_once("::")
        .ok_or_else(|| RuntimeError::CallError(format!("invalid stdlib path: {}", name)))?;

    match module {
        "math" => math::call(function, args),
        "string" => string::call(function, args),
        "env" => env::call(function, args),
        "time" => time::call(function, args),
        "json" => json::call(function, args),
        "fmt" => fmt::call(function, args),
        "log" => log::call(function, args),
        "fs" => fs::call(function, args),
        "collections" => collections::call(function, args),
        "http" => http::call(function, args),
        "crypto" => crypto::call(function, args),
        "prompt" => prompt::call(function, args),
        _ => Err(RuntimeError::CallError(format!(
            "unknown stdlib module: std::{}",
            module
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_module_error() {
        let result = call_stdlib("std::nonexistent::foo", vec![]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown stdlib module"));
    }

    #[test]
    fn invalid_path_error() {
        let result = call_stdlib("std::nofunction", vec![]);
        assert!(result.is_err());
    }
}
