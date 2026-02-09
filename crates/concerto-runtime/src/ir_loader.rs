use std::collections::HashMap;

use concerto_common::ir::*;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// A pre-processed IR module ready for VM execution.
///
/// Converts the deserialized IR types into lookup tables and a
/// runtime-friendly constant pool.
#[derive(Debug)]
pub struct LoadedModule {
    /// Constant pool: IrConstant values converted to runtime Values.
    pub constants: Vec<Value>,
    /// Function lookup by name.
    pub functions: HashMap<String, IrFunction>,
    /// Agent definitions by name.
    pub agents: HashMap<String, IrAgent>,
    /// Tool definitions by name.
    pub tools: HashMap<String, IrTool>,
    /// Schema definitions by name.
    pub schemas: HashMap<String, IrSchema>,
    /// Connection configurations by name.
    pub connections: HashMap<String, IrConnection>,
    /// HashMap declarations by name.
    pub hashmaps: HashMap<String, IrHashMap>,
    /// Ledger declarations by name.
    pub ledgers: HashMap<String, IrLedger>,
    /// Memory declarations by name.
    pub memories: HashMap<String, IrMemory>,
    /// Host declarations by name.
    pub hosts: HashMap<String, IrHost>,
    /// Listen definitions by name.
    pub listens: HashMap<String, IrListen>,
    /// Pipeline definitions by name.
    pub pipelines: HashMap<String, IrPipeline>,
    /// Type definitions by name.
    pub types: HashMap<String, IrType>,
    /// Entry point function name (usually "main").
    pub entry_point: String,
}

impl LoadedModule {
    /// Load a `.conc-ir` JSON file from disk.
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let ir_module: IrModule = serde_json::from_str(&content)?;
        Self::from_ir(ir_module)
    }

    /// Convert an IrModule into a LoadedModule with lookup tables.
    pub fn from_ir(module: IrModule) -> Result<Self> {
        // Convert constant pool
        let constants: Vec<Value> = module
            .constants
            .iter()
            .map(convert_constant)
            .collect::<Result<Vec<_>>>()?;

        // Build function table
        let mut functions: HashMap<String, IrFunction> = HashMap::new();
        for func in module.functions {
            functions.insert(func.name.clone(), func);
        }

        // Register tool methods as qualified functions (e.g., "FileManager::read_file")
        for tool in &module.tools {
            for method in &tool.methods {
                let qualified = format!("{}::{}", tool.name, method.name);
                functions.insert(qualified, method.clone());
            }
        }

        // Register agent methods as qualified functions
        for agent in &module.agents {
            for method in &agent.methods {
                let qualified = format!("{}::{}", agent.name, method.name);
                functions.insert(qualified, method.clone());
            }
        }

        // Build agent table
        let agents: HashMap<String, IrAgent> = module
            .agents
            .into_iter()
            .map(|a| (a.name.clone(), a))
            .collect();

        // Build tool table
        let tools: HashMap<String, IrTool> = module
            .tools
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();

        // Build schema table
        let schemas: HashMap<String, IrSchema> = module
            .schemas
            .into_iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        // Build connection table
        let connections: HashMap<String, IrConnection> = module
            .connections
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();

        // Build hashmap table
        let hashmaps: HashMap<String, IrHashMap> = module
            .hashmaps
            .into_iter()
            .map(|d| (d.name.clone(), d))
            .collect();

        // Build ledger table
        let ledgers: HashMap<String, IrLedger> = module
            .ledgers
            .into_iter()
            .map(|l| (l.name.clone(), l))
            .collect();

        // Build memory table
        let memories: HashMap<String, IrMemory> = module
            .memories
            .into_iter()
            .map(|m| (m.name.clone(), m))
            .collect();

        // Build host table
        let hosts: HashMap<String, IrHost> = module
            .hosts
            .into_iter()
            .map(|h| (h.name.clone(), h))
            .collect();

        // Build listen table
        let listens: HashMap<String, IrListen> = module
            .listens
            .into_iter()
            .map(|l| (l.name.clone(), l))
            .collect();

        // Build pipeline table
        let pipelines: HashMap<String, IrPipeline> = module
            .pipelines
            .into_iter()
            .map(|p| (p.name.clone(), p))
            .collect();

        // Build type table
        let types: HashMap<String, IrType> = module
            .types
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();

        let entry_point = module.metadata.entry_point.clone();

        // Validate entry point exists
        if !functions.contains_key(&entry_point) {
            return Err(RuntimeError::LoadError(format!(
                "entry point '{}' not found in module",
                entry_point
            )));
        }

        Ok(LoadedModule {
            constants,
            functions,
            agents,
            tools,
            schemas,
            connections,
            hashmaps,
            ledgers,
            memories,
            hosts,
            listens,
            pipelines,
            types,
            entry_point,
        })
    }
}

