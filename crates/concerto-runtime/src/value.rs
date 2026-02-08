use std::collections::HashMap;
use std::fmt;

use crate::error::{PropagatedValue, RuntimeError};

/// A runtime value on the VM stack or in local variables.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,
    Array(Vec<Value>),
    /// Ordered key-value pairs (preserves insertion order).
    Map(Vec<(String, Value)>),
    Struct {
        type_name: String,
        fields: HashMap<String, Value>,
    },
    /// Result<T, E> — tagged with is_ok flag.
    /// INDEX_GET at index 0 returns the inner value.
    Result {
        is_ok: bool,
        value: Box<Value>,
    },
    /// Option<T>.
    Option(Option<Box<Value>>),
    /// Reference to a function by name (for CALL dispatch).
    Function(String),
    /// Reference to an agent (for CALL_METHOD dispatch).
    AgentRef(String),
    /// Reference to a schema.
    SchemaRef(String),
    /// Reference to a hashmap.
    HashMapRef(String),
    /// Reference to a ledger (fault-tolerant knowledge store).
    LedgerRef(String),
    /// Reference to a pipeline.
    PipelineRef(String),
    /// Reference to a memory (conversation history store).
    MemoryRef(String),
    /// Reference to a host (external agent system).
    HostRef(String),
    /// A deferred computation (function name + captured args).
    /// Created by SpawnAsync, resolved by Await/AwaitAll.
    Thunk {
        function: String,
        args: Vec<Value>,
    },
    /// Transient builder for Agent/Host execution with chained config.
    /// Created by Agent.with_memory(), Agent.with_tools(), etc.
    AgentBuilder {
        source_name: String,
        source_kind: BuilderSourceKind,
        memory: Option<String>,
        memory_auto_append: bool,
        extra_tools: Vec<String>,
        exclude_default_tools: bool,
        context: Option<Box<Value>>,
    },
}

/// Whether an AgentBuilder wraps an Agent or a Host.
#[derive(Debug, Clone, PartialEq)]
pub enum BuilderSourceKind {
    Agent,
    Host,
}

// ============================================================================
// Arithmetic operations
// ============================================================================

impl Value {
    pub fn add(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_add(*b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => {
                let mut s = a.clone();
                s.push_str(b);
                Ok(Value::String(s))
            }
            // String + anything => concat
            (Value::String(a), other) => {
                let mut s = a.clone();
                s.push_str(&other.display_string());
                Ok(Value::String(s))
            }
            (other, Value::String(b)) => {
                let mut s = other.display_string();
                s.push_str(b);
                Ok(Value::String(s))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "cannot add {} and {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn sub(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_sub(*b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot subtract {} from {}",
                other.type_name(),
                self.type_name()
            ))),
        }
    }

    pub fn mul(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.wrapping_mul(*b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot multiply {} and {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn div(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot divide {} by {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn modulo(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 % b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a % *b as f64)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot modulo {} by {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn neg(&self) -> crate::error::Result<Value> {
        match self {
            Value::Int(a) => Ok(Value::Int(-a)),
            Value::Float(a) => Ok(Value::Float(-a)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot negate {}",
                self.type_name()
            ))),
        }
    }
}

// ============================================================================
// Comparison operations
// ============================================================================

impl Value {
    pub fn eq_val(&self, other: &Value) -> Value {
        Value::Bool(self == other)
    }

    pub fn neq_val(&self, other: &Value) -> Value {
        Value::Bool(self != other)
    }

    pub fn lt(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) < *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a < *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a < b)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot compare {} < {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn gt(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(*a as f64 > *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a > *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a > b)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot compare {} > {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn lte(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(*a as f64 <= *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a <= *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a <= b)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot compare {} <= {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    pub fn gte(&self, other: &Value) -> crate::error::Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(*a as f64 >= *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a >= *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a >= b)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot compare {} >= {}",
                self.type_name(),
                other.type_name()
            ))),
        }
    }

    /// Truthiness for conditionals and logical operations.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Nil => false,
            Value::Int(0) => false,
            Value::Int(_) => true,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Map(m) => !m.is_empty(),
            Value::Option(None) => false,
            Value::Option(Some(_)) => true,
            Value::Result { is_ok, .. } => *is_ok,
            _ => true,
        }
    }

    pub fn not(&self) -> crate::error::Result<Value> {
        Ok(Value::Bool(!self.is_truthy()))
    }

    pub fn and(&self, other: &Value) -> crate::error::Result<Value> {
        Ok(Value::Bool(self.is_truthy() && other.is_truthy()))
    }

    pub fn or(&self, other: &Value) -> crate::error::Result<Value> {
        Ok(Value::Bool(self.is_truthy() || other.is_truthy()))
    }
}

