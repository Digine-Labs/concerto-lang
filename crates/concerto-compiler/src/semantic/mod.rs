pub mod resolver;
pub mod scope;
pub mod type_checker;
pub mod types;
pub mod validator;

use concerto_common::DiagnosticBag;

use crate::ast::nodes::Program;

/// Run all semantic analysis passes on the given program.
///
/// Returns a `DiagnosticBag` containing any errors and warnings found.
/// The analysis performs:
///  1. Name resolution (with forward references for top-level decls)
///  2. Basic type checking (operators, conditions, function signatures)
///  3. Control-flow validation (break/continue in loops, ? in Result fns, etc.)
///  4. Declaration-level validation (agent/tool/schema field rules)
///  5. Unused variable warnings
pub fn analyze(program: &Program) -> DiagnosticBag {
    let mut all_diagnostics = DiagnosticBag::new();

    // Pass 1–3: name resolution + type checking + control-flow validation.
    let resolve_diags = resolver::Resolver::new().resolve(program);
    for diag in resolve_diags.into_diagnostics() {
        all_diagnostics.report(diag);
    }

    // Pass 4: declaration-level structural validation.
    let validate_diags = validator::Validator::new().validate(program);
    for diag in validate_diags.into_diagnostics() {
        all_diagnostics.report(diag);
    }

    all_diagnostics
}
