use serde::{Deserialize, Serialize};

use crate::ir_opcodes::Opcode;

/// Top-level IR module, the output of compilation.
/// Serialized as the `.conc-ir` JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrModule {
    pub version: String,
    pub module: String,
    pub source_file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constants: Vec<IrConstant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<IrType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<IrFunction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<IrAgent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<IrTool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<IrSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<IrConnection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub databases: Vec<IrDatabase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipelines: Vec<IrPipeline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ledgers: Vec<IrLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_map: Option<IrSourceMap>,
    pub metadata: IrMetadata,
}

/// A constant in the constant pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrConstant {
    pub index: u32,
    #[serde(rename = "type")]
    pub const_type: String,
    pub value: serde_json::Value,
}

/// A type definition (schema, struct, or enum).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrType {
    pub name: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<IrTypeField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<IrEnumVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrTypeField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrEnumVariant {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<IrTypeField>,
}

/// A compiled function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrFunction {
    pub name: String,
    pub module: String,
    pub visibility: String,
    pub params: Vec<IrParam>,
    pub return_type: serde_json::Value,
    pub is_async: bool,
    pub locals: Vec<String>,
    pub instructions: Vec<IrInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrParam {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: serde_json::Value,
}

/// A single IR instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInstruction {
    pub op: Opcode,
    /// Instruction argument (varies by opcode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg: Option<serde_json::Value>,
    /// Local variable name (for LOAD_LOCAL, STORE_LOCAL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Agent name (for CALL_AGENT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Method name (for CALL_METHOD, CALL_AGENT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Schema name (for CALL_AGENT_SCHEMA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Tool name (for CALL_TOOL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Database name (for DB_* ops).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_name: Option<String>,
    /// Type name (for CHECK_TYPE, CAST, BUILD_STRUCT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// Argument count (for CALL, CALL_METHOD, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argc: Option<u32>,
    /// Jump offset (for JUMP, JUMP_IF_TRUE, JUMP_IF_FALSE, TRY_BEGIN).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i32>,
    /// Element count (for BUILD_ARRAY, BUILD_MAP, AWAIT_ALL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Source span [line, column].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<[u32; 2]>,
}

/// An agent definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrAgent {
    pub name: String,
    pub module: String,
    pub connection: String,
    pub config: IrAgentConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<IrDecorator>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<IrFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrAgentConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrDecorator {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

/// A tool definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrTool {
    pub name: String,
    pub module: String,
    pub methods: Vec<IrFunction>,
}

/// A schema definition in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrSchema {
    pub name: String,
    pub json_schema: serde_json::Value,
    #[serde(default = "default_validation_mode")]
    pub validation_mode: String,
}

fn default_validation_mode() -> String {
    "strict".to_string()
}

/// A connection (LLM provider) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrConnection {
    pub name: String,
    pub config: serde_json::Value,
}

/// An in-memory database declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrDatabase {
    pub name: String,
    pub key_type: String,
    pub value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<String>,
}

/// A ledger declaration (fault-tolerant knowledge store).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrLedger {
    pub name: String,
}

/// A pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrPipeline {
    pub name: String,
    pub stages: Vec<IrPipelineStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrPipelineStage {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<IrParam>,
    pub input_type: serde_json::Value,
    pub output_type: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<IrDecorator>,
    pub instructions: Vec<IrInstruction>,
}

/// Source map for instruction-to-source mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrSourceMap {
    pub file: String,
    pub mappings: Vec<IrSourceMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrSourceMapping {
    pub instruction: u32,
    pub line: u32,
    pub column: u32,
}

/// Compilation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrMetadata {
    pub compiler_version: String,
    pub compiled_at: String,
    #[serde(default)]
    pub optimization_level: u32,
    #[serde(default)]
    pub debug_info: bool,
    pub entry_point: String,
}