/// Convert an IR constant to a runtime Value.
fn convert_constant(constant: &IrConstant) -> Result<Value> {
    match constant.const_type.as_str() {
        "int" => {
            let n = constant.value.as_i64().ok_or_else(|| {
                RuntimeError::LoadError(format!(
                    "constant {} has type 'int' but non-integer value",
                    constant.index
                ))
            })?;
            Ok(Value::Int(n))
        }
        "float" => {
            let f = constant.value.as_f64().ok_or_else(|| {
                RuntimeError::LoadError(format!(
                    "constant {} has type 'float' but non-float value",
                    constant.index
                ))
            })?;
            Ok(Value::Float(f))
        }
        "string" => {
            let s = constant.value.as_str().ok_or_else(|| {
                RuntimeError::LoadError(format!(
                    "constant {} has type 'string' but non-string value",
                    constant.index
                ))
            })?;
            Ok(Value::String(s.to_string()))
        }
        "bool" => {
            let b = constant.value.as_bool().ok_or_else(|| {
                RuntimeError::LoadError(format!(
                    "constant {} has type 'bool' but non-bool value",
                    constant.index
                ))
            })?;
            Ok(Value::Bool(b))
        }
        "nil" => Ok(Value::Nil),
        other => Err(RuntimeError::LoadError(format!(
            "unknown constant type: '{}'",
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_minimal_ir() {
        let json = r#"{
            "version": "0.1.0",
            "module": "test",
            "source_file": "test.conc",
            "constants": [
                {"index": 0, "type": "int", "value": 42},
                {"index": 1, "type": "string", "value": "hello"},
                {"index": 2, "type": "bool", "value": true},
                {"index": 3, "type": "nil", "value": null},
                {"index": 4, "type": "float", "value": 3.14}
            ],
            "functions": [
                {
                    "name": "main",
                    "module": "test",
                    "visibility": "private",
                    "params": [],
                    "return_type": "nil",
                    "is_async": false,
                    "locals": [],
                    "instructions": [
                        {"op": "LOAD_CONST", "arg": 0, "span": [1, 0]},
                        {"op": "RETURN", "span": [2, 0]}
                    ]
                }
            ],
            "metadata": {
                "compiler_version": "0.1.0",
                "compiled_at": "",
                "optimization_level": 0,
                "debug_info": true,
                "entry_point": "main"
            }
        }"#;

        let ir: IrModule = serde_json::from_str(json).unwrap();
        let module = LoadedModule::from_ir(ir).unwrap();

        assert_eq!(module.constants.len(), 5);
        assert_eq!(module.constants[0], Value::Int(42));
        assert_eq!(module.constants[1], Value::String("hello".to_string()));
        assert_eq!(module.constants[2], Value::Bool(true));
        assert_eq!(module.constants[3], Value::Nil);
        assert_eq!(module.constants[4], Value::Float(3.14));
        assert_eq!(module.entry_point, "main");
        assert!(module.functions.contains_key("main"));
    }

    #[test]
    fn missing_entry_point() {
        let json = r#"{
            "version": "0.1.0",
            "module": "test",
            "source_file": "test.conc",
            "functions": [],
            "metadata": {
                "compiler_version": "0.1.0",
                "compiled_at": "",
                "optimization_level": 0,
                "debug_info": true,
                "entry_point": "main"
            }
        }"#;

        let ir: IrModule = serde_json::from_str(json).unwrap();
        let result = LoadedModule::from_ir(ir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("entry point"));
    }
}
