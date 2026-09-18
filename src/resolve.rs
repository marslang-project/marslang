use std::collections::HashMap;
use crate::ast::*;

#[derive(Clone)]
struct Binding { name: String, fixed: bool, ty: Option<String>, hot: Option<Expr> }
struct Resolver { scopes: Vec<HashMap<String, Binding>>, next: usize }

pub fn resolve(program: &mut Program) -> Result<(), String> {
    let mut resolver = Resolver { scopes: vec![HashMap::new()], next: 0 };
    for item in &program.items {
        let name = match item { Item::Func(f) => Some(&f.name), Item::Family(f) => Some(&f.name), _ => None };
        if let Some(name) = name {
            if resolver.scopes[0].contains_key(name) { return Err(format!("duplicate name '{name}'")); }
            resolver.scopes[0].insert(name.clone(), Binding { name: name.clone(), fixed: true, ty: None, hot: None });
        }
    }
    for item in &mut program.items {
        match item {
            Item::Var(v) => {
                let mut stmt = Stmt::Var(v.clone()); resolver.stmt(&mut stmt)?;
                *item = match stmt { Stmt::Var(v) => Item::Var(v), other => Item::Stmt(other) };
            }
            Item::Stmt(s) => resolver.stmt(s)?,
            Item::Import(import) => {
                if let Some(alias) = &import.alias {
                    if alias == "*" { return Err("wildcard imports are not implemented yet".into()); }
                    if resolver.scopes[0].contains_key(alias) { return Err(format!("duplicate import alias '{alias}'")); }
                    resolver.scopes[0].insert(alias.clone(), Binding { name: alias.clone(), fixed: true, ty: None, hot: None });
                }
            }
            _ => {}
        }
    }
    for item in &mut program.items {
        match item {
            Item::Func(f) => resolver.function(f)?,
            Item::Family(f) => for method in &mut f.methods { resolver.function(method)?; },
            _ => {}
        }
    }
    Ok(())
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call { callee: Box::new(Expr::Ident(format!("__mars.{name}"))), args }
}
fn typed(value: Expr, ty: &Option<String>) -> Expr {
    if let Some(ty) = ty {
        let value = if ty == "longint" { long_literal(value) } else { value };
        call("typed", vec![value, Expr::String(format!("\"{}\"", ty.replace(' ', "")))])
    } else { value }
}
fn long_literal(value: Expr) -> Expr {
    match value {
        Expr::Number(n) if n.chars().all(|c| c.is_ascii_digit()) => Expr::Raw(format!("{n}n")),
        Expr::Unary { op, value } if op == "-" => Expr::Unary { op, value: Box::new(long_literal(*value)) },
        other => other,
    }
}

