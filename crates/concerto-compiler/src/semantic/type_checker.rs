use crate::ast::nodes::{BinaryOp, UnaryOp};

use super::types::Type;

/// Check a binary operation and return the result type, or an error message.
pub fn check_binary_op(left: &Type, op: BinaryOp, right: &Type) -> Result<Type, String> {
    // Error/Unknown/Any propagate without additional errors.
    if matches!(left, Type::Error | Type::Unknown) || matches!(right, Type::Error | Type::Unknown) {
        return Ok(Type::Unknown);
    }
    if matches!(left, Type::Any) || matches!(right, Type::Any) {
        return Ok(Type::Any);
    }

    match op {
        BinaryOp::Add => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Float, Type::Float) | (Type::Int, Type::Float) | (Type::Float, Type::Int) => {
                Ok(Type::Float)
            }
            (Type::String, Type::String) => Ok(Type::String),
            _ => Err(format!(
                "operator '+' cannot be applied to {} and {}",
                left.display_name(),
                right.display_name()
            )),
        },
        BinaryOp::Sub | BinaryOp::Mul => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Float, Type::Float) | (Type::Int, Type::Float) | (Type::Float, Type::Int) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operator '{}' cannot be applied to {} and {}",
                binary_op_symbol(op),
                left.display_name(),
                right.display_name()
            )),
        },
        BinaryOp::Div | BinaryOp::Mod => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Float, Type::Float) | (Type::Int, Type::Float) | (Type::Float, Type::Int) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operator '{}' cannot be applied to {} and {}",
                binary_op_symbol(op),
                left.display_name(),
                right.display_name()
            )),
        },
        BinaryOp::Eq | BinaryOp::Neq => {
            if types_comparable(left, right) {
                Ok(Type::Bool)
            } else {
                Err(format!(
                    "cannot compare {} and {} for equality",
                    left.display_name(),
                    right.display_name()
                ))
            }
        }
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Lte | BinaryOp::Gte => {
            if types_ordered(left, right) {
                Ok(Type::Bool)
            } else {
                Err(format!(
                    "operator '{}' cannot be applied to {} and {}",
                    binary_op_symbol(op),
                    left.display_name(),
                    right.display_name()
                ))
            }
        }
        BinaryOp::And | BinaryOp::Or => match (left, right) {
            (Type::Bool, Type::Bool) => Ok(Type::Bool),
            _ => Err(format!(
                "operator '{}' requires Bool operands, got {} and {}",
                binary_op_symbol(op),
                left.display_name(),
                right.display_name()
            )),
        },
    }
}

/// Check a unary operation and return the result type.
pub fn check_unary_op(op: UnaryOp, operand: &Type) -> Result<Type, String> {
    if matches!(operand, Type::Error | Type::Unknown | Type::Any) {
        return Ok(operand.clone());
    }
    match op {
        UnaryOp::Neg => match operand {
            Type::Int => Ok(Type::Int),
            Type::Float => Ok(Type::Float),
            _ => Err(format!(
                "operator '-' cannot be applied to {}",
                operand.display_name()
            )),
        },
        UnaryOp::Not => match operand {
            Type::Bool => Ok(Type::Bool),
            _ => Err(format!(
                "operator '!' requires Bool operand, got {}",
                operand.display_name()
            )),
        },
    }
}

/// Check if two types can be compared for equality.
fn types_comparable(left: &Type, right: &Type) -> bool {
    if left == right {
        return true;
    }
    if left.is_numeric() && right.is_numeric() {
        return true;
    }
    // Nil can be compared with anything (for Option checks)
    if matches!(left, Type::Nil) || matches!(right, Type::Nil) {
        return true;
    }
    false
}

/// Check if two types support ordering comparison (`<`, `>`, `<=`, `>=`).
fn types_ordered(left: &Type, right: &Type) -> bool {
    matches!(
        (left, right),
        (Type::Int, Type::Int)
            | (Type::Float, Type::Float)
            | (Type::Int, Type::Float)
            | (Type::Float, Type::Int)
            | (Type::String, Type::String)
    )
}

