use std::collections::HashSet;

use concerto_common::{Diagnostic, DiagnosticBag};

use crate::ast::nodes::*;
use crate::semantic::types::Type;

/// Declaration-level validation pass.
///
/// Checks structural constraints that don't depend on name resolution
/// or type information:
///  - Function parameters must have type annotations.
///  - Agents must have a `provider` field.
///  - Tools must have a `description` field; public methods need `@describe`.
///  - Schemas/structs must not have duplicate fields.
///  - Pipelines should have at least one stage.
pub struct Validator {
    diagnostics: DiagnosticBag,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    pub fn new() -> Self {
        Self {
            diagnostics: DiagnosticBag::new(),
        }
    }

    /// Run validation on the entire program and return diagnostics.
    pub fn validate(mut self, program: &Program) -> DiagnosticBag {
        for decl in &program.declarations {
            self.validate_declaration(decl);
        }
        self.diagnostics
    }

    fn validate_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f) => self.validate_function(f),
            Declaration::Model(a) => self.validate_model(a),
            Declaration::Tool(t) => self.validate_tool(t),
            Declaration::Schema(s) => self.validate_schema(s),
            Declaration::Struct(s) => self.validate_struct(s),
            Declaration::Pipeline(p) => self.validate_pipeline(p),
            _ => {}
        }
    }

    fn validate_function(&mut self, func: &FunctionDecl) {
        let is_test = func.decorators.iter().any(|d| d.name == "test");
        if is_test {
            // @test functions must not have parameters
            if !func.params.is_empty() {
                self.diagnostics.error(
                    format!("`@test` function `{}` must not have parameters", func.name),
                    func.span.clone(),
                );
            }
            // @test functions must not have self
            if func.self_param != SelfParam::None {
                self.diagnostics.error(
                    format!("`@test` function `{}` must not have `self`", func.name),
                    func.span.clone(),
                );
            }
            // @test functions must not have return type
            if func.return_type.is_some() {
                self.diagnostics.error(
                    format!(
                        "`@test` function `{}` must not have a return type",
                        func.name
                    ),
                    func.span.clone(),
                );
            }
        }
        for param in &func.params {
            if param.type_ann.is_none() {
                self.diagnostics.error(
                    format!("parameter `{}` is missing a type annotation", param.name),
                    param.span.clone(),
                );
            }
        }
    }

    fn validate_model(&mut self, model: &ModelDecl) {
        let has_provider = model.fields.iter().any(|f| f.name == "provider");
        if !has_provider {
            self.diagnostics.report(
                Diagnostic::error(format!(
                    "model `{}` is missing required field `provider`",
                    model.name
                ))
                .with_span(model.span.clone())
                .with_suggestion(
                    "add a 'provider: <connection>' field referencing a connect block",
                ),
            );
        }
    }

    fn validate_tool(&mut self, tool: &ToolDecl) {
        let has_description = tool.fields.iter().any(|f| f.name == "description");
        if !has_description {
            self.diagnostics.report(
                Diagnostic::error(format!(
                    "tool `{}` is missing required field `description`",
                    tool.name
                ))
                .with_span(tool.span.clone())
                .with_suggestion("add a 'description: \"...\"' field to the tool declaration"),
            );
        }

        for method in &tool.methods {
            if method.is_public {
                let has_describe = method.decorators.iter().any(|d| d.name == "describe");
                if !has_describe {
                    self.diagnostics.error(
                        format!(
                            "public method `{}` in tool `{}` requires a @describe decorator",
                            method.name, tool.name
                        ),
                        method.span.clone(),
                    );
                }

                let param_count: usize = method
                    .decorators
                    .iter()
                    .filter(|d| d.name == "param")
                    .count();
                let non_self_params = method.params.len();
                if param_count > 0 && param_count != non_self_params {
                    self.diagnostics.warning(
                        format!(
                            "method `{}` has {} @param decorators but {} parameters",
                            method.name, param_count, non_self_params
                        ),
                        method.span.clone(),
                    );
                }
            }
        }
    }

    fn validate_schema(&mut self, schema: &SchemaDecl) {
        let mut seen = HashSet::new();
        for field in &schema.fields {
            if !seen.insert(&field.name) {
                self.diagnostics.error(
                    format!(
                        "duplicate field `{}` in schema `{}`",
                        field.name, schema.name
                    ),
                    field.span.clone(),
                );
            }
        }
    }

    fn validate_struct(&mut self, s: &StructDecl) {
        let mut seen = HashSet::new();
        for field in &s.fields {
            if !seen.insert(&field.name) {
                self.diagnostics.error(
                    format!("duplicate field `{}` in struct `{}`", field.name, s.name),
                    field.span.clone(),
                );
            }
        }
    }

    fn validate_pipeline(&mut self, pipeline: &PipelineDecl) {
        if pipeline.stages.is_empty() {
            self.diagnostics.warning(
                format!("pipeline `{}` has no stages", pipeline.name),
                pipeline.span.clone(),
            );
        }

        // Required return type annotations (promoted from warning to error)
        for stage in &pipeline.stages {
            if stage.return_type.is_none() {
                self.diagnostics.error(
                    format!(
                        "stage `{}` in pipeline `{}` must have a return type annotation",
                        stage.name, pipeline.name
                    ),
                    stage.span.clone(),
                );
            }
        }

        // Stage adjacency type checking
        for i in 0..pipeline.stages.len().saturating_sub(1) {
            let current = &pipeline.stages[i];
            let next = &pipeline.stages[i + 1];

            let output_type = current
                .return_type
                .as_ref()
                .map(Type::from_annotation)
                .unwrap_or(Type::Any);

            if let Some(first_param) = next.params.first() {
                let input_type = first_param
                    .type_ann
                    .as_ref()
                    .map(Type::from_annotation)
                    .unwrap_or(Type::Any);

                if !Type::is_pipeline_assignable(&output_type, &input_type) {
                    self.diagnostics.report(
                        Diagnostic::error(format!(
                            "pipeline `{}` stage type mismatch: `{}` returns `{}` but `{}` expects `{}`",
                            pipeline.name,
                            current.name,
                            output_type.display_name(),
                            next.name,
                            input_type.display_name()
                        ))
                        .with_span(next.params[0].span.clone())
                        .with_suggestion(
                            "align stage signatures or insert a conversion stage",
                        ),
                    );
                }
            }
        }

        // Pipeline-level signature validation
        if let Some(ref input_param) = pipeline.input_param {
            if let Some(first_stage) = pipeline.stages.first() {
                if let Some(first_param) = first_stage.params.first() {
                    let pipeline_input = input_param
                        .type_ann
                        .as_ref()
                        .map(Type::from_annotation)
                        .unwrap_or(Type::Any);
                    let stage_input = first_param
                        .type_ann
                        .as_ref()
                        .map(Type::from_annotation)
                        .unwrap_or(Type::Any);

                    if !Type::is_pipeline_assignable(&pipeline_input, &stage_input) {
                        self.diagnostics.error(
                            format!(
                                "pipeline `{}` input type mismatch: pipeline declares `{}` but first stage `{}` expects `{}`",
                                pipeline.name,
                                pipeline_input.display_name(),
                                first_stage.name,
                                stage_input.display_name()
                            ),
                            first_param.span.clone(),
                        );
                    }
                }
            }
        }

        if let Some(ref ret_ann) = pipeline.return_type {
            let pipeline_output = Type::from_annotation(ret_ann);
            if let Some(last_stage) = pipeline.stages.last() {
                let stage_output = last_stage
                    .return_type
                    .as_ref()
                    .map(Type::from_annotation)
                    .unwrap_or(Type::Any);

                if !Type::is_pipeline_assignable(&stage_output, &pipeline_output) {
                    self.diagnostics.error(
                        format!(
                            "pipeline `{}` output type mismatch: last stage `{}` returns `{}` but pipeline declares `{}`",
                            pipeline.name,
                            last_stage.name,
                            stage_output.display_name(),
                            pipeline_output.display_name()
                        ),
                        last_stage.span.clone(),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::Lexer;
    use crate::parser;
    use concerto_common::Severity;

    fn validate(source: &str) -> Vec<(Severity, String)> {
        let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(!lex_diags.has_errors(), "lexer errors: {:?}", lex_diags);
        let (program, parse_diags) = parser::Parser::new(tokens).parse();
        assert!(
            !parse_diags.has_errors(),
            "parser errors: {:?}",
            parse_diags
        );
        let diags = super::Validator::new().validate(&program);
        diags
            .into_diagnostics()
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect()
    }

    fn val_errors(source: &str) -> Vec<String> {
        validate(source)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Error)
            .map(|(_, m)| m)
            .collect()
    }

    fn val_warnings(source: &str) -> Vec<String> {
        validate(source)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Warning)
            .map(|(_, m)| m)
            .collect()
    }

    #[test]
    fn function_param_missing_type() {
        let errs = val_errors("fn foo(x) { }");
        assert!(errs.iter().any(|e| e.contains("missing a type annotation")));
    }

    #[test]
    fn function_param_with_type_ok() {
        let errs = val_errors("fn foo(x: Int) { }");
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn agent_missing_provider() {
        let errs = val_errors(
            r#"
            model MyAgent {
                base: "gpt-4o",
            }
            "#,
        );
        assert!(errs
            .iter()
            .any(|e| e.contains("missing required field `provider`")));
    }

    #[test]
    fn agent_with_provider_ok() {
        let errs = val_errors(
            r#"
            model MyAgent {
                provider: openai,
                base: "gpt-4o",
            }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn tool_missing_description() {
        let errs = val_errors(
            r#"
            tool MyTool {
                fn run(self) { }
            }
            "#,
        );
        assert!(errs
            .iter()
            .any(|e| e.contains("missing required field `description`")));
    }

    #[test]
    fn tool_pub_method_missing_describe() {
        let errs = val_errors(
            r#"
            tool MyTool {
                description: "a tool",
                pub fn run(self) { }
            }
            "#,
        );
        assert!(errs
            .iter()
            .any(|e| e.contains("requires a @describe decorator")));
    }

    #[test]
    fn tool_pub_method_with_describe_ok() {
        let errs = val_errors(
            r#"
            tool MyTool {
                description: "a tool",
                @describe("does something")
                pub fn run(self) { }
            }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn pipeline_empty_warning() {
        let warns = val_warnings(
            r#"
            pipeline Empty {
            }
            "#,
        );
        assert!(warns.iter().any(|w| w.contains("has no stages")));
    }

    /// Helper: return full Diagnostic objects from validator.
    fn full_val_diagnostics(source: &str) -> Vec<concerto_common::Diagnostic> {
        let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(!lex_diags.has_errors(), "lexer errors: {:?}", lex_diags);
        let (program, parse_diags) = parser::Parser::new(tokens).parse();
        assert!(
            !parse_diags.has_errors(),
            "parser errors: {:?}",
            parse_diags
        );
        let diags = super::Validator::new().validate(&program);
        diags.into_diagnostics()
    }

    // ===== pipeline type contract tests =====

    #[test]
    fn pipeline_stage_missing_return_type_is_error() {
        let errs = val_errors(
            r#"
            pipeline P {
                stage s(x: String) {
                    return x;
                }
            }
            "#,
        );
        assert!(
            errs.iter().any(|e| e.contains("must have a return type")),
            "expected return type error, got: {:?}",
            errs
        );
    }

    #[test]
    fn pipeline_stage_adjacency_mismatch() {
        let errs = val_errors(
            r#"
            pipeline P {
                stage a(x: String) -> Int {
                    return 42;
                }
                stage b(y: String) -> String {
                    return y;
                }
            }
            "#,
        );
        assert!(
            errs.iter().any(|e| e.contains("type mismatch") && e.contains("Int") && e.contains("String")),
            "expected adjacency type mismatch error, got: {:?}",
            errs
        );
    }

    #[test]
    fn pipeline_stage_adjacency_result_unwrap() {
        // Result<Int, Error> output → Int input should be OK (runtime unwraps)
        let errs = val_errors(
            r#"
            pipeline P {
                stage a(x: String) -> Result<Int, String> {
                    return Ok(42);
                }
                stage b(y: Int) -> Int {
                    return y;
                }
            }
            "#,
        );
        let adjacency_errors: Vec<_> = errs.iter().filter(|e| e.contains("type mismatch")).collect();
        assert!(
            adjacency_errors.is_empty(),
            "Result<Int, String> → Int should be compatible, got: {:?}",
            adjacency_errors
        );
    }

    #[test]
    fn pipeline_stage_adjacency_any_accepts_all() {
        let errs = val_errors(
            r#"
            pipeline P {
                stage a(x: String) -> Int {
                    return 42;
                }
                stage b(y: Any) -> String {
                    return "ok";
                }
            }
            "#,
        );
        let adjacency_errors: Vec<_> = errs.iter().filter(|e| e.contains("type mismatch")).collect();
        assert!(
            adjacency_errors.is_empty(),
            "Any input should accept Int output, got: {:?}",
            adjacency_errors
        );
    }

    #[test]
    fn pipeline_signature_input_mismatch() {
        let errs = val_errors(
            r#"
            pipeline P(input: String) -> Int {
                stage s(x: Int) -> Int {
                    return x;
                }
            }
            "#,
        );
        assert!(
            errs.iter().any(|e| e.contains("input type mismatch")),
            "expected pipeline input type mismatch, got: {:?}",
            errs
        );
    }

    #[test]
    fn pipeline_signature_output_mismatch() {
        let errs = val_errors(
            r#"
            pipeline P(input: String) -> Int {
                stage s(x: String) -> String {
                    return x;
                }
            }
            "#,
        );
        assert!(
            errs.iter().any(|e| e.contains("output type mismatch")),
            "expected pipeline output type mismatch, got: {:?}",
            errs
        );
    }

    #[test]
    fn pipeline_signature_valid() {
        let errs = val_errors(
            r#"
            pipeline P(input: String) -> Int {
                stage parse(x: String) -> Int {
                    return 42;
                }
            }
            "#,
        );
        assert!(errs.is_empty(), "expected no errors, got: {:?}", errs);
    }

    #[test]
    fn pipeline_adjacency_compatible_types_ok() {
        let errs = val_errors(
            r#"
            pipeline P {
                stage a(x: String) -> Int {
                    return 42;
                }
                stage b(y: Int) -> String {
                    return "done";
                }
            }
            "#,
        );
        let adjacency_errors: Vec<_> = errs.iter().filter(|e| e.contains("type mismatch")).collect();
        assert!(
            adjacency_errors.is_empty(),
            "Int → Int should be compatible, got: {:?}",
            adjacency_errors
        );
    }

    #[test]
    fn agent_missing_provider_has_suggestion() {
        let diags = full_val_diagnostics(
            r#"
            model MyAgent {
                base: "gpt-4o",
            }
            "#,
        );
        let diag = diags
            .iter()
            .find(|d| d.message.contains("missing required field `provider`"))
            .expect("should have missing provider error");
        assert!(
            diag.suggestion
                .as_ref()
                .is_some_and(|s| s.contains("connect")),
            "expected suggestion mentioning 'connect', got: {:?}",
            diag.suggestion
        );
    }
}
