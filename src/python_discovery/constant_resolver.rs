//! Resolves compile-time constants from Python AST.
//!
//! This module builds a scope tree from a Python module's AST to resolve
//! constant expressions like `X`, `Color.RED`, or `Outer.Inner.VALUE`.
//! When a [`ModuleResolver`] is provided, imported names from other project
//! modules are resolved as well.

use ruff_python_ast::name::UnqualifiedName;
use ruff_python_ast::{helpers::map_subscript, Expr, ModModule, Stmt};
use ruff_python_stdlib::sys::is_known_standard_library;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use super::cases::LiteralValue;
use super::module_resolver::{resolve_relative_import, ModuleResolver};

/// Maximum import-chain depth when loading modules on demand.
const MAX_IMPORT_DEPTH: usize = 8;

/// A scope containing constants (module-level or class body).
///
/// Supports nested classes via recursive structure.
#[derive(Debug, Default)]
struct ConstantScope {
    /// Direct constant assignments in this scope (e.g., `X = 42`).
    constants: RefCell<HashMap<String, LiteralValue>>,
    /// Nested class scopes.
    children: HashMap<String, ConstantScope>,
    /// Base class paths (`class Foo(Bar)` → `"Bar"`).
    bases: Vec<String>,
    /// True when this class inherits from `Enum` / `IntEnum` / `StrEnum` / `Flag`.
    is_enum: bool,
    /// Sequence assignments whose elements are literals or attribute paths,
    /// resolved after imported modules are loaded.
    pending_sequences: Vec<(String, Vec<PendingElement>)>,
    /// Track bindings even when their values are unknown. Repeated class bindings
    /// need decorator evaluation order, so they use collection fallback.
    assignments: HashMap<String, usize>,
}

/// A list/tuple element that could not be folded on the first pass.
#[derive(Debug)]
enum PendingElement {
    Literal(LiteralValue),
    Path(Vec<String>),
}

/// Binding created by `import` / `from ... import`.
#[derive(Debug)]
struct ImportBinding {
    module_path: Vec<String>,
    /// Empty for `import foo` (the local name is a module). Non-empty for
    /// `from foo import Bar` (the local name is `Bar` inside `foo`).
    imported_name: String,
}

#[derive(Debug)]
struct LoadedModule {
    scope: ConstantScope,
    imports: HashMap<String, ImportBinding>,
}

impl LoadedModule {
    fn from_module(module: &ModModule, module_path: &[String], is_package: bool) -> Self {
        // Relative imports start from the file's package, including in __init__.py.
        let context: Vec<_> = module_path
            .iter()
            .cloned()
            .chain(is_package.then(|| "__init__".to_string()))
            .collect();
        let imports = collect_imports(module, &context);
        Self {
            scope: ConstantScope::from_statements(&module.body, &imports),
            imports,
        }
    }
}

/// A constant resolved from a name or attribute expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConstant {
    pub value: LiteralValue,
    /// Source path as written (`["Color", "RED"]` or `["DATA"]`).
    pub source_path: Vec<String>,
}

/// Resolver for constant expressions in a module.
pub struct ConstantResolver {
    current_path: Vec<String>,
    modules: RefCell<HashMap<Vec<String>, Rc<LoadedModule>>>,
    /// Present when imported modules may be loaded on demand.
    loader: Option<RefCell<ModuleResolver>>,
}

