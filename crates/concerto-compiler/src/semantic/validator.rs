use std::collections::HashSet;

use concerto_common::{Diagnostic, DiagnosticBag};

use crate::ast::nodes::*;

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
            Declaration::Agent(a) => self.validate_agent(a),
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

    fn validate_agent(&mut self, agent: &AgentDecl) {
        let has_provider = agent.fields.iter().any(|f| f.name == "provider");
        if !has_provider {
            self.diagnostics.report(
                Diagnostic::error(format!(
                    "agent `{}` is missing required field `provider`",
                    agent.name
                ))
                .with_span(agent.span.clone())
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

        for stage in &pipeline.stages {
            if stage.return_type.is_none() {
                self.diagnostics.warning(
                    format!(
                        "stage `{}` in pipeline `{}` has no return type annotation",
                        stage.name, pipeline.name
                    ),
                    stage.span.clone(),
                );
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
            agent MyAgent {
                model: "gpt-4o",
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
            agent MyAgent {
                provider: openai,
                model: "gpt-4o",
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

    #[test]
    fn agent_missing_provider_has_suggestion() {
        let diags = full_val_diagnostics(
            r#"
            agent MyAgent {
                model: "gpt-4o",
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
