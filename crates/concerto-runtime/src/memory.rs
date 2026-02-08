use std::collections::HashMap;

use crate::error::{Result, RuntimeError};
use crate::provider::ChatMessage;
use crate::value::Value;

/// Stores conversation history for named memory instances.
///
/// Each memory is a named list of ChatMessages. Supports sliding window
/// via optional max_messages limit.
#[derive(Debug, Default)]
pub struct MemoryStore {
    /// Conversation histories keyed by memory name.
    memories: HashMap<String, MemoryInstance>,
}

#[derive(Debug, Clone)]
struct MemoryInstance {
    messages: Vec<ChatMessage>,
    max_messages: Option<u32>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            memories: HashMap::new(),
        }
    }

    /// Initialize a memory with an optional max message limit.
    pub fn init_memory(&mut self, name: &str, max_messages: Option<u32>) {
        self.memories.insert(
            name.to_string(),
            MemoryInstance {
                messages: Vec::new(),
                max_messages,
            },
        );
    }

    /// Append a message to a memory.
    pub fn append(&mut self, name: &str, role: &str, content: &str) -> Result<()> {
        let mem = self
            .memories
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("memory '{}' not found", name)))?;
        mem.messages.push(ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
        });
        // Enforce sliding window
        if let Some(max) = mem.max_messages {
            let max = max as usize;
            if mem.messages.len() > max {
                let excess = mem.messages.len() - max;
                mem.messages.drain(..excess);
            }
        }
        Ok(())
    }

    /// Get all messages from a memory.
    pub fn messages(&self, name: &str) -> Result<Vec<ChatMessage>> {
        let mem = self
            .memories
            .get(name)
            .ok_or_else(|| RuntimeError::CallError(format!("memory '{}' not found", name)))?;
        Ok(mem.messages.clone())
    }

    /// Get the last N messages from a memory.
    pub fn last(&self, name: &str, count: usize) -> Result<Vec<ChatMessage>> {
        let mem = self
            .memories
            .get(name)
            .ok_or_else(|| RuntimeError::CallError(format!("memory '{}' not found", name)))?;
        let start = mem.messages.len().saturating_sub(count);
        Ok(mem.messages[start..].to_vec())
    }

    /// Get message count.
    pub fn len(&self, name: &str) -> Result<usize> {
        let mem = self
            .memories
            .get(name)
            .ok_or_else(|| RuntimeError::CallError(format!("memory '{}' not found", name)))?;
        Ok(mem.messages.len())
    }

    /// Clear all messages from a memory.
    pub fn clear(&mut self, name: &str) -> Result<()> {
        let mem = self
            .memories
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("memory '{}' not found", name)))?;
        mem.messages.clear();
        Ok(())
    }

    /// Convert messages to Value::Array of Message structs.
    pub fn messages_to_value(&self, name: &str) -> Result<Value> {
        let msgs = self.messages(name)?;
        Ok(Value::Array(
            msgs.iter().map(chat_message_to_value).collect(),
        ))
    }

    /// Convert last N messages to Value::Array.
    pub fn last_to_value(&self, name: &str, count: usize) -> Result<Value> {
        let msgs = self.last(name, count)?;
        Ok(Value::Array(
            msgs.iter().map(chat_message_to_value).collect(),
        ))
    }
}

/// Convert a ChatMessage to a Value::Struct { type_name: "Message", fields: {role, content} }.
fn chat_message_to_value(msg: &ChatMessage) -> Value {
    let mut fields = HashMap::new();
    fields.insert("role".to_string(), Value::String(msg.role.clone()));
    fields.insert("content".to_string(), Value::String(msg.content.clone()));
    Value::Struct {
        type_name: "Message".to_string(),
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_append_and_retrieve() {
        let mut store = MemoryStore::new();
        store.init_memory("conv", None);
        store.append("conv", "user", "Hello").unwrap();
        store.append("conv", "assistant", "Hi there!").unwrap();

        let msgs = store.messages("conv").unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Hello");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "Hi there!");
    }

    #[test]
    fn sliding_window() {
        let mut store = MemoryStore::new();
        store.init_memory("conv", Some(3));
        store.append("conv", "user", "msg1").unwrap();
        store.append("conv", "assistant", "msg2").unwrap();
        store.append("conv", "user", "msg3").unwrap();
        store.append("conv", "assistant", "msg4").unwrap();

        let msgs = store.messages("conv").unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].content, "msg2"); // msg1 dropped
        assert_eq!(msgs[2].content, "msg4");
    }

    #[test]
    fn last_n_messages() {
        let mut store = MemoryStore::new();
        store.init_memory("conv", None);
        for i in 0..10 {
            store.append("conv", "user", &format!("msg{}", i)).unwrap();
        }

        let last5 = store.last("conv", 5).unwrap();
        assert_eq!(last5.len(), 5);
        assert_eq!(last5[0].content, "msg5");
        assert_eq!(last5[4].content, "msg9");
    }

    #[test]
    fn len_and_clear() {
        let mut store = MemoryStore::new();
        store.init_memory("conv", None);
        store.append("conv", "user", "Hello").unwrap();
        store.append("conv", "assistant", "Hi").unwrap();
        assert_eq!(store.len("conv").unwrap(), 2);

        store.clear("conv").unwrap();
        assert_eq!(store.len("conv").unwrap(), 0);
    }

    #[test]
    fn messages_to_value_format() {
        let mut store = MemoryStore::new();
        store.init_memory("conv", None);
        store.append("conv", "user", "Hello").unwrap();

        let val = store.messages_to_value("conv").unwrap();
        match val {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 1);
                match &arr[0] {
                    Value::Struct { type_name, fields } => {
                        assert_eq!(type_name, "Message");
                        assert_eq!(fields.get("role"), Some(&Value::String("user".to_string())));
                        assert_eq!(
                            fields.get("content"),
                            Some(&Value::String("Hello".to_string()))
                        );
                    }
                    _ => panic!("expected Struct"),
                }
            }
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn unknown_memory_errors() {
        let store = MemoryStore::new();
        assert!(store.messages("nonexistent").is_err());
    }
}
