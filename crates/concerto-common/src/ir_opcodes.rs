use serde::{Deserialize, Serialize};

/// All IR opcodes for the Concerto stack-based VM.
///
/// See spec/16-ir-specification.md for full documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Opcode {
    // === Stack Operations ===
    /// Push immediate value onto stack.
    Push,
    /// Pop top of stack.
    Pop,
    /// Duplicate top of stack.
    Dup,
    /// Swap top two stack values.
    Swap,

    // === Constants ===
    /// Push constant from pool onto stack (arg: index).
    LoadConst,

    // === Variables ===
    /// Push local variable value onto stack (arg: name).
    LoadLocal,
    /// Pop stack top and store in local variable (arg: name).
    StoreLocal,
    /// Push global/module variable onto stack (arg: name).
    LoadGlobal,
    /// Store in global/module variable (arg: name).
    StoreGlobal,

    // === Arithmetic ===
    /// Pop two, push sum.
    Add,
    /// Pop two, push difference.
    Sub,
    /// Pop two, push product.
    Mul,
    /// Pop two, push quotient.
    Div,
    /// Pop two, push remainder.
    Mod,
    /// Pop one, push negation.
    Neg,

    // === Comparison ===
    /// Pop two, push equality result (Bool).
    Eq,
    /// Pop two, push inequality result.
    Neq,
    /// Pop two, push less-than result.
    Lt,
    /// Pop two, push greater-than result.
    Gt,
    /// Pop two, push less-or-equal result.
    Lte,
    /// Pop two, push greater-or-equal result.
    Gte,

    // === Logical ===
    /// Pop two, push logical AND.
    And,
    /// Pop two, push logical OR.
    Or,
    /// Pop one, push logical NOT.
    Not,

    // === Control Flow ===
    /// Unconditional jump to instruction offset.
    Jump,
    /// Jump if stack top is true (pops).
    JumpIfTrue,
    /// Jump if stack top is false (pops).
    JumpIfFalse,
    /// Return from function (stack top is return value).
    Return,

    // === Function Calls ===
    /// Call function with argc args from stack.
    Call,
    /// Call method on stack-top object.
    CallMethod,
    /// Call native/built-in function.
    CallNative,

    // === Model Operations ===
    /// Call model method (prompt on stack).
    CallModel,
    /// Call model with schema validation.
    CallModelSchema,
    /// Call model in streaming mode.
    CallModelStream,
    /// Call model with message history.
    CallModelChat,

    // === Tool Operations ===
    /// Invoke tool method.
    CallTool,

    // === HashMap Operations ===
    /// Get value (key on stack, pushes Option).
    HashMapGet,
    /// Set value (key and value on stack).
    HashMapSet,
    /// Delete entry (key on stack).
    HashMapDelete,
    /// Check existence (key on stack, pushes Bool).
    HashMapHas,
    /// Query with predicate (closure on stack).
    HashMapQuery,

    // === Emit ===
    /// Fire-and-forget emit (channel and payload on stack).
    Emit,
    /// Bidirectional emit (channel and payload on stack, pushes response).
    EmitAwait,

    // === Error Handling ===
    /// Mark start of try block, register catch handler.
    TryBegin,
    /// Mark end of try block (no error occurred).
    TryEnd,
    /// Begin catch block for specific error type.
    Catch,
    /// Throw error (error value on stack).
    Throw,
    /// `?` operator: unwrap Ok or return Err.
    Propagate,

    // === Object / Array / Map ===
    /// Pop count values, push Array.
    BuildArray,
    /// Pop count key-value pairs, push Map.
    BuildMap,
    /// Pop count field values, push Struct.
    BuildStruct,
    /// Pop object, push field value.
    FieldGet,
    /// Pop object and value, set field.
    FieldSet,
    /// Pop collection and index, push value.
    IndexGet,
    /// Pop collection, index, and value; set index.
    IndexSet,

    // === Type Operations ===
    /// Pop value, push Bool (is instance?).
    CheckType,
    /// Pop value, push casted value (or error).
    Cast,

    // === Async Operations ===
    /// Await async value on stack top.
    Await,
    /// Await count async values, push tuple of results.
    AwaitAll,
    /// Spawn async task from closure on stack.
    SpawnAsync,

    // === Agent Streaming ===
    /// Begin listen loop on agent. Pops argc args from stack.
    /// name: agent name, arg: listen definition reference.
    ListenBegin,

    // === Testing ===
    /// Install a mock response for a model during test execution.
    /// name: model name, arg: mock config JSON (response/error fields).
    MockModel,
}
