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
                let alias = import.alias.as_ref().expect("the package loader sets import aliases");
                if resolver.scopes[0].contains_key(alias) {
                    return Err(format!("import alias '{alias}' is already declared"));
                }
                import.alias = Some(resolver.bind(alias, true, None, None)?.name);
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

fn typed(value: Expr, ty: &Option<String>) -> Expr {
    match ty {
        Some(ty) => Expr::Typed { value: Box::new(value), ty: ty.replace(char::is_whitespace, "") },
        None => value,
    }
}

/// Lower numeric literal text to its runtime kind, following the numeric contract:
/// decimal integers up to 2^53-1 are `int`, larger ones `longint`, and literals with
/// a fraction or exponent are `float`. Out-of-range longints stay as text and raise
/// `longint overflow` when evaluated.
fn number(text: &str, negative: bool) -> Option<Expr> {
    if text.chars().all(|c| c.is_ascii_digit()) {
        let magnitude: i128 = text.parse().ok()?;
        let value = if negative { -magnitude } else { magnitude };
        if magnitude <= MAX_SAFE { return Some(Expr::Int(value as i64)); }
        return i64::try_from(value).ok().map(Expr::Long);
    }
    let value: f64 = text.parse().ok()?;
    Some(Expr::Float(if negative { -value } else { value }))
}

pub(crate) const MAX_SAFE: i128 = 9_007_199_254_740_991;

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
            param.ty.retain(|c| !c.is_whitespace());
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
                    Some(typed(v.value.clone(), &v.ty))
                } else { None };
                let binding = self.bind(&v.name, v.is_fixed || v.temp == TempKind::Hot, v.ty.clone(), hot)?;
                v.name = binding.name;
                v.value = typed(v.value.clone(), &v.ty);
                if v.is_fixed { v.value = Expr::Freeze(Box::new(v.value.clone())); }
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
        match e {
            Expr::Number(text) => { if let Some(lowered) = number(text, false) { *e = lowered; } }
            Expr::Unary { op, value } if op == "-" && matches!(value.as_ref(), Expr::Number(t) if t.chars().all(|c| c.is_ascii_digit())) => {
                // A negated large literal is one longint literal, so -2^63 stays representable.
                let Expr::Number(text) = value.as_ref() else { unreachable!() };
                if text.parse::<i128>().map(|v| v > MAX_SAFE).unwrap_or(true) {
                    if let Some(lowered) = number(text, true) { *e = lowered; return Ok(()); }
                }
                self.expr(value)?;
            }
            Expr::Ident(name) => {
                if let Some(binding) = self.find(name) {
                    *e = binding.hot.unwrap_or(Expr::Ident(binding.name));
                } else if !matches!(name.as_str(), "me" | "out" | "slout" | "in" | "inln" | "arr" | "set" | "pair" | "map" | "dict" | "a" | "s" | "p" | "int" | "longint" | "float" | "string") {
                    return Err(format!("unknown name '{name}'"));
                }
            }
            Expr::Unary { value, .. } | Expr::Freeze(value) | Expr::Typed { value, .. } => self.expr(value)?,
            Expr::Binary { left, right, .. } => {
                self.expr(left)?; self.expr(right)?;
            }
            Expr::Call { callee, args } => { self.expr(callee)?; for arg in args { self.expr(arg)?; } }
            Expr::Member { object, .. } => self.expr(object)?,
            _ => {}
        }
        Ok(())
    }
}
fn constant(e: &Expr) -> bool {
    match e {
        Expr::Number(_) | Expr::Int(_) | Expr::Long(_) | Expr::Float(_)
        | Expr::String(_) | Expr::Bool(_) | Expr::Null => true,
        Expr::Unary { value, .. } | Expr::Typed { value, .. } => constant(value),
        Expr::Binary { left, right, .. } => constant(left) && constant(right),
        _ => false,
    }
}
