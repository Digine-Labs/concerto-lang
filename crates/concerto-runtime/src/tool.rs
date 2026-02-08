use std::collections::HashMap;

use crate::value::Value;

/// Manages tool instances and their state at runtime.
///
/// Each tool can have per-instance state (a set of named fields).
/// The ToolRegistry stores this state and provides a `self` value
/// when tool methods are called.
pub struct ToolRegistry {
    /// Per-tool instance state: tool_name -> field_name -> value.
    tool_state: HashMap<String, HashMap<String, Value>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tool_state: HashMap::new(),
        }
    }

    /// Register a tool with empty initial state.
    pub fn register_tool(&mut self, name: &str) {
        self.tool_state.entry(name.to_string()).or_default();
    }

    /// Get a tool's state as a Value::Struct (used as `self` in method calls).
    pub fn get_self_value(&self, tool_name: &str) -> Value {
        let fields = self.tool_state.get(tool_name).cloned().unwrap_or_default();
        Value::Struct {
            type_name: tool_name.to_string(),
            fields,
        }
    }

    /// Update a tool's state from a Value::Struct returned after method execution.
    pub fn update_state(&mut self, tool_name: &str, value: &Value) {
        if let Value::Struct { fields, .. } = value {
            self.tool_state
                .insert(tool_name.to_string(), fields.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get_self() {
        let mut registry = ToolRegistry::new();
        registry.register_tool("MyTool");
        let self_val = registry.get_self_value("MyTool");
        assert!(matches!(self_val, Value::Struct { type_name, .. } if type_name == "MyTool"));
    }

    #[test]
    fn update_state() {
        let mut registry = ToolRegistry::new();
        registry.register_tool("MyTool");

        let mut fields = HashMap::new();
        fields.insert("count".to_string(), Value::Int(42));
        let updated = Value::Struct {
            type_name: "MyTool".to_string(),
            fields,
        };
        registry.update_state("MyTool", &updated);

        let self_val = registry.get_self_value("MyTool");
        match self_val {
            Value::Struct { fields, .. } => {
                assert_eq!(fields.get("count"), Some(&Value::Int(42)));
            }
            _ => panic!("expected Struct"),
        }
    }
}
