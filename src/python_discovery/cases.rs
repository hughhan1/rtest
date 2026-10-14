//! AST-based expansion of `@rtest.mark.cases` and `@pytest.mark.parametrize` decorators.
//!
//! This module extracts test case information from decorator AST nodes and expands
//! parametrized tests into individual test cases during collection.

use ruff_python_ast::name::UnqualifiedName;
use ruff_python_ast::{Decorator, Expr, ExprList, ExprTuple, Keyword};

use super::constant_resolver::ConstantResolver;

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
    /// A value we can count but cannot statically evaluate (e.g., dataclass instances, dicts).
    /// Used for generating positional fallback IDs.
    Opaque,
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
    /// Auto-generated IDs for each value (from literal or resolved source path).
    /// Same length as `argvalues`.
    pub value_ids: Vec<String>,
    /// Optional custom IDs for each case (overrides `value_ids`).
    pub ids: Option<Vec<String>>,
}

/// Parsed decorator information (specs, not yet expanded).
///
/// This intermediate representation allows combining class-level and method-level
/// specs before expansion, which is necessary for proper inheritance handling.
#[derive(Debug, Clone)]
pub enum MethodCasesInfo {
    /// No `@cases` or `@parametrize` decorators found.
    NotDecorated,
    /// Successfully parsed decorator specs.
    Specs(Vec<CasesSpec>),
    /// Cannot statically parse; will fall back to base test name.
    CannotExpand(CannotExpandReason),
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
///
/// If a `resolver` is provided, it will be used to resolve constant references
/// (like `Color.RED` or `CONFIG_VALUE`) to their literal values.
pub fn parse_decorators_for_cases(
    decorators: &[Decorator],
    resolver: Option<&ConstantResolver>,
) -> CasesExpansion {
    match parse_decorators_to_specs(decorators, resolver) {
        MethodCasesInfo::NotDecorated => CasesExpansion::NotDecorated,
        MethodCasesInfo::Specs(specs) => CasesExpansion::Expanded(expand_cases(&specs)),
        MethodCasesInfo::CannotExpand(reason) => CasesExpansion::CannotExpand(reason),
    }
}

/// Parse decorators into specs without expanding.
///
/// This is useful for caching method-level specs separately from class-level specs,
/// allowing proper combination during inheritance.
pub fn parse_decorators_to_specs(
    decorators: &[Decorator],
    resolver: Option<&ConstantResolver>,
) -> MethodCasesInfo {
    let mut specs = Vec::new();

    for decorator in decorators {
        match parse_single_decorator(decorator, resolver) {
            Ok(spec) => specs.extend(spec),
            Err(reason) => {
                return MethodCasesInfo::CannotExpand(reason);
            }
        }
    }

    if specs.is_empty() {
        MethodCasesInfo::NotDecorated
    } else {
        MethodCasesInfo::Specs(specs)
    }
}

/// Combine class-level and method-level specs, then expand.
///
/// Class specs become outer (slower-varying) parameters, method specs become
/// inner (faster-varying) parameters. This matches pytest's behavior for
/// class-level `@parametrize` decorators.
pub fn combine_and_expand_specs(
    class_info: &MethodCasesInfo,
    method_info: &MethodCasesInfo,
) -> CasesExpansion {
    // Collect specs: class first (outer), then method (inner)
    let mut combined_specs = Vec::new();
    for info in [class_info, method_info] {
        match info {
            MethodCasesInfo::CannotExpand(reason) => {
                return CasesExpansion::CannotExpand(reason.clone());
            }
            MethodCasesInfo::Specs(specs) => combined_specs.extend(specs.iter().cloned()),
            MethodCasesInfo::NotDecorated => {}
        }
    }

    if combined_specs.is_empty() {
        CasesExpansion::NotDecorated
    } else {
        CasesExpansion::Expanded(expand_cases(&combined_specs))
    }
}

/// Parse a single decorator to extract cases information.
fn parse_single_decorator(
    decorator: &Decorator,
    resolver: Option<&ConstantResolver>,
) -> Result<Option<CasesSpec>, CannotExpandReason> {
    let Expr::Call(call) = &decorator.expression else {
        return Ok(None);
    };

    if !is_cases_or_parametrize_call(&call.func) {
        return Ok(None);
    }

    if call.arguments.args.len() < 2 {
        return Err(CannotExpandReason::UnsupportedExpression(
            "missing required arguments".to_string(),
        ));
    }

    let argnames = extract_argnames(&call.arguments.args[0])?;
    let (argvalues, value_ids) = extract_argvalues(&call.arguments.args[1], resolver, &argnames)?;

    let ids = extract_ids_kwarg(&call.arguments.keywords, resolver);

    Ok(Some(CasesSpec {
        argnames,
        argvalues,
        value_ids,
        ids,
    }))
}

/// Check if the call func is `rtest.mark.cases` or `pytest.mark.parametrize`.
fn is_cases_or_parametrize_call(func: &Expr) -> bool {
    let Some(name) = UnqualifiedName::from_expr(func) else {
        return false;
    };
    matches!(
        name.segments(),
        ["rtest", "mark", "cases"] | ["pytest", "mark", "parametrize"]
    )
}

/// Extract argument names from the first decorator argument.
fn extract_argnames(expr: &Expr) -> Result<Vec<String>, CannotExpandReason> {
    let names = match expr {
        Expr::StringLiteral(s) => s
            .value
            .to_str()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>(),
        // Support list/tuple of strings: ["a", "b", "c"] or ("a", "b", "c")
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            let mut names = Vec::with_capacity(elts.len());
            for elt in elts {
                match elt {
                    Expr::StringLiteral(s) => {
                        names.push(s.value.to_str().to_string());
                    }
                    _ => {
                        return Err(CannotExpandReason::UnsupportedExpression(
                            "argnames list must contain only strings".to_string(),
                        ))
                    }
                }
            }
            names
        }
        Expr::Name(name) => return Err(CannotExpandReason::VariableReference(name.id.to_string())),
        _ => {
            return Err(CannotExpandReason::UnsupportedExpression(
                "argnames must be a string or list/tuple of strings".to_string(),
            ))
        }
    };
    if names.is_empty() {
        Err(CannotExpandReason::UnsupportedExpression(
            "empty argnames".to_string(),
        ))
    } else {
        Ok(names)
    }
}