impl ConstantScope {
    /// Build a scope tree from a list of statements.
    fn from_statements(stmts: &[Stmt], imports: &HashMap<String, ImportBinding>) -> Self {
        let mut scope = Self::default();

        for stmt in stmts {
            let (target, value) = match stmt {
                Stmt::Assign(assign) if assign.targets.len() == 1 => {
                    (&assign.targets[0], Some(assign.value.as_ref()))
                }
                Stmt::AnnAssign(ann) => (ann.target.as_ref(), ann.value.as_deref()),
                Stmt::ClassDef(class_def) => {
                    let mut child = Self::from_statements(&class_def.body, imports);
                    child.bases = class_def
                        .bases()
                        .iter()
                        .map(map_subscript)
                        .filter_map(|base| {
                            imported_expression_name(base, imports)
                                .filter(|name| matches!(name.as_str(), "enum.Enum" | "enum.Flag"))
                                .or_else(|| {
                                    UnqualifiedName::from_expr(base).map(|name| name.to_string())
                                })
                        })
                        .collect();
                    child.is_enum = child.bases.iter().any(|base| is_enum_class_name(base));
                    scope.children.insert(class_def.name.to_string(), child);
                    continue;
                }
                _ => continue,
            };
            if let (Expr::Name(name), Some(value_expr)) = (target, value) {
                *scope.assignments.entry(name.id.to_string()).or_default() += 1;
                record_assignment(&mut scope, name.id.as_str(), value_expr);
            }
        }

        scope
    }

    /// Resolve a path like `["Outer", "Inner", "X"]` to a value.
    fn resolve_path(&self, path: &[&str]) -> Option<LiteralValue> {
        let (member, parents) = path.split_last()?;
        match parents.split_last() {
            None => self.constants.borrow().get(*member).cloned(),
            Some((class_name, ancestors)) => {
                let scope = ancestors
                    .iter()
                    .try_fold(self, |scope, name| scope.children.get(*name))?;
                scope.lookup_class_member(class_name, member)
            }
        }
    }

    fn lookup_class_member(&self, class_name: &str, member: &str) -> Option<LiteralValue> {
        let class = self.children.get(class_name)?;
        if class.assignments.contains_key(member) {
            return class.constants.borrow().get(member).cloned();
        }
        // Multiple inheritance needs Python's C3 order; leave it to fallback.
        let [base] = class.bases.as_slice() else {
            return None;
        };
        self.lookup_class_member(base, member)
    }

    fn class_is_enum(&self, class_name: &str) -> bool {
        let Some(class) = self.children.get(class_name) else {
            return false;
        };
        if class.is_enum {
            return true;
        }
        class.bases.iter().any(|base| self.class_is_enum(base))
    }

    fn collect_pending<'a>(
        &'a self,
        module_path: &'a [String],
        pending: &mut Vec<PendingAssignment<'a>>,
    ) {
        for (name, elements) in &self.pending_sequences {
            if !self.constants.borrow().contains_key(name) {
                pending.push(PendingAssignment {
                    module_path,
                    scope: self,
                    name,
                    elements,
                });
            }
        }
        for child in self.children.values() {
            child.collect_pending(module_path, pending);
        }
    }
}

fn record_assignment(scope: &mut ConstantScope, name: &str, value: &Expr) {
    scope.constants.get_mut().remove(name);
    scope
        .pending_sequences
        .retain(|(pending, _)| pending != name);
    if let Some(literal) = try_extract_literal(value) {
        scope.constants.get_mut().insert(name.to_string(), literal);
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
        } else {
            elements.push(PendingElement::Path(
                UnqualifiedName::from_expr(elt)?
                    .segments()
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
            ));
        }
    }
    Some(elements)
}

