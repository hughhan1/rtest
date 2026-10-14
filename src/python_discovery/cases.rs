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
    /// An enum member. Pytest IDs use `str(member)` → `Class.MEMBER`.
    EnumMember(String),
}

/// Specification for a single `@cases` or `@parametrize` decorator.
#[derive(Debug, Clone)]
pub struct CasesSpec {
    /// Final case IDs, including any explicit overrides.
    pub ids: Vec<String>,
    /// pytest 7.3 suffixes duplicate IDs as `{id}0`, `{id}1` (no underscore).
    pub pytest_unique_ids: bool,
}

/// Parsed decorators cached for inheritance; an empty list means no cases decorators.
pub type MethodCasesInfo = Result<Vec<CasesSpec>, CannotExpandReason>;

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

/// How parametrize case IDs are generated from resolved constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamIdStyle {
    /// `@rtest.mark.cases`: use the source path (`Color.RED`, `TEST_DATA[1]`).
    SourcePath,
    /// `@pytest.mark.parametrize`: match pytest IDs (runtime values; `str(enum)`).
    RuntimeValue,
}

/// Parse decorators and return the cases expansion result.
///
/// The resolver supplies literal values for constant references such as `Color.RED`.
pub fn parse_decorators_for_cases(
    decorators: &[Decorator],
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
) -> CasesExpansion {
    match parse_decorators_to_specs(decorators, resolver, enclosing_class) {
        Ok(specs) => expand_cases(specs.iter()),
        Err(reason) => CasesExpansion::CannotExpand(reason),
    }
}

/// Parse decorators into specs without expanding.
///
/// This is useful for caching method-level specs separately from class-level specs,
/// allowing proper combination during inheritance.
pub fn parse_decorators_to_specs(
    decorators: &[Decorator],
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
) -> MethodCasesInfo {
    let mut specs = Vec::new();
    for decorator in decorators {
        if let Some(spec) = parse_single_decorator(decorator, resolver, enclosing_class)? {
            specs.push(spec);
        }
    }
    Ok(specs)
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
    match (class_info, method_info) {
        (Err(reason), _) | (_, Err(reason)) => CasesExpansion::CannotExpand(reason.clone()),
        (Ok(class), Ok(method)) => expand_cases(class.iter().chain(method)),
    }
}

/// Parse a single decorator to extract cases information.
fn parse_single_decorator(
    decorator: &Decorator,
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
) -> Result<Option<CasesSpec>, CannotExpandReason> {
    let Expr::Call(call) = &decorator.expression else {
        return Ok(None);
    };

    let Some(id_style) = decorator_id_style(&call.func, resolver) else {
        return Ok(None);
    };

    if call.arguments.args.len() < 2 {
        return Err(CannotExpandReason::UnsupportedExpression(
            "missing required arguments".to_string(),
        ));
    }

    let (argnames, unpack_rows) = extract_argnames(&call.arguments.args[0])?;

    let value_ids = extract_argvalues(
        &call.arguments.args[1],
        resolver,
        enclosing_class,
        &argnames,
        id_style,
        unpack_rows,
    )?;

    let ids_kwarg_present = call
        .arguments
        .keywords
        .iter()
        .any(|kw| kw.arg.as_ref().is_some_and(|arg| arg.as_str() == "ids"));
    let ids = extract_ids_kwarg(
        &call.arguments.keywords,
        resolver,
        enclosing_class,
        id_style,
    );
    if id_style == ParamIdStyle::RuntimeValue && ids_kwarg_present && ids.is_none() {
        return Err(CannotExpandReason::UnsupportedExpression(
            "ids= is not a static list of strings".to_string(),
        ));
    }

    let ids = match ids {
        Some(ids) => ids
            .iter()
            .take(value_ids.len())
            .cloned()
            .chain((ids.len()..value_ids.len()).map(|i| i.to_string()))
            .collect(),
        None => value_ids,
    };
    Ok(Some(CasesSpec {
        ids,
        pytest_unique_ids: id_style == ParamIdStyle::RuntimeValue,
    }))
}

