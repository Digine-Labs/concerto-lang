use concerto_common::ir::IrConstant;
use std::collections::HashMap;

/// Deduplicating constant pool for IR generation.
///
/// Ensures each unique constant value is stored only once.
pub struct ConstantPool {
    constants: Vec<IrConstant>,
    /// Maps constant values (as JSON string) to their index for deduplication.
    dedup: HashMap<String, u32>,
}

impl ConstantPool {
    pub fn new() -> Self {
        Self {
            constants: Vec::new(),
            dedup: HashMap::new(),
        }
    }

    /// Add an integer constant, returning its index.
    pub fn add_int(&mut self, value: i64) -> u32 {
        let key = format!("int:{}", value);
        self.get_or_insert(key, || IrConstant {
            index: 0, // filled in by get_or_insert
            const_type: "int".to_string(),
            value: serde_json::Value::Number(serde_json::Number::from(value)),
        })
    }

    /// Add a float constant, returning its index.
    pub fn add_float(&mut self, value: f64) -> u32 {
        let key = format!("float:{}", value);
        self.get_or_insert(key, || IrConstant {
            index: 0,
            const_type: "float".to_string(),
            value: serde_json::json!(value),
        })
    }

    /// Add a string constant, returning its index.
    pub fn add_string(&mut self, value: &str) -> u32 {
        let key = format!("string:{}", value);
        let value = value.to_string();
        self.get_or_insert(key, || IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::Value::String(value),
        })
    }

    /// Add a boolean constant, returning its index.
    pub fn add_bool(&mut self, value: bool) -> u32 {
        let key = format!("bool:{}", value);
        self.get_or_insert(key, || IrConstant {
            index: 0,
            const_type: "bool".to_string(),
            value: serde_json::Value::Bool(value),
        })
    }

    /// Add a nil constant, returning its index.
    pub fn add_nil(&mut self) -> u32 {
        let key = "nil".to_string();
        self.get_or_insert(key, || IrConstant {
            index: 0,
            const_type: "nil".to_string(),
            value: serde_json::Value::Null,
        })
    }

    /// Get or insert a constant, deduplicating by key.
    fn get_or_insert(&mut self, key: String, make: impl FnOnce() -> IrConstant) -> u32 {
        if let Some(&idx) = self.dedup.get(&key) {
            return idx;
        }
        let idx = self.constants.len() as u32;
        let mut constant = make();
        constant.index = idx;
        self.constants.push(constant);
        self.dedup.insert(key, idx);
        idx
    }

    /// Consume the pool and return all constants.
    pub fn into_constants(self) -> Vec<IrConstant> {
        self.constants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_same_values() {
        let mut pool = ConstantPool::new();
        let a = pool.add_int(42);
        let b = pool.add_int(42);
        assert_eq!(a, b);
        assert_eq!(pool.constants.len(), 1);
    }

    #[test]
    fn different_values_get_different_indices() {
        let mut pool = ConstantPool::new();
        let a = pool.add_int(1);
        let b = pool.add_int(2);
        let c = pool.add_string("hello");
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_eq!(pool.constants.len(), 3);
    }
}
