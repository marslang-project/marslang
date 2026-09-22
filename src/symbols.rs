//! `marslang symbols`: what a program declares, as JSON, for editors.
//!
//! Every function, family, and method with its parameters, source lines, and
//! docstring, and every variable with the scope it belongs to and its type: the
//! annotation when it has one, otherwise what its first value shows (`x = 1` is
//! an `int`, `c = Circle(2.0)` a `Circle`), otherwise `null`. Marslang checks
//! types while running, so an inferred type describes the first value only.
//!
//! The walk runs after parsing and decorators, before names are resolved, so
//! names are exactly as written. Imported packages are described too, by the
//! alias the program uses: their public functions, families, and values, so an
//! editor can explain `math.sqrt` as well as the program's own names.

use crate::package::{Export, LoadedPackage, Source};

use std::collections::HashSet;
use std::fmt::Write;

use crate::ast::*;

/// `imports` are the program's imports as (alias, package) pairs.
pub(crate) fn describe(program: &Program, imports: &[(String, &LoadedPackage)]) -> String {
    let families: HashSet<&str> = program.items.iter().filter_map(|item| match item {
        Item::Family(family) => Some(family.name.as_str()),
        _ => None,
    }).collect();
    // `alias.Family` for every family an imported package exports, so that
    // `s = containers.stack();` is known to hold a containers.stack.
    let package_families: HashSet<String> = imports.iter().flat_map(|(alias, package)| {
        package.exports.iter().filter_map(move |(name, export)| match export {
            Export::Family(_) => Some(format!("{alias}.{name}")),
            _ => None,
        })
    }).collect();
    let mut walker = Walker { families, package_families, functions: Vec::new(), family_json: Vec::new(), variables: Vec::new() };

    let mut globals = HashSet::new();
    for item in &program.items {
        match item {
            Item::Var(var) => {
                globals.insert(var.name.clone());
                walker.declared(var, None);
            }
            Item::Stmt(Stmt::Assign { target: Expr::Ident(name), value }) if globals.insert(name.clone()) => {
                let ty = walker.infer(value);
                walker.variable(name, "variable", ty, true, None);
            }
            _ => {}
        }
    }
    for item in &program.items {
        match item {
            Item::Func(function) => {
                let json = walker.function(function, None, &globals);
                walker.functions.push(json);
            }
            Item::Family(family) => {
                let methods: Vec<String> = family.methods.iter()
                    .map(|method| walker.function(method, Some(&family.name), &globals))
                    .collect();
                walker.family_json.push(format!(
                    "{{\"name\":{},\"parent\":{},\"line\":{},\"end\":{},\"doc\":{},\"methods\":[{}]}}",
                    string(&family.name), optional(family.extends.as_deref()), family.line, family.end_line,
                    optional(family.doc.as_deref()), methods.join(",")));
            }
            _ => {}
        }
    }
    let packages: Vec<String> = imports.iter().filter_map(|(alias, package)| describe_package(alias, package)).collect();
    format!("{{\"functions\":[{}],\"families\":[{}],\"variables\":[{}],\"packages\":[{}]}}",
        walker.functions.join(","), walker.family_json.join(","), walker.variables.join(","), packages.join(","))
}

/// What a package exports, as JSON. Native packages have no source to describe.
fn describe_package(alias: &str, package: &LoadedPackage) -> Option<String> {
    let Source::Program(program) = &package.source else { return None };
    let families: HashSet<&str> = program.items.iter().filter_map(|item| match item {
        Item::Family(family) => Some(family.name.as_str()),
        _ => None,
    }).collect();
    let walker = Walker { families, package_families: HashSet::new(), functions: Vec::new(), family_json: Vec::new(), variables: Vec::new() };
    let (mut functions, mut family_json, mut values) = (Vec::new(), Vec::new(), Vec::new());
    for (name, export) in &package.exports {
        match export {
            Export::Function(resolved) => {
                let found = program.items.iter().find_map(|item| match item {
                    Item::Func(function) if &function.name == resolved => Some(function),
                    _ => None,
                });
                if let Some(function) = found { functions.push(exported_function(function, None)); }
            }
            Export::Family(resolved) => {
                let found = program.items.iter().find_map(|item| match item {
                    Item::Family(family) if &family.name == resolved => Some(family),
                    _ => None,
                });
                if let Some(family) = found {
                    let methods: Vec<String> = family.methods.iter()
                        .filter(|method| method.access == Access::Public && !method.name.starts_with('_'))
                        .map(|method| exported_function(method, Some(name)))
                        .collect();
                    family_json.push(format!(
                        "{{\"name\":{},\"parent\":{},\"line\":{},\"end\":{},\"doc\":{},\"methods\":[{}]}}",
                        string(name), optional(family.extends.as_deref()), family.line, family.end_line,
                        optional(family.doc.as_deref()), methods.join(",")));
                }
            }
            Export::Binding(resolved) => {
                let found = program.items.iter().find_map(|item| match item {
                    Item::Var(var) if &var.name == resolved => Some(var),
                    _ => None,
                });
                if let Some(var) = found {
                    let (ty, inferred) = match &var.ty {
                        Some(ty) => (Some(ty.clone()), false),
                        None => (walker.infer(&var.value), true),
                    };
                    let inferred = inferred && ty.is_some();
                    values.push(format!("{{\"name\":{},\"kind\":{},\"type\":{},\"inferred\":{inferred}}}",
                        string(name), string(var_kind(var)), optional(ty.as_deref())));
                }
            }
        }
    }
    Some(format!("{{\"alias\":{},\"name\":{},\"functions\":[{}],\"families\":[{}],\"values\":[{}]}}",
        string(alias), string(&package.name), functions.join(","), family_json.join(","), values.join(",")))
}