/// Identify `@rtest.mark.cases` and imported aliases of `@pytest.mark.parametrize`.
fn decorator_id_style(func: &Expr, resolver: &ConstantResolver) -> Option<ParamIdStyle> {
    match UnqualifiedName::from_expr(func)?.segments() {
        ["rtest", "mark", "cases"] => Some(ParamIdStyle::SourcePath),
        _ => (resolver.imported_expression_name(func).as_deref()
            == Some("pytest.mark.parametrize"))
        .then_some(ParamIdStyle::RuntimeValue),
    }
}

/// Extract argument names from the first decorator argument.
fn extract_argnames(expr: &Expr) -> Result<(Vec<String>, bool), CannotExpandReason> {
    let (names, unpack_rows) = match expr {
        Expr::StringLiteral(s) => {
            let names: Vec<String> = s
                .value
                .to_str()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (names, false)
        }
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
            (names, true)
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
        Ok((names, unpack_rows))
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
    let filled: Vec<String> = id
        .split('-')
        .enumerate()
        .map(|(pos, part)| {
            if part.is_empty() {
                let argname = argnames.get(pos).map(|s| s.as_str()).unwrap_or("arg");
                format!("{}{}", argname, tuple_idx)
            } else {
                part.to_string()
            }
        })
        .collect();
    filled.join("-")
}

/// Extract argument values from the second decorator argument.
///
/// Returns the case ID for each value; values are only needed while deriving IDs.
/// For opaque values (complex objects we can't evaluate), generates positional IDs
/// using the corresponding argname (e.g., `data0`, `num1`).
fn extract_argvalues(
    expr: &Expr,
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
    argnames: &[String],
    id_style: ParamIdStyle,
    unpack_rows: bool,
) -> Result<Vec<String>, CannotExpandReason> {
    if contains_starred(expr) {
        return Err(CannotExpandReason::UnsupportedExpression(
            "starred unpack in argvalues".to_string(),
        ));
    }

    let first_argname = argnames.first().map(|s| s.as_str()).unwrap_or("arg");

    match expr {
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => elts
            .iter()
            .enumerate()
            .map(|(idx, elt)| {
                if id_style == ParamIdStyle::RuntimeValue {
                    return extract_pytest_case(
                        elt,
                        argnames,
                        idx,
                        resolver,
                        enclosing_class,
                        unpack_rows,
                    );
                }
                let (value, id) = extract_literal(elt, resolver, enclosing_class, id_style)?;
                Ok(if id.is_empty() {
                    format!("{}{}", first_argname, idx)
                } else if matches!(value, LiteralValue::Sequence(_)) {
                    fill_opaque_placeholders(&id, argnames, idx)
                } else {
                    id
                })
            })
            .collect(),
        // A named sequence keeps source-path IDs for cases and value IDs for parametrize.
        Expr::Name(name) => {
            if let Some(resolved) = resolver.resolve_in_class(enclosing_class, expr) {
                if let LiteralValue::Sequence(seq) = resolved.value {
                    return Ok(match id_style {
                        ParamIdStyle::SourcePath => {
                            let source_name = resolved.source_path.join(".");
                            seq.iter()
                                .map(|v| format!("{}[{}]", source_name, literal_to_id_string(v)))
                                .collect()
                        }
                        ParamIdStyle::RuntimeValue => seq
                            .iter()
                            .enumerate()
                            .map(|(idx, v)| id_for_resolved_case_value(v, argnames, idx))
                            .collect(),
                    });
                }
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

/// One pytest parametrize case: nested list/tuple/dict values use `argnameN` IDs.
fn extract_pytest_case(
    elt: &Expr,
    argnames: &[String],
    idx: usize,
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
    unpack_rows: bool,
) -> Result<String, CannotExpandReason> {
    // Sequence-form argnames unpack rows even for one argument. String "x"
    // keeps a list/tuple row as the value; both forms share the component logic.
    if unpack_rows || argnames.len() > 1 {
        if let Some(components) = sequence_elts(elt) {
            let mut parts = Vec::with_capacity(components.len());
            for (i, component) in components.iter().enumerate() {
                let (value, id) = extract_literal(
                    component,
                    resolver,
                    enclosing_class,
                    ParamIdStyle::RuntimeValue,
                )?;
                if needs_positional_id(&value, &id) {
                    let name = argnames.get(i).map(String::as_str).unwrap_or("arg");
                    parts.push(format!("{}{}", name, idx));
                } else {
                    parts.push(id);
                }
            }
            return Ok(parts.join("-"));
        }
    }

    let (value, id) = extract_literal(elt, resolver, enclosing_class, ParamIdStyle::RuntimeValue)?;
    let final_id = finalize_pytest_id(&id, &value, argnames, idx);
    Ok(final_id)
}

fn contains_starred(expr: &Expr) -> bool {
    match expr {
        Expr::Starred(_) => true,
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            elts.iter().any(contains_starred)
        }
        Expr::Call(call) => call.arguments.args.iter().any(contains_starred),
        _ => false,
    }
}

fn eval_literal_binop(
    op: ruff_python_ast::Operator,
    left: &LiteralValue,
    right: &LiteralValue,
) -> Option<LiteralValue> {
    use ruff_python_ast::Operator;
    if let (LiteralValue::String(a), LiteralValue::String(b), Operator::Add) = (left, right, op) {
        return Some(LiteralValue::String(format!("{a}{b}")));
    }
    let (LiteralValue::Int(l), LiteralValue::Int(r)) = (left, right) else {
        return None;
    };
    let value = match op {
        Operator::Add => l.checked_add(*r)?,
        Operator::Sub => l.checked_sub(*r)?,
        Operator::Mult => l.checked_mul(*r)?,
        Operator::LShift if (0..64).contains(r) => l.checked_shl(*r as u32)?,
        Operator::RShift if (0..64).contains(r) => l.checked_shr(*r as u32)?,
        Operator::BitOr => *l | *r,
        Operator::BitXor => *l ^ *r,
        Operator::BitAnd => *l & *r,
        Operator::Pow if *r >= 0 && *r <= 63 => l.checked_pow(*r as u32)?,
        _ => return None,
    };
    Some(LiteralValue::Int(value))
}

fn sequence_elts(expr: &Expr) -> Option<&[Expr]> {
    match expr {
        Expr::Tuple(ExprTuple { elts, .. }) | Expr::List(ExprList { elts, .. }) => Some(elts),
        _ => None,
    }
}

fn finalize_pytest_id(id: &str, value: &LiteralValue, argnames: &[String], idx: usize) -> String {
    let first_argname = argnames.first().map(String::as_str).unwrap_or("arg");
    if needs_positional_id(value, id) {
        format!("{}{}", first_argname, idx)
    } else if matches!(value, LiteralValue::Sequence(_)) {
        fill_opaque_placeholders(id, argnames, idx)
    } else {
        id.to_string()
    }
}

/// True when pytest would use `argnameN`. Empty strings are real IDs (`test[]`); explicit `id=` is too.
fn needs_positional_id(value: &LiteralValue, id: &str) -> bool {
    match value {
        LiteralValue::String(_) => false,
        _ => id.is_empty(),
    }
}

fn id_for_resolved_case_value(value: &LiteralValue, argnames: &[String], idx: usize) -> String {
    let parts = match value {
        LiteralValue::Sequence(parts) if argnames.len() > 1 => parts.as_slice(),
        _ => std::slice::from_ref(value),
    };
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| match part {
            LiteralValue::Sequence(_) | LiteralValue::Opaque => {
                format!(
                    "{}{}",
                    argnames.get(i).map(String::as_str).unwrap_or("arg"),
                    idx
                )
            }
            _ => literal_to_id_string(part),
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Extract a literal value from an expression.
///
/// Returns `(value, id)` where `id` is the string representation for test case IDs.
fn extract_literal(
    expr: &Expr,
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    let literal = match expr {
        Expr::NumberLiteral(num) => {
            use ruff_python_ast::Number;
            match &num.value {
                Number::Int(i) => i
                    .as_i64()
                    .map_or_else(|| LiteralValue::String(i.to_string()), LiteralValue::Int),
                Number::Float(f) => LiteralValue::Float(*f),
                Number::Complex { .. } => {
                    return Err(CannotExpandReason::UnsupportedExpression(
                        "complex numbers".to_string(),
                    ))
                }
            }
        }
        Expr::StringLiteral(s) => LiteralValue::String(s.value.to_str().to_string()),
        Expr::BytesLiteral(bytes) => {
            // pytest STRING_TYPES includes bytes; IDs use ascii_escaped(decode).
            LiteralValue::String(bytes.value.bytes().map(char::from).collect())
        }
        Expr::BooleanLiteral(b) => LiteralValue::Bool(b.value),
        Expr::UnaryOp(unary) => {
            use ruff_python_ast::UnaryOp;
            if !matches!(unary.op, UnaryOp::USub) {
                return Ok((LiteralValue::Opaque, String::new()));
            }
            let (inner, _) = extract_literal(&unary.operand, resolver, enclosing_class, id_style)?;
            match inner {
                LiteralValue::Int(i) => LiteralValue::Int(-i),
                LiteralValue::Float(f) => LiteralValue::Float(-f),
                _ => LiteralValue::Opaque,
            }
        }
        Expr::BinOp(binop) => {
            let (left, _) = extract_literal(&binop.left, resolver, enclosing_class, id_style)?;
            let (right, _) = extract_literal(&binop.right, resolver, enclosing_class, id_style)?;
            if let Some(lit) = eval_literal_binop(binop.op, &left, &right) {
                let id = literal_to_id_string(&lit);
                return Ok((lit, id));
            }
            if id_style == ParamIdStyle::RuntimeValue {
                return Err(CannotExpandReason::UnsupportedExpression(format!(
                    "binary op '{}'",
                    binop.op.as_str()
                )));
            }
            LiteralValue::Opaque
        }
        Expr::Lambda(_) if id_style == ParamIdStyle::RuntimeValue => {
            // pytest `_idval_from_value` uses `__name__` → `<lambda>`.
            return Ok((LiteralValue::Opaque, "<lambda>".to_string()));
        }
        Expr::Starred(_) => {
            return Err(CannotExpandReason::UnsupportedExpression(
                "starred unpack".to_string(),
            ))
        }
        Expr::NoneLiteral(_) => LiteralValue::None,
        Expr::Tuple(ExprTuple { elts, .. }) | Expr::List(ExprList { elts, .. }) => {
            if id_style == ParamIdStyle::RuntimeValue {
                // Nested list/tuple as a parameter value is opaque to pytest (`argnameN`).
                return Ok((LiteralValue::Opaque, String::new()));
            }
            let mut values = Vec::with_capacity(elts.len());
            let mut sub_ids = Vec::with_capacity(elts.len());
            for elt in elts.iter() {
                let (v, id) = extract_literal(elt, resolver, enclosing_class, id_style)?;
                values.push(v);
                sub_ids.push(id);
            }
            let lit = LiteralValue::Sequence(values);
            let id = sub_ids.join("-");
            return Ok((lit, id));
        }
        Expr::Name(_) | Expr::Attribute(_) => {
            if let Some(resolved) = resolver.resolve_in_class(enclosing_class, expr) {
                let id = id_for_resolved(&resolved, id_style);
                return Ok((resolved.value, id));
            }
            if id_style == ParamIdStyle::RuntimeValue {
                // Pytest would use str(runtime value). Guessing `argnameN` is a silent mismatch.
                return Err(CannotExpandReason::VariableReference(get_call_name(expr)));
            }
            // Unresolved name/attribute - treat as opaque (can count but not evaluate)
            LiteralValue::Opaque
        }
        Expr::Call(call) if is_pytest_param(&call.func) => {
            return extract_pytest_param(call, resolver, enclosing_class, id_style);
        }
        Expr::Call(call) if id_style == ParamIdStyle::RuntimeValue => {
            return Err(CannotExpandReason::FunctionCall(get_call_name(&call.func)));
        }
        // Comprehensions cannot be counted without evaluation
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
            return Err(CannotExpandReason::Comprehension);
        }
        // Dicts, sets, source-path calls, and other countable expressions are opaque.
        _ => LiteralValue::Opaque,
    };
    let id = literal_to_id_string(&literal);
    Ok((literal, id))
}

fn is_pytest_param(func: &Expr) -> bool {
    match func {
        Expr::Name(name) => name.id.as_str() == "param",
        Expr::Attribute(attr) => attr.attr.as_str() == "param",
        _ => false,
    }
}

fn extract_pytest_param(
    call: &ruff_python_ast::ExprCall,
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    // Explicit id= is pytest-compatible even when the value is a helper call.
    if let Some(keyword) = call
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_deref() == Some("id") && !matches!(kw.value, Expr::NoneLiteral(_)))
    {
        let (LiteralValue::String(id), _) =
            extract_literal(&keyword.value, resolver, enclosing_class, id_style)?
        else {
            return Err(CannotExpandReason::UnsupportedExpression(
                "pytest.param id= is not a static string".into(),
            ));
        };
        return Ok((LiteralValue::String(id.clone()), id));
    }
    if call.arguments.args.is_empty() {
        return Ok((LiteralValue::Opaque, String::new()));
    }
    if call.arguments.args.len() == 1 {
        return extract_literal(&call.arguments.args[0], resolver, enclosing_class, id_style);
    }
    let mut values = Vec::with_capacity(call.arguments.args.len());
    let mut ids = Vec::with_capacity(call.arguments.args.len());
    for arg in &call.arguments.args {
        let (value, id) = extract_literal(arg, resolver, enclosing_class, id_style)?;
        values.push(value);
        ids.push(id);
    }
    Ok((LiteralValue::Sequence(values), ids.join("-")))
}

fn id_for_resolved(
    resolved: &super::constant_resolver::ResolvedConstant,
    id_style: ParamIdStyle,
) -> String {
    match id_style {
        ParamIdStyle::SourcePath => resolved.source_path.join("."),
        ParamIdStyle::RuntimeValue => {
            let id = literal_to_id_string(&resolved.value);
            if id.is_empty() {
                resolved.source_path.join(".")
            } else {
                id
            }
        }
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
fn extract_ids_kwarg(
    keywords: &[Keyword],
    resolver: &ConstantResolver,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Option<Vec<String>> {
    let kw = keywords
        .iter()
        .find(|kw| kw.arg.as_deref() == Some("ids"))?;
    if let Ok((LiteralValue::Sequence(seq), _)) =
        extract_literal(&kw.value, resolver, enclosing_class, id_style)
    {
        Some(seq.iter().map(literal_to_id_string).collect())
    } else {
        sequence_elts(&kw.value)?
            .iter()
            .map(|elt| match elt {
                Expr::StringLiteral(s) => Some(s.value.to_str().to_string()),
                _ => extract_literal(elt, resolver, enclosing_class, id_style)
                    .ok()
                    .map(|(value, _)| literal_to_id_string(&value)),
            })
            .collect()
    }
}

/// Convert a literal value to its string representation for use as a case ID.
pub fn literal_to_id_string(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Int(i) => i.to_string(),
        LiteralValue::Float(f) => python_str_float(*f),
        LiteralValue::String(s) => ascii_escape_string(s),
        LiteralValue::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        LiteralValue::None => "None".to_string(),
        LiteralValue::Sequence(seq) => {
            let parts: Vec<String> = seq.iter().map(literal_to_id_string).collect();
            parts.join("-")
        }
        // Opaque values get positional IDs assigned in extract_argvalues
        LiteralValue::Opaque => String::new(),
        LiteralValue::EnumMember(id) => id.clone(),
    }
}

/// JSON supplies correctly rounded shortest digits; Python uses a different
/// exponent spelling and switches to scientific notation at 1e-4, not 1e-5.
fn python_str_float(f: f64) -> String {
    let s = serde_json::Number::from_f64(f).map_or_else(
        || f.to_string().to_ascii_lowercase(),
        |number| number.to_string(),
    );
    if let Some((mantissa, exponent)) = s.split_once('e') {
        if let Ok(exponent) = exponent.parse::<i32>() {
            return format!("{mantissa}e{exponent:+03}");
        }
    }
    if (1e-5..1e-4).contains(&f.abs()) {
        let mut mantissa = s.replace("0.0000", "");
        mantissa.insert(usize::from(f.is_sign_negative()) + 1, '.');
        return format!("{}e-05", mantissa.trim_end_matches('.'));
    }
    s
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
                // Match Python `str.encode("unicode_escape")` / pytest `ascii_escaped`:
                // latin-1 is `\xHH`, BMP `\uXXXX`, supplementary `\UXXXXXXXX`.
                let code = c as u32;
                if code <= 0xFF {
                    result.push_str(&format!("\\x{:02x}", code));
                } else if code <= 0xFFFF {
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
fn expand_cases<'a>(specs: impl DoubleEndedIterator<Item = &'a CasesSpec>) -> CasesExpansion {
    // Process the innermost decorator first, preserving empty IDs as real components.
    let mut specs = specs.rev();
    let Some(first) = specs.next() else {
        return CasesExpansion::NotDecorated;
    };
    let mut ids = first.ids.clone();
    let mut pytest_unique_ids = first.pytest_unique_ids;
    for spec in specs {
        pytest_unique_ids |= spec.pytest_unique_ids;
        ids = ids
            .iter()
            .flat_map(|existing| spec.ids.iter().map(move |id| format!("{existing}-{id}")))
            .collect();
    }
    let unique = if pytest_unique_ids {
        deduplicate_ids_pytest(ids)
    } else {
        deduplicate_ids(ids)
    };
    CasesExpansion::Expanded(
        unique
            .into_iter()
            .map(|case_id| ExpandedCase { case_id })
            .collect(),
    )
}

/// pytest 7.3: every copy of a duplicated ID gets a trailing counter, including the first (`id0`, `id1`).
fn deduplicate_ids_pytest(ids: Vec<String>) -> Vec<String> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for id in &ids {
        *counts.entry(id.clone()).or_insert(0) += 1;
    }
    let mut suffixes: std::collections::HashMap<String, usize> = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, _)| (id.clone(), 0))
        .collect();
    ids.into_iter()
        .map(|id| {
            if let Some(n) = suffixes.get_mut(&id) {
                let separator = if id.ends_with(|c: char| c.is_ascii_digit()) {
                    "_"
                } else {
                    ""
                };
                let unique = loop {
                    let candidate = format!("{id}{separator}{n}");
                    *n += 1;
                    if counts.get(&candidate).copied().unwrap_or(0) == 0 {
                        break candidate;
                    }
                };
                // Match pytest's evolving ID list: release replaced IDs and reserve new ones.
                *counts.entry(id).or_default() -= 1;
                *counts.entry(unique.clone()).or_default() += 1;
                unique
            } else {
                id
            }
        })
        .collect()
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
