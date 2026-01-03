//! AST-based expansion of `@rtest.mark.cases` and `@pytest.mark.parametrize` decorators.
//!
//! This module extracts test case information from decorator AST nodes and expands
//! parametrized tests into individual test cases during collection.

use ruff_python_ast::{Decorator, Expr, ExprAttribute, ExprList, ExprName, ExprTuple, Keyword};

/// A literal value that can be statically extracted from AST.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    None,
    /// A tuple/list of literal values (for multi-param cases like `(1, "a")`).
    Sequence(Vec<LiteralValue>),
}

/// Specification for a single `@cases` or `@parametrize` decorator.
#[derive(Debug, Clone)]
pub struct CasesSpec {
    /// Argument names, e.g., `["x"]` or `["x", "y"]`.
    /// Note: Currently used for validation; will be used in future phases for value association.
    #[allow(dead_code)]
    pub argnames: Vec<String>,
    /// Argument values as literals.
    pub argvalues: Vec<LiteralValue>,
    /// Optional custom IDs for each case.
    pub ids: Option<Vec<String>>,
}

/// Reason why cases could not be statically expanded.
#[derive(Debug, Clone)]
pub enum CannotExpandReason {
    /// Argvalues references a variable, e.g., `DATA`.
    VariableReference(String),
    /// Argvalues contains a function call, e.g., `get_data()`.
    FunctionCall(String),
    /// Argvalues contains a list/dict/set comprehension.
    Comprehension,
    /// Catch-all for other unsupported expressions.
    UnsupportedExpression(String),
}

impl std::fmt::Display for CannotExpandReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VariableReference(name) => {
                write!(f, "argvalues references variable '{}'", name)
            }
            Self::FunctionCall(name) => {
                write!(f, "argvalues contains function call '{}'", name)
            }
            Self::Comprehension => {
                write!(f, "argvalues contains a comprehension")
            }
            Self::UnsupportedExpression(desc) => {
                write!(f, "argvalues contains unsupported expression: {}", desc)
            }
        }
    }
}

/// Result of attempting to expand test cases from decorators.
#[derive(Debug, Clone)]
pub enum CasesExpansion {
    /// No `@cases` or `@parametrize` decorators found.
    NotDecorated,
    /// Successfully expanded to multiple test cases.
    Expanded(Vec<ExpandedCase>),
    /// Cannot statically expand; fall back to base test name.
    CannotExpand(CannotExpandReason),
}

/// A single expanded test case.
#[derive(Debug, Clone)]
pub struct ExpandedCase {
    /// The case ID suffix, e.g., `"0"`, `"a-b"`, `"my_custom_id"`.
    pub case_id: String,
}

/// Format a warning message for tests that cannot be statically expanded.
pub fn format_cannot_expand_warning(nodeid: &str, reason: &CannotExpandReason) -> String {
    format!(
        "warning: Cannot statically expand test cases for '{}': {}",
        nodeid, reason
    )
}

/// Parse decorators and return the cases expansion result.
pub fn parse_decorators_for_cases(decorators: &[Decorator]) -> CasesExpansion {
    let mut specs = Vec::new();

    for decorator in decorators {
        match parse_single_decorator(decorator) {
            DecoratorParseResult::CasesSpec(spec) => specs.push(spec),
            DecoratorParseResult::CannotExpand(reason) => {
                return CasesExpansion::CannotExpand(reason);
            }
            DecoratorParseResult::NotCasesDecorator => {}
        }
    }

    if specs.is_empty() {
        CasesExpansion::NotDecorated
    } else {
        CasesExpansion::Expanded(expand_cases(&specs))
    }
}

/// Result of parsing a single decorator.
enum DecoratorParseResult {
    /// Successfully parsed a cases/parametrize decorator.
    CasesSpec(CasesSpec),
    /// Recognized as cases decorator but cannot expand.
    CannotExpand(CannotExpandReason),
    /// Not a cases/parametrize decorator.
    NotCasesDecorator,
}