/// A package's function or method as JSON, with parameter names as written:
/// resolving renames them (`__v12_x`), and the original is what readers know.
fn exported_function(function: &FuncDecl, family: Option<&str>) -> String {
    let params: Vec<String> = function.params.iter().map(|param| {
        let ty = (!param.ty.is_empty()).then_some(param.ty.as_str());
        format!("{{\"name\":{},\"type\":{}}}", string(written(&param.name)), optional(ty))
    }).collect();
    format!(
        "{{\"name\":{},\"family\":{},\"line\":{},\"end\":{},\"params\":[{}],\"doc\":{},\"access\":\"public\"}}",
        string(&function.name), optional(family), function.line, function.end_line,
        params.join(","), optional(function.doc.as_deref()))
}

/// A resolved binding name (`__v12_x`) as it was written (`x`).
fn written(name: &str) -> &str {
    name.strip_prefix("__v")
        .and_then(|rest| rest.split_once('_'))
        .filter(|(digits, _)| digits.chars().all(|c| c.is_ascii_digit()))
        .map_or(name, |(_, original)| original)
}

struct Walker<'a> {
    families: HashSet<&'a str>,
    package_families: HashSet<String>,
    functions: Vec<String>,
    family_json: Vec<String>,
    variables: Vec<String>,
}