fn imported_expression_name(
    expr: &Expr,
    imports: &HashMap<String, ImportBinding>,
) -> Option<String> {
    let name = UnqualifiedName::from_expr(expr)?;
    let (first, rest) = name.segments().split_first()?;
    let import = imports.get(*first)?;
    Some(
        import
            .module_path
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(import.imported_name.as_str()).filter(|name| !name.is_empty()))
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn is_enum_class_name(name: &str) -> bool {
    matches!(
        name,
        "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag" | "enum.Enum" | "enum.Flag"
    )
}

fn wrap_enum_member(value: LiteralValue, is_enum: bool, path: &[String]) -> LiteralValue {
    if is_enum {
        LiteralValue::EnumMember(path[path.len().saturating_sub(2)..].join("."))
    } else {
        value
    }
}

impl ConstantResolver {
    /// Build a resolver from a parsed module (same-module constants only).
    pub fn from_module(module: &ModModule) -> Self {
        Self::new(module, Vec::new(), None, false)
    }

    /// Build a resolver that follows project-local imports on demand.
    ///
    /// Imported modules are parsed only when a name lookup actually needs them,
    /// so collecting a file that imports a large graph stays cheap.
    pub fn from_module_with_imports(
        module: &ModModule,
        module_path: &[String],
        module_resolver: ModuleResolver,
        file_path: &Path,
    ) -> Self {
        Self::new(
            module,
            module_path.to_vec(),
            Some(module_resolver),
            file_path.ends_with("__init__.py"),
        )
    }

    fn new(
        module: &ModModule,
        current_path: Vec<String>,
        loader: Option<ModuleResolver>,
        is_package: bool,
    ) -> Self {
        let loaded = LoadedModule::from_module(module, &current_path, is_package);
        let resolver = Self {
            modules: RefCell::new(HashMap::from([(current_path.clone(), Rc::new(loaded))])),
            current_path,
            loader: loader.map(RefCell::new),
        };
        resolver.resolve_pending_assignments();
        resolver
    }

    pub fn imported_expression_name(&self, expr: &Expr) -> Option<String> {
        imported_expression_name(
            expr,
            &self.modules.borrow().get(&self.current_path)?.imports,
        )
    }

    /// Resolve an expression, optionally looking up names on an enclosing class first.
    pub fn resolve_in_class(
        &self,
        enclosing_class: Option<&str>,
        expr: &Expr,
    ) -> Option<ResolvedConstant> {
        let name = UnqualifiedName::from_expr(expr)?;
        let (value, is_enum_member) =
            self.lookup_path_at(&self.current_path, enclosing_class, name.segments(), 0)?;
        let path: Vec<String> = name
            .segments()
            .iter()
            .map(|segment| (*segment).into())
            .collect();
        Some(ResolvedConstant {
            value: wrap_enum_member(value, is_enum_member, &path),
            source_path: path,
        })
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
                Ok(parsed) => LoadedModule::from_module(
                    &parsed.module,
                    module_path,
                    parsed.path.ends_with("__init__.py"),
                ),
                Err(_) => return,
            }
        };
        self.modules
            .borrow_mut()
            .insert(module_path.to_vec(), Rc::new(loaded));
    }

    fn resolve_pending_assignments(&self) {
        for _ in 0..8 {
            // Hold stable modules rather than copying every scope. Freeze the worklist
            // before resolving: lookups may cache values and load additional modules.
            let modules: Vec<_> = self
                .modules
                .borrow()
                .iter()
                .map(|(path, module)| (path.clone(), Rc::clone(module)))
                .collect();
            let mut pending = Vec::new();
            for (path, module) in &modules {
                module.scope.collect_pending(path, &mut pending);
            }
            let updates: Vec<_> = pending
                .into_iter()
                .filter_map(|assignment| {
                    self.resolve_pending_elements(assignment.module_path, assignment.elements, 0)
                        .map(|value| (assignment, value))
                })
                .collect();
            if updates.is_empty() {
                break;
            }
            for (assignment, value) in updates {
                assignment
                    .scope
                    .constants
                    .borrow_mut()
                    .insert(assignment.name.into(), value);
            }
        }
    }

    fn lookup_path_at(
        &self,
        module_path: &[String],
        enclosing_class: Option<&str>,
        path: &[&str],
        depth: usize,
    ) -> Option<(LiteralValue, bool)> {
        if depth > MAX_IMPORT_DEPTH {
            return None;
        }
        let (&first, rest) = path.split_first()?;
        let module = Rc::clone(self.modules.borrow().get(module_path)?);
        if enclosing_class
            .and_then(|name| module.scope.children.get(name))
            .and_then(|scope| scope.assignments.get(first))
            .is_some_and(|count| *count > 1)
        {
            return None;
        }
        if rest.is_empty() {
            if let Some(found) = enclosing_class
                .and_then(|class| self.lookup_class_member(module_path, class, first, depth))
            {
                return Some(found);
            }
            if enclosing_class
                .and_then(|name| module.scope.children.get(name))
                .is_some_and(|scope| scope.assignments.contains_key(first))
            {
                return None;
            }
            if let Some(value) = module.scope.constants.borrow().get(first) {
                return Some((value.clone(), false));
            }
            let pending = module
                .scope
                .pending_sequences
                .iter()
                .find(|(name, _)| name == first);
            if let Some(value) = pending.and_then(|(_, elements)| {
                self.resolve_pending_elements(module_path, elements, depth + 1)
            }) {
                module
                    .scope
                    .constants
                    .borrow_mut()
                    .insert(first.to_string(), value.clone());
                return Some((value, false));
            }
            if module.scope.assignments.contains_key(first) {
                return None;
            }
        } else if module.scope.children.contains_key(first) {
            return self.lookup_class_path(module_path, first, rest, depth);
        }

        let import = module.imports.get(first)?;
        if import.imported_name.is_empty() && rest.is_empty() {
            return None;
        }
        let abs = &import.module_path;
        self.ensure_loaded(abs, depth + 1);
        let imported_path: Vec<&str> = std::iter::once(import.imported_name.as_str())
            .filter(|name| !name.is_empty())
            .chain(rest.iter().copied())
            .collect();
        self.lookup_path_at(abs, None, &imported_path, depth + 1)
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
                    let name: UnqualifiedName = path.iter().map(String::as_str).collect();
                    let (value, is_enum) =
                        self.lookup_path_at(module_path, None, name.segments(), depth)?;
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
        rest: &[&str],
        depth: usize,
    ) -> Option<(LiteralValue, bool)> {
        match rest {
            [member] => self.lookup_class_member(module_path, class_name, member, depth),
            _ => {
                let modules = self.modules.borrow();
                let module = modules.get(module_path)?;
                let path: Vec<&str> = std::iter::once(class_name)
                    .chain(rest.iter().copied())
                    .collect();
                module.scope.resolve_path(&path).map(|value| (value, false))
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
        let module = Rc::clone(self.modules.borrow().get(module_path)?);
        if let Some(value) = module.scope.lookup_class_member(class_name, member) {
            return Some((value, module.scope.class_is_enum(class_name)));
        }
        let class = module.scope.children.get(class_name)?;
        if class.assignments.contains_key(member) {
            return None;
        }
        let [base] = class.bases.as_slice() else {
            return None;
        };
        let import = module.imports.get(base)?;
        if import.imported_name.is_empty() {
            return None;
        }
        self.ensure_loaded(&import.module_path, depth + 1);
        self.lookup_class_member(
            &import.module_path,
            &import.imported_name,
            member,
            depth + 1,
        )
    }
}

struct PendingAssignment<'a> {
    module_path: &'a [String],
    scope: &'a ConstantScope,
    name: &'a str,
    elements: &'a [PendingElement],
}

fn collect_imports(module: &ModModule, context: &[String]) -> HashMap<String, ImportBinding> {
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

                let Some(module_path) = resolve_relative_import(context, level, &module_path)
                else {
                    continue;
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
                            },
                        );
                    } else if let Some(first) = parts.first() {
                        imports.insert(
                            first.clone(),
                            ImportBinding {
                                module_path: vec![first.clone()],
                                imported_name: String::new(),
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
        Expr::List(ruff_python_ast::ExprList { elts, .. })
        | Expr::Tuple(ruff_python_ast::ExprTuple { elts, .. }) => {
            let values: Option<Vec<_>> = elts.iter().map(try_extract_literal).collect();
            Some(LiteralValue::Sequence(values?))
        }
        _ => None,
    }
}