/// Parse a single decorator to extract cases information.
fn parse_single_decorator(decorator: &Decorator) -> DecoratorParseResult {
    let Expr::Call(call) = &decorator.expression else {
        return DecoratorParseResult::NotCasesDecorator;
    };

    if !is_cases_or_parametrize_call(&call.func) {
        return DecoratorParseResult::NotCasesDecorator;
    }

    if call.arguments.args.len() < 2 {
        return DecoratorParseResult::CannotExpand(CannotExpandReason::UnsupportedExpression(
            "missing required arguments".to_string(),
        ));
    }

    let argnames = match extract_argnames(&call.arguments.args[0]) {
        Ok(names) => names,
        Err(reason) => return DecoratorParseResult::CannotExpand(reason),
    };

    let argvalues = match extract_argvalues(&call.arguments.args[1]) {
        Ok(values) => values,
        Err(reason) => return DecoratorParseResult::CannotExpand(reason),
    };

    let ids = extract_ids_kwarg(&call.arguments.keywords);

    DecoratorParseResult::CasesSpec(CasesSpec {
        argnames,
        argvalues,
        ids,
    })
}

/// Check if the call func is `rtest.mark.cases` or `pytest.mark.parametrize`.
fn is_cases_or_parametrize_call(func: &Expr) -> bool {
    let Expr::Attribute(ExprAttribute { attr, value, .. }) = func else {
        return false;
    };

    let decorator_name = attr.as_str();
    if decorator_name != "cases" && decorator_name != "parametrize" {
        return false;
    }

    let Expr::Attribute(ExprAttribute {
        attr: mark_attr,
        value: module_value,
        ..
    }) = value.as_ref()
    else {
        return false;
    };

    if mark_attr.as_str() != "mark" {
        return false;
    }

    let Expr::Name(ExprName {
        id: module_name, ..
    }) = module_value.as_ref()
    else {
        return false;
    };

    let module = module_name.as_str();
    (module == "rtest" && decorator_name == "cases")
        || (module == "pytest" && decorator_name == "parametrize")
}

/// Extract argument names from the first decorator argument.
fn extract_argnames(expr: &Expr) -> Result<Vec<String>, CannotExpandReason> {
    match expr {
        Expr::StringLiteral(s) => {
            let names: Vec<String> = s
                .value
                .to_str()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if names.is_empty() {
                Err(CannotExpandReason::UnsupportedExpression(
                    "empty argnames".to_string(),
                ))
            } else {
                Ok(names)
            }
        }
        Expr::Name(name) => Err(CannotExpandReason::VariableReference(name.id.to_string())),
        _ => Err(CannotExpandReason::UnsupportedExpression(
            "argnames must be a string".to_string(),
        )),
    }
}

/// Extract argument values from the second decorator argument.
fn extract_argvalues(expr: &Expr) -> Result<Vec<LiteralValue>, CannotExpandReason> {
    match expr {
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            let mut values = Vec::with_capacity(elts.len());
            for elt in elts.iter() {
                values.push(extract_literal(elt)?);
            }
            Ok(values)
        }
        Expr::Name(name) => Err(CannotExpandReason::VariableReference(name.id.to_string())),
        Expr::Call(call) => {
            let func_name = get_call_name(&call.func);
            Err(CannotExpandReason::FunctionCall(func_name))
        }
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
            Err(CannotExpandReason::Comprehension)
        }
        _ => Err(CannotExpandReason::UnsupportedExpression(
            "argvalues must be a list or tuple".to_string(),
        )),
    }
}

/// Extract a literal value from an expression.
fn extract_literal(expr: &Expr) -> Result<LiteralValue, CannotExpandReason> {
    match expr {
        Expr::NumberLiteral(num) => {
            use ruff_python_ast::Number;
            match &num.value {
                Number::Int(i) => {
                    // Try to convert to i64, fall back to string representation for large ints
                    match i.as_i64() {
                        Some(v) => Ok(LiteralValue::Int(v)),
                        None => Ok(LiteralValue::String(i.to_string())),
                    }
                }
                Number::Float(f) => Ok(LiteralValue::Float(*f)),
                Number::Complex { .. } => Err(CannotExpandReason::UnsupportedExpression(
                    "complex numbers".to_string(),
                )),
            }
        }
        Expr::StringLiteral(s) => Ok(LiteralValue::String(s.value.to_str().to_string())),
        Expr::BooleanLiteral(b) => Ok(LiteralValue::Bool(b.value)),
        Expr::NoneLiteral(_) => Ok(LiteralValue::None),
        Expr::Tuple(ExprTuple { elts, .. }) | Expr::List(ExprList { elts, .. }) => {
            let mut values = Vec::with_capacity(elts.len());
            for elt in elts.iter() {
                values.push(extract_literal(elt)?);
            }
            Ok(LiteralValue::Sequence(values))
        }
        Expr::Name(name) => Err(CannotExpandReason::VariableReference(name.id.to_string())),
        Expr::Call(call) => {
            let func_name = get_call_name(&call.func);
            Err(CannotExpandReason::FunctionCall(func_name))
        }
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
            Err(CannotExpandReason::Comprehension)
        }
        _ => Err(CannotExpandReason::UnsupportedExpression(
            expr_type_name(expr).to_string(),
        )),
    }
}

