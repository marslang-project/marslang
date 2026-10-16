use std::collections::{HashMap, HashSet};
use crate::ast::*;

#[derive(Clone)]
struct Binding { name: String, fixed: bool, ty: Option<String>, hot: Option<Expr> }
struct Resolver {
    scopes: Vec<HashMap<String, Binding>>,
    next: usize,
    /// The line of the statement being resolved, which an anonymous function
    /// written in it takes as its own, for runtime errors.
    line: usize,
    /// Families a match pattern may name: the program's own and the built-in
    /// error families, and `alias.Name` for what its imported packages export.
    families: HashSet<String>,
    imported: HashSet<String>,
}

/// `imported` holds `alias.Name` for each family (or value, which may hold a
/// family) that the program's imported packages export.
pub fn resolve(program: &mut Program, imported: &HashSet<String>) -> Result<(), String> {
    let mut families: HashSet<String> = crate::value::ErrorKind::ALL.iter().map(|kind| kind.name().to_string()).collect();
    families.extend(program.items.iter().filter_map(|item| match item { Item::Family(f) => Some(f.name.clone()), _ => None }));
    let mut resolver = Resolver { scopes: vec![HashMap::new()], next: 0, line: 0, families, imported: imported.clone() };
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
                let mut stmt = Stmt { kind: StmtKind::Var(v.clone()), line: v.line }; resolver.stmt(&mut stmt)?;
                *item = match stmt.kind { StmtKind::Var(v) => Item::Var(v), kind => Item::Stmt(Stmt { kind, line: stmt.line }) };
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
/// decimal integers that fit in 32 bits are `int`, in 64 bits `longint`, larger ones
/// `bigint`, and literals with a fraction or exponent are `float`. A literal beyond even
/// a bigint's size limit stays as text and raises `RangeError` when evaluated.
fn number(text: &str, negative: bool) -> Option<Expr> {
    if text.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(magnitude) = text.parse::<i128>() {
            let value = if negative { -magnitude } else { magnitude };
            if i32::try_from(value).is_ok() { return Some(Expr::Int(value as i64)); }
            if let Ok(long) = i64::try_from(value) { return Some(Expr::Long(long)); }
        }
        return crate::bigint::from_decimal(text, negative).map(Expr::Big);
    }
    let value: f64 = text.parse().ok()?;
    Some(Expr::Float(if negative { -value } else { value }))
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
        // An anonymous function in a body that is one expression is on the
        // function's own line; afterwards the enclosing statement's line holds.
        let outer = std::mem::replace(&mut self.line, f.line);
        let line = f.line;
        let result = self.function_body(f).map_err(|message| crate::parser::at_line(line, message));
        self.line = outer;
        result
    }
    fn function_body(&mut self, f: &mut FuncDecl) -> Result<(), String> {
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
    /// Errors say which line they are on, as syntax errors do.
    fn stmt(&mut self, stmt: &mut Stmt) -> Result<(), String> {
        let line = stmt.line;
        self.statement(stmt).map_err(|message| crate::parser::at_line(line, message))
    }
    fn statement(&mut self, stmt: &mut Stmt) -> Result<(), String> {
        let line = stmt.line;
        self.line = line;
        match &mut stmt.kind {
            StmtKind::Func { name, decl } => {
                // Bound before its body is resolved, so it can call itself.
                *name = self.bind(name, true, None, None)?.name;
                self.function(std::sync::Arc::make_mut(decl))?;
            }
            StmtKind::Var(v) => {
                if let Some(ty) = &v.ty {
                    if ty.split(|c: char| !c.is_alphanumeric()).any(|word| word == "Function" || word == "Family") {
                        return Err(format!("'{}': Function and Family are parameter types, as in func apply(Function f)", v.name));
                    }
                }
                let explicit = v.is_fixed || v.temp != TempKind::Default || v.ty.is_some();
                if !explicit && self.find(&v.name).is_some() {
                    let mut replacement = Stmt { kind: StmtKind::Assign { target: Expr::Ident(v.name.clone()), value: v.value.clone() }, line };
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
            StmtKind::Assign { target, value } => {
                self.expr(value)?;
                match target {
                    Expr::Ident(name) => {
                        let binding = self.find(name).ok_or_else(|| format!("unknown assignment target '{name}'"))?;
                        if binding.fixed { return Err(format!("cannot reassign fixed/hot binding '{name}'")); }
                        *name = binding.name; *value = typed(value.clone(), &binding.ty);
                    }
                    Expr::Member { object, .. } => self.expr(object)?,
                    Expr::Index { .. } => return Err(
                        "cannot assign through an index: use .modify(index, value) on an array; strings cannot be changed".into()),
                    _ => return Err("invalid assignment target".into()),
                }
            }
            StmtKind::Ret(e) | StmtKind::Expr(e) => self.expr(e)?,
            StmtKind::If { cond, then_block, elif_blocks, else_block } => {
                self.expr(cond)?; self.block(then_block)?;
                for (e, body) in elif_blocks { self.expr(e)?; self.block(body)?; }
                if let Some(body) = else_block { self.block(body)?; }
            }
            StmtKind::Repeat { times, body } => { self.expr(times)?; self.block(body)?; }
            StmtKind::While { cond, body } => { self.expr(cond)?; self.block(body)?; }
            StmtKind::ForEach { name, iterable, body } => {
                self.expr(iterable)?; self.scopes.push(HashMap::new());
                *name = self.bind(name, false, None, None)?.name;
                for s in body { self.stmt(s)?; } self.scopes.pop();
            }
            StmtKind::For { init, cond, step, body } => {
                self.scopes.push(HashMap::new());
                for s in init { self.stmt(s)?; }
                self.expr(cond)?;
                for s in step.iter_mut() {
                    self.stmt(s)?;
                    if matches!(s.kind, StmtKind::Var(_)) { return Err("for change cannot declare a new binding".into()); }
                }
                self.block(body)?; self.scopes.pop();
            }
            StmtKind::Run { body, handlers, then_block } => {
                self.block(body)?;
                for handler in handlers {
                    self.scopes.push(HashMap::new());
                    if let Some(name) = &mut handler.name { *name = self.bind(name, false, None, None)?.name; }
                    for s in &mut handler.body { self.stmt(s)?; }
                    self.scopes.pop();
                }
                if let Some(body) = then_block { self.block(body)?; }
            }
            StmtKind::Match { subject, arms } => {
                self.expr(subject)?;
                for arm in arms.iter_mut() {
                    self.scopes.push(HashMap::new());
                    let line = arm.line;
                    self.arm_head(&mut arm.pattern, &mut arm.guard, line)?;
                    for s in &mut arm.body { self.stmt(s)?; }
                    self.scopes.pop();
                }
                repeated_literals(arms.iter().map(|arm| (&arm.pattern, arm.guard.is_some(), arm.line)))?;
            }
            _ => {}
        }
        Ok(())
    }

    /// An arm's pattern and guard, in the arm's own scope: literals become
    /// values, type names are checked, and a type pattern's name is bound.
    fn arm_head(&mut self, pattern: &mut Pattern, guard: &mut Option<Expr>, line: usize) -> Result<(), String> {
        let at = |message: String| if line > 0 { crate::parser::at_line(line, message) } else { message };
        match pattern {
            Pattern::Literals(literals) => for literal in literals.iter_mut() {
                self.expr(literal).map_err(at)?;
                fold_negation(literal);
            },
            Pattern::Type { types, name } => {
                for ty in types.iter() { self.pattern_type(ty).map_err(at)?; }
                if let Some(name) = name { *name = self.bind(name, false, None, None).map_err(at)?.name; }
            }
            Pattern::Wildcard => {}
        }
        if let Some(guard) = guard { self.expr(guard).map_err(at)?; }
        Ok(())
    }

    /// Whether a type pattern names a type: a built-in kind or a family that is
    /// declared here, built in, or exported by an imported package.
    fn pattern_type(&self, ty: &str) -> Result<(), String> {
        if PATTERN_TYPES.contains(&ty) || self.families.contains(ty) || self.imported.contains(ty) { return Ok(()); }
        if matches!(ty, "Function" | "Family") {
            return Err(format!("{ty} is a parameter type, not a pattern; match a function or family with any"));
        }
        if let Some((alias, name)) = ty.split_once('.') {
            return Err(format!("unknown type '{ty}' in a match pattern: {alias} is not an imported package that exports a family {name}"));
        }
        Err(format!("unknown type '{ty}' in a match pattern; a type pattern is a built-in type such as int or string, \
                     or a family. To compare with a variable, use a guard: _ if (value == {ty})"))
    }
    fn expr(&mut self, e: &mut Expr) -> Result<(), String> {
        match e {
            Expr::Number(text) => { if let Some(lowered) = number(text, false) { *e = lowered; } }
            Expr::Unary { op, value } if op == "-" && matches!(value.as_ref(), Expr::Number(t) if t.chars().all(|c| c.is_ascii_digit())) => {
                // A negated large literal is one literal, so -2^31 stays an int and
                // -2^63 a representable longint.
                let Expr::Number(text) = value.as_ref() else { unreachable!() };
                if text.parse::<i128>().map(|v| v > i32::MAX as i128).unwrap_or(true) {
                    if let Some(lowered) = number(text, true) { *e = lowered; return Ok(()); }
                }
                self.expr(value)?;
            }
            Expr::Ident(name) => {
                if let Some(binding) = self.find(name) {
                    *e = binding.hot.unwrap_or(Expr::Ident(binding.name));
                } else if !matches!(name.as_str(), "me" | "out" | "slout" | "in" | "inln" | "arr" | "set" | "pair" | "map" | "dict" | "a" | "s" | "p" | "int" | "longint" | "float" | "bigint" | "string"
                    | "err" | "lasterr" | "ord" | "chr" | "date" | "datetime" | "duration" | "Error" | "TypeError" | "RangeError" | "OutOfBoundsError" | "SyntaxError") {
                    return Err(format!("unknown name '{name}'"));
                }
            }
            Expr::Unary { value, .. } | Expr::Freeze(value) | Expr::Typed { value, .. } => self.expr(value)?,
            Expr::Binary { left, right, .. } => {
                self.expr(left)?; self.expr(right)?;
            }
            Expr::Call { callee, args } => { self.expr(callee)?; for arg in args { self.expr(arg)?; } }
            Expr::Template(parts) => for part in parts { self.expr(part)?; },
            Expr::Member { object, .. } => self.expr(object)?,
            Expr::Index { object, index } => { self.expr(object)?; self.expr(index)?; }
            // Resolved where it stands, so it sees the variables around it.
            Expr::Lambda(decl) => {
                let decl = std::sync::Arc::make_mut(decl);
                if decl.line == 0 { (decl.line, decl.end_line) = (self.line, self.line); }
                self.function(decl)?;
            }
            Expr::Match { subject, arms } => {
                self.expr(subject)?;
                for arm in arms.iter_mut() {
                    self.scopes.push(HashMap::new());
                    let line = arm.line;
                    self.arm_head(&mut arm.pattern, &mut arm.guard, line)?;
                    let at = |message: String| if line > 0 { crate::parser::at_line(line, message) } else { message };
                    self.expr(&mut arm.body).map_err(at)?;
                    self.scopes.pop();
                }
                repeated_literals(arms.iter().map(|arm| (&arm.pattern, arm.guard.is_some(), arm.line)))?;
            }
            _ => {}
        }
        Ok(())
    }
}
/// The types a match pattern may name besides families.
const PATTERN_TYPES: [&str; 15] = ["int", "longint", "bigint", "float", "string", "bool", "array", "set", "map", "dict",
    "pair", "date", "datetime", "duration", "any"];

/// `-5` in a pattern is the literal -5, so it compares without arithmetic.
fn fold_negation(literal: &mut Expr) {
    let Expr::Unary { op, value } = literal else { return };
    if op != "-" { return; }
    let folded = match value.as_ref() {
        Expr::Int(i) => Expr::Int(-i),
        Expr::Long(l) => Expr::Long(-l),
        Expr::Float(f) => Expr::Float(-f),
        Expr::Big(b) => Expr::Big(-b.clone()),
        _ => return,
    };
    *literal = folded;
}

/// A literal's value as text that equal values share (`1`, `1.0`, and a
/// bigint 1 are one value, as `==` says), and as the program wrote it.
fn literal_key(literal: &Expr) -> Option<(String, String)> {
    Some(match literal {
        Expr::Int(i) | Expr::Long(i) => (i.to_string(), i.to_string()),
        Expr::Big(b) => (b.to_string(), b.to_string()),
        Expr::Float(f) if *f == 0.0 => ("0".into(), crate::value::format_float(*f)),
        Expr::Float(f) if f.is_finite() && f.fract() == 0.0 => (format!("{f:.0}"), crate::value::format_float(*f)),
        Expr::Float(f) => (format!("float {f:?}"), crate::value::format_float(*f)),
        Expr::String(s) => (format!("string {s}"), format!("{s:?}")),
        Expr::Bool(b) => (format!("bool {b}"), b.to_string()),
        Expr::Null => ("null".into(), "null".into()),
        _ => return None,
    })
}

/// A literal that an earlier arm without a guard, or the same arm, already
/// matches: its arm could never run for that value.
fn repeated_literals<'a>(arms: impl Iterator<Item = (&'a Pattern, bool, usize)>) -> Result<(), String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (pattern, guarded, line) in arms {
        let Pattern::Literals(literals) = pattern else { continue };
        let mut here = Vec::new();
        for literal in literals {
            let Some((key, written)) = literal_key(literal) else { continue };
            if let Some(first) = seen.get(&key).copied().or_else(|| here.contains(&key).then_some(line)) {
                let earlier = if first == line || first == 0 { "earlier in this match".to_string() } else { format!("on line {first}") };
                let message = format!("{written} appears twice in this match: it is already matched {earlier}, so this arm can never run for it");
                return Err(if line > 0 { crate::parser::at_line(line, message) } else { message });
            }
            here.push(key);
        }
        if !guarded { for key in here { seen.insert(key, line); } }
    }
    Ok(())
}

fn constant(e: &Expr) -> bool {
    match e {
        Expr::Number(_) | Expr::Int(_) | Expr::Long(_) | Expr::Big(_) | Expr::Float(_)
        | Expr::String(_) | Expr::Bool(_) | Expr::Null => true,
        Expr::Unary { value, .. } | Expr::Typed { value, .. } => constant(value),
        Expr::Binary { left, right, .. } => constant(left) && constant(right),
        _ => false,
    }
}