// ============================================================================
// Field and index access
// ============================================================================

impl Value {
    /// Access a named field on a struct or map.
    pub fn field_get(&self, name: &str) -> crate::error::Result<Value> {
        match self {
            Value::Struct { type_name, fields } => fields.get(name).cloned().ok_or_else(|| {
                RuntimeError::FieldError {
                    type_name: type_name.clone(),
                    field: name.to_string(),
                }
            }),
            Value::Map(pairs) => Ok(pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Nil)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot access field '{}' on {}",
                name,
                self.type_name()
            ))),
        }
    }

    /// Access by index (Array, Map by string key, Result inner value).
    pub fn index_get(&self, index: &Value) -> crate::error::Result<Value> {
        match (self, index) {
            // Array[Int]
            (Value::Array(arr), Value::Int(i)) => {
                let idx = *i as usize;
                arr.get(idx).cloned().ok_or(RuntimeError::IndexError {
                    index: *i,
                    len: arr.len(),
                })
            }
            // Map[String]
            (Value::Map(pairs), Value::String(key)) => Ok(pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Nil)),
            // Result[0] => inner value (used by compiler for pattern destructuring)
            (Value::Result { value, .. }, Value::Int(0)) => Ok(*value.clone()),
            // Option[0] => inner value or Nil
            (Value::Option(opt), Value::Int(0)) => {
                Ok(opt.as_ref().map(|v| *v.clone()).unwrap_or(Value::Nil))
            }
            // Struct[String] => field access
            (Value::Struct { .. }, Value::String(key)) => self.field_get(key),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot index {} with {}",
                self.type_name(),
                index.type_name()
            ))),
        }
    }
}

// ============================================================================
// Type introspection
// ============================================================================

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bool(_) => "Bool",
            Value::Nil => "Nil",
            Value::Array(_) => "Array",
            Value::Map(_) => "Map",
            Value::Struct { type_name, .. } => type_name,
            Value::Result { .. } => "Result",
            Value::Option(_) => "Option",
            Value::Function(_) => "Function",
            Value::AgentRef(_) => "AgentRef",
            Value::SchemaRef(_) => "SchemaRef",
            Value::HashMapRef(_) => "HashMapRef",
            Value::LedgerRef(_) => "LedgerRef",
            Value::PipelineRef(_) => "PipelineRef",
            Value::MemoryRef(_) => "MemoryRef",
            Value::HostRef(_) => "HostRef",
            Value::Thunk { .. } => "Thunk",
            Value::AgentBuilder { .. } => "AgentBuilder",
        }
    }

    /// Human-readable string representation for print/emit output.
    pub fn display_string(&self) -> String {
        format!("{}", self)
    }

    /// Convert to a PropagatedValue for the error system.
    pub fn to_propagated(&self) -> PropagatedValue {
        PropagatedValue {
            display: self.display_string(),
            json: self.to_json(),
        }
    }

    /// Convert to a JSON value for serialization.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Int(n) => serde_json::Value::Number((*n).into()),
            Value::Float(f) => serde_json::json!(*f),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Nil => serde_json::Value::Null,
            Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| v.to_json()).collect())
            }
            Value::Map(pairs) => {
                let map: serde_json::Map<String, serde_json::Value> = pairs
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect();
                serde_json::Value::Object(map)
            }
            Value::Struct { fields, .. } => {
                let map: serde_json::Map<String, serde_json::Value> = fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect();
                serde_json::Value::Object(map)
            }
            Value::Result { is_ok, value } => {
                if *is_ok {
                    serde_json::json!({"Ok": value.to_json()})
                } else {
                    serde_json::json!({"Err": value.to_json()})
                }
            }
            Value::Option(opt) => match opt {
                Some(v) => serde_json::json!({"Some": v.to_json()}),
                None => serde_json::Value::Null,
            },
            Value::Function(name) => serde_json::json!(format!("<fn {}>", name)),
            Value::AgentRef(name) => serde_json::json!(format!("<agent {}>", name)),
            Value::SchemaRef(name) => serde_json::json!(format!("<schema {}>", name)),
            Value::HashMapRef(name) => serde_json::json!(format!("<hashmap {}>", name)),
            Value::LedgerRef(name) => serde_json::json!(format!("<ledger {}>", name)),
            Value::PipelineRef(name) => serde_json::json!(format!("<pipeline {}>", name)),
            Value::MemoryRef(name) => serde_json::json!(format!("<memory {}>", name)),
            Value::HostRef(name) => serde_json::json!(format!("<host {}>", name)),
            Value::Thunk { function, .. } => serde_json::json!(format!("<thunk {}>", function)),
            Value::AgentBuilder { source_name, .. } => serde_json::json!(format!("<builder {}>", source_name)),
        }
    }
}

