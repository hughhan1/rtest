//! AST-based expansion of `@rtest.mark.cases` and `@pytest.mark.parametrize` decorators.
//!
//! This module extracts test case information from decorator AST nodes and expands
//! parametrized tests into individual test cases during collection.

use ruff_python_ast::{Decorator, Expr, ExprAttribute, ExprList, ExprName, ExprTuple, Keyword};

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
    EnumMember {
        class_name: String,
        member_name: String,
    },
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
    /// pytest 7.3 suffixes duplicate IDs as `{id}0`, `{id}1` (no underscore).
    pub pytest_unique_ids: bool,
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

/// Prefix peach (and other collectors) scan for. Uncolored, one line per unexpanded test.
pub const CANNOT_EXPAND_MARKER: &str = "rtest-cannot-expand:";

/// Format a warning message for tests that cannot be statically expanded.
pub fn format_cannot_expand_warning(nodeid: &str, reason: &CannotExpandReason) -> String {
    format!(
        "warning: Cannot statically expand test cases for '{}': {}",
        nodeid, reason
    )
}

/// Machine-readable line peach uses to send the whole file to pytest.
pub fn format_cannot_expand_marker(nodeid: &str) -> String {
    format!("{} {}", CANNOT_EXPAND_MARKER, nodeid)
}

