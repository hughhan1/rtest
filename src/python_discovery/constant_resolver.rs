//! Resolves compile-time constants from Python AST.
//!
//! This module builds a scope tree from a Python module's AST to resolve
//! constant expressions like `X`, `Color.RED`, or `Outer.Inner.VALUE`.

use ruff_python_ast::name::UnqualifiedName;
use ruff_python_ast::{Expr, ExprList, ExprTuple, ModModule, Stmt};
use std::collections::HashMap;

use super::cases::LiteralValue;

/// A scope containing constants (module-level or class body).
///
/// Supports nested classes via recursive structure.
#[derive(Debug, Clone, Default)]
pub struct ConstantScope {
    /// Direct constant assignments in this scope (e.g., `X = 42`).
    pub constants: HashMap<String, LiteralValue>,
    /// Nested class scopes.
    pub children: HashMap<String, ConstantScope>,
}

impl ConstantScope {
    /// Build a scope tree from a list of statements.
    pub fn from_statements(stmts: &[Stmt]) -> Self {
        let mut scope = Self::default();

        for stmt in stmts {
            let (target, value) = match stmt {
                Stmt::Assign(assign) if assign.targets.len() == 1 => {
                    (&assign.targets[0], Some(assign.value.as_ref()))
                }
                Stmt::AnnAssign(ann) => (ann.target.as_ref(), ann.value.as_deref()),
                Stmt::ClassDef(class_def) => {
                    let child = Self::from_statements(&class_def.body);
                    scope.children.insert(class_def.name.to_string(), child);
                    continue;
                }
                _ => continue,
            };
            if let (Expr::Name(name), Some(value_expr)) = (target, value) {
                if let Some(value) = try_extract_literal(value_expr) {
                    scope.constants.insert(name.id.to_string(), value);
                }
            }
        }

        scope
    }

    /// Resolve a path like `["Outer", "Inner", "X"]` to a value.
    pub fn resolve_path(&self, path: &[&str]) -> Option<LiteralValue> {
        let (name, parents) = path.split_last()?;
        let scope = parents
            .iter()
            .try_fold(self, |scope, parent| scope.children.get(*parent))?;
        scope.constants.get(*name).cloned()
    }
}

/// Resolver for constant expressions in a module.
#[derive(Debug)]
pub struct ConstantResolver {
    root: ConstantScope,
}

impl ConstantResolver {
    /// Build a resolver from a parsed module.
    pub fn from_module(module: &ModModule) -> Self {
        Self {
            root: ConstantScope::from_statements(&module.body),
        }
    }

    /// Resolve an expression to a literal value.
    ///
    /// Returns `Some((value, source_path))` where `source_path` is the
    /// dotted path used for ID generation (e.g., `["Color", "RED"]`).
    pub fn resolve(&self, expr: &Expr) -> Option<(LiteralValue, Vec<String>)> {
        let name = UnqualifiedName::from_expr(expr)?;
        let value = self.root.resolve_path(name.segments())?;
        // Allocate the ID path only after resolving the borrowed name successfully.
        let path = name
            .segments()
            .iter()
            .map(|segment| (*segment).into())
            .collect();
        Some((value, path))
    }
}

/// Try to extract a literal value from an expression.
///
/// This is intentionally limited to simple literals to avoid complexity.
fn try_extract_literal(expr: &Expr) -> Option<LiteralValue> {
    match expr {
        Expr::NumberLiteral(num) => {
            use ruff_python_ast::Number;
            match &num.value {
                Number::Int(i) => Some(LiteralValue::Int(i.as_i64().unwrap_or(0))),
                Number::Float(f) => Some(LiteralValue::Float(*f)),
                Number::Complex { .. } => None,
            }
        }
        Expr::StringLiteral(s) => Some(LiteralValue::String(s.value.to_str().to_string())),
        Expr::BooleanLiteral(b) => Some(LiteralValue::Bool(b.value)),
        Expr::NoneLiteral(_) => Some(LiteralValue::None),
        Expr::List(ExprList { elts, .. }) | Expr::Tuple(ExprTuple { elts, .. }) => {
            let values: Option<Vec<_>> = elts.iter().map(try_extract_literal).collect();
            Some(LiteralValue::Sequence(values?))
        }
        _ => None,
    }
}