// ============================================================================
// PartialEq — structural equality
// ============================================================================

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Struct { type_name: t1, fields: f1 }, Value::Struct { type_name: t2, fields: f2 }) => {
                t1 == t2 && f1 == f2
            }
            (Value::Result { is_ok: ok1, value: v1 }, Value::Result { is_ok: ok2, value: v2 }) => {
                ok1 == ok2 && v1 == v2
            }
            (Value::Option(a), Value::Option(b)) => a == b,
            _ => false,
        }
    }
}

// ============================================================================
// Display
// ============================================================================

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(pairs) => {
                write!(f, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Struct { type_name, fields } => {
                write!(f, "{} {{", type_name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Result { is_ok: true, value } => write!(f, "Ok({})", value),
            Value::Result { is_ok: false, value } => write!(f, "Err({})", value),
            Value::Option(Some(v)) => write!(f, "Some({})", v),
            Value::Option(None) => write!(f, "None"),
            Value::Function(name) => write!(f, "<fn {}>", name),
            Value::AgentRef(name) => write!(f, "<agent {}>", name),
            Value::SchemaRef(name) => write!(f, "<schema {}>", name),
            Value::HashMapRef(name) => write!(f, "<hashmap {}>", name),
            Value::LedgerRef(name) => write!(f, "<ledger {}>", name),
            Value::PipelineRef(name) => write!(f, "<pipeline {}>", name),
            Value::MemoryRef(name) => write!(f, "<memory {}>", name),
            Value::HostRef(name) => write!(f, "<host {}>", name),
            Value::Thunk { function, .. } => write!(f, "<thunk {}>", function),
            Value::AgentBuilder { source_name, .. } => write!(f, "<builder {}>", source_name),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_arithmetic() {
        assert_eq!(Value::Int(3).add(&Value::Int(4)).unwrap(), Value::Int(7));
        assert_eq!(Value::Int(10).sub(&Value::Int(3)).unwrap(), Value::Int(7));
        assert_eq!(Value::Int(3).mul(&Value::Int(4)).unwrap(), Value::Int(12));
        assert_eq!(Value::Int(10).div(&Value::Int(3)).unwrap(), Value::Int(3));
        assert_eq!(Value::Int(10).modulo(&Value::Int(3)).unwrap(), Value::Int(1));
        assert_eq!(Value::Int(5).neg().unwrap(), Value::Int(-5));
    }

    #[test]
    fn float_arithmetic() {
        assert_eq!(
            Value::Float(1.5).add(&Value::Float(2.5)).unwrap(),
            Value::Float(4.0)
        );
        assert_eq!(
            Value::Float(5.0).div(&Value::Float(2.0)).unwrap(),
            Value::Float(2.5)
        );
    }

    #[test]
    fn mixed_numeric() {
        assert_eq!(
            Value::Int(3).add(&Value::Float(0.5)).unwrap(),
            Value::Float(3.5)
        );
        assert_eq!(
            Value::Float(10.0).sub(&Value::Int(3)).unwrap(),
            Value::Float(7.0)
        );
    }

    #[test]
    fn string_concat() {
        assert_eq!(
            Value::String("hello ".to_string())
                .add(&Value::String("world".to_string()))
                .unwrap(),
            Value::String("hello world".to_string())
        );
    }

    #[test]
    fn string_coerce_concat() {
        assert_eq!(
            Value::String("x=".to_string())
                .add(&Value::Int(42))
                .unwrap(),
            Value::String("x=42".to_string())
        );
    }

    #[test]
    fn division_by_zero() {
        assert!(Value::Int(5).div(&Value::Int(0)).is_err());
    }

    #[test]
    fn comparison() {
        assert_eq!(Value::Int(3).lt(&Value::Int(5)).unwrap(), Value::Bool(true));
        assert_eq!(Value::Int(5).gt(&Value::Int(3)).unwrap(), Value::Bool(true));
        assert_eq!(
            Value::Int(3).lte(&Value::Int(3)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            Value::Int(3).gte(&Value::Int(5)).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn equality() {
        assert_eq!(Value::Int(42).eq_val(&Value::Int(42)), Value::Bool(true));
        assert_eq!(
            Value::Int(42).neq_val(&Value::Int(43)),
            Value::Bool(true)
        );
        assert_eq!(Value::Nil.eq_val(&Value::Nil), Value::Bool(true));
        assert_eq!(Value::Nil.neq_val(&Value::Int(0)), Value::Bool(true));
    }

    #[test]
    fn truthiness() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(Value::String("x".to_string()).is_truthy());
        assert!(!Value::String(String::new()).is_truthy());
    }

    #[test]
    fn field_access_struct() {
        let mut fields = HashMap::new();
        fields.insert("x".to_string(), Value::Int(10));
        fields.insert("y".to_string(), Value::Int(20));
        let s = Value::Struct {
            type_name: "Point".to_string(),
            fields,
        };
        assert_eq!(s.field_get("x").unwrap(), Value::Int(10));
        assert!(s.field_get("z").is_err());
    }

    #[test]
    fn field_access_map() {
        let m = Value::Map(vec![
            ("a".to_string(), Value::Int(1)),
            ("b".to_string(), Value::Int(2)),
        ]);
        assert_eq!(m.field_get("a").unwrap(), Value::Int(1));
        assert_eq!(m.field_get("c").unwrap(), Value::Nil);
    }

    #[test]
    fn index_access_array() {
        let arr = Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
        assert_eq!(arr.index_get(&Value::Int(1)).unwrap(), Value::Int(20));
        assert!(arr.index_get(&Value::Int(5)).is_err());
    }

    #[test]
    fn index_access_result() {
        let ok = Value::Result {
            is_ok: true,
            value: Box::new(Value::Int(42)),
        };
        assert_eq!(ok.index_get(&Value::Int(0)).unwrap(), Value::Int(42));

        let err = Value::Result {
            is_ok: false,
            value: Box::new(Value::String("oops".to_string())),
        };
        assert_eq!(
            err.index_get(&Value::Int(0)).unwrap(),
            Value::String("oops".to_string())
        );
    }

    #[test]
    fn display_values() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
        assert_eq!(format!("{}", Value::String("hi".to_string())), "hi");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Nil), "nil");
        assert_eq!(
            format!("{}", Value::Array(vec![Value::Int(1), Value::Int(2)])),
            "[1, 2]"
        );
    }
}
