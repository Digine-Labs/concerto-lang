use crate::ast::types::{TypeAnnotation, TypeKind};

/// Internal type representation for semantic analysis.
///
/// Separate from the AST `TypeAnnotation` so the semantic layer can
/// reason about types without caring about spans/syntax.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // -- Primitives --
    Int,
    Float,
    String,
    Bool,
    Nil,

    // -- Collections --
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),

    // -- Option / Result --
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),

    // -- Function --
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },

    // -- AI-specific --
    Prompt,
    Response,
    Message,
    ToolCall,
    AgentRef,
    DatabaseRef,
    LedgerRef,

    // -- User-defined (struct, enum, schema, etc.) --
    Named(std::string::String),

    // -- Unknown: not-yet-resolved / inference placeholder --
    Unknown,

    // -- Any: dynamic escape hatch --
    Any,

    // -- Error sentinel: used after error recovery to avoid cascading errors --
    Error,
}

impl Type {
    /// Convert an AST `TypeAnnotation` to the internal `Type`.
    pub fn from_annotation(ann: &TypeAnnotation) -> Self {
        match &ann.kind {
            TypeKind::Named(name) => Self::from_name(name),
            TypeKind::Generic { name, args } => {
                let type_args: Vec<Type> = args.iter().map(Type::from_annotation).collect();
                match name.as_str() {
                    "Array" if type_args.len() == 1 => {
                        Type::Array(Box::new(type_args.into_iter().next().unwrap()))
                    }
                    "Map" if type_args.len() == 2 => {
                        let mut it = type_args.into_iter();
                        Type::Map(
                            Box::new(it.next().unwrap()),
                            Box::new(it.next().unwrap()),
                        )
                    }
                    "Option" if type_args.len() == 1 => {
                        Type::Option(Box::new(type_args.into_iter().next().unwrap()))
                    }
                    "Result" if type_args.len() == 2 => {
                        let mut it = type_args.into_iter();
                        Type::Result(
                            Box::new(it.next().unwrap()),
                            Box::new(it.next().unwrap()),
                        )
                    }
                    _ => Type::Named(name.clone()),
                }
            }
            TypeKind::Tuple(elems) => {
                Type::Tuple(elems.iter().map(Type::from_annotation).collect())
            }
            TypeKind::Function {
                params,
                return_type,
            } => Type::Function {
                params: params.iter().map(Type::from_annotation).collect(),
                return_type: Box::new(Type::from_annotation(return_type)),
            },
            TypeKind::Union(_) => {
                // Union types (e.g., "legal" | "technical") are treated as String at runtime
                Type::String
            }
            TypeKind::StringLiteral(_) => {
                // String literal types are String at runtime
                Type::String
            }
            TypeKind::Inferred => Type::Unknown,
        }
    }

    /// Map a simple type name string to the corresponding `Type`.
    pub fn from_name(name: &str) -> Self {
        match name {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Bool" => Type::Bool,
            "Nil" => Type::Nil,
            "Prompt" => Type::Prompt,
            "Response" => Type::Response,
            "Message" => Type::Message,
            "ToolCall" => Type::ToolCall,
            "AgentRef" => Type::AgentRef,
            "DatabaseRef" => Type::DatabaseRef,
            "Ledger" | "LedgerRef" => Type::LedgerRef,
            "Any" => Type::Any,
            other => Type::Named(other.to_string()),
        }
    }

    /// Whether this type is numeric (`Int` or `Float`).
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    /// Whether this type is a `Result<_, _>`.
    pub fn is_result(&self) -> bool {
        matches!(self, Type::Result(_, _))
    }

    /// Whether this type is an `Option<_>`.
    pub fn is_option(&self) -> bool {
        matches!(self, Type::Option(_))
    }

    /// Whether this type can be coerced to `target`.
    pub fn can_coerce_to(&self, target: &Type) -> bool {
        if self == target {
            return true;
        }
        match (self, target) {
            // Int -> Float (widening)
            (Type::Int, Type::Float) => true,
            // String -> Prompt
            (Type::String, Type::Prompt) => true,
            // T -> Option<T>
            (t, Type::Option(inner)) => t == inner.as_ref() || t.can_coerce_to(inner),
            // Any is the universal escape hatch
            (_, Type::Any) | (Type::Any, _) => true,
            // Error / Unknown match anything (avoid cascading errors)
            (Type::Error, _) | (_, Type::Error) => true,
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            _ => false,
        }
    }

    /// Human-readable name for error messages.
    pub fn display_name(&self) -> std::string::String {
        match self {
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::String => "String".into(),
            Type::Bool => "Bool".into(),
            Type::Nil => "Nil".into(),
            Type::Array(inner) => format!("Array<{}>", inner.display_name()),
            Type::Map(k, v) => format!("Map<{}, {}>", k.display_name(), v.display_name()),
            Type::Tuple(elems) => {
                let inner: Vec<_> = elems.iter().map(|t| t.display_name()).collect();
                format!("({})", inner.join(", "))
            }
            Type::Option(inner) => format!("Option<{}>", inner.display_name()),
            Type::Result(ok, err) => {
                format!("Result<{}, {}>", ok.display_name(), err.display_name())
            }
            Type::Function {
                params,
                return_type,
            } => {
                let p: Vec<_> = params.iter().map(|t| t.display_name()).collect();
                format!("fn({}) -> {}", p.join(", "), return_type.display_name())
            }
            Type::Prompt => "Prompt".into(),
            Type::Response => "Response".into(),
            Type::Message => "Message".into(),
            Type::ToolCall => "ToolCall".into(),
            Type::AgentRef => "AgentRef".into(),
            Type::DatabaseRef => "DatabaseRef".into(),
            Type::LedgerRef => "LedgerRef".into(),
            Type::Named(n) => n.clone(),
            Type::Unknown => "unknown".into(),
            Type::Any => "Any".into(),
            Type::Error => "<error>".into(),
        }
    }
}
