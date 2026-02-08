use std::collections::HashMap;

use concerto_common::Span;

use super::types::Type;

/// The kind of a declared symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Function,
    Parameter,
    Agent,
    Host,
    Tool,
    Schema,
    Struct,
    Enum,
    Trait,
    Const,
    TypeAlias,
    Module,
    Connection,
    HashMap,
    Pipeline,
    Ledger,
    Memory,
    Mcp,
}

/// A declared symbol in the program.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub ty: Type,
    pub mutable: bool,
    pub defined_at: Span,
    pub used: bool,
    pub is_public: bool,
}

/// The kind of scope, which affects control flow rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Function,
    Block,
    Loop,
}

/// A lexical scope containing symbol declarations.
#[derive(Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub symbols: HashMap<String, Symbol>,
    parent: Option<usize>,
}

/// Stack of nested scopes for lexical scoping.
///
/// Scopes are stored in a flat `Vec` and linked by parent indices.
/// `push` creates a child of the current scope; `pop` returns to the parent.
#[derive(Debug)]
pub struct ScopeStack {
    scopes: Vec<Scope>,
    current: usize,
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeStack {
    pub fn new() -> Self {
        let global = Scope {
            kind: ScopeKind::Global,
            symbols: HashMap::new(),
            parent: None,
        };
        Self {
            scopes: vec![global],
            current: 0,
        }
    }

    /// Push a new child scope of the given kind.
    pub fn push(&mut self, kind: ScopeKind) {
        let parent = self.current;
        let idx = self.scopes.len();
        self.scopes.push(Scope {
            kind,
            symbols: HashMap::new(),
            parent: Some(parent),
        });
        self.current = idx;
    }

    /// Pop the current scope, returning its index (for later inspection).
    pub fn pop(&mut self) -> usize {
        let old = self.current;
        self.current = self.scopes[old].parent.expect("cannot pop global scope");
        old
    }

    /// Get a scope by index (for reading after pop).
    pub fn get_scope(&self, idx: usize) -> &Scope {
        &self.scopes[idx]
    }

    /// Define a symbol in the current scope.
    /// Returns `Err` with the previous definition's span on duplicate.
    pub fn define(&mut self, symbol: Symbol) -> Result<(), Span> {
        let scope = &mut self.scopes[self.current];
        if let Some(existing) = scope.symbols.get(&symbol.name) {
            return Err(existing.defined_at.clone());
        }
        scope.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Look up a symbol by name, walking up the scope chain.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut idx = self.current;
        loop {
            if let Some(sym) = self.scopes[idx].symbols.get(name) {
                return Some(sym);
            }
            match self.scopes[idx].parent {
                Some(parent) => idx = parent,
                None => return None,
            }
        }
    }

    /// Look up a symbol mutably by name, walking up the scope chain.
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        // Find which scope contains the name first (immutable walk).
        let mut idx = self.current;
        let target_idx = loop {
            if self.scopes[idx].symbols.contains_key(name) {
                break idx;
            }
            match self.scopes[idx].parent {
                Some(parent) => idx = parent,
                None => return None,
            }
        };
        self.scopes[target_idx].symbols.get_mut(name)
    }

    /// Check if we are inside a loop (at any nesting depth).
    pub fn in_loop(&self) -> bool {
        let mut idx = self.current;
        loop {
            if self.scopes[idx].kind == ScopeKind::Loop {
                return true;
            }
            match self.scopes[idx].parent {
                Some(parent) => idx = parent,
                None => return false,
            }
        }
    }

    /// Check if we are inside a function (at any nesting depth).
    pub fn in_function(&self) -> bool {
        let mut idx = self.current;
        loop {
            if self.scopes[idx].kind == ScopeKind::Function {
                return true;
            }
            match self.scopes[idx].parent {
                Some(parent) => idx = parent,
                None => return false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_symbol(name: &str, mutable: bool) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Variable,
            ty: Type::Int,
            mutable,
            defined_at: Span::dummy(),
            used: false,
            is_public: false,
        }
    }

    #[test]
    fn define_and_lookup() {
        let mut stack = ScopeStack::new();
        stack.define(dummy_symbol("x", false)).unwrap();
        assert!(stack.lookup("x").is_some());
        assert!(stack.lookup("y").is_none());
    }

    #[test]
    fn nested_scope_lookup() {
        let mut stack = ScopeStack::new();
        stack.define(dummy_symbol("x", false)).unwrap();
        stack.push(ScopeKind::Block);
        // Should find x from parent
        assert!(stack.lookup("x").is_some());
        stack.define(dummy_symbol("y", false)).unwrap();
        assert!(stack.lookup("y").is_some());
        stack.pop();
        // y no longer visible
        assert!(stack.lookup("y").is_none());
    }

    #[test]
    fn duplicate_definition() {
        let mut stack = ScopeStack::new();
        stack.define(dummy_symbol("x", false)).unwrap();
        assert!(stack.define(dummy_symbol("x", false)).is_err());
    }

    #[test]
    fn shadow_in_child_scope() {
        let mut stack = ScopeStack::new();
        stack.define(dummy_symbol("x", false)).unwrap();
        stack.push(ScopeKind::Block);
        // Shadowing in child scope is OK
        stack.define(dummy_symbol("x", true)).unwrap();
        let sym = stack.lookup("x").unwrap();
        assert!(sym.mutable); // gets the shadowed version
        stack.pop();
        let sym = stack.lookup("x").unwrap();
        assert!(!sym.mutable); // back to original
    }

    #[test]
    fn in_loop_detection() {
        let mut stack = ScopeStack::new();
        assert!(!stack.in_loop());
        stack.push(ScopeKind::Function);
        assert!(!stack.in_loop());
        stack.push(ScopeKind::Loop);
        assert!(stack.in_loop());
        stack.push(ScopeKind::Block);
        assert!(stack.in_loop()); // still in loop (nested block)
        stack.pop();
        stack.pop();
        assert!(!stack.in_loop()); // back to function
    }

    #[test]
    fn in_function_detection() {
        let mut stack = ScopeStack::new();
        assert!(!stack.in_function());
        stack.push(ScopeKind::Function);
        assert!(stack.in_function());
        stack.push(ScopeKind::Block);
        assert!(stack.in_function());
    }
}
