pub mod builtins;
pub mod decorator;
pub mod error;
pub mod ir_loader;
pub mod ledger;
pub mod mcp;
pub mod provider;
pub mod providers;
pub mod schema;
pub mod tool;
pub mod value;
pub mod vm;

pub use error::RuntimeError;
pub use ir_loader::LoadedModule;
pub use value::Value;
pub use vm::VM;

/// Load and execute a .conc-ir file, returning the result value.
pub fn run_file(path: &str) -> error::Result<Value> {
    let module = LoadedModule::load_from_file(path)?;
    let mut vm = VM::new(module);
    vm.execute()
}