/// Check if a value of type `from` can be assigned to a target of type `to`.
/// Returns true for compatible types, Unknown/Any/Error pass-through.
pub fn types_assignable(from: &Type, to: &Type) -> bool {
    // Unknown/Any/Error always compatible (inference, dynamic, error recovery)
    if matches!(from, Type::Unknown | Type::Any | Type::Error)
        || matches!(to, Type::Unknown | Type::Any | Type::Error)
    {
        return true;
    }
    // Exact match
    if from == to {
        return true;
    }
    // Numeric promotion: Int assignable to Float
    if matches!((from, to), (Type::Int, Type::Float)) {
        return true;
    }
    // Nil assignable to Option
    if matches!(from, Type::Nil) && matches!(to, Type::Option(_)) {
        return true;
    }
    // Result<T, E> assignability (inner types checked recursively)
    if let (Type::Result(ft, fe), Type::Result(tt, te)) = (from, to) {
        return types_assignable(ft, tt) && types_assignable(fe, te);
    }
    // T assignable to Result<T, E> (implicit Ok wrapping, e.g. pipeline stages)
    if let Type::Result(inner, _) = to {
        if types_assignable(from, inner) {
            return true;
        }
    }
    // Option<T> assignability
    if let (Type::Option(fi), Type::Option(ti)) = (from, to) {
        return types_assignable(fi, ti);
    }
    // Array<T> assignability
    if let (Type::Array(fi), Type::Array(ti)) = (from, to) {
        return types_assignable(fi, ti);
    }
    // Named types: same name matches (we don't deep-check struct fields here)
    if let (Type::Named(a), Type::Named(b)) = (from, to) {
        return a == b;
    }
    false
}

fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "==",
        BinaryOp::Neq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::Lte => "<=",
        BinaryOp::Gte => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_arithmetic() {
        assert_eq!(
            check_binary_op(&Type::Int, BinaryOp::Add, &Type::Int),
            Ok(Type::Int)
        );
        assert_eq!(
            check_binary_op(&Type::Int, BinaryOp::Sub, &Type::Int),
            Ok(Type::Int)
        );
    }

    #[test]
    fn mixed_numeric() {
        assert_eq!(
            check_binary_op(&Type::Int, BinaryOp::Add, &Type::Float),
            Ok(Type::Float)
        );
    }

    #[test]
    fn string_concat() {
        assert_eq!(
            check_binary_op(&Type::String, BinaryOp::Add, &Type::String),
            Ok(Type::String)
        );
    }

    #[test]
    fn string_sub_fails() {
        assert!(check_binary_op(&Type::String, BinaryOp::Sub, &Type::String).is_err());
    }

    #[test]
    fn comparison_returns_bool() {
        assert_eq!(
            check_binary_op(&Type::Int, BinaryOp::Lt, &Type::Int),
            Ok(Type::Bool)
        );
        assert_eq!(
            check_binary_op(&Type::Int, BinaryOp::Eq, &Type::Float),
            Ok(Type::Bool)
        );
    }

    #[test]
    fn logical_requires_bool() {
        assert_eq!(
            check_binary_op(&Type::Bool, BinaryOp::And, &Type::Bool),
            Ok(Type::Bool)
        );
        assert!(check_binary_op(&Type::Int, BinaryOp::And, &Type::Bool).is_err());
    }

    #[test]
    fn unary_neg() {
        assert_eq!(check_unary_op(UnaryOp::Neg, &Type::Int), Ok(Type::Int));
        assert_eq!(check_unary_op(UnaryOp::Neg, &Type::Float), Ok(Type::Float));
        assert!(check_unary_op(UnaryOp::Neg, &Type::String).is_err());
    }

    #[test]
    fn unary_not() {
        assert_eq!(check_unary_op(UnaryOp::Not, &Type::Bool), Ok(Type::Bool));
        assert!(check_unary_op(UnaryOp::Not, &Type::Int).is_err());
    }

    #[test]
    fn unknown_propagates() {
        assert_eq!(
            check_binary_op(&Type::Unknown, BinaryOp::Add, &Type::Int),
            Ok(Type::Unknown)
        );
    }
}