/// Nodeid quoted in a CannotExpand warning, if any.
pub fn cannot_expand_nodeid_from_message(message: &str) -> Option<&str> {
    const PREFIX: &str = "Cannot statically expand test cases for '";
    let start = message.find(PREFIX)? + PREFIX.len();
    let rest = &message[start..];
    let end = rest.find('\'')?;
    Some(&rest[..end])
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
/// If a `resolver` is provided, it will be used to resolve constant references
/// (like `Color.RED` or `CONFIG_VALUE`) to their literal values.
pub fn parse_decorators_for_cases(
    decorators: &[Decorator],
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
) -> CasesExpansion {
    let mut specs = Vec::new();

    for decorator in decorators {
        match parse_single_decorator(decorator, resolver, enclosing_class) {
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

/// Parse decorators into specs without expanding.
///
/// This is useful for caching method-level specs separately from class-level specs,
/// allowing proper combination during inheritance.
pub fn parse_decorators_to_specs(
    decorators: &[Decorator],
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
) -> MethodCasesInfo {
    let mut specs = Vec::new();

    for decorator in decorators {
        match parse_single_decorator(decorator, resolver, enclosing_class) {
            DecoratorParseResult::CasesSpec(spec) => specs.push(spec),
            DecoratorParseResult::CannotExpand(reason) => {
                return MethodCasesInfo::CannotExpand(reason);
            }
            DecoratorParseResult::NotCasesDecorator => {}
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
    // Handle CannotExpand cases first
    if let MethodCasesInfo::CannotExpand(reason) = class_info {
        return CasesExpansion::CannotExpand(reason.clone());
    }
    if let MethodCasesInfo::CannotExpand(reason) = method_info {
        return CasesExpansion::CannotExpand(reason.clone());
    }

    // Collect specs: class first (outer), then method (inner)
    let mut combined_specs = Vec::new();

    if let MethodCasesInfo::Specs(specs) = class_info {
        combined_specs.extend(specs.iter().cloned());
    }
    if let MethodCasesInfo::Specs(specs) = method_info {
        combined_specs.extend(specs.iter().cloned());
    }

    if combined_specs.is_empty() {
        CasesExpansion::NotDecorated
    } else {
        CasesExpansion::Expanded(expand_cases(&combined_specs))
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
fn parse_single_decorator(
    decorator: &Decorator,
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
) -> DecoratorParseResult {
    let Expr::Call(call) = &decorator.expression else {
        return DecoratorParseResult::NotCasesDecorator;
    };

    let Some(id_style) = decorator_id_style(&call.func) else {
        return DecoratorParseResult::NotCasesDecorator;
    };

    if call.arguments.args.len() < 2 {
        return DecoratorParseResult::CannotExpand(CannotExpandReason::UnsupportedExpression(
            "missing required arguments".to_string(),
        ));
    }

    let (argnames, unpack_rows) = match extract_argnames(&call.arguments.args[0]) {
        Ok(names) => names,
        Err(reason) => return DecoratorParseResult::CannotExpand(reason),
    };

    let (argvalues, value_ids) = match extract_argvalues(
        &call.arguments.args[1],
        resolver,
        enclosing_class,
        &argnames,
        id_style,
        unpack_rows,
    ) {
        Ok(result) => result,
        Err(reason) => return DecoratorParseResult::CannotExpand(reason),
    };

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
        return DecoratorParseResult::CannotExpand(CannotExpandReason::UnsupportedExpression(
            "ids= is not a static list of strings".to_string(),
        ));
    }

    DecoratorParseResult::CasesSpec(CasesSpec {
        argnames,
        argvalues,
        value_ids,
        ids,
        pytest_unique_ids: id_style == ParamIdStyle::RuntimeValue,
    })
}

/// Identify `@rtest.mark.cases`, `@pytest.mark.parametrize`, and `@mark.parametrize`.
///
/// Bare `@parametrize(...)` is not recognized — custom wrappers (e.g. Front Porch's
/// `ParametrizeParameters` helper) would produce IDs that do not match pytest.
fn decorator_id_style(func: &Expr) -> Option<ParamIdStyle> {
    let Expr::Attribute(ExprAttribute { attr, value, .. }) = func else {
        return None;
    };

    match attr.as_str() {
        "cases" => {
            let Expr::Attribute(ExprAttribute {
                attr: mark_attr,
                value: module_value,
                ..
            }) = value.as_ref()
            else {
                return None;
            };
            if mark_attr.as_str() != "mark" {
                return None;
            }
            let Expr::Name(ExprName {
                id: module_name, ..
            }) = module_value.as_ref()
            else {
                return None;
            };
            (module_name.as_str() == "rtest").then_some(ParamIdStyle::SourcePath)
        }
        "parametrize" => match value.as_ref() {
            Expr::Name(name) if name.id.as_str() == "mark" => Some(ParamIdStyle::RuntimeValue),
            Expr::Attribute(ExprAttribute {
                attr: mark_attr, ..
            }) if mark_attr.as_str() == "mark" => Some(ParamIdStyle::RuntimeValue),
            _ => None,
        },
        _ => None,
    }
}

/// Extract argument names from the first decorator argument.
fn extract_argnames(expr: &Expr) -> Result<(Vec<String>, bool), CannotExpandReason> {
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
                // `"evaluate_all"` is one value; `"a, b"` unpacks each row.
                Ok((names, false))
            }
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
            if names.is_empty() {
                Err(CannotExpandReason::UnsupportedExpression(
                    "empty argnames".to_string(),
                ))
            } else {
                Ok((names, true))
            }
        }
        Expr::Name(name) => Err(CannotExpandReason::VariableReference(name.id.to_string())),
        _ => Err(CannotExpandReason::UnsupportedExpression(
            "argnames must be a string or list/tuple of strings".to_string(),
        )),
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
    enclosing_class: Option<&str>,
    argnames: &[String],
    id_style: ParamIdStyle,
    unpack_rows: bool,
) -> Result<(Vec<LiteralValue>, Vec<String>), CannotExpandReason> {
    if contains_starred(expr) {
        return Err(CannotExpandReason::UnsupportedExpression(
            "starred unpack in argvalues".to_string(),
        ));
    }

    let first_argname = argnames.first().map(|s| s.as_str()).unwrap_or("arg");

    match expr {
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            let mut values = Vec::with_capacity(elts.len());
            let mut ids = Vec::with_capacity(elts.len());
            for (idx, elt) in elts.iter().enumerate() {
                let (value, id) = if id_style == ParamIdStyle::RuntimeValue {
                    extract_pytest_case(
                        elt,
                        argnames,
                        idx,
                        resolver,
                        enclosing_class,
                        id_style,
                        unpack_rows,
                    )?
                } else {
                    let (value, id) = extract_literal(elt, resolver, enclosing_class, id_style)?;
                    let final_id = if id.is_empty() {
                        format!("{}{}", first_argname, idx)
                    } else if matches!(value, LiteralValue::Sequence(_)) && id.contains('-') {
                        fill_opaque_placeholders(&id, argnames, idx)
                    } else if matches!(value, LiteralValue::Sequence(_)) {
                        if id.is_empty() {
                            format!("{}{}", first_argname, idx)
                        } else {
                            fill_opaque_placeholders(&id, argnames, idx)
                        }
                    } else {
                        id
                    };
                    (value, final_id)
                };
                values.push(value);
                ids.push(id);
            }
            Ok((values, ids))
        }
        // Try resolving as a constant (e.g., `DATA = [1, 2, 3]` then `@cases("x", DATA)`)
        Expr::Name(name) => {
            if let Some(resolver) = resolver {
                if let Some(resolved) = resolver.resolve_in_class(enclosing_class, expr) {
                    if let LiteralValue::Sequence(seq) = resolved.value {
                        let ids: Vec<String> = match id_style {
                            ParamIdStyle::SourcePath => {
                                let source_name = resolved.source_path.join(".");
                                seq.iter()
                                    .map(|v| {
                                        format!("{}[{}]", source_name, literal_to_id_string(v))
                                    })
                                    .collect()
                            }
                            ParamIdStyle::RuntimeValue => seq
                                .iter()
                                .enumerate()
                                .map(|(idx, v)| id_for_resolved_case_value(v, argnames, idx))
                                .collect(),
                        };
                        return Ok((seq, ids));
                    }
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
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
    unpack_rows: bool,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    if let Expr::Call(call) = elt {
        if is_pytest_param(&call.func) {
            let (value, id) = extract_pytest_param(call, resolver, enclosing_class, id_style)?;
            let final_id = finalize_pytest_id(&id, &value, argnames, idx);
            return Ok((value, final_id));
        }
    }

    // pytest unpacks a 1-tuple row only when argnames is a list/tuple of names,
    // e.g. `("admin_action",)` + `("deactivate",)`. A string argnames `"x"` plus
    // `[(True,)]` keeps the tuple as the value (ID `x0`). Never unwrap lists:
    // `[["WFLD-missing"]]` is a list value, not one unpacked string.
    if unpack_rows && argnames.len() == 1 {
        if let Expr::Tuple(ExprTuple { elts, .. }) = elt {
            if let [only] = elts.as_slice() {
                let (value, id) = extract_literal(only, resolver, enclosing_class, id_style)?;
                let final_id = finalize_pytest_id(&id, &value, argnames, idx);
                return Ok((value, final_id));
            }
        }
    }

    if argnames.len() > 1 {
        if let Some(components) = sequence_elts(elt) {
            let mut values = Vec::with_capacity(components.len());
            let mut parts = Vec::with_capacity(components.len());
            for (i, component) in components.iter().enumerate() {
                let (value, id) = extract_literal(component, resolver, enclosing_class, id_style)?;
                if needs_positional_id(&value, &id) {
                    let name = argnames.get(i).map(String::as_str).unwrap_or("arg");
                    parts.push(format!("{}{}", name, idx));
                } else {
                    parts.push(id);
                }
                values.push(value);
            }
            return Ok((LiteralValue::Sequence(values), parts.join("-")));
        }
    }

    let (value, id) = extract_literal(elt, resolver, enclosing_class, id_style)?;
    let final_id = finalize_pytest_id(&id, &value, argnames, idx);
    Ok((value, final_id))
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
        Operator::FloorDiv if *r != 0 => l.div_euclid(*r),
        Operator::Mod if *r != 0 => l.rem_euclid(*r),
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
    if argnames.len() <= 1 {
        return match value {
            LiteralValue::Sequence(_) | LiteralValue::Opaque => {
                format!(
                    "{}{}",
                    argnames.first().map(String::as_str).unwrap_or("arg"),
                    idx
                )
            }
            _ => {
                let id = literal_to_id_string(value);
                if needs_positional_id(value, &id) {
                    format!(
                        "{}{}",
                        argnames.first().map(String::as_str).unwrap_or("arg"),
                        idx
                    )
                } else {
                    id
                }
            }
        };
    }
    match value {
        LiteralValue::Sequence(parts) => {
            let ids: Vec<String> = parts
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
                    _ => {
                        let id = literal_to_id_string(part);
                        if needs_positional_id(part, &id) {
                            format!(
                                "{}{}",
                                argnames.get(i).map(String::as_str).unwrap_or("arg"),
                                idx
                            )
                        } else {
                            id
                        }
                    }
                })
                .collect();
            ids.join("-")
        }
        LiteralValue::Opaque => format!(
            "{}{}",
            argnames.first().map(String::as_str).unwrap_or("arg"),
            idx
        ),
        _ => literal_to_id_string(value),
    }
}

/// Extract a literal value from an expression.
///
/// Returns `(value, id)` where `id` is the string representation for test case IDs.
fn extract_literal(
    expr: &Expr,
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    match expr {
        Expr::NumberLiteral(num) => {
            use ruff_python_ast::Number;
            match &num.value {
                Number::Int(i) => {
                    // Try to convert to i64, fall back to string representation for large ints
                    match i.as_i64() {
                        Some(v) => {
                            let lit = LiteralValue::Int(v);
                            let id = literal_to_id_string(&lit);
                            Ok((lit, id))
                        }
                        None => {
                            let lit = LiteralValue::String(i.to_string());
                            let id = literal_to_id_string(&lit);
                            Ok((lit, id))
                        }
                    }
                }
                Number::Float(f) => {
                    let lit = LiteralValue::Float(*f);
                    let id = literal_to_id_string(&lit);
                    Ok((lit, id))
                }
                Number::Complex { .. } => Err(CannotExpandReason::UnsupportedExpression(
                    "complex numbers".to_string(),
                )),
            }
        }
        Expr::StringLiteral(s) => {
            let lit = LiteralValue::String(s.value.to_str().to_string());
            let id = literal_to_id_string(&lit);
            Ok((lit, id))
        }
        Expr::BytesLiteral(bytes) => {
            // pytest STRING_TYPES includes bytes; IDs use ascii_escaped(decode).
            let s: String = bytes.value.bytes().map(char::from).collect();
            let lit = LiteralValue::String(s);
            let id = literal_to_id_string(&lit);
            Ok((lit, id))
        }
        Expr::BooleanLiteral(b) => {
            let lit = LiteralValue::Bool(b.value);
            let id = literal_to_id_string(&lit);
            Ok((lit, id))
        }
        Expr::UnaryOp(unary) => {
            use ruff_python_ast::UnaryOp;
            if !matches!(unary.op, UnaryOp::USub) {
                return Ok((LiteralValue::Opaque, String::new()));
            }
            let (inner, _) = extract_literal(&unary.operand, resolver, enclosing_class, id_style)?;
            match inner {
                LiteralValue::Int(i) => {
                    let lit = LiteralValue::Int(-i);
                    let id = literal_to_id_string(&lit);
                    Ok((lit, id))
                }
                LiteralValue::Float(f) => {
                    let lit = LiteralValue::Float(-f);
                    let id = literal_to_id_string(&lit);
                    Ok((lit, id))
                }
                _ => Ok((LiteralValue::Opaque, String::new())),
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
            Ok((LiteralValue::Opaque, String::new()))
        }
        Expr::Lambda(_) if id_style == ParamIdStyle::RuntimeValue => {
            // pytest `_idval_from_value` uses `__name__` → `<lambda>`.
            Ok((LiteralValue::Opaque, "<lambda>".to_string()))
        }
        Expr::Starred(_) => Err(CannotExpandReason::UnsupportedExpression(
            "starred unpack".to_string(),
        )),
        Expr::NoneLiteral(_) => {
            let lit = LiteralValue::None;
            let id = literal_to_id_string(&lit);
            Ok((lit, id))
        }
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
            Ok((lit, id))
        }
        Expr::Name(_) | Expr::Attribute(_) => {
            if let Some(resolver) = resolver {
                if let Some(resolved) = resolver.resolve_in_class(enclosing_class, expr) {
                    let id = id_for_resolved(&resolved, id_style);
                    return Ok((resolved.value, id));
                }
            }
            if id_style == ParamIdStyle::RuntimeValue {
                // Pytest would use str(runtime value). Guessing `argnameN` is a silent mismatch.
                let name = match expr {
                    Expr::Name(name) => name.id.to_string(),
                    Expr::Attribute(attr) => attr.attr.to_string(),
                    _ => "attribute".to_string(),
                };
                return Err(CannotExpandReason::VariableReference(name));
            }
            // Unresolved name/attribute - treat as opaque (can count but not evaluate)
            Ok((LiteralValue::Opaque, String::new()))
        }
        Expr::Call(call) if is_pytest_param(&call.func) => {
            extract_pytest_param(call, resolver, enclosing_class, id_style)
        }
        Expr::Call(call) if id_style == ParamIdStyle::RuntimeValue => {
            // Constructors (`MyData(1)`, `date(2024, 1, 1)`) become objects; pytest uses
            // argnameN. Helper calls that return str/int/enum must not be guessed as
            // argnameN or workqueue will skip them.
            if is_constructor_call(&call.func) {
                Ok((LiteralValue::Opaque, String::new()))
            } else {
                Err(CannotExpandReason::FunctionCall(get_call_name(&call.func)))
            }
        }
        // Function calls (including dataclass/class instantiation) - opaque for rtest.mark.cases
        Expr::Call(_) => Ok((LiteralValue::Opaque, String::new())),
        // Dict and set literals - opaque (we can count them but not stringify nicely)
        Expr::Dict(_) | Expr::Set(_) => Ok((LiteralValue::Opaque, String::new())),
        // Comprehensions cannot be counted without evaluation
        Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
            Err(CannotExpandReason::Comprehension)
        }
        // Other expressions - treat as opaque if we can count them
        _ => Ok((LiteralValue::Opaque, String::new())),
    }
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
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Result<(LiteralValue, String), CannotExpandReason> {
    let custom_id = string_kwarg(&call.arguments.keywords, "id");
    // Explicit id= is pytest-compatible even when the value is a helper call.
    if let Some(id) = &custom_id {
        return Ok((LiteralValue::Opaque, id.clone()));
    }
    if call.arguments.args.is_empty() {
        return Ok((LiteralValue::Opaque, custom_id.unwrap_or_default()));
    }
    if call.arguments.args.len() == 1 {
        let (value, auto_id) =
            extract_literal(&call.arguments.args[0], resolver, enclosing_class, id_style)?;
        return Ok((value, custom_id.unwrap_or(auto_id)));
    }
    let mut values = Vec::with_capacity(call.arguments.args.len());
    let mut ids = Vec::with_capacity(call.arguments.args.len());
    for arg in &call.arguments.args {
        let (value, id) = extract_literal(arg, resolver, enclosing_class, id_style)?;
        values.push(value);
        ids.push(id);
    }
    Ok((
        LiteralValue::Sequence(values),
        custom_id.unwrap_or_else(|| ids.join("-")),
    ))
}

fn string_kwarg(keywords: &[Keyword], name: &str) -> Option<String> {
    for kw in keywords {
        if kw.arg.as_ref().is_some_and(|arg| arg.as_str() == name) {
            if let Expr::StringLiteral(s) = &kw.value {
                return Some(s.value.to_str().to_string());
            }
        }
    }
    None
}

fn id_for_resolved(
    resolved: &super::constant_resolver::ResolvedConstant,
    id_style: ParamIdStyle,
) -> String {
    match id_style {
        ParamIdStyle::SourcePath => resolved.source_path.join("."),
        ParamIdStyle::RuntimeValue => {
            if resolved.is_enum_member {
                enum_member_id(&resolved.source_path)
            } else {
                let id = literal_to_id_string(&resolved.value);
                if id.is_empty() {
                    resolved.source_path.join(".")
                } else {
                    id
                }
            }
        }
    }
}

fn enum_member_id(path: &[String]) -> String {
    if path.len() >= 2 {
        format!("{}.{}", path[path.len() - 2], path[path.len() - 1])
    } else {
        path.join(".")
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
    resolver: Option<&ConstantResolver>,
    enclosing_class: Option<&str>,
    id_style: ParamIdStyle,
) -> Option<Vec<String>> {
    for kw in keywords {
        if let Some(arg) = &kw.arg {
            if arg.as_str() == "ids" {
                if let Ok((LiteralValue::Sequence(seq), _)) =
                    extract_literal(&kw.value, resolver, enclosing_class, id_style)
                {
                    let ids: Vec<String> =
                        seq.into_iter().map(|v| literal_to_id_string(&v)).collect();
                    return Some(ids);
                } else if let Expr::List(list) = &kw.value {
                    let mut ids = Vec::with_capacity(list.elts.len());
                    for elt in list.elts.iter() {
                        if let Expr::StringLiteral(s) = elt {
                            ids.push(s.value.to_str().to_string());
                        } else if let Ok((lit, _)) =
                            extract_literal(elt, resolver, enclosing_class, id_style)
                        {
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
        LiteralValue::EnumMember {
            class_name,
            member_name,
        } => format!("{}.{}", class_name, member_name),
    }
}

/// Match Python `str(float)` so `0.0` is `0.0`, not Rust's `0`.
fn python_str_float(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f.is_sign_negative() {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    if f == 0.0 && f.is_sign_negative() {
        return "-0.0".to_string();
    }
    let mut s = f.to_string();
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    s
}

/// Stdlib datetime types are constructors but named in lowercase (`from datetime import date`).
/// Pytest IDs those instances as `argnameN`, same as PascalCase classes.
const DATETIME_CONSTRUCTOR_NAMES: &[&str] = &["date", "datetime", "timedelta", "time"];

fn is_constructor_call(func: &Expr) -> bool {
    match func {
        Expr::Name(name) => {
            let id = name.id.as_str();
            DATETIME_CONSTRUCTOR_NAMES.contains(&id)
                || id.chars().next().is_some_and(|c| c.is_uppercase())
        }
        // `datetime.date(...)` / `dt.timedelta(...)` after `import datetime` or `import datetime as dt`.
        // Do not treat `time.time()` (stdlib clock) as a constructor: pytest IDs the float.
        Expr::Attribute(attr) => {
            let attr_name = attr.attr.as_str();
            if !DATETIME_CONSTRUCTOR_NAMES.contains(&attr_name) {
                return false;
            }
            match attr.value.as_ref() {
                Expr::Name(n) if n.id.as_str() == "time" && attr_name == "time" => false,
                Expr::Name(_) => true,
                _ => false,
            }
        }
        _ => false,
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
    let unique = if specs.iter().any(|spec| spec.pytest_unique_ids) {
        deduplicate_ids_pytest(ids)
    } else {
        deduplicate_ids(ids)
    };

    unique
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

/// pytest 7.3: every copy of a duplicated ID gets a trailing counter, including the first (`id0`, `id1`).
fn deduplicate_ids_pytest(ids: Vec<String>) -> Vec<String> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for id in &ids {
        *counts.entry(id.clone()).or_insert(0) += 1;
    }
    let mut suffixes: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    ids.into_iter()
        .map(|id| {
            if counts.get(&id).copied().unwrap_or(0) > 1 {
                let n = suffixes.entry(id.clone()).or_insert(0);
                let unique = format!("{id}{n}");
                *n += 1;
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
    fn test_deduplicate_ids_pytest_suffixes_all_copies() {
        let ids = vec![
            "True-True-True-False".to_string(),
            "True-True-True-False".to_string(),
            "unique".to_string(),
        ];
        assert_eq!(
            deduplicate_ids_pytest(ids),
            vec!["True-True-True-False0", "True-True-True-False1", "unique"]
        );
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
            value_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            ids: None,
            pytest_unique_ids: false,
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
            value_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            ids: Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
            pytest_unique_ids: false,
        };
        assert_eq!(expand_single_spec(&spec), vec!["one", "two", "three"]);
    }

    #[test]
    fn test_expand_cases_cartesian_product() {
        let specs = vec![
            CasesSpec {
                argnames: vec!["x".to_string()],
                argvalues: vec![LiteralValue::Int(1), LiteralValue::Int(2)],
                value_ids: vec!["1".to_string(), "2".to_string()],
                ids: None,
                pytest_unique_ids: false,
            },
            CasesSpec {
                argnames: vec!["y".to_string()],
                argvalues: vec![
                    LiteralValue::String("a".to_string()),
                    LiteralValue::String("b".to_string()),
                ],
                value_ids: vec!["a".to_string(), "b".to_string()],
                ids: None,
                pytest_unique_ids: false,
            },
        ];
        let cases = expand_cases(&specs);
        let ids: Vec<&str> = cases.iter().map(|c| c.case_id.as_str()).collect();
        // Bottom decorator (y) processed first, so y varies before x
        assert_eq!(ids, vec!["a-1", "a-2", "b-1", "b-2"]);
    }

    #[test]
    fn test_literal_to_id_string() {
        assert_eq!(literal_to_id_string(&LiteralValue::Int(42)), "42");
        // Use 2.5 rather than 3.14: clippy::approx_constant treats 3.14 as π
        // and CI runs clippy with -D warnings.
        assert_eq!(literal_to_id_string(&LiteralValue::Float(2.5)), "2.5");
        assert_eq!(literal_to_id_string(&LiteralValue::Float(0.0)), "0.0");
        assert_eq!(literal_to_id_string(&LiteralValue::Float(1.0)), "1.0");
        assert_eq!(
            literal_to_id_string(&LiteralValue::String(String::new())),
            ""
        );
        assert_eq!(
            literal_to_id_string(&LiteralValue::String("hello".to_string())),
            "hello"
        );
        assert_eq!(literal_to_id_string(&LiteralValue::Bool(true)), "True");
        assert_eq!(literal_to_id_string(&LiteralValue::Bool(false)), "False");
        assert_eq!(literal_to_id_string(&LiteralValue::None), "None");
        assert_eq!(
            literal_to_id_string(&LiteralValue::EnumMember {
                class_name: "Color".to_string(),
                member_name: "RED".to_string(),
            }),
            "Color.RED"
        );
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
        assert_eq!(
            cannot_expand_nodeid_from_message(&warning),
            Some("test_foo.py::test_x")
        );
        assert_eq!(
            format_cannot_expand_marker("test_foo.py::test_x"),
            "rtest-cannot-expand: test_foo.py::test_x"
        );
    }

    #[test]
    fn test_expand_single_spec_empty_argvalues() {
        let spec = CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![],
            value_ids: vec![],
            ids: None,
            pytest_unique_ids: false,
        };
        assert_eq!(expand_single_spec(&spec), Vec::<String>::new());
    }

    #[test]
    fn test_ascii_escape_string_backslash() {
        // Backslash escaping - the main issue from #124
        assert_eq!(ascii_escape_string("\\u2603"), "\\\\u2603");
        assert_eq!(ascii_escape_string("\"\\u2603\""), "\"\\\\u2603\"");
    }

    #[test]
    fn test_ascii_escape_string_unicode() {
        // Latin-1 matches Python unicode_escape / pytest ascii_escaped (`\xHH`).
        assert_eq!(ascii_escape_string("Ñoño"), "\\xd1o\\xf1o");
        // Non-ASCII BMP to Unicode escape
        assert_eq!(ascii_escape_string("☃"), "\\u2603");
        assert_eq!(ascii_escape_string("\"☃\""), "\"\\u2603\"");

        // Supplementary plane character (code point > U+FFFF)
        assert_eq!(ascii_escape_string("𝄞"), "\\U0001d11e");
    }

    #[test]
    fn test_ascii_escape_string_control_chars() {
        assert_eq!(ascii_escape_string("a\nb"), "a\\nb");
        assert_eq!(ascii_escape_string("a\tb"), "a\\tb");
        assert_eq!(ascii_escape_string("a\rb"), "a\\rb");
        assert_eq!(ascii_escape_string("\x00"), "\\x00");
    }

    #[test]
    fn test_ascii_escape_string_plain_ascii() {
        // Plain ASCII unchanged
        assert_eq!(ascii_escape_string("hello"), "hello");
        assert_eq!(ascii_escape_string("Hello World 123!"), "Hello World 123!");
    }

    #[test]
    fn test_ascii_escape_string_mixed() {
        // Mixed content
        assert_eq!(
            ascii_escape_string("hello\\world☃"),
            "hello\\\\world\\u2603"
        );
    }

    #[test]
    fn test_combine_and_expand_specs_class_only() {
        // When class has parametrize but method doesn't, should still expand
        let class_specs = MethodCasesInfo::Specs(vec![CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![LiteralValue::Int(1), LiteralValue::Int(2)],
            value_ids: vec!["1".to_string(), "2".to_string()],
            ids: None,
            pytest_unique_ids: false,
        }]);
        let method_specs = MethodCasesInfo::NotDecorated;

        let result = combine_and_expand_specs(&class_specs, &method_specs);

        match result {
            CasesExpansion::Expanded(cases) => {
                assert_eq!(cases.len(), 2);
                assert_eq!(cases[0].case_id, "1");
                assert_eq!(cases[1].case_id, "2");
            }
            _ => panic!("Expected Expanded, got {:?}", result),
        }
    }

    #[test]
    fn test_combine_and_expand_specs_both() {
        // When both class and method have parametrize, should combine
        let class_specs = MethodCasesInfo::Specs(vec![CasesSpec {
            argnames: vec!["x".to_string()],
            argvalues: vec![LiteralValue::Int(1), LiteralValue::Int(2)],
            value_ids: vec!["1".to_string(), "2".to_string()],
            ids: None,
            pytest_unique_ids: false,
        }]);
        let method_specs = MethodCasesInfo::Specs(vec![CasesSpec {
            argnames: vec!["y".to_string()],
            argvalues: vec![
                LiteralValue::String("a".to_string()),
                LiteralValue::String("b".to_string()),
            ],
            value_ids: vec!["a".to_string(), "b".to_string()],
            ids: None,
            pytest_unique_ids: false,
        }]);

        let result = combine_and_expand_specs(&class_specs, &method_specs);

        match result {
            CasesExpansion::Expanded(cases) => {
                assert_eq!(cases.len(), 4);
                // Method params vary fastest (innermost)
                assert_eq!(cases[0].case_id, "a-1");
                assert_eq!(cases[1].case_id, "a-2");
                assert_eq!(cases[2].case_id, "b-1");
                assert_eq!(cases[3].case_id, "b-2");
            }
            _ => panic!("Expected Expanded, got {:?}", result),
        }
    }

    #[test]
    fn test_fill_opaque_placeholders_first_opaque() {
        // Issue #137: Complex first argument with trailing literals
        // (MyData(1), 2, 3) with argnames ["data", "num", "count"] at index 0
        let argnames = vec!["data".to_string(), "num".to_string(), "count".to_string()];
        assert_eq!(fill_opaque_placeholders("-2-3", &argnames, 0), "data0-2-3");
        assert_eq!(fill_opaque_placeholders("-5-6", &argnames, 1), "data1-5-6");
    }

    #[test]
    fn test_fill_opaque_placeholders_last_opaque() {
        // (1, Config(10)) with argnames ["a", "b"] at index 0
        let argnames = vec!["a".to_string(), "b".to_string()];
        assert_eq!(fill_opaque_placeholders("1-", &argnames, 0), "1-b0");
        assert_eq!(fill_opaque_placeholders("2-", &argnames, 1), "2-b1");
    }

    #[test]
    fn test_fill_opaque_placeholders_middle_opaque() {
        // (1, Config(10), 100) with argnames ["start", "config", "end"] at index 0
        let argnames = vec!["start".to_string(), "config".to_string(), "end".to_string()];
        assert_eq!(
            fill_opaque_placeholders("1--100", &argnames, 0),
            "1-config0-100"
        );
        assert_eq!(
            fill_opaque_placeholders("2--200", &argnames, 1),
            "2-config1-200"
        );
    }

    #[test]
    fn test_fill_opaque_placeholders_all_opaque() {
        // (Data(1), Data(2)) with argnames ["a", "b"] at index 0
        let argnames = vec!["a".to_string(), "b".to_string()];
        assert_eq!(fill_opaque_placeholders("-", &argnames, 0), "a0-b0");
        assert_eq!(fill_opaque_placeholders("-", &argnames, 1), "a1-b1");
    }

    #[test]
    fn test_fill_opaque_placeholders_no_opaque() {
        // All literals - should be unchanged
        let argnames = vec!["x".to_string(), "y".to_string()];
        assert_eq!(fill_opaque_placeholders("1-2", &argnames, 0), "1-2");
        assert_eq!(
            fill_opaque_placeholders("hello-world", &argnames, 1),
            "hello-world"
        );
    }
}