/// Get a human-readable name for an expression type.
fn expr_type_name(expr: &Expr) -> &'static str {
    match expr {
        Expr::BoolOp(_) => "boolean operation",
        Expr::Named(_) => "named expression",
        Expr::BinOp(_) => "binary operation",
        Expr::UnaryOp(_) => "unary operation",
        Expr::Lambda(_) => "lambda",
        Expr::If(_) => "conditional expression",
        Expr::Dict(_) => "dict literal",
        Expr::Set(_) => "set literal",
        Expr::ListComp(_) => "list comprehension",
        Expr::SetComp(_) => "set comprehension",
        Expr::DictComp(_) => "dict comprehension",
        Expr::Generator(_) => "generator expression",
        Expr::Await(_) => "await expression",
        Expr::Yield(_) => "yield expression",
        Expr::YieldFrom(_) => "yield from expression",
        Expr::Compare(_) => "comparison",
        Expr::Call(_) => "function call",
        Expr::FString(_) => "f-string",
        Expr::TString(_) => "t-string",
        Expr::StringLiteral(_) => "string literal",
        Expr::BytesLiteral(_) => "bytes literal",
        Expr::NumberLiteral(_) => "number literal",
        Expr::BooleanLiteral(_) => "boolean literal",
        Expr::NoneLiteral(_) => "None",
        Expr::EllipsisLiteral(_) => "ellipsis",
        Expr::Attribute(_) => "attribute access",
        Expr::Subscript(_) => "subscript",
        Expr::Starred(_) => "starred expression",
        Expr::Name(_) => "variable reference",
        Expr::List(_) => "list",
        Expr::Tuple(_) => "tuple",
        Expr::Slice(_) => "slice",
        Expr::IpyEscapeCommand(_) => "IPython escape command",
    }
}

/// Get the name of a called function for error messages.
fn get_call_name(func: &Expr) -> String {
    match func {
        Expr::Name(name) => name.id.to_string(),
        Expr::Attribute(attr) => attr.attr.to_string(),
        _ => "unknown".to_string(),
    }
}

/// Extract the `ids` keyword argument if present.
fn extract_ids_kwarg(keywords: &[Keyword]) -> Option<Vec<String>> {
    for kw in keywords {
        if let Some(arg) = &kw.arg {
            if arg.as_str() == "ids" {
                if let Ok(LiteralValue::Sequence(seq)) = extract_literal(&kw.value) {
                    let ids: Vec<String> =
                        seq.into_iter().map(|v| literal_to_id_string(&v)).collect();
                    return Some(ids);
                } else if let Expr::List(list) = &kw.value {
                    let mut ids = Vec::with_capacity(list.elts.len());
                    for elt in list.elts.iter() {
                        if let Expr::StringLiteral(s) = elt {
                            ids.push(s.value.to_str().to_string());
                        } else if let Ok(lit) = extract_literal(elt) {
                            ids.push(literal_to_id_string(&lit));
                        } else {
                            return None;
                        }
                    }
                    return Some(ids);
                }
            }
        }
    }
    None
}

/// Convert a literal value to its string representation for use as a case ID.
fn literal_to_id_string(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Int(i) => i.to_string(),
        LiteralValue::Float(f) => f.to_string(),
        LiteralValue::String(s) => s.clone(),
        LiteralValue::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        LiteralValue::None => "None".to_string(),
        LiteralValue::Sequence(seq) => {
            let parts: Vec<String> = seq.iter().map(literal_to_id_string).collect();
            parts.join("-")
        }
    }
}

