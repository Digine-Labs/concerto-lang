use std::fmt;

use thiserror::Error;

/// Runtime errors that occur during VM execution.
#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("IR load error: {0}")]
    LoadError(String),

    #[error("type error: {0}")]
    TypeError(String),

    #[error("name error: undefined variable '{0}'")]
    NameError(String),

    #[error("stack underflow")]
    StackUnderflow,

    #[error("call error: {0}")]
    CallError(String),

    #[error("division by zero")]
    DivisionByZero,

    #[error("field error: no field '{field}' on {type_name}")]
    FieldError { type_name: String, field: String },

    #[error("index error: index {index} out of bounds (length {len})")]
    IndexError { index: i64, len: usize },

    #[error("unhandled error: {0}")]
    UnhandledThrow(String),

    #[error("propagated error")]
    Propagated(Box<PropagatedValue>),

    #[error("max call depth exceeded ({0})")]
    StackOverflow(usize),

    #[error("schema validation error: {0}")]
    SchemaError(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Wrapper to carry a runtime Value through the error system.
/// We can't use Value directly in RuntimeError because of circular dependency
/// during definition, so we use a newtype wrapper.
pub struct PropagatedValue {
    /// Display representation of the propagated error value.
    pub display: String,
    /// The raw JSON-serializable representation.
    pub json: serde_json::Value,
}

impl fmt::Debug for PropagatedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PropagatedValue({})", self.display)
    }
}

impl fmt::Display for PropagatedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display)
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let e = RuntimeError::NameError("x".to_string());
        assert_eq!(e.to_string(), "name error: undefined variable 'x'");
    }

    #[test]
    fn field_error_display() {
        let e = RuntimeError::FieldError {
            type_name: "Point".to_string(),
            field: "z".to_string(),
        };
        assert_eq!(e.to_string(), "field error: no field 'z' on Point");
    }
}
