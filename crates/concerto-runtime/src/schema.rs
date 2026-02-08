use std::collections::HashMap;

use concerto_common::ir::IrSchema;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Maximum number of retry attempts for schema validation.
const MAX_RETRIES: usize = 3;

/// Validates JSON strings against Concerto schema definitions.
pub struct SchemaValidator;

impl SchemaValidator {
    /// Validate a JSON string against an IrSchema.
    /// Returns a typed Value::Struct on success, or an error with details.
    pub fn validate(json_str: &str, schema: &IrSchema) -> Result<Value> {
        // Parse the JSON string
        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| RuntimeError::SchemaError(format!("invalid JSON: {}", e)))?;

        // Normalize Concerto types to JSON Schema types before validation
        let normalized = Self::normalize_schema(&schema.json_schema);

        // Validate against the normalized JSON Schema
        Self::validate_json(&json, &normalized)?;

        // Convert to typed Value::Struct
        Ok(Self::json_to_struct(&json, &schema.name))
    }

    /// Normalize Concerto type names to standard JSON Schema type names.
    /// Concerto uses `String`, `Int`, `Float`, `Bool`, `Array<T>`, `Map<K,V>`;
    /// JSON Schema uses `string`, `integer`, `number`, `boolean`, `array`, `object`.
    fn normalize_schema(schema: &serde_json::Value) -> serde_json::Value {
        match schema {
            serde_json::Value::Object(obj) => {
                let mut normalized = serde_json::Map::new();
                for (key, val) in obj {
                    if key == "type" {
                        if let Some(t) = val.as_str() {
                            // Check for compound types like Array<String>
                            if let Some(inner) =
                                t.strip_prefix("Array<").and_then(|s| s.strip_suffix('>'))
                            {
                                normalized.insert("type".to_string(), serde_json::json!("array"));
                                normalized.insert(
                                    "items".to_string(),
                                    serde_json::json!({"type": Self::normalize_type_name(inner)}),
                                );
                            } else if t.starts_with("Map<") || t.starts_with("Option<") {
                                // Map and Option are complex; treat as permissive object/any
                                normalized.insert(
                                    key.clone(),
                                    serde_json::Value::String("object".to_string()),
                                );
                            } else {
                                normalized.insert(
                                    key.clone(),
                                    serde_json::Value::String(
                                        Self::normalize_type_name(t).to_string(),
                                    ),
                                );
                            }
                        } else {
                            normalized.insert(key.clone(), Self::normalize_schema(val));
                        }
                    } else {
                        normalized.insert(key.clone(), Self::normalize_schema(val));
                    }
                }
                serde_json::Value::Object(normalized)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::normalize_schema).collect())
            }
            other => other.clone(),
        }
    }

    /// Map a Concerto type name to its JSON Schema equivalent.
    fn normalize_type_name(t: &str) -> &str {
        match t {
            "String" => "string",
            "Int" => "integer",
            "Float" => "number",
            "Bool" => "boolean",
            "Nil" => "null",
            other => other,
        }
    }

    /// Validate a serde_json::Value against a JSON Schema.
    fn validate_json(json: &serde_json::Value, json_schema: &serde_json::Value) -> Result<()> {
        let validator = jsonschema::validator_for(json_schema)
            .map_err(|e| RuntimeError::SchemaError(format!("invalid schema: {}", e)))?;

        let errors: Vec<String> = validator
            .iter_errors(json)
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(RuntimeError::SchemaError(errors.join("; ")))
        }
    }

    /// Build a retry prompt that includes the original prompt, the validation
    /// error, and the schema definition for the LLM to try again.
    pub fn retry_prompt(original_prompt: &str, error: &str, schema: &IrSchema) -> String {
        format!(
            "Your previous response did not match the required JSON schema.\n\
             Error: {}\n\
             Schema: {}\n\n\
             Original request: {}\n\n\
             Please respond with valid JSON matching the schema exactly.",
            error,
            serde_json::to_string_pretty(&schema.json_schema).unwrap_or_default(),
            original_prompt,
        )
    }

    /// Convert a JSON value into a schema-typed Value::Struct.
    pub fn json_to_struct(json: &serde_json::Value, type_name: &str) -> Value {
        let mut fields = HashMap::new();
        if let Some(obj) = json.as_object() {
            for (key, val) in obj {
                fields.insert(key.clone(), Self::json_to_value(val));
            }
        }
        Value::Struct {
            type_name: type_name.to_string(),
            fields,
        }
    }

    /// Recursively convert a serde_json::Value to a runtime Value.
    pub fn json_to_value(json: &serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::Nil,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Nil
                }
            }
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => {
                Value::Array(arr.iter().map(Self::json_to_value).collect())
            }
            serde_json::Value::Object(obj) => {
                let pairs: Vec<(String, Value)> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_value(v)))
                    .collect();
                Value::Map(pairs)
            }
        }
    }

    /// Maximum number of retries for schema validation.
    pub fn max_retries() -> usize {
        MAX_RETRIES
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> IrSchema {
        IrSchema {
            name: "Greeting".to_string(),
            json_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["message", "count"]
            }),
            validation_mode: "strict".to_string(),
        }
    }

    #[test]
    fn validate_valid_json() {
        let schema = test_schema();
        let json = r#"{"message": "hello", "count": 5}"#;
        let result = SchemaValidator::validate(json, &schema).unwrap();
        match result {
            Value::Struct { type_name, fields } => {
                assert_eq!(type_name, "Greeting");
                assert_eq!(fields.get("message"), Some(&Value::String("hello".into())));
                assert_eq!(fields.get("count"), Some(&Value::Int(5)));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn validate_missing_required_field() {
        let schema = test_schema();
        let json = r#"{"message": "hello"}"#;
        let result = SchemaValidator::validate(json, &schema);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("schema validation error"));
    }

    #[test]
    fn validate_wrong_type() {
        let schema = test_schema();
        let json = r#"{"message": "hello", "count": "not a number"}"#;
        let result = SchemaValidator::validate(json, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn validate_invalid_json_string() {
        let schema = test_schema();
        let result = SchemaValidator::validate("not json", &schema);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn json_to_value_conversion() {
        let json = serde_json::json!({
            "name": "test",
            "count": 42,
            "ratio": 3.14,
            "active": true,
            "tags": ["a", "b"],
            "empty": null
        });
        let val = SchemaValidator::json_to_value(&json);
        // Should be a Map (since it's an object but not going through json_to_struct)
        match val {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 6);
                // Check a few values
                let map: HashMap<String, Value> = pairs.into_iter().collect();
                assert_eq!(map.get("name"), Some(&Value::String("test".into())));
                assert_eq!(map.get("count"), Some(&Value::Int(42)));
                assert_eq!(map.get("active"), Some(&Value::Bool(true)));
                assert_eq!(map.get("empty"), Some(&Value::Nil));
            }
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn validate_with_concerto_types() {
        // Schema using Concerto type names (String, Int) instead of JSON Schema names
        let schema = IrSchema {
            name: "Greeting".to_string(),
            json_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {"type": "String"},
                    "count": {"type": "Int"}
                },
                "required": ["message", "count"]
            }),
            validation_mode: "strict".to_string(),
        };
        let json = r#"{"message": "hello", "count": 5}"#;
        let result = SchemaValidator::validate(json, &schema).unwrap();
        match result {
            Value::Struct { type_name, fields } => {
                assert_eq!(type_name, "Greeting");
                assert_eq!(fields.get("message"), Some(&Value::String("hello".into())));
                assert_eq!(fields.get("count"), Some(&Value::Int(5)));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn retry_prompt_includes_context() {
        let schema = test_schema();
        let prompt = SchemaValidator::retry_prompt("Say hello", "missing field 'count'", &schema);
        assert!(prompt.contains("missing field 'count'"));
        assert!(prompt.contains("Say hello"));
        assert!(prompt.contains("message"));
    }
}
