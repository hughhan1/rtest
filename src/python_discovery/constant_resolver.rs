//! Resolves compile-time constants from Python AST.
//!
//! This module builds a scope tree from a Python module's AST to resolve
//! constant expressions like `X`, `Color.RED`, or `Outer.Inner.VALUE`.
//! When a [`ModuleResolver`] is provided, imported names from other project
//! modules are resolved as well.

use ruff_python_ast::{Expr, ExprAttribute, ExprName, ModModule, Stmt};
use ruff_python_stdlib::sys::is_known_standard_library;
use std::cell::RefCell;
use std::collections::HashMap;

use super::cases::LiteralValue;
use super::module_resolver::{resolve_relative_import, ModuleResolver};

/// Maximum import-chain depth when loading modules on demand.
const MAX_IMPORT_DEPTH: usize = 8;

/// A scope containing constants (module-level or class body).
///
/// Supports nested classes via recursive structure.
#[derive(Debug, Clone, Default)]
pub struct ConstantScope {
    /// Direct constant assignments in this scope (e.g., `X = 42`).
    pub constants: HashMap<String, LiteralValue>,
    /// Nested class scopes.
    pub children: HashMap<String, ConstantScope>,
    /// Simple base class names (`class Foo(Bar)` → `"Bar"`).
    bases: Vec<String>,
    /// True when this class inherits from `Enum` / `IntEnum` / `StrEnum` / `Flag`.
    is_enum: bool,
    /// Sequence assignments whose elements are literals or attribute paths,
    /// resolved after imported modules are loaded.
    pending_sequences: Vec<(String, Vec<PendingElement>)>,
}

/// A list/tuple element that could not be folded on the first pass.
#[derive(Debug, Clone)]
enum PendingElement {
    Literal(LiteralValue),
    Path(Vec<String>),
}

/// Binding created by `import` / `from ... import`.
#[derive(Debug, Clone)]
struct ImportBinding {
    module_path: Vec<String>,
    /// Empty for `import foo` (the local name is a module). Non-empty for
    /// `from foo import Bar` (the local name is `Bar` inside `foo`).
    imported_name: String,
    relative_level: usize,
}

#[derive(Debug, Clone, Default)]
struct LoadedModule {
    scope: ConstantScope,
    imports: HashMap<String, ImportBinding>,
}

/// A constant resolved from a name or attribute expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConstant {
    pub value: LiteralValue,
    /// Source path as written (`["Color", "RED"]` or `["DATA"]`).
    pub source_path: Vec<String>,
    /// True when the value is a member of an `Enum` subclass.
    pub is_enum_member: bool,
}

/// Resolver for constant expressions in a module.
pub struct ConstantResolver {
    current_path: Vec<String>,
    modules: RefCell<HashMap<Vec<String>, LoadedModule>>,
    /// Present when imported modules may be loaded on demand.
    loader: Option<RefCell<ModuleResolver>>,
    resolving_pending: std::cell::Cell<bool>,
}

