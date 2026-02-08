pub mod errors;
pub mod ir;
pub mod ir_opcodes;
pub mod span;

pub use errors::{Diagnostic, DiagnosticBag, Severity};
pub use ir::IrModule;
pub use ir_opcodes::Opcode;
pub use span::{Position, Span};