impl Walker<'_> {
    /// A function or method as JSON; its parameters and locals are recorded as
    /// variables scoped to it (`name`, or `Family.name` for a method).
    fn function(&mut self, function: &FuncDecl, family: Option<&str>, globals: &HashSet<String>) -> String {
        let scope = match family {
            Some(family) => format!("{family}.{}", function.name),
            None => function.name.clone(),
        };
        let mut known: HashSet<String> = globals.clone();
        let params: Vec<String> = function.params.iter().map(|param| {
            let ty = (!param.ty.is_empty()).then(|| param.ty.clone());
            known.insert(param.name.clone());
            self.variable(&param.name, "parameter", ty.clone(), false, Some(&scope));
            format!("{{\"name\":{},\"type\":{}}}", string(&param.name), optional(ty.as_deref()))
        }).collect();
        if let FuncBody::Block(body) = &function.body { self.block(body, &scope, &mut known); }
        let access = match function.access {
            Access::Public => "public",
            Access::Private => "private",
            Access::Subclass => "subclass",
        };
        format!(
            "{{\"name\":{},\"family\":{},\"line\":{},\"end\":{},\"params\":[{}],\"doc\":{},\"access\":{}}}",
            string(&function.name), optional(family), function.line, function.end_line,
            params.join(","), optional(function.doc.as_deref()), string(access))
    }

    fn block(&mut self, body: &[Stmt], scope: &str, known: &mut HashSet<String>) {
        for stmt in body { self.stmt(stmt, scope, known); }
    }

    fn stmt(&mut self, stmt: &Stmt, scope: &str, known: &mut HashSet<String>) {
        match stmt {
            Stmt::Var(var) if known.insert(var.name.clone()) => self.declared(var, Some(scope)),
            // A bare assignment declares a local unless the name already exists.
            Stmt::Assign { target: Expr::Ident(name), value } if known.insert(name.clone()) => {
                let ty = self.infer(value);
                self.variable(name, "variable", ty, true, Some(scope));
            }
            Stmt::If { then_block, elif_blocks, else_block, .. } => {
                self.block(then_block, scope, known);
                for (_, block) in elif_blocks { self.block(block, scope, known); }
                if let Some(block) = else_block { self.block(block, scope, known); }
            }
            Stmt::Repeat { body, .. } | Stmt::While { body, .. } => self.block(body, scope, known),
            Stmt::ForEach { name, body, .. } => {
                if known.insert(name.clone()) { self.variable(name, "loop", None, false, Some(scope)); }
                self.block(body, scope, known);
            }
            Stmt::For { init, step, body, .. } => {
                self.block(init, scope, known);
                self.block(step, scope, known);
                self.block(body, scope, known);
            }
            Stmt::Run { body, handlers, then_block } => {
                self.block(body, scope, known);
                for handler in handlers {
                    if let Some(name) = &handler.name {
                        if known.insert(name.clone()) {
                            self.variable(name, "error", Some(handler.types.join(" | ")), false, Some(scope));
                        }
                    }
                    self.block(&handler.body, scope, known);
                }
                if let Some(block) = then_block { self.block(block, scope, known); }
            }
            _ => {}
        }
    }

    /// A variable declared with `fixed`, `hot`, `cold`, or an annotation.
    fn declared(&mut self, var: &VarDecl, scope: Option<&str>) {
        match &var.ty {
            Some(ty) => self.variable(&var.name, var_kind(var), Some(ty.clone()), false, scope),
            None => {
                let ty = self.infer(&var.value);
                self.variable(&var.name, var_kind(var), ty, true, scope);
            }
        }
    }

    /// `inferred` says the type was read off the first value, not written.
    fn variable(&mut self, name: &str, kind: &str, ty: Option<String>, inferred: bool, scope: Option<&str>) {
        let inferred = inferred && ty.is_some();
        self.variables.push(format!("{{\"name\":{},\"kind\":{},\"type\":{},\"inferred\":{inferred},\"scope\":{}}}",
            string(name), string(kind), optional(ty.as_deref()), optional(scope)));
    }

    /// The type a value's expression shows, when it shows one.
    fn infer(&self, value: &Expr) -> Option<String> {
        Some(match value {
            Expr::Number(text) if text.chars().all(|c| c.is_ascii_digit()) => {
                match text.parse::<i64>() {
                    Ok(n) if n <= i32::MAX as i64 => "int",
                    Ok(_) => "longint",
                    Err(_) => return None,
                }.to_string()
            }
            Expr::Number(_) | Expr::Float(_) => "float".into(),
            Expr::Int(_) => "int".into(),
            Expr::Long(_) => "longint".into(),
            Expr::String(_) => "string".into(),
            Expr::Bool(_) => "bool".into(),
            Expr::Typed { ty, .. } => ty.clone(),
            Expr::Freeze(value) => return self.infer(value),
            Expr::Unary { op, .. } if op == "!" || op == "not" => "bool".into(),
            Expr::Unary { value, .. } => return self.infer(value),
            Expr::Binary { op, .. } if matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||") => "bool".into(),
            Expr::Binary { op, left, right } if op == "+" => {
                let (left, right) = (self.infer(left), self.infer(right));
                if left.as_deref() == Some("string") || right.as_deref() == Some("string") { return Some("string".into()); }
                return if left == right { left } else { None };
            }
            Expr::Binary { left, right, .. } => {
                let (left, right) = (self.infer(left), self.infer(right));
                return if left == right { left } else { None };
            }
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(name) => match name.as_str() {
                    "arr" | "a" => "array".into(),
                    "set" | "s" => "set".into(),
                    "pair" | "p" => "pair".into(),
                    "map" | "dict" => "map".into(),
                    "int" | "longint" | "float" | "string" => name.clone(),
                    "in" | "inln" => "string".into(),
                    family if self.families.contains(family) => family.to_string(),
                    _ => return None,
                },
                // containers.stack(): a family from an imported package.
                Expr::Member { object, field } => match object.as_ref() {
                    Expr::Ident(alias) if self.package_families.contains(&format!("{alias}.{field}")) => format!("{alias}.{field}"),
                    _ => return None,
                },
                _ => return None,
            },
            _ => return None,
        })
    }
}

fn var_kind(var: &VarDecl) -> &'static str {
    if var.is_fixed { return "fixed"; }
    match var.temp {
        TempKind::Hot => "hot",
        TempKind::Cold => "cold",
        TempKind::Default => "variable",
    }
}

fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => { let _ = write!(out, "\\u{:04x}", c as u32); }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn optional(text: Option<&str>) -> String {
    text.map(string).unwrap_or_else(|| "null".into())
}