impl ConstantScope {
    /// Build a scope tree from a list of statements.
    pub fn from_statements(stmts: &[Stmt]) -> Self {
        let mut scope = Self::default();

        for stmt in stmts {
            match stmt {
                Stmt::Assign(assign) => {
                    if let [Expr::Name(name)] = assign.targets.as_slice() {
                        record_assignment(&mut scope, name.id.as_str(), &assign.value);
                    }
                }
                Stmt::AnnAssign(ann) => {
                    if let (Expr::Name(name), Some(value_expr)) = (ann.target.as_ref(), &ann.value)
                    {
                        record_assignment(&mut scope, name.id.as_str(), value_expr);
                    }
                }
                Stmt::ClassDef(class_def) => {
                    let mut child = Self::from_statements(&class_def.body);
                    child.bases = class_base_simple_names(class_def);
                    child.is_enum = child.bases.iter().any(|base| is_enum_class_name(base));
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
            [class_name, member] => self
                .lookup_class_member(class_name, member)
                .or_else(|| self.children.get(*class_name)?.resolve_path(&[member])),
            [first, rest @ ..] => self.children.get(*first)?.resolve_path(rest),
        }
    }

    fn lookup_class_member(&self, class_name: &str, member: &str) -> Option<LiteralValue> {
        let class = self.children.get(class_name)?;
        if let Some(value) = class.constants.get(member) {
            return Some(value.clone());
        }
        for base in &class.bases {
            if is_enum_class_name(base) {
                continue;
            }
            if let Some(value) = self.lookup_class_member(base, member) {
                return Some(value);
            }
        }
        None
    }

    fn class_is_enum(&self, class_name: &str) -> bool {
        let Some(class) = self.children.get(class_name) else {
            return false;
        };
        if class.is_enum {
            return true;
        }
        class
            .bases
            .iter()
            .any(|base| !is_enum_class_name(base) && self.class_is_enum(base))
    }
}

fn record_assignment(scope: &mut ConstantScope, name: &str, value: &Expr) {
    if let Some(literal) = try_extract_literal(value) {
        scope.constants.insert(name.to_string(), literal);
        return;
    }
    if let Some(elements) = try_pending_sequence(value) {
        scope.pending_sequences.push((name.to_string(), elements));
    }
}

fn try_pending_sequence(expr: &Expr) -> Option<Vec<PendingElement>> {
    let elts = match expr {
        Expr::List(list) => &list.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        _ => return None,
    };
    let mut elements = Vec::with_capacity(elts.len());
    for elt in elts {
        if let Some(literal) = try_extract_literal(elt) {
            elements.push(PendingElement::Literal(literal));
        } else if let Some(path) = expr_to_path(elt) {
            elements.push(PendingElement::Path(path));
        } else {
            return None;
        }
    }
    Some(elements)
}

fn class_base_simple_names(class_def: &ruff_python_ast::StmtClassDef) -> Vec<String> {
    let Some(arguments) = class_def.arguments.as_ref() else {
        return Vec::new();
    };
    arguments
        .args
        .iter()
        .filter_map(|expr| match expr {
            Expr::Name(name) => Some(name.id.to_string()),
            Expr::Subscript(sub) => match sub.value.as_ref() {
                Expr::Name(name) => Some(name.id.to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn is_enum_class_name(name: &str) -> bool {
    matches!(name, "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag")
}

fn wrap_enum_member(value: LiteralValue, is_enum: bool, path: &[String]) -> LiteralValue {
    if is_enum && path.len() >= 2 {
        LiteralValue::EnumMember {
            class_name: path[path.len() - 2].clone(),
            member_name: path[path.len() - 1].clone(),
        }
    } else {
        value
    }
}

impl ConstantResolver {
    /// Build a resolver from a parsed module (same-module constants only).
    pub fn from_module(module: &ModModule) -> Self {
        let resolver = Self {
            current_path: Vec::new(),
            modules: RefCell::new(HashMap::new()),
            loader: None,
            resolving_pending: std::cell::Cell::new(false),
        };
        resolver.insert_module(Vec::new(), module);
        resolver.resolve_pending_assignments();
        resolver
    }

    /// Build a resolver that follows project-local imports on demand.
    ///
    /// Imported modules are parsed only when a name lookup actually needs them,
    /// so collecting a file that imports a large graph stays cheap.
    pub fn from_module_with_imports(
        module: &ModModule,
        module_path: &[String],
        module_resolver: &mut ModuleResolver,
    ) -> Self {
        let resolver = Self {
            current_path: module_path.to_vec(),
            modules: RefCell::new(HashMap::new()),
            loader: Some(RefCell::new(module_resolver.with_search_paths())),
            resolving_pending: std::cell::Cell::new(false),
        };
        resolver.insert_module(module_path.to_vec(), module);
        resolver.resolve_pending_assignments();
        resolver
    }

    /// Resolve an expression to a literal value.
    ///
    /// Returns `Some((value, source_path))` where `source_path` is the
    /// dotted path used for ID generation (e.g., `["Color", "RED"]`).
    #[cfg(test)]
    pub fn resolve(&self, expr: &Expr) -> Option<(LiteralValue, Vec<String>)> {
        self.resolve_in_class(None, expr)
            .map(|resolved| (resolved.value, resolved.source_path))
    }

    /// Resolve an expression, optionally looking up names on an enclosing class first.
    pub fn resolve_in_class(
        &self,
        enclosing_class: Option<&str>,
        expr: &Expr,
    ) -> Option<ResolvedConstant> {
        let path = expr_to_path(expr)?;
        let (value, is_enum_member) =
            self.lookup_path(&self.current_path, enclosing_class, &path)?;
        Some(ResolvedConstant {
            value: wrap_enum_member(value, is_enum_member, &path),
            source_path: path,
            is_enum_member,
        })
    }

    /// Get the root scope for testing.
    #[cfg(test)]
    pub fn root(&self) -> std::cell::Ref<'_, ConstantScope> {
        std::cell::Ref::map(self.modules.borrow(), |modules| {
            &modules
                .get(&self.current_path)
                .expect("current module is always inserted")
                .scope
        })
    }

    fn insert_module(&self, module_path: Vec<String>, module: &ModModule) {
        if self.modules.borrow().contains_key(&module_path) {
            return;
        }
        self.modules.borrow_mut().insert(
            module_path,
            LoadedModule {
                scope: ConstantScope::from_statements(&module.body),
                imports: collect_imports(module),
            },
        );
    }

    /// Parse `module_path` if it is not already in `self.modules`.
    fn ensure_loaded(&self, module_path: &[String], depth: usize) {
        if self.modules.borrow().contains_key(module_path) || depth > MAX_IMPORT_DEPTH {
            return;
        }
        if is_skipped_module(module_path) {
            return;
        }
        let Some(loader) = &self.loader else {
            return;
        };
        let loaded = {
            let mut loader = loader.borrow_mut();
            match loader.resolve_and_load(module_path) {
                Ok(parsed) => LoadedModule {
                    scope: ConstantScope::from_statements(&parsed.module.body),
                    imports: collect_imports(&parsed.module),
                },
                Err(_) => return,
            }
        };
        self.modules
            .borrow_mut()
            .insert(module_path.to_vec(), loaded);
    }

    fn resolve_pending_assignments(&self) {
        if self.resolving_pending.get() {
            return;
        }
        self.resolving_pending.set(true);
        for _ in 0..8 {
            let updates = self.collect_pending_updates();
            if updates.is_empty() {
                break;
            }
            for update in updates {
                if let Some(module) = self.modules.borrow_mut().get_mut(&update.module_path) {
                    insert_constant_at_class_path(
                        &mut module.scope,
                        &update.class_path,
                        update.name,
                        update.value,
                    );
                }
            }
        }
        self.resolving_pending.set(false);
    }

    fn collect_pending_updates(&self) -> Vec<PendingUpdate> {
        // Snapshot scopes so lookup_path can load imported modules without
        // holding a RefCell borrow of `self.modules`.
        let snapshots: Vec<(Vec<String>, ConstantScope)> = self
            .modules
            .borrow()
            .iter()
            .map(|(path, module)| (path.clone(), module.scope.clone()))
            .collect();
        let mut updates = Vec::new();
        for (module_path, scope) in &snapshots {
            self.collect_pending_in_scope(module_path, &[], scope, &mut updates);
        }
        updates
    }

    fn collect_pending_in_scope(
        &self,
        module_path: &[String],
        class_path: &[String],
        scope: &ConstantScope,
        updates: &mut Vec<PendingUpdate>,
    ) {
        for (name, elements) in &scope.pending_sequences {
            if scope.constants.contains_key(name) {
                continue;
            }
            let mut values = Vec::with_capacity(elements.len());
            let mut ok = true;
            for element in elements {
                match element {
                    PendingElement::Literal(value) => values.push(value.clone()),
                    PendingElement::Path(path) => {
                        if let Some((value, is_enum)) = self.lookup_path(module_path, None, path) {
                            values.push(wrap_enum_member(value, is_enum, path));
                        } else {
                            ok = false;
                            break;
                        }
                    }
                }
            }
            if ok {
                updates.push(PendingUpdate {
                    module_path: module_path.to_vec(),
                    class_path: class_path.to_vec(),
                    name: name.clone(),
                    value: LiteralValue::Sequence(values),
                });
            }
        }
        for (child_name, child) in &scope.children {
            let mut child_path = class_path.to_vec();
            child_path.push(child_name.clone());
            self.collect_pending_in_scope(module_path, &child_path, child, updates);
        }
    }

    fn lookup_path(
        &self,
        module_path: &[String],
        enclosing_class: Option<&str>,
        path: &[String],
    ) -> Option<(LiteralValue, bool)> {
        self.lookup_path_at(module_path, enclosing_class, path, 0)
    }

    fn lookup_path_at(
        &self,
        module_path: &[String],
        enclosing_class: Option<&str>,
        path: &[String],
        depth: usize,
    ) -> Option<(LiteralValue, bool)> {
        if depth > MAX_IMPORT_DEPTH {
            return None;
        }
        match path {
            [] => None,
            [name] => {
                if let Some(class_name) = enclosing_class {
                    if let Some((value, is_enum)) =
                        self.lookup_class_member(module_path, class_name, name, depth)
                    {
                        return Some((value, is_enum));
                    }
                }
                let (constant, import, pending) = {
                    let modules = self.modules.borrow();
                    let module = modules.get(module_path)?;
                    (
                        module.scope.constants.get(name).cloned(),
                        module.imports.get(name).cloned(),
                        module
                            .scope
                            .pending_sequences
                            .iter()
                            .find(|(pending_name, _)| pending_name == name)
                            .map(|(_, elements)| elements.clone()),
                    )
                };
                if let Some(value) = constant {
                    return Some((value, false));
                }
                if let Some(elements) = pending {
                    if let Some(value) =
                        self.resolve_pending_elements(module_path, &elements, depth)
                    {
                        if let Some(module) = self.modules.borrow_mut().get_mut(module_path) {
                            module.scope.constants.insert(name.clone(), value.clone());
                        }
                        return Some((value, false));
                    }
                }
                let import = import?;
                let abs = resolve_relative_import(
                    module_path,
                    import.relative_level,
                    &import.module_path,
                )?;
                if import.imported_name.is_empty() {
                    return None;
                }
                self.ensure_loaded(&abs, depth + 1);
                self.lookup_path_at(&abs, None, &[import.imported_name], depth + 1)
            }
            [first, rest @ ..] => {
                let (has_class, import) = {
                    let modules = self.modules.borrow();
                    let module = modules.get(module_path)?;
                    (
                        module.scope.children.contains_key(first),
                        module.imports.get(first).cloned(),
                    )
                };
                if has_class {
                    return self.lookup_class_path(module_path, first, rest, depth);
                }
                let import = import?;
                let abs = resolve_relative_import(
                    module_path,
                    import.relative_level,
                    &import.module_path,
                )?;
                self.ensure_loaded(&abs, depth + 1);
                if import.imported_name.is_empty() {
                    self.lookup_path_at(&abs, None, rest, depth + 1)
                } else {
                    let mut imported_path = vec![import.imported_name];
                    imported_path.extend(rest.iter().cloned());
                    self.lookup_path_at(&abs, None, &imported_path, depth + 1)
                }
            }
        }
    }

    fn resolve_pending_elements(
        &self,
        module_path: &[String],
        elements: &[PendingElement],
        depth: usize,
    ) -> Option<LiteralValue> {
        let mut values = Vec::with_capacity(elements.len());
        for element in elements {
            match element {
                PendingElement::Literal(value) => values.push(value.clone()),
                PendingElement::Path(path) => {
                    let (value, is_enum) =
                        self.lookup_path_at(module_path, None, path, depth + 1)?;
                    values.push(wrap_enum_member(value, is_enum, path));
                }
            }
        }
        Some(LiteralValue::Sequence(values))
    }

    fn lookup_class_path(
        &self,
        module_path: &[String],
        class_name: &str,
        rest: &[String],
        depth: usize,
    ) -> Option<(LiteralValue, bool)> {
        match rest {
            [] => None,
            [member] => self.lookup_class_member(module_path, class_name, member, depth),
            [nested, rest @ ..] => {
                let modules = self.modules.borrow();
                let module = modules.get(module_path)?;
                let class = module.scope.children.get(class_name)?;
                if class.children.contains_key(nested) {
                    let mut full_path = vec![class_name.to_string(), nested.clone()];
                    full_path.extend(rest.iter().cloned());
                    let path_refs: Vec<&str> = full_path.iter().map(String::as_str).collect();
                    let value = module.scope.resolve_path(&path_refs)?;
                    Some((value, false))
                } else {
                    None
                }
            }
        }
    }

    fn lookup_class_member(
        &self,
        module_path: &[String],
        class_name: &str,
        member: &str,
        depth: usize,
    ) -> Option<(LiteralValue, bool)> {
        let (bases, imports) = {
            let modules = self.modules.borrow();
            let module = modules.get(module_path)?;
            if let Some(value) = module.scope.lookup_class_member(class_name, member) {
                let is_enum = module.scope.class_is_enum(class_name)
                    || defining_enum_class(&module.scope, class_name, member).is_some();
                return Some((value, is_enum));
            }
            (
                module.scope.children.get(class_name)?.bases.clone(),
                module.imports.clone(),
            )
        };
        for base in bases {
            if is_enum_class_name(&base) {
                continue;
            }
            if let Some(import) = imports.get(&base) {
                let abs = resolve_relative_import(
                    module_path,
                    import.relative_level,
                    &import.module_path,
                );
                let Some(abs) = abs else {
                    continue;
                };
                if import.imported_name.is_empty() {
                    continue;
                }
                self.ensure_loaded(&abs, depth + 1);
                if let Some(found) =
                    self.lookup_class_member(&abs, &import.imported_name, member, depth + 1)
                {
                    return Some(found);
                }
            }
        }
        None
    }
}

struct PendingUpdate {
    module_path: Vec<String>,
    class_path: Vec<String>,
    name: String,
    value: LiteralValue,
}

fn insert_constant_at_class_path(
    scope: &mut ConstantScope,
    class_path: &[String],
    name: String,
    value: LiteralValue,
) {
    if class_path.is_empty() {
        scope.constants.insert(name, value);
        return;
    }
    let Some((first, rest)) = class_path.split_first() else {
        return;
    };
    if let Some(child) = scope.children.get_mut(first) {
        insert_constant_at_class_path(child, rest, name, value);
    }
}

fn defining_enum_class(scope: &ConstantScope, class_name: &str, member: &str) -> Option<String> {
    let class = scope.children.get(class_name)?;
    if class.constants.contains_key(member) {
        return class.is_enum.then(|| class_name.to_string());
    }
    for base in &class.bases {
        if is_enum_class_name(base) {
            continue;
        }
        if let Some(found) = defining_enum_class(scope, base, member) {
            return Some(found);
        }
    }
    None
}

fn collect_imports(module: &ModModule) -> HashMap<String, ImportBinding> {
    let mut imports = HashMap::new();

    for stmt in &module.body {
        match stmt {
            Stmt::ImportFrom(import_from) => {
                let level = import_from.level as usize;
                let module_path = if let Some(module_name) = &import_from.module {
                    module_name.split('.').map(String::from).collect()
                } else {
                    Vec::new()
                };

                for alias in &import_from.names {
                    let imported_name = alias.name.to_string();
                    if imported_name == "*" {
                        continue;
                    }
                    let local_name = alias
                        .asname
                        .as_ref()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| imported_name.clone());
                    imports.insert(
                        local_name,
                        ImportBinding {
                            module_path: module_path.clone(),
                            imported_name,
                            relative_level: level,
                        },
                    );
                }
            }
            Stmt::Import(import) => {
                for alias in &import.names {
                    let parts: Vec<String> = alias.name.split('.').map(String::from).collect();
                    if let Some(asname) = &alias.asname {
                        imports.insert(
                            asname.to_string(),
                            ImportBinding {
                                module_path: parts,
                                imported_name: String::new(),
                                relative_level: 0,
                            },
                        );
                    } else if let Some(first) = parts.first() {
                        imports.insert(
                            first.clone(),
                            ImportBinding {
                                module_path: vec![first.clone()],
                                imported_name: String::new(),
                                relative_level: 0,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    imports
}

fn is_skipped_module(module_path: &[String]) -> bool {
    module_path.first().is_some_and(|name| {
        is_known_standard_library(11, name) || name == "pytest" || name == "rtest"
    })
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
        Expr::UnaryOp(unary) => {
            use ruff_python_ast::UnaryOp;
            if !matches!(unary.op, UnaryOp::USub) {
                return None;
            }
            match try_extract_literal(&unary.operand) {
                Some(LiteralValue::Int(i)) => Some(LiteralValue::Int(-i)),
                Some(LiteralValue::Float(f)) => Some(LiteralValue::Float(-f)),
                _ => None,
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

    fn expr_from(source: &str) -> Expr {
        let module = parse_module(source);
        module.body[0]
            .as_expr_stmt()
            .unwrap()
            .value
            .as_ref()
            .clone()
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

        let result = resolver.resolve(&expr_from("X"));
        assert_eq!(result, Some((LiteralValue::Int(42), vec!["X".into()])));

        let result = resolver.resolve(&expr_from("Config.MAX"));
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

    #[test]
    fn test_inherited_class_member() {
        let module = parse_module(
            r#"
class Parent:
    READER = "reader"

class Role(Parent):
    ADMIN = "admin"
"#,
        );
        let resolver = ConstantResolver::from_module(&module);

        assert_eq!(
            resolver.resolve(&expr_from("Role.ADMIN")),
            Some((
                LiteralValue::String("admin".into()),
                vec!["Role".into(), "ADMIN".into()]
            ))
        );
        assert_eq!(
            resolver.resolve(&expr_from("Role.READER")),
            Some((
                LiteralValue::String("reader".into()),
                vec!["Role".into(), "READER".into()]
            ))
        );
    }

    #[test]
    fn test_enum_member_flag() {
        let module = parse_module(
            r#"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2
"#,
        );
        let resolver = ConstantResolver::from_module(&module);
        let resolved = resolver
            .resolve_in_class(None, &expr_from("Color.RED"))
            .unwrap();
        assert_eq!(
            resolved.value,
            LiteralValue::EnumMember {
                class_name: "Color".into(),
                member_name: "RED".into(),
            }
        );
        assert!(resolved.is_enum_member);
    }

    #[test]
    fn test_sequence_of_class_attributes() {
        let module = parse_module(
            r#"
class Color:
    RED = 1
    GREEN = 2

DATA = [Color.RED, Color.GREEN]
"#,
        );
        let resolver = ConstantResolver::from_module(&module);
        assert_eq!(
            resolver.resolve(&expr_from("DATA")),
            Some((
                LiteralValue::Sequence(vec![LiteralValue::Int(1), LiteralValue::Int(2)]),
                vec!["DATA".into()]
            ))
        );
    }

    #[test]
    fn test_sequence_of_enum_members() {
        let module = parse_module(
            r#"
from enum import Enum

class Color(Enum):
    RED = 1
    GREEN = 2

DATA = [Color.RED, Color.GREEN]
"#,
        );
        let resolver = ConstantResolver::from_module(&module);
        assert_eq!(
            resolver.resolve(&expr_from("DATA")),
            Some((
                LiteralValue::Sequence(vec![
                    LiteralValue::EnumMember {
                        class_name: "Color".into(),
                        member_name: "RED".into(),
                    },
                    LiteralValue::EnumMember {
                        class_name: "Color".into(),
                        member_name: "GREEN".into(),
                    },
                ]),
                vec!["DATA".into()]
            ))
        );
    }

    #[test]
    fn test_enclosing_class_constant() {
        let module = parse_module(
            r#"
class TestFoo:
    PACKAGES = ["a", "b"]
"#,
        );
        let resolver = ConstantResolver::from_module(&module);
        let resolved = resolver
            .resolve_in_class(Some("TestFoo"), &expr_from("PACKAGES"))
            .unwrap();
        assert_eq!(
            resolved.value,
            LiteralValue::Sequence(vec![
                LiteralValue::String("a".into()),
                LiteralValue::String("b".into()),
            ])
        );
    }

    #[test]
    fn test_cross_module_imported_constant() {
        let constants = parse_module("DATA = [1, 2, 3]\nVALUE = 100\n");
        let tests = parse_module("from constants import DATA, VALUE\n");
        let resolver = ConstantResolver {
            current_path: vec!["test_mod".into()],
            modules: RefCell::new(HashMap::new()),
            loader: None,
            resolving_pending: std::cell::Cell::new(false),
        };
        resolver.insert_module(vec!["test_mod".into()], &tests);
        resolver.insert_module(vec!["constants".into()], &constants);
        resolver.resolve_pending_assignments();

        assert_eq!(
            resolver.resolve(&expr_from("VALUE")),
            Some((LiteralValue::Int(100), vec!["VALUE".into()]))
        );
        assert_eq!(
            resolver.resolve(&expr_from("DATA")),
            Some((
                LiteralValue::Sequence(vec![
                    LiteralValue::Int(1),
                    LiteralValue::Int(2),
                    LiteralValue::Int(3),
                ]),
                vec!["DATA".into()]
            ))
        );
    }

    #[test]
    fn test_cross_module_imported_enum_member() {
        let colors = parse_module(
            r#"
from enum import Enum
class Color(Enum):
    RED = 1
"#,
        );
        let tests = parse_module("from colors import Color\n");
        let resolver = ConstantResolver {
            current_path: vec!["test_mod".into()],
            modules: RefCell::new(HashMap::new()),
            loader: None,
            resolving_pending: std::cell::Cell::new(false),
        };
        resolver.insert_module(vec!["test_mod".into()], &tests);
        resolver.insert_module(vec!["colors".into()], &colors);
        resolver.resolve_pending_assignments();

        let resolved = resolver
            .resolve_in_class(None, &expr_from("Color.RED"))
            .unwrap();
        assert_eq!(
            resolved.value,
            LiteralValue::EnumMember {
                class_name: "Color".into(),
                member_name: "RED".into(),
            }
        );
        assert!(resolved.is_enum_member);
        assert_eq!(
            resolved.source_path,
            vec!["Color".to_string(), "RED".into()]
        );
    }
}
