use concerto_common::Span;

/// A type annotation in the source code (e.g., `Int`, `Array<String>`, `Result<T, E>`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub kind: TypeKind,
    pub span: Span,
}

/// The kinds of type annotations.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// Simple named type: `Int`, `String`, `MyStruct`
    Named(String),

    /// Generic type: `Array<Int>`, `Map<String, Any>`, `Result<T, E>`
    Generic {
        name: String,
        args: Vec<TypeAnnotation>,
    },

    /// Tuple type: `(Int, String)`
    Tuple(Vec<TypeAnnotation>),

    /// Function type: `fn(Int, Int) -> Bool`
    Function {
        params: Vec<TypeAnnotation>,
        return_type: Box<TypeAnnotation>,
    },

    /// Optional shorthand - currently represented via Generic("Option", [T])
    /// but may get sugar later.

    /// Union type: `"legal" | "technical" | "financial"`
    Union(Vec<TypeAnnotation>),

    /// String literal type: `"legal"` (used in union types)
    StringLiteral(String),

    /// Inferred type (no annotation given).
    Inferred,
}