/// Expand cases specs into individual test cases using cartesian product.
pub fn expand_cases(specs: &[CasesSpec]) -> Vec<ExpandedCase> {
    if specs.is_empty() {
        return vec![];
    }

    let expanded_specs: Vec<Vec<String>> = specs.iter().map(expand_single_spec).collect();

    let mut result: Vec<Vec<String>> = vec![vec![]];
    for spec_ids in expanded_specs {
        let mut new_result = Vec::new();
        for existing in &result {
            for id in &spec_ids {
                let mut combined = existing.clone();
                combined.push(id.clone());
                new_result.push(combined);
            }
        }
        result = new_result;
    }

    let ids: Vec<String> = result.iter().map(|parts| parts.join("-")).collect();

    deduplicate_ids(ids)
        .into_iter()
        .map(|case_id| ExpandedCase { case_id })
        .collect()
}

/// Expand a single spec into case IDs.
fn expand_single_spec(spec: &CasesSpec) -> Vec<String> {
    let count = spec.argvalues.len();

    if let Some(ids) = &spec.ids {
        ids.iter()
            .take(count)
            .cloned()
            .chain((ids.len()..count).map(|i| i.to_string()))
            .collect()
    } else {
        // Generate value-based IDs
        spec.argvalues.iter().map(literal_to_id_string).collect()
    }
}

/// Deduplicate IDs by adding `_1`, `_2` suffixes for duplicates.
fn deduplicate_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut result = Vec::with_capacity(ids.len());

    for id in ids {
        let count = seen.entry(id.clone()).or_insert(0);
        if *count == 0 {
            result.push(id);
        } else {
            result.push(format!("{}_{}", id, count));
        }
        *count += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deduplicate_ids_no_duplicates() {
        let ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(deduplicate_ids(ids), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_deduplicate_ids_with_duplicates() {
        let ids = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "a".to_string(),
        ];
        assert_eq!(deduplicate_ids(ids), vec!["a", "b", "a_1", "a_2"]);
    }

    #[test]
    fn test_expand_single_spec_numeric() {
        let spec = CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![
                LiteralValue::Int(1),
                LiteralValue::Int(2),
                LiteralValue::Int(3),
            ],
            ids: None,
        };
        assert_eq!(expand_single_spec(&spec), vec!["1", "2", "3"]);
    }

    #[test]
    fn test_expand_single_spec_custom_ids() {
        let spec = CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![
                LiteralValue::Int(1),
                LiteralValue::Int(2),
                LiteralValue::Int(3),
            ],
            ids: Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
        };
        assert_eq!(expand_single_spec(&spec), vec!["one", "two", "three"]);
    }

    #[test]
    fn test_expand_cases_cartesian_product() {
        let specs = vec![
            CasesSpec {
                argnames: vec!["x".to_string()],
                argvalues: vec![LiteralValue::Int(1), LiteralValue::Int(2)],
                ids: None,
            },
            CasesSpec {
                argnames: vec!["y".to_string()],
                argvalues: vec![
                    LiteralValue::String("a".to_string()),
                    LiteralValue::String("b".to_string()),
                ],
                ids: None,
            },
        ];
        let cases = expand_cases(&specs);
        let ids: Vec<&str> = cases.iter().map(|c| c.case_id.as_str()).collect();
        assert_eq!(ids, vec!["1-a", "1-b", "2-a", "2-b"]);
    }

    #[test]
    fn test_literal_to_id_string() {
        assert_eq!(literal_to_id_string(&LiteralValue::Int(42)), "42");
        assert_eq!(literal_to_id_string(&LiteralValue::Float(3.14)), "3.14");
        assert_eq!(
            literal_to_id_string(&LiteralValue::String("hello".to_string())),
            "hello"
        );
        assert_eq!(literal_to_id_string(&LiteralValue::Bool(true)), "True");
        assert_eq!(literal_to_id_string(&LiteralValue::Bool(false)), "False");
        assert_eq!(literal_to_id_string(&LiteralValue::None), "None");
        assert_eq!(
            literal_to_id_string(&LiteralValue::Sequence(vec![
                LiteralValue::Int(1),
                LiteralValue::String("a".to_string()),
            ])),
            "1-a"
        );
    }

    #[test]
    fn test_format_cannot_expand_warning() {
        let warning = format_cannot_expand_warning(
            "test_foo.py::test_x",
            &CannotExpandReason::VariableReference("DATA".to_string()),
        );
        assert_eq!(
            warning,
            "warning: Cannot statically expand test cases for 'test_foo.py::test_x': argvalues references variable 'DATA'"
        );
    }

    #[test]
    fn test_expand_single_spec_empty_argvalues() {
        let spec = CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![],
            ids: None,
        };
        assert_eq!(expand_single_spec(&spec), Vec::<String>::new());
    }
}