impl Resolver {
    fn find(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }
    fn bind(&mut self, name: &str, fixed: bool, ty: Option<String>, hot: Option<Expr>) -> Result<Binding, String> {
        if self.scopes.last().unwrap().contains_key(name) { return Err(format!("duplicate local '{name}'")); }
        self.next += 1;
        let binding = Binding { name: format!("__v{}_{}", self.next, name), fixed, ty, hot };
        self.scopes.last_mut().unwrap().insert(name.into(), binding.clone()); Ok(binding)
    }
    fn function(&mut self, f: &mut FuncDecl) -> Result<(), String> {
        self.scopes.push(HashMap::new());
        for param in &mut f.params {
            param.name = self.bind(&param.name, false, Some(param.ty.clone()), None)?.name;
        }
        match &mut f.body {
            FuncBody::Block(body) => for s in body { self.stmt(s)?; },
            FuncBody::Expr(e) => self.expr(e)?,
        }
        self.scopes.pop(); Ok(())
    }
    fn block(&mut self, body: &mut [Stmt]) -> Result<(), String> {
        self.scopes.push(HashMap::new());
        for stmt in body { self.stmt(stmt)?; }
        self.scopes.pop(); Ok(())
    }
    fn stmt(&mut self, stmt: &mut Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Var(v) => {
                let explicit = v.is_fixed || v.temp != TempKind::Default || v.ty.is_some();
                if !explicit && self.find(&v.name).is_some() {
                    let mut replacement = Stmt::Assign { target: Expr::Ident(v.name.clone()), value: v.value.clone() };
                    self.stmt(&mut replacement)?; *stmt = replacement; return Ok(());
                }
                let inherited = match &v.value { Expr::Ident(n) => self.find(n).map(|b| b.fixed).unwrap_or(false), _ => false };
                self.expr(&mut v.value)?;
                v.is_fixed |= inherited;
                let hot = if v.temp == TempKind::Hot {
                    if !constant(&v.value) { return Err(format!("hot '{}' needs a compile-time constant expression", v.name)); }
                    Some(v.value.clone())
                } else { None };
                let binding = self.bind(&v.name, v.is_fixed || v.temp == TempKind::Hot, v.ty.clone(), hot)?;
                v.name = binding.name;
                v.value = typed(v.value.clone(), &v.ty);
                if v.is_fixed { v.value = call("freeze", vec![v.value.clone()]); }
            }
            Stmt::Assign { target, value } => {
                self.expr(value)?;
                match target {
                    Expr::Ident(name) => {
                        let binding = self.find(name).ok_or_else(|| format!("unknown assignment target '{name}'"))?;
                        if binding.fixed { return Err(format!("cannot reassign fixed/hot binding '{name}'")); }
                        *name = binding.name; *value = typed(value.clone(), &binding.ty);
                    }
                    Expr::Member { object, .. } => self.expr(object)?,
                    _ => return Err("invalid assignment target".into()),
                }
            }
            Stmt::Ret(e) | Stmt::Expr(e) => self.expr(e)?,
            Stmt::If { cond, then_block, elif_blocks, else_block } => {
                self.expr(cond)?; self.block(then_block)?;
                for (e, body) in elif_blocks { self.expr(e)?; self.block(body)?; }
                if let Some(body) = else_block { self.block(body)?; }
            }
            Stmt::Repeat { times, body } => { self.expr(times)?; self.block(body)?; }
            Stmt::While { cond, body } => { self.expr(cond)?; self.block(body)?; }
            Stmt::ForEach { name, iterable, body } => {
                self.expr(iterable)?; self.scopes.push(HashMap::new());
                *name = self.bind(name, false, None, None)?.name;
                for s in body { self.stmt(s)?; } self.scopes.pop();
            }
            Stmt::For { init, cond, step, body } => {
                self.scopes.push(HashMap::new());
                for s in init { self.stmt(s)?; }
                self.expr(cond)?;
                for s in step.iter_mut() {
                    self.stmt(s)?;
                    if matches!(s, Stmt::Var(_)) { return Err("for change cannot declare a new binding".into()); }
                }
                self.block(body)?; self.scopes.pop();
            }
            _ => {}
        }
        Ok(())
    }
    fn expr(&self, e: &mut Expr) -> Result<(), String> {
        let numeric_type = self.numeric_type(e);
        match e {
            Expr::Ident(name) => {
                if let Some(binding) = self.find(name) {
                    *e = binding.hot.unwrap_or(Expr::Ident(binding.name));
                } else if !matches!(name.as_str(), "me" | "out" | "slout" | "in" | "inln" | "arr" | "set" | "pair" | "map" | "dict" | "a" | "s" | "p" | "int" | "longint" | "float" | "string") {
                    return Err(format!("unknown name '{name}'"));
                }
            }
            Expr::Unary { value, op } => {
                self.expr(value)?;
                if op != "not" && numeric_type.is_some() { *e = typed(e.clone(), &numeric_type); }
            }
            Expr::Binary { left, right, op } => {
                self.expr(left)?; self.expr(right)?;
                if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%" | "**") && numeric_type.is_some() {
                    if numeric_type.as_deref() == Some("float") {
                        *e = call("floatbin", vec![Expr::String(format!("\"{op}\"")), *left.clone(), *right.clone()]);
                    } else { *e = typed(e.clone(), &numeric_type); }
                }
            }
            Expr::Call { callee, args } => { self.expr(callee)?; for arg in args { self.expr(arg)?; } }
            Expr::Member { object, .. } => self.expr(object)?,
            _ => {}
        }
        Ok(())
    }
    fn numeric_type(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Number(n) if n.contains('.') || n.contains('e') || n.contains('E') => Some("float".into()),
            Expr::Ident(n) => self.find(n).and_then(|b| b.ty).filter(|t| t == "int" || t == "longint" || t == "float"),
            Expr::Unary { value, .. } => self.numeric_type(value),
            Expr::Binary { left, right, .. } => {
                let l = self.numeric_type(left); let r = self.numeric_type(right);
                if l.as_deref() == Some("float") || r.as_deref() == Some("float") { Some("float".into()) }
                else if l.as_deref() == Some("longint") || r.as_deref() == Some("longint") { Some("longint".into()) }
                else { l.or(r) }
            }
            _ => None,
        }
    }
}
fn constant(e: &Expr) -> bool {
    match e {
        Expr::Number(_) | Expr::String(_) | Expr::Bool(_) | Expr::Null => true,
        Expr::Unary { value, .. } => constant(value),
        Expr::Binary { left, right, .. } => constant(left) && constant(right),
        Expr::Call { callee, args } if matches!(callee.as_ref(), Expr::Ident(n) if n == "__mars.typed" || n == "__mars.floatbin") => args.iter().all(constant),
        _ => false,
    }
}