/// Replace empty segments in a multi-param ID with positional IDs.
///
/// For ID "-2-3" with argnames ["data", "num", "count"] at tuple index 0:
/// - Position 0: "" (empty) -> "data0"
/// - Position 1: "2" (non-empty, keep)
/// - Position 2: "3" (non-empty, keep)
///
/// Result: "data0-2-3"
fn fill_opaque_placeholders(id: &str, argnames: &[String], tuple_idx: usize) -> String {
    let parts: Vec<&str> = id.split('-').collect();
    let filled: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(pos, part)| {
            if part.is_empty() {
                let argname = argnames.get(pos).map(|s| s.as_str()).unwrap_or("arg");
                format!("{}{}", argname, tuple_idx)
            } else {
                (*part).to_string()
            }
        })
        .collect();
    filled.join("-")
}

/// Extract argument values from the second decorator argument.
///
/// Returns `(values, value_ids)` where `value_ids` contains the ID string for each value.
/// For opaque values (complex objects we can't evaluate), generates positional IDs
/// using the corresponding argname (e.g., `data0`, `num1`).
fn extract_argvalues(
    expr: &Expr,
    resolver: Option<&ConstantResolver>,
    argnames: &[String],
) -> Result<(Vec<LiteralValue>, Vec<String>), CannotExpandReason> {
    let first_argname = argnames.first().map(|s| s.as_str()).unwrap_or("arg");

    match expr {
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            let mut values = Vec::with_capacity(elts.len());
            let mut ids = Vec::with_capacity(elts.len());
            for (idx, elt) in elts.iter().enumerate() {
                let (value, id) = extract_literal(elt, resolver)?;

                // Fill in positional IDs for opaque positions
                let final_id = if id.is_empty() {
                    // Single opaque value - use first argname
                    format!("{}{}", first_argname, idx)
                } else if matches!(value, LiteralValue::Sequence(_)) {
                    fill_opaque_placeholders(&id, argnames, idx)
                } else {
                    id
                };

                values.push(value);
                ids.push(final_id);
            }
            Ok((values, ids))
        }
        // Try resolving as a constant (e.g., `DATA = [1, 2, 3]` then `@cases("x", DATA)`)
        Expr::Name(name) => {
            if let Some((LiteralValue::Sequence(seq), path)) =
                resolver.and_then(|r| r.resolve(expr))
            {
                let source_name = path.join(".");
                let ids: Vec<String> = seq
                    .iter()
                    .map(|v| format!("{}[{}]", source_name, literal_to_id_string(v)))
                    .collect();
                return Ok((seq, ids));
            }
            Err(CannotExpandReason::VariableReference(name.id.to_string()))
        }
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
///
/// Returns `(value, id)` where `id` is the string representation for test case IDs.
/// For resolved constants (e.g., `Color.RED`), `id` is the source path (`"Color.RED"`).
fn extract_literal(
    expr: &Expr,
    resolver: Option<&ConstantResolver>,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    let literal = match expr {
        Expr::NumberLiteral(num) => {
            use ruff_python_ast::Number;
            match &num.value {
                Number::Int(i) => {
                    // Try to convert to i64, fall back to string representation for large ints
                    i.as_i64()
                        .map_or_else(|| LiteralValue::String(i.to_string()), LiteralValue::Int)
                }
                Number::Float(f) => LiteralValue::Float(*f),
                Number::Complex { .. } => {
                    return Err(CannotExpandReason::UnsupportedExpression(
                        "complex numbers".to_string(),
                    ))
                }
            }
        }
        Expr::StringLiteral(s) => LiteralValue::String(s.value.to_str().to_string()),
        Expr::BooleanLiteral(b) => LiteralValue::Bool(b.value),
        Expr::NoneLiteral(_) => LiteralValue::None,
        Expr::Tuple(ExprTuple { elts, .. }) | Expr::List(ExprList { elts, .. }) => {
            let mut values = Vec::with_capacity(elts.len());
            let mut sub_ids = Vec::with_capacity(elts.len());
            for elt in elts.iter() {
                let (v, id) = extract_literal(elt, resolver)?;
                values.push(v);
                sub_ids.push(id);
            }
            let lit = LiteralValue::Sequence(values);
            // Join sub_ids, preserving empty strings as placeholders for opaque elements.
            // The caller (extract_argvalues) will fill in positional IDs for empty positions.
            let id = sub_ids.join("-");
            return Ok((lit, id));
        }
        Expr::Name(_) | Expr::Attribute(_) => {
            if let Some((value, path)) = resolver.and_then(|r| r.resolve(expr)) {
                let id = path.join(".");
                return Ok((value, id));
            }
            // Unresolved name/attribute - treat as opaque (can count but not evaluate)
            LiteralValue::Opaque
        }
        // Comprehensions cannot be counted without evaluation
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
            return Err(CannotExpandReason::Comprehension);
        }
        // Other expressions - treat as opaque if we can count them
        _ => LiteralValue::Opaque,
    };
    let id = literal_to_id_string(&literal);
    Ok((literal, id))
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
fn extract_ids_kwarg(
    keywords: &[Keyword],
    resolver: Option<&ConstantResolver>,
) -> Option<Vec<String>> {
    for kw in keywords {
        if let Some(arg) = &kw.arg {
            if arg.as_str() == "ids" {
                if let Ok((LiteralValue::Sequence(seq), _)) = extract_literal(&kw.value, resolver) {
                    let ids: Vec<String> =
                        seq.into_iter().map(|v| literal_to_id_string(&v)).collect();
                    return Some(ids);
                } else if matches!(kw.value, Expr::List(_)) {
                    // A list can fail extraction only when one of its elements fails.
                    return None;
                }
            }
        }
    }
    None
}

/// Convert a literal value to its string representation for use as a case ID.
pub fn literal_to_id_string(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Int(i) => i.to_string(),
        LiteralValue::Float(f) => f.to_string(),
        LiteralValue::String(s) => ascii_escape_string(s),
        LiteralValue::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        LiteralValue::None => "None".to_string(),
        LiteralValue::Sequence(seq) => {
            let parts: Vec<String> = seq.iter().map(literal_to_id_string).collect();
            parts.join("-")
        }
        // Opaque values get positional IDs assigned in extract_argvalues
        LiteralValue::Opaque => String::new(),
    }
}

