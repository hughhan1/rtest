//! Resolves compile-time constants from Python AST.
//!
//! This module builds a scope tree from a Python module's AST to resolve
//! constant expressions like `X`, `Color.RED`, or `Outer.Inner.VALUE`.

use ruff_python_ast::{Expr, ExprAttribute, ExprName, ModModule, Stmt};
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
            match stmt {
                Stmt::Assign(assign) => {
                    if let [Expr::Name(name)] = assign.targets.as_slice() {
                        if let Some(value) = try_extract_literal(&assign.value) {
                            scope.constants.insert(name.id.to_string(), value);
                        }
                    }
                }
                Stmt::AnnAssign(ann) => {
                    if let (Expr::Name(name), Some(value_expr)) = (ann.target.as_ref(), &ann.value)
                    {
                        if let Some(value) = try_extract_literal(value_expr) {
                            scope.constants.insert(name.id.to_string(), value);
                        }
                    }
                }
                Stmt::ClassDef(class_def) => {
                    let child = Self::from_statements(&class_def.body);
                    scope.children.insert(class_def.name.to_string(), child);
                }
                _ => {}
            }
        }

        scope
    }

    /// Resolve a path like `["Outer", "Inner", "X"]` to a value.
    pub fn resolve_path(&self, path: &[&str]) -> Option<LiteralValue> {
        match path {
            [] => None,
            [name] => self.constants.get(*name).cloned(),
            [first, rest @ ..] => self.children.get(*first)?.resolve_path(rest),
        }
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
        let path = expr_to_path(expr)?;
        let path_refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
        let value = self.root.resolve_path(&path_refs)?;
        Some((value, path))
    }

    /// Get the root scope for testing.
    #[cfg(test)]
    pub fn root(&self) -> &ConstantScope {
        &self.root
    }
}

/// Convert an attribute chain to a path: `a.b.c` -> `["a", "b", "c"]`.
fn expr_to_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Name(ExprName { id, .. }) => Some(vec![id.to_string()]),
        Expr::Attribute(ExprAttribute { value, attr, .. }) => {
            let mut path = expr_to_path(value)?;
            path.push(attr.to_string());
            Some(path)
        }
        _ => None,
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
        Expr::List(list) => {
            let values: Option<Vec<_>> = list.elts.iter().map(try_extract_literal).collect();
            Some(LiteralValue::Sequence(values?))
        }
        Expr::Tuple(tuple) => {
            let values: Option<Vec<_>> = tuple.elts.iter().map(try_extract_literal).collect();
            Some(LiteralValue::Sequence(values?))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_ast::Mod;
    use ruff_python_parser::{parse, Mode, ParseOptions};

    fn parse_module(source: &str) -> ModModule {
        let parsed = parse(source, ParseOptions::from(Mode::Module)).unwrap();
        match parsed.into_syntax() {
            Mod::Module(m) => m,
            _ => panic!("expected module"),
        }
    }

    #[test]
    fn test_module_constant() {
        let module = parse_module("X = 42");
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.root().resolve_path(&["X"]),
            Some(LiteralValue::Int(42))
        );
    }

    #[test]
    fn test_class_constant() {
        let module = parse_module(
            r#"
class Config:
    MAX = 100
    NAME = "test"
"#,
        );
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.root().resolve_path(&["Config", "MAX"]),
            Some(LiteralValue::Int(100))
        );
        assert_eq!(
            resolver.root().resolve_path(&["Config", "NAME"]),
            Some(LiteralValue::String("test".into()))
        );
    }

    #[test]
    fn test_nested_class() {
        let module = parse_module(
            r#"
class Outer:
    A = 1
    class Inner:
        B = 2
        class Deep:
            C = 3
"#,
        );
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.root().resolve_path(&["Outer", "A"]),
            Some(LiteralValue::Int(1))
        );
        assert_eq!(
            resolver.root().resolve_path(&["Outer", "Inner", "B"]),
            Some(LiteralValue::Int(2))
        );
        assert_eq!(
            resolver
                .root()
                .resolve_path(&["Outer", "Inner", "Deep", "C"]),
            Some(LiteralValue::Int(3))
        );
    }

    #[test]
    fn test_resolve_expression() {
        let module = parse_module(
            r#"
X = 42
class Config:
    MAX = 100
"#,
        );
        let resolver = ConstantResolver::from_module(&module);

        let x_module = parse_module("X");
        let x_expr = x_module.body[0].as_expr_stmt().unwrap();
        let result = resolver.resolve(&x_expr.value);
        assert_eq!(result, Some((LiteralValue::Int(42), vec!["X".into()])));

        let config_module = parse_module("Config.MAX");
        let config_max = config_module.body[0].as_expr_stmt().unwrap();
        let result = resolver.resolve(&config_max.value);
        assert_eq!(
            result,
            Some((LiteralValue::Int(100), vec!["Config".into(), "MAX".into()]))
        );
    }

    #[test]
    fn test_annotated_assignment() {
        let module = parse_module("X: int = 42");
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.root().resolve_path(&["X"]),
            Some(LiteralValue::Int(42))
        );
    }

    #[test]
    fn test_sequence_constant() {
        let module = parse_module("DATA = [1, 2, 3]");
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.root().resolve_path(&["DATA"]),
            Some(LiteralValue::Sequence(vec![
                LiteralValue::Int(1),
                LiteralValue::Int(2),
                LiteralValue::Int(3),
            ]))
        );
    }

    #[test]
    fn test_nonexistent_path() {
        let module = parse_module("X = 42");
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(resolver.root().resolve_path(&["Y"]), None);
        assert_eq!(resolver.root().resolve_path(&["X", "Y"]), None);
    }
}