/// Escape a string for use in test IDs.
///
/// Escapes backslashes, control characters, and non-ASCII characters.
fn ascii_escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_ascii_control() => {
                result.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if c.is_ascii() => result.push(c),
            c => {
                let code = c as u32;
                if code <= 0xFFFF {
                    result.push_str(&format!("\\u{:04x}", code));
                } else {
                    result.push_str(&format!("\\U{:08x}", code));
                }
            }
        }
    }
    result
}

/// Expand cases specs into individual test cases using cartesian product.
pub fn expand_cases(specs: &[CasesSpec]) -> Vec<ExpandedCase> {
    if specs.is_empty() {
        return vec![];
    }

    // Reverse specs to process innermost decorator first (bottom-to-top order)
    let expanded_specs: Vec<Vec<String>> = specs.iter().rev().map(expand_single_spec).collect();

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
        // Custom IDs override everything
        ids.iter()
            .take(count)
            .cloned()
            .chain((ids.len()..count).map(|i| i.to_string()))
            .collect()
    } else {
        // Use pre-computed value_ids (from literals or resolved source paths)
        spec.value_ids.clone()
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
    fn test_decorator_names() {
        for (source, expected) in [
            ("rtest.mark.cases", true),
            ("pytest.mark.parametrize", true),
            ("pytest.mark.cases", false),
            ("rtest.mark.parametrize", false),
            ("other.mark.parametrize", false),
            ("pkg.pytest.mark.parametrize", false),
            ("pytest.parametrize", false),
            ("parametrize", false),
            ("factory().mark.parametrize", false),
        ] {
            let expression = ruff_python_parser::parse_expression(source)
                .expect("valid expression")
                .into_expr();
            assert_eq!(
                is_cases_or_parametrize_call(&expression),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn test_expand_cases_cartesian_product() {
        let specs = vec![
            CasesSpec {
                argnames: vec!["x".to_string()],
                argvalues: vec![LiteralValue::Int(1), LiteralValue::Int(2)],
                value_ids: vec!["1".to_string(), "2".to_string()],
                ids: None,
            },
            CasesSpec {
                argnames: vec!["y".to_string()],
                argvalues: vec![
                    LiteralValue::String("a".to_string()),
                    LiteralValue::String("b".to_string()),
                ],
                value_ids: vec!["a".to_string(), "b".to_string()],
                ids: None,
            },
        ];
        let cases = expand_cases(&specs);
        let ids: Vec<&str> = cases.iter().map(|c| c.case_id.as_str()).collect();
        // Bottom decorator (y) processed first, so y varies before x
        assert_eq!(ids, vec!["a-1", "a-2", "b-1", "b-2"]);
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
}
