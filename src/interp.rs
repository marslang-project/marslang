//! Tree-walking interpreter: executes resolved Marslang programs directly.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;

use indexmap::IndexMap;
use unicode_segmentation::UnicodeSegmentation;

use crate::ast::*;
use crate::package::{Export, LoadedPackage, Source};
use crate::value::*;

/// Maximum nesting of Marslang calls before raising `RangeError`.
pub const MAX_CALL_DEPTH: usize = 10_000;

enum Flow { Next, Break, Continue, Return(Value) }

/// Globals and declarations of one source unit (the program or a bundled package).
struct Unit {
    /// Package name, or `None` for the program itself.
    name: Option<String>,
    globals: RefCell<HashMap<String, Value>>,
    functions: HashMap<String, Rc<Function>>,
    families: HashMap<String, Rc<Family>>,
}

struct Env {
    unit: usize,
    /// `None` while running top-level statements, which declare globals.
    locals: Option<HashMap<String, Value>>,
    me: Option<Value>,
}

pub enum InputSource { Stdin, Text(String) }

struct Input { source: InputSource, lines: Option<Vec<String>>, line: usize }

impl Input {
    fn read_all(&mut self) -> String {
        match &mut self.source {
            InputSource::Stdin => {
                let mut text = String::new();
                let _ = std::io::stdin().read_to_string(&mut text);
                text
            }
            InputSource::Text(text) => std::mem::take(text),
        }
    }
}

pub struct Interp<'o> {
    units: Vec<Unit>,
    out: &'o mut dyn Write,
    input: Input,
    depth: usize,
    /// Loaded packages by key.
    packages: HashMap<String, Value>,
}

type Exec = RResult<Flow>;

impl<'o> Interp<'o> {
    pub fn new(out: &'o mut dyn Write, input: InputSource) -> Self {
        Interp { units: Vec::new(), out, input: Input { source: input, lines: None, line: 0 },
            depth: 0, packages: HashMap::new() }
    }

    /// Run a resolved program: load its packages (each after its own imports),
    /// execute top-level statements in order, then call `m` when it is declared.
    pub(crate) fn run(&mut self, program: &Program, packages: &[LoadedPackage]) -> RResult<()> {
        for package in packages {
            let value = match &package.source {
                Source::Native(package) => package(),
                Source::Program(source) => self.load_package(package, source)?,
            };
            self.packages.insert(package.key.clone(), value);
        }
        let unit = self.load_unit(program, None)?;
        self.run_top_level(program, unit)?;
        let entry = self.units[unit].functions.get("m").cloned();
        if let Some(entry) = entry { self.call_function(&entry, None, Vec::new())?; }
        Ok(())
    }

    fn load_package(&mut self, package: &LoadedPackage, program: &Program) -> RResult<Value> {
        let unit = self.load_unit(program, Some(package.name.clone()))?;
        self.run_top_level(program, unit)?;
        let mut members = IndexMap::new();
        for (name, export) in &package.exports {
            let value = match export {
                Export::Binding(binding) => self.units[unit].globals.borrow().get(binding).cloned().unwrap_or(Value::Null),
                Export::Function(f) => Value::Func(Rc::new(Callable::Func(self.units[unit].functions[f].clone()))),
                Export::Family(f) => Value::Func(Rc::new(Callable::Family(self.units[unit].families[f].clone()))),
            };
            members.insert(name.clone(), value);
        }
        Ok(Value::Package(Rc::new(Package { name: package.name.clone(), members })))
    }

    fn load_unit(&mut self, program: &Program, name: Option<String>) -> RResult<usize> {
        let index = self.units.len();
        let mut functions = HashMap::new();
        let mut decls = HashMap::new();
        for item in &program.items {
            match item {
                Item::Func(f) => { functions.insert(f.name.clone(), Rc::new(Function { decl: f.clone(), package: index })); }
                Item::Family(f) => { decls.insert(f.name.clone(), f); }
                _ => {}
            }
        }
        let mut families = HashMap::new();
        for name in decls.keys() { build_family(name, &decls, &mut families, index, &mut Vec::new())?; }
        self.units.push(Unit { name, globals: RefCell::new(HashMap::new()), functions, families });
        Ok(index)
    }

    fn run_top_level(&mut self, program: &Program, unit: usize) -> RResult<()> {
        let mut env = Env { unit, locals: None, me: None };
        for item in &program.items {
            match item {
                Item::Import(import) => {
                    let package = import.key.as_ref().and_then(|key| self.packages.get(key)).cloned()
                        .ok_or_else(|| RuntimeError { kind: ErrorKind::Error, message: format!("package '{}' is not loaded", import.module) })?;
                    self.units[unit].globals.borrow_mut().insert(import.alias.clone().unwrap_or_default(), package);
                }
                Item::Var(v) => { let value = self.eval(&v.value, &mut env)?; self.declare(&mut env, &v.name, value); }
                Item::Stmt(stmt) => match self.exec(stmt, &mut env)? {
                    Flow::Next => {}
                    _ => return err(ErrorKind::SyntaxError, "ret/break/continue outside of a function"),
                },
                Item::Func(_) | Item::Family(_) => {}
            }
        }
        Ok(())
    }

    // ----- variables -----

    fn lookup(&self, name: &str, env: &Env) -> RResult<Value> {
        if let Some(value) = env.locals.as_ref().and_then(|locals| locals.get(name)) { return Ok(value.clone()); }
        if name == "me" {
            return env.me.clone().ok_or_else(|| RuntimeError {
                kind: ErrorKind::Error, message: "'me' is only available inside family methods".into() });
        }
        let unit = &self.units[env.unit];
        if let Some(value) = unit.globals.borrow().get(name) { return Ok(value.clone()); }
        if let Some(function) = unit.functions.get(name) { return Ok(Value::Func(Rc::new(Callable::Func(function.clone())))); }
        if let Some(family) = unit.families.get(name) { return Ok(Value::Func(Rc::new(Callable::Family(family.clone())))); }
        if let Some(builtin) = Builtin::lookup(name) { return Ok(Value::Func(Rc::new(Callable::Builtin(builtin)))); }
        err(ErrorKind::Error, format!("'{}' is used before it is initialized", display_name(name)))
    }

    fn declare(&self, env: &mut Env, name: &str, value: Value) {
        match &mut env.locals {
            Some(locals) => { locals.insert(name.to_string(), value); }
            None => { self.units[env.unit].globals.borrow_mut().insert(name.to_string(), value); }
        }
    }

    fn assign(&self, env: &mut Env, name: &str, value: Value) {
        if let Some(slot) = env.locals.as_mut().and_then(|locals| locals.get_mut(name)) { *slot = value; return; }
        let mut globals = self.units[env.unit].globals.borrow_mut();
        if let Some(slot) = globals.get_mut(name) { *slot = value; return; }
        drop(globals);
        self.declare(env, name, value);
    }

    // ----- statements -----

    fn exec_block(&mut self, body: &[Stmt], env: &mut Env) -> Exec {
        for stmt in body {
            match self.exec(stmt, env)? {
                Flow::Next => {}
                flow => return Ok(flow),
            }
        }
        Ok(Flow::Next)
    }

    fn exec(&mut self, stmt: &Stmt, env: &mut Env) -> Exec {
        match stmt {
            Stmt::Var(v) => {
                let value = self.eval(&v.value, env)?;
                self.declare(env, &v.name, value);
            }
            Stmt::Assign { target, value } => match target {
                Expr::Ident(name) => {
                    let value = self.eval(value, env)?;
                    self.assign(env, name, value);
                }
                Expr::Member { object, field } => {
                    let object = self.eval(object, env)?;
                    let value = self.eval(value, env)?;
                    self.set_member(&object, field, value)?;
                }
                _ => return type_err("invalid assignment target"),
            },
            Stmt::Ret(e) => return Ok(Flow::Return(self.eval(e, env)?)),
            Stmt::Expr(e) => { self.eval(e, env)?; }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
            Stmt::If { cond, then_block, elif_blocks, else_block } => {
                if self.eval(cond, env)?.truth() { return self.exec_block(then_block, env); }
                for (cond, body) in elif_blocks {
                    if self.eval(cond, env)?.truth() { return self.exec_block(body, env); }
                }
                if let Some(body) = else_block { return self.exec_block(body, env); }
            }
            Stmt::Repeat { times, body } => {
                let count = repeat_count(&self.eval(times, env)?)?;
                for _ in 0..count {
                    match self.exec_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
            }
            Stmt::While { cond, body } => {
                while self.eval(cond, env)?.truth() {
                    match self.exec_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
            }
            Stmt::ForEach { name, iterable, body } => {
                let items = iterate(&self.eval(iterable, env)?)?;
                for item in items {
                    self.declare(env, name, item);
                    match self.exec_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
            }
            Stmt::For { init, cond, step, body } => {
                for s in init { self.exec(s, env)?; }
                while self.eval(cond, env)?.truth() {
                    match self.exec_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                    for s in step { self.exec(s, env)?; }
                }
            }
        }
        Ok(Flow::Next)
    }

    // ----- expressions -----

    fn eval(&mut self, e: &Expr, env: &mut Env) -> RResult<Value> {
        Ok(match e {
            Expr::Int(i) => Value::Int(*i),
            Expr::Long(l) => Value::Long(*l),
            Expr::Float(f) => Value::Float(*f),
            Expr::Number(text) => return number_literal(text),
            Expr::String(s) => Value::str(s),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Null => Value::Null,
            Expr::Ident(name) => self.lookup(name, env)?,
            Expr::Typed { value, ty } => {
                let value = self.eval(value, env)?;
                self.typed(value, ty)?
            }
            Expr::Freeze(value) => {
                let value = self.eval(value, env)?;
                freeze(&value);
                value
            }
            Expr::Unary { op, value } => {
                let value = self.eval(value, env)?;
                if op == "not" { Value::Bool(!value.truth()) } else { unary(op, value)? }
            }
            Expr::Binary { left, op, right } => {
                let left = self.eval(left, env)?;
                match op.as_str() {
                    "and" => Value::Bool(left.truth() && self.eval(right, env)?.truth()),
                    "or" => Value::Bool(left.truth() || self.eval(right, env)?.truth()),
                    _ => {
                        let right = self.eval(right, env)?;
                        self.binary(op, left, right)?
                    }
                }
            }
            Expr::Member { object, field } => {
                let object = self.eval(object, env)?;
                self.get_member(&object, field)?
            }
            Expr::Call { callee, args } => return self.eval_call(callee, args, env),
        })
    }

    fn eval_args(&mut self, args: &[Expr], env: &mut Env) -> RResult<Vec<Value>> {
        args.iter().map(|arg| self.eval(arg, env)).collect()
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr], env: &mut Env) -> RResult<Value> {
        match callee {
            Expr::Member { object, field } => {
                let object = self.eval(object, env)?;
                if field == "copy" && args.is_empty() { return Ok(deep_copy(&object, &mut HashMap::new())); }
                let args = self.eval_args(args, env)?;
                if let Value::Str(text) = &object {
                    if let Some(result) = string_method(text, field, &args)? { return Ok(result); }
                }
                self.call_method(object, field, args)
            }
            Expr::Ident(name) => {
                let function = self.lookup(name, env)?;
                let args = self.eval_args(args, env)?;
                self.call_value(&function, args)
            }
            other => {
                let function = self.eval(other, env)?;
                let args = self.eval_args(args, env)?;
                self.call_value(&function, args)
            }
        }
    }

    fn call_value(&mut self, function: &Value, args: Vec<Value>) -> RResult<Value> {
        let Value::Func(callable) = function else {
            return type_err(format!("{} is not callable", function.type_name()));
        };
        match callable.as_ref() {
            Callable::Func(f) => self.call_function(f, None, args),
            Callable::Method(receiver, f) => self.call_function(f, Some(receiver.clone()), args),
            Callable::Family(family) => self.construct(family, args),
            Callable::Builtin(builtin) => self.call_builtin(*builtin, args),
            Callable::Native(function) => {
                if args.len() != function.arity {
                    return type_err(format!("{} expects {} argument(s), got {}", function.name, function.arity, args.len()));
                }
                (function.imp)(&args)
            }
        }
    }

    fn call_function(&mut self, function: &Rc<Function>, me: Option<Value>, args: Vec<Value>) -> RResult<Value> {
        let decl = &function.decl;
        if args.len() != decl.params.len() {
            let name = match &self.units[function.package].name {
                Some(package) => format!("{package}.{}", decl.name),
                None => decl.name.clone(),
            };
            return type_err(format!("{name} expects {} argument(s), got {}", decl.params.len(), args.len()));
        }
        if self.depth >= MAX_CALL_DEPTH { return range_err("maximum call depth exceeded"); }
        let mut locals = HashMap::with_capacity(decl.params.len());
        for (param, arg) in decl.params.iter().zip(args) {
            locals.insert(param.name.clone(), self.typed(arg, &param.ty)?);
        }
        let mut env = Env { unit: function.package, locals: Some(locals), me };
        self.depth += 1;
        let result = match &decl.body {
            FuncBody::Expr(e) => self.eval(e, &mut env),
            FuncBody::Block(body) => self.exec_block(body, &mut env).map(|flow| match flow {
                Flow::Return(value) => value,
                _ => Value::Null,
            }),
        };
        self.depth -= 1;
        result
    }

    fn construct(&mut self, family: &Rc<Family>, args: Vec<Value>) -> RResult<Value> {
        let instance = Value::Instance(Rc::new(Instance {
            family: family.clone(), fields: RefCell::new(IndexMap::new()), meta: Meta::default() }));
        match family.method("init") {
            Some(init) => { self.call_function(&init, Some(instance.clone()), args)?; }
            None if !args.is_empty() => return type_err(format!("{} has no init and takes no arguments", family.name)),
            None => {}
        }
        Ok(instance)
    }

    fn call_method(&mut self, object: Value, name: &str, args: Vec<Value>) -> RResult<Value> {
        match &object {
            Value::Instance(instance) => {
                let field = instance.fields.borrow().get(name).cloned();
                if let Some(field) = field { return self.call_value(&field, args); }
                match instance.family.method(name) {
                    Some(method) => self.call_function(&method, Some(object.clone()), args),
                    None => type_err(format!("{} has no method '{name}'", instance.family.name)),
                }
            }
            Value::Array(array) => self.array_method(&object, array, name, args),
            Value::Set(set) => self.set_method(&object, set, name, args),
            Value::Map(map) => self.map_method(&object, map, name, args),
            Value::Package(_) | Value::Pair(_) => {
                let member = self.get_member(&object, name)?;
                self.call_value(&member, args)
            }
            other => type_err(format!("{} has no method '{name}'", other.type_name())),
        }
    }

    fn get_member(&self, object: &Value, field: &str) -> RResult<Value> {
        match object {
            Value::Instance(instance) => {
                if let Some(value) = instance.fields.borrow().get(field) { return Ok(value.clone()); }
                match instance.family.method(field) {
                    Some(method) => Ok(Value::Func(Rc::new(Callable::Method(object.clone(), method)))),
                    None => type_err(format!("{} has no field '{field}'", instance.family.name)),
                }
            }
            Value::Pair(pair) => match field {
                "first" => Ok(pair.first.borrow().clone()),
                "second" => Ok(pair.second.borrow().clone()),
                _ => type_err(format!("pair has no field '{field}'")),
            },
            Value::Package(package) => package.members.get(field).cloned()
                .ok_or_else(|| RuntimeError { kind: ErrorKind::TypeError, message: format!("{} has no member '{field}'", package.name) }),
            other => type_err(format!("{} has no field '{field}'", other.type_name())),
        }
    }

    fn set_member(&mut self, object: &Value, field: &str, value: Value) -> RResult<()> {
        match object {
            Value::Instance(instance) => {
                mutable(&instance.meta)?;
                instance.fields.borrow_mut().insert(Rc::from(field), value);
                Ok(())
            }
            Value::Pair(pair) => {
                mutable(&pair.meta)?;
                let part = match field {
                    "first" => 0,
                    "second" => 1,
                    _ => return err(ErrorKind::Error, "invalid pair field"),
                };
                let value = self.check(&pair.meta, value, part)?;
                *(if part == 0 { &pair.first } else { &pair.second }).borrow_mut() = value;
                Ok(())
            }
            Value::Package(package) => type_err(format!("cannot assign to read-only member {}.{field}", package.name)),
            other => type_err(format!("cannot set field '{field}' on {}", other.type_name())),
        }
    }

    // ----- builtins -----

    fn write(&mut self, text: &str) -> RResult<()> {
        self.out.write_all(text.as_bytes()).or_else(|e| err(ErrorKind::Error, format!("failed to write output: {e}")))
    }

    fn call_builtin(&mut self, builtin: Builtin, args: Vec<Value>) -> RResult<Value> {
        let arity = |n: usize| -> RResult<()> {
            if args.len() == n { Ok(()) }
            else { type_err(format!("{}() expects {n} argument(s), got {}", format!("{builtin:?}").to_lowercase(), args.len())) }
        };
        match builtin {
            Builtin::Out => {
                let mut line = args.iter().map(to_output_string).collect::<Vec<_>>().join(" ");
                line.push('\n');
                self.write(&line)?;
                Ok(Value::Null)
            }
            Builtin::Slout => {
                let text: String = args.iter().map(to_display_string).collect();
                self.write(&text)?;
                Ok(Value::Null)
            }
            Builtin::In => { arity(0)?; Ok(Value::str(&self.input.read_all())) }
            Builtin::Inln => {
                arity(0)?;
                if self.input.lines.is_none() {
                    let text = self.input.read_all();
                    self.input.lines = Some(text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect());
                }
                let line = self.input.lines.as_ref().unwrap().get(self.input.line).cloned().unwrap_or_default();
                self.input.line += 1;
                Ok(Value::str(&line))
            }
            Builtin::Arr => Ok(Value::array(args)),
            Builtin::Set => {
                let set = Rc::new(Set::default());
                for item in args { set.items.borrow_mut().entry(Key::of(&item)).or_insert(item); }
                Ok(Value::Set(set))
            }
            Builtin::Pair => {
                arity(2)?;
                let mut args = args.into_iter();
                Ok(Value::pair(args.next().unwrap(), args.next().unwrap()))
            }
            Builtin::Map => { arity(0)?; Ok(Value::Map(Rc::new(Map::default()))) }
            Builtin::Int => {
                arity(1)?;
                let number = to_number(&args[0]);
                let value = if number.is_finite() && number.fract() == 0.0 { Value::Int(number as i64) } else { Value::Float(number) };
                // Conversion requires an exact integer: 2.5 is rejected, not truncated.
                if let Value::Float(_) = value { return type_err("expected int"); }
                self.typed(value, "int")
            }
            Builtin::Long => {
                arity(1)?;
                let value = match &args[0] {
                    Value::Str(text) => parse_bigint(text)?,
                    other => other.clone(),
                };
                self.typed(value, "longint")
            }
            Builtin::Float => { arity(1)?; Ok(Value::Float(to_float(&args[0]))) }
            Builtin::String => { arity(1)?; Ok(Value::str(&to_display_string(&args[0]))) }
        }
    }

    // ----- collections -----

    fn array_method(&mut self, object: &Value, array: &Rc<Array>, name: &str, args: Vec<Value>) -> RResult<Value> {
        let arity = |n: usize| method_arity("array", name, &args, n);
        match name {
            "slice" | "lenslice" => {
                let items = slice_items(&array.items.borrow(), &args, name == "lenslice")?;
                return Ok(Value::Array(Rc::new(Array { items: RefCell::new(items), meta: array.meta.inherit() })));
            }
            "len" => { arity(0)?; return Ok(Value::Int(array.items.borrow().len() as i64)); }
            "is_empty" => { arity(0)?; return Ok(Value::Bool(array.items.borrow().is_empty())); }
            "has" => { arity(1)?; return Ok(Value::Bool(array.items.borrow().iter().any(|x| equal(x, &args[0])))); }
            "get" => { arity(1)?; return Ok(Value::Int(position(&array.items.borrow(), &args[0]).map_or(-1, |i| i as i64))); }
            "rget" => {
                arity(1)?;
                let found = array.items.borrow().iter().rposition(|x| equal(x, &args[0]));
                return Ok(Value::Int(found.map_or(-1, |i| i as i64)));
            }
            "iget" => {
                arity(1)?;
                let index = array_index(&args[0], array.items.borrow().len())?;
                return Ok(array.items.borrow()[index].clone());
            }
            _ => {}
        }
        // Everything below mutates the array.
        match name {
            "add" | "pop" | "lpop" | "iremove" | "remove" | "modify" | "change" | "sort" | "asort" | "rev" | "reverse" => {
                mutable(&array.meta)?;
            }
            _ => return type_err(format!("array has no method '{name}'")),
        }
        match name {
            "add" => {
                arity(1)?;
                let value = self.check(&array.meta, args[0].clone(), 0)?;
                array.items.borrow_mut().push(value);
                Ok(object.clone())
            }
            "pop" => {
                arity(0)?;
                let mut items = array.items.borrow_mut();
                Ok(if items.is_empty() { Value::Null } else { items.remove(0) })
            }
            "lpop" => { arity(0)?; Ok(array.items.borrow_mut().pop().unwrap_or(Value::Null)) }
            "iremove" => {
                arity(1)?;
                let index = array_index(&args[0], array.items.borrow().len())?;
                Ok(array.items.borrow_mut().remove(index))
            }
            "remove" => {
                arity(1)?;
                let found = position(&array.items.borrow(), &args[0]);
                Ok(match found { Some(i) => array.items.borrow_mut().remove(i), None => Value::Null })
            }
            "modify" => {
                arity(2)?;
                let index = array_index(&args[0], array.items.borrow().len())?;
                let value = self.check(&array.meta, args[1].clone(), 0)?;
                array.items.borrow_mut()[index] = value;
                Ok(object.clone())
            }
            "change" => {
                arity(2)?;
                let replacement = self.check(&array.meta, args[1].clone(), 0)?;
                for item in array.items.borrow_mut().iter_mut() {
                    if equal(item, &args[0]) { *item = replacement.clone(); }
                }
                Ok(object.clone())
            }
            "sort" | "asort" => {
                arity(0)?;
                let descending = name == "sort";
                let mut items = std::mem::take(&mut *array.items.borrow_mut());
                merge_sort(&mut items, &|a, b| {
                    let order = sort_order(a, b);
                    if descending { order.reverse() } else { order }
                });
                *array.items.borrow_mut() = items;
                Ok(object.clone())
            }
            _ => { arity(0)?; array.items.borrow_mut().reverse(); Ok(object.clone()) }
        }
    }

    fn set_method(&mut self, object: &Value, set: &Rc<Set>, name: &str, args: Vec<Value>) -> RResult<Value> {
        let arity = |n: usize| method_arity("set", name, &args, n);
        match name {
            "len" => { arity(0)?; Ok(Value::Int(set.items.borrow().len() as i64)) }
            "is_empty" => { arity(0)?; Ok(Value::Bool(set.items.borrow().is_empty())) }
            "has" => { arity(1)?; Ok(Value::Bool(set.items.borrow().contains_key(&Key::of(&args[0])))) }
            "add" => {
                arity(1)?;
                mutable(&set.meta)?;
                let value = self.check(&set.meta, args[0].clone(), 0)?;
                set.items.borrow_mut().entry(Key::of(&value)).or_insert(value);
                Ok(object.clone())
            }
            "remove" => {
                arity(1)?;
                mutable(&set.meta)?;
                Ok(Value::Bool(set.items.borrow_mut().shift_remove(&Key::of(&args[0])).is_some()))
            }
            _ => type_err(format!("set has no method '{name}'")),
        }
    }

    fn map_method(&mut self, object: &Value, map: &Rc<Map>, name: &str, args: Vec<Value>) -> RResult<Value> {
        let arity = |n: usize| method_arity("map", name, &args, n);
        match name {
            "len" => { arity(0)?; Ok(Value::Int(map.items.borrow().len() as i64)) }
            "is_empty" => { arity(0)?; Ok(Value::Bool(map.items.borrow().is_empty())) }
            "has" => { arity(1)?; Ok(Value::Bool(map.items.borrow().contains_key(&Key::of(&args[0])))) }
            "get" => {
                arity(1)?;
                Ok(map.items.borrow().get(&Key::of(&args[0])).map_or(Value::Null, |(_, v)| v.clone()))
            }
            "set" => {
                arity(2)?;
                mutable(&map.meta)?;
                let key = self.check(&map.meta, args[0].clone(), 0)?;
                let value = self.check(&map.meta, args[1].clone(), 1)?;
                let mut items = map.items.borrow_mut();
                // Updating an existing key keeps its original key value and position.
                match items.get_mut(&Key::of(&key)) {
                    Some(entry) => entry.1 = value,
                    None => { items.insert(Key::of(&key), (key, value)); }
                }
                Ok(object.clone())
            }
            "remove" => {
                arity(1)?;
                mutable(&map.meta)?;
                Ok(Value::Bool(map.items.borrow_mut().shift_remove(&Key::of(&args[0])).is_some()))
            }
            "keys" => { arity(0)?; Ok(Value::array(map.items.borrow().values().map(|(k, _)| k.clone()).collect())) }
            "values" => { arity(0)?; Ok(Value::array(map.items.borrow().values().map(|(_, v)| v.clone()).collect())) }
            _ => type_err(format!("map has no method '{name}'")),
        }
    }

    // ----- type annotations -----

    /// Apply every element restriction recorded on a container to a new value.
    fn check(&mut self, meta: &Meta, mut value: Value, part: usize) -> RResult<Value> {
        let restrictions = meta.restrictions.borrow().clone();
        for args in restrictions.iter() { value = self.typed(value, &args[part])?; }
        Ok(value)
    }

    /// Check or convert a value against a (whitespace-free) type annotation.
    pub fn typed(&mut self, value: Value, ty: &str) -> RResult<Value> {
        if let Some(inner) = ty.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            let options = split_types(inner);
            // Prefer the actual numeric kind before trying compatible conversions.
            if let Some(kind) = value.num_kind() {
                if options.iter().any(|o| o == kind.name()) { return self.typed(value, kind.name()); }
            }
            for option in &options {
                if let Ok(result) = self.typed(value.clone(), option) { return Ok(result); }
            }
            return type_err(format!("value does not match {ty}"));
        }
        match ty {
            "int" => match value {
                Value::Int(i) => int_in_range(i as i128),
                Value::Float(f) if f.is_finite() && f.fract() == 0.0 => {
                    if f.abs() > i32::MAX as f64 + 1.0 { range_err("int overflow") } else { int_in_range(f as i128) }
                }
                _ => type_err("expected int"),
            },
            "longint" => match value {
                Value::Int(i) | Value::Long(i) => Ok(Value::Long(i)),
                Value::Float(f) if f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64 => Ok(Value::Long(f as i64)),
                _ => type_err("expected exact longint"),
            },
            "float" => match value {
                Value::Int(i) => Ok(Value::Float(i as f64)),
                Value::Float(_) => Ok(value),
                _ => type_err("expected float"),
            },
            "string" => match value { Value::Str(_) => Ok(value), _ => type_err("expected string") },
            _ => {
                if let Some((kind, args)) = collection_type(ty) {
                    return self.typed_collection(value, ty, kind, args);
                }
                match &value {
                    Value::Instance(instance) if instance.family.name == ty => Ok(value),
                    _ => type_err(format!("unknown or mismatched type {ty}")),
                }
            }
        }
    }

    fn typed_collection(&mut self, value: Value, ty: &str, kind: &str, args: Option<&str>) -> RResult<Value> {
        let matches = matches!((&value, kind), (Value::Array(_), "array") | (Value::Set(_), "set")
            | (Value::Pair(_), "pair") | (Value::Map(_), "map"));
        if !matches { return type_err(format!("expected {kind}")); }
        let Some(args) = args else { return Ok(value) };
        let args: Rc<[String]> = split_types(args).into();
        if args.len() != if kind == "map" || kind == "pair" { 2 } else { 1 } {
            return type_err(format!("wrong type arity: {ty}"));
        }
        let meta = value.meta().unwrap();
        let frozen = meta.frozen.get();
        // Validate every element first; converting a fixed container's numeric kind is a mutation.
        let parts: Vec<(Value, usize)> = match &value {
            Value::Array(a) => a.items.borrow().iter().map(|v| (v.clone(), 0)).collect(),
            Value::Set(s) => s.items.borrow().values().map(|v| (v.clone(), 0)).collect(),
            Value::Map(m) => m.items.borrow().values().flat_map(|(k, v)| [(k.clone(), 0), (v.clone(), 1)]).collect(),
            Value::Pair(p) => vec![(p.first.borrow().clone(), 0), (p.second.borrow().clone(), 1)],
            _ => unreachable!(),
        };
        for (item, part) in parts {
            let converted = self.typed(item.clone(), &args[part])?;
            if frozen && converted.num_kind() != item.num_kind() { mutable(meta)?; }
        }
        meta.restrictions.borrow_mut().push(args);
        match &value {
            Value::Array(a) => {
                let items = a.items.borrow().clone();
                let items = items.into_iter().map(|v| self.check(meta, v, 0)).collect::<RResult<Vec<_>>>()?;
                *a.items.borrow_mut() = items;
            }
            Value::Set(s) => {
                let items: Vec<Value> = s.items.borrow().values().cloned().collect();
                let mut rebuilt = IndexMap::new();
                for v in items { let v = self.check(meta, v, 0)?; rebuilt.entry(Key::of(&v)).or_insert(v); }
                *s.items.borrow_mut() = rebuilt;
            }
            Value::Map(m) => {
                let entries: Vec<(Value, Value)> = m.items.borrow().values().cloned().collect();
                let mut rebuilt = IndexMap::new();
                for (k, v) in entries {
                    let k = self.check(meta, k, 0)?;
                    let v = self.check(meta, v, 1)?;
                    rebuilt.insert(Key::of(&k), (k, v));
                }
                *m.items.borrow_mut() = rebuilt;
            }
            Value::Pair(p) => {
                let first = self.check(meta, p.first.borrow().clone(), 0)?;
                let second = self.check(meta, p.second.borrow().clone(), 1)?;
                if first.num_kind() != p.first.borrow().num_kind() { mutable(meta)?; *p.first.borrow_mut() = first; }
                if second.num_kind() != p.second.borrow().num_kind() { mutable(meta)?; *p.second.borrow_mut() = second; }
            }
            _ => unreachable!(),
        }
        Ok(value)
    }

    // ----- arithmetic -----

    fn binary(&self, op: &str, a: Value, b: Value) -> RResult<Value> {
        match op {
            "==" => return Ok(Value::Bool(equal(&a, &b))),
            "!=" => return Ok(Value::Bool(!equal(&a, &b))),
            "<" | "<=" | ">" | ">=" => return compare(op, &a, &b),
            _ => {}
        }
        if let (Value::Str(x), Value::Str(y), "+") = (&a, &b, op) {
            let mut joined = String::with_capacity(x.len() + y.len());
            joined.push_str(x);
            joined.push_str(y);
            return Ok(Value::str(&joined));
        }
        if a.num_kind().is_none() || b.num_kind().is_none() { return type_err("arithmetic requires numbers"); }
        let floating = matches!(a, Value::Float(_)) || matches!(b, Value::Float(_));
        let long = matches!(a, Value::Long(_)) || matches!(b, Value::Long(_));
        if floating && long { return type_err("mixed longint/float arithmetic"); }
        if long {
            let (Value::Int(x) | Value::Long(x)) = a else { unreachable!() };
            let (Value::Int(y) | Value::Long(y)) = b else { unreachable!() };
            return long_arithmetic(op, x as i128, y as i128);
        }
        if floating {
            let (x, y) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            if (op == "/" || op == "%") && y == 0.0 { return range_err("division by zero"); }
            let result = match op {
                "+" => x + y, "-" => x - y, "*" => x * y, "/" => x / y, "%" => x % y,
                _ => float_pow(x, y),
            };
            return Ok(Value::Float(result));
        }
        let (Value::Int(x), Value::Int(y)) = (a, b) else { unreachable!() };
        self.int_arithmetic(op, x, y)
    }

    fn int_arithmetic(&self, op: &str, x: i64, y: i64) -> RResult<Value> {
        let (wide_x, wide_y) = (x as i128, y as i128);
        let exact = match op {
            "+" => wide_x + wide_y,
            "-" => wide_x - wide_y,
            "*" => wide_x * wide_y,
            "/" | "%" if y == 0 => return range_err("division by zero"),
            "/" if x % y != 0 => return Ok(Value::Float(x as f64 / y as f64)),
            "/" => wide_x / wide_y,
            "%" => wide_x % wide_y,
            _ => return self.int_power(x, y),
        };
        // Overflow of 32-bit operands is int overflow; only operands beyond 32 bits
        // (large int literals) can reach the unsafe range.
        let small = |v: i64| (i32::MIN as i64..=i32::MAX as i64).contains(&v);
        if exact.abs() > MAX_SAFE as i128 && !(small(x) && small(y)) {
            return range_err("unsafe integer arithmetic; use longint");
        }
        int_in_range(exact)
    }

    fn int_power(&self, x: i64, y: i64) -> RResult<Value> {
        if y < 0 {
            let result = float_pow(x as f64, y as f64);
            if result.is_finite() && result.fract() == 0.0 { return int_in_range(result as i128); }
            return Ok(Value::Float(result));
        }
        let exact = match x {
            0 => Some(if y == 0 { 1 } else { 0 }),
            1 => Some(1),
            -1 => Some(if y % 2 == 0 { 1 } else { -1 }),
            _ if y as f64 * (x.unsigned_abs() as f64).log2() > 62.0 => None,
            _ => Some((x as i128).pow(y as u32)),
        };
        match exact {
            Some(value) if value.abs() <= MAX_SAFE as i128 => int_in_range(value),
            _ if (i32::MIN as i64..=i32::MAX as i64).contains(&x) => range_err("int overflow"),
            _ => {
                let result = float_pow(x as f64, y as f64);
                if result.is_finite() { range_err("unsafe integer arithmetic; use longint") } else { Ok(Value::Float(result)) }
            }
        }
    }
}

// ----- free helpers -----

fn build_family(name: &str, decls: &HashMap<String, &FamilyDecl>, built: &mut HashMap<String, Rc<Family>>,
                unit: usize, visiting: &mut Vec<String>) -> RResult<Rc<Family>> {
    if let Some(family) = built.get(name) { return Ok(family.clone()); }
    if visiting.iter().any(|v| v == name) { return type_err(format!("family '{name}' inherits from itself")); }
    let decl = decls[name];
    visiting.push(name.to_string());
    let parent = match &decl.extends {
        Some(parent) if decls.contains_key(parent) => Some(build_family(parent, decls, built, unit, visiting)?),
        Some(parent) => return type_err(format!("family '{name}' extends unknown family '{parent}'")),
        None => None,
    };
    visiting.pop();
    let methods = decl.methods.iter()
        .map(|m| (m.name.clone(), Rc::new(Function { decl: m.clone(), package: unit })))
        .collect();
    let family = Rc::new(Family { name: name.to_string(), parent, methods });
    built.insert(name.to_string(), family.clone());
    Ok(family)
}

/// Undo resolver renaming (`__v3_count` -> `count`) for diagnostics.
fn display_name(name: &str) -> &str {
    name.strip_prefix("__v")
        .and_then(|rest| rest.split_once('_'))
        .filter(|(digits, _)| digits.chars().all(|c| c.is_ascii_digit()))
        .map_or(name, |(_, original)| original)
}

fn number_literal(text: &str) -> RResult<Value> {
    if text.chars().all(|c| c.is_ascii_digit()) { return range_err("longint overflow"); }
    text.parse().map(Value::Float).or_else(|_| type_err(format!("invalid number literal {text}")))
}

fn int_in_range(value: i128) -> RResult<Value> {
    if (i32::MIN as i128..=i32::MAX as i128).contains(&value) { Ok(Value::Int(value as i64)) } else { range_err("int overflow") }
}

fn long_in_range(value: i128) -> RResult<Value> {
    if (i64::MIN as i128..=i64::MAX as i128).contains(&value) { Ok(Value::Long(value as i64)) } else { range_err("longint overflow") }
}

fn long_arithmetic(op: &str, x: i128, y: i128) -> RResult<Value> {
    long_in_range(match op {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "/" | "%" if y == 0 => return range_err("division by zero"),
        "/" => x / y,
        "%" => x % y,
        _ => {
            if y < 0 { return range_err("exponent must be non-negative"); }
            let (mut result, mut factor, mut power) = (1i128, x, y);
            while power > 0 {
                if power % 2 == 1 {
                    result = result.checked_mul(factor).filter(|v| v.abs() <= 1 << 64)
                        .ok_or_else(|| RuntimeError { kind: ErrorKind::RangeError, message: "longint overflow".into() })?;
                }
                power /= 2;
                if power > 0 {
                    // Once |factor| exceeds 2^64 any further use overflows, unless it is never used.
                    factor = factor.checked_mul(factor).filter(|v| v.abs() <= 1 << 64).unwrap_or(i128::MAX);
                }
            }
            result
        }
    })
}

fn unary(op: &str, value: Value) -> RResult<Value> {
    let negate = op == "-";
    match value {
        Value::Int(i) => int_in_range(if negate { -(i as i128) } else { i as i128 }),
        Value::Long(l) => long_in_range(if negate { -(l as i128) } else { l as i128 }),
        Value::Float(f) => Ok(Value::Float(if negate { -f } else { f })),
        _ => type_err("unary arithmetic requires a number"),
    }
}

/// `<`, `<=`, `>`, `>=`: numbers of any kinds compare by exact value, strings
/// by UTF-16 code units, and booleans with booleans.
fn compare(op: &str, a: &Value, b: &Value) -> RResult<Value> {
    let order = match (a, b) {
        _ if a.num_kind().is_some() && b.num_kind().is_some() => compare_numbers(a, b),
        (Value::Str(x), Value::Str(y)) => Some(x.encode_utf16().cmp(y.encode_utf16())),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        _ => return type_err("comparison requires matching types"),
    };
    Ok(Value::Bool(match (op, order) {
        (_, None) => false,
        ("<", Some(o)) => o == Ordering::Less,
        ("<=", Some(o)) => o != Ordering::Greater,
        (">", Some(o)) => o == Ordering::Greater,
        (_, Some(o)) => o != Ordering::Less,
    }))
}

/// Order used by `sort`/`asort`: numbers numerically across kinds, strings by
/// UTF-16 code units, booleans as 0/1. Incomparable values keep their order.
fn sort_order(a: &Value, b: &Value) -> Ordering {
    let number = |v: &Value| match v {
        Value::Bool(b) => Value::Int(*b as i64),
        other => other.clone(),
    };
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => x.encode_utf16().cmp(y.encode_utf16()),
        _ => compare_numbers(&number(a), &number(b)).unwrap_or(Ordering::Equal),
    }
}

/// Stable merge sort that tolerates inconsistent orderings (for example NaN).
fn merge_sort(items: &mut Vec<Value>, order: &dyn Fn(&Value, &Value) -> Ordering) {
    if items.len() < 2 { return; }
    let right = items.split_off(items.len() / 2);
    let mut left = std::mem::take(items);
    let mut right = right;
    merge_sort(&mut left, order);
    merge_sort(&mut right, order);
    let (mut l, mut r) = (left.into_iter().peekable(), right.into_iter().peekable());
    while let (Some(a), Some(b)) = (l.peek(), r.peek()) {
        if order(b, a) == Ordering::Less { items.push(r.next().unwrap()); } else { items.push(l.next().unwrap()); }
    }
    items.extend(l);
    items.extend(r);
}

fn position(items: &[Value], value: &Value) -> Option<usize> { items.iter().position(|x| equal(x, value)) }

fn method_arity(owner: &str, name: &str, args: &[Value], n: usize) -> RResult<()> {
    if args.len() == n { Ok(()) } else { type_err(format!("{owner}.{name}() expects {n} argument(s), got {}", args.len())) }
}

fn array_index(value: &Value, len: usize) -> RResult<usize> {
    let index = match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) if f.fract() == 0.0 => Some(*f),
        _ => None,
    };
    match index {
        Some(i) if i >= 0.0 && (i as usize) < len => Ok(i as usize),
        _ => range_err("array index out of bounds"),
    }
}

fn out_of_bounds<T>(message: &str) -> RResult<T> { err(ErrorKind::OutOfBoundsError, message) }

fn slice_index(value: &Value) -> RResult<usize> {
    match value {
        Value::Long(l) => if *l < 0 || *l > MAX_SAFE { out_of_bounds("slice index out of bounds") } else { Ok(*l as usize) },
        Value::Int(_) | Value::Float(_) => {
            let v = value.as_f64().unwrap();
            if !v.is_finite() || v.fract() != 0.0 { return type_err("slice positions and lengths must be integers"); }
            if v < 0.0 || v > MAX_SAFE as f64 { return out_of_bounds("slice index out of bounds"); }
            Ok(v as usize)
        }
        _ => type_err("slice positions and lengths must be integers"),
    }
}

/// `slice(start,end)` / `lenslice(start,length)` with optional `reverse`, which
/// counts positions from the end while preserving the original order.
fn slice_items<T: Clone>(items: &[T], args: &[Value], by_length: bool) -> RResult<Vec<T>> {
    if args.len() < 2 || args.len() > 3 { return type_err("slicing requires two arguments and an optional reverse boolean"); }
    let (start, second) = (slice_index(&args[0])?, slice_index(&args[1])?);
    let reverse = match args.get(2) {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return type_err("slice reverse must be a boolean"),
    };
    let size = items.len();
    if start > size || (if by_length { second > size - start } else { second < start || second > size }) {
        return out_of_bounds("slice extends outside the available range");
    }
    let end = if by_length { start + second } else { second };
    Ok(if reverse { items[size - end..size - start].to_vec() } else { items[start..end].to_vec() })
}

/// String methods operate on grapheme clusters. Returns `None` for names that
/// are not string methods.
fn string_method(text: &str, name: &str, args: &[Value]) -> RResult<Option<Value>> {
    let no_args = |method: &str| if args.is_empty() { Ok(()) } else { type_err(format!("string.{method}() takes no arguments")) };
    Ok(Some(match name {
        "len" => { no_args("len")?; Value::Int(text.graphemes(true).count() as i64) }
        "reverse" => { no_args("reverse")?; Value::str(&text.graphemes(true).rev().collect::<String>()) }
        "slice" | "lenslice" => {
            let graphemes: Vec<&str> = text.graphemes(true).collect();
            Value::str(&slice_items(&graphemes, args, name == "lenslice")?.concat())
        }
        _ => return type_err(format!("string has no method '{name}'")),
    }))
}

fn iterate(value: &Value) -> RResult<Vec<Value>> {
    Ok(match value {
        Value::Str(text) => text.graphemes(true).map(Value::str).collect(),
        Value::Array(a) => a.items.borrow().clone(),
        Value::Set(s) => s.items.borrow().values().cloned().collect(),
        Value::Map(m) => m.items.borrow().values().map(|(k, _)| k.clone()).collect(),
        _ => return type_err("expected an iterable array, set, map, or string"),
    })
}

fn repeat_count(value: &Value) -> RResult<u64> {
    let count = match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    };
    match count {
        Some(n) if n >= 0.0 && n.fract() == 0.0 && n <= MAX_SAFE as f64 => Ok(n as u64),
        _ => range_err("repeat count must be a nonnegative integer"),
    }
}

fn mutable(meta: &Meta) -> RResult<()> {
    if meta.frozen.get() { err(ErrorKind::Error, "cannot mutate fixed value") } else { Ok(()) }
}

/// Recursively mark containers and instances fixed. Already-fixed objects were
/// frozen together with everything they contain.
fn freeze(value: &Value) {
    let Some(meta) = value.meta() else { return };
    if meta.frozen.replace(true) { return; }
    match value {
        Value::Array(a) => a.items.borrow().iter().for_each(freeze),
        Value::Set(s) => s.items.borrow().values().for_each(freeze),
        Value::Map(m) => m.items.borrow().values().for_each(|(k, v)| { freeze(k); freeze(v); }),
        Value::Pair(p) => { freeze(&p.first.borrow()); freeze(&p.second.borrow()); }
        Value::Instance(o) => o.fields.borrow().values().for_each(freeze),
        _ => {}
    }
}

/// Deep copy preserving cycles and shared structure; the copy is mutable and
/// keeps element restrictions.
fn deep_copy(value: &Value, seen: &mut HashMap<usize, Value>) -> Value {
    if value.meta().is_none() { return value.clone(); }
    let id = value.object_id().unwrap();
    if let Some(copy) = seen.get(&id) { return copy.clone(); }
    let meta = value.meta().unwrap().inherit();
    match value {
        Value::Array(a) => {
            let copy = Rc::new(Array { items: RefCell::new(Vec::new()), meta });
            seen.insert(id, Value::Array(copy.clone()));
            let items: Vec<Value> = a.items.borrow().iter().map(|v| deep_copy(v, seen)).collect();
            *copy.items.borrow_mut() = items;
            Value::Array(copy)
        }
        Value::Set(s) => {
            let copy = Rc::new(Set { items: RefCell::new(IndexMap::new()), meta });
            seen.insert(id, Value::Set(copy.clone()));
            let items: Vec<Value> = s.items.borrow().values().map(|v| deep_copy(v, seen)).collect();
            for v in items { copy.items.borrow_mut().entry(Key::of(&v)).or_insert(v); }
            Value::Set(copy)
        }
        Value::Map(m) => {
            let copy = Rc::new(Map { items: RefCell::new(IndexMap::new()), meta });
            seen.insert(id, Value::Map(copy.clone()));
            let entries: Vec<(Value, Value)> = m.items.borrow().values().cloned().collect();
            for (k, v) in entries {
                let (k, v) = (deep_copy(&k, seen), deep_copy(&v, seen));
                copy.items.borrow_mut().insert(Key::of(&k), (k, v));
            }
            Value::Map(copy)
        }
        Value::Pair(p) => {
            let copy = Rc::new(Pair { first: RefCell::new(Value::Null), second: RefCell::new(Value::Null), meta });
            seen.insert(id, Value::Pair(copy.clone()));
            let (first, second) = (p.first.borrow().clone(), p.second.borrow().clone());
            *copy.first.borrow_mut() = deep_copy(&first, seen);
            *copy.second.borrow_mut() = deep_copy(&second, seen);
            Value::Pair(copy)
        }
        Value::Instance(o) => {
            let copy = Rc::new(Instance { family: o.family.clone(), fields: RefCell::new(IndexMap::new()), meta });
            seen.insert(id, Value::Instance(copy.clone()));
            let fields: Vec<(Rc<str>, Value)> = o.fields.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (k, v) in fields {
                let v = deep_copy(&v, seen);
                copy.fields.borrow_mut().insert(k, v);
            }
            Value::Instance(copy)
        }
        _ => value.clone(),
    }
}

fn split_types(text: &str) -> Vec<String> {
    let (mut depth, mut start, mut parts) = (0i32, 0, Vec::new());
    for (i, c) in text.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 0 => { parts.push(text[start..i].to_string()); start = i + 1; }
            _ => {}
        }
    }
    parts.push(text[start..].to_string());
    parts
}

/// `array`, `set[int]`, `map[string,int]`, ... -> (kind, type arguments).
fn collection_type(ty: &str) -> Option<(&'static str, Option<&str>)> {
    for (name, kind) in [("array", "array"), ("set", "set"), ("pair", "pair"), ("map", "map"), ("dict", "map")] {
        if let Some(rest) = ty.strip_prefix(name) {
            if rest.is_empty() { return Some((kind, None)); }
            if let Some(args) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                return Some((kind, if args.is_empty() { None } else { Some(args) }));
            }
        }
    }
    None
}

/// Numeric conversion used by `int(...)` and `float(...)`.
fn to_number(value: &Value) -> f64 {
    match value {
        Value::Null => 0.0,
        Value::Bool(b) => *b as u8 as f64,
        Value::Int(i) | Value::Long(i) => *i as f64,
        Value::Float(f) => *f,
        Value::Str(s) => string_to_number(s),
        _ => f64::NAN,
    }
}

/// Decimal (optional sign, fraction, exponent), `0x`/`0o`/`0b` integers, and
/// `Infinity`; surrounding whitespace is ignored and an empty string is 0.
fn string_to_number(text: &str) -> f64 {
    let text = text.trim();
    if text.is_empty() { return 0.0; }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = text.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) { return f64::NAN; }
            return digits.chars().fold(0.0, |acc, c| acc * radix as f64 + c.to_digit(radix).unwrap() as f64);
        }
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned == "Infinity" { return if text.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }; }
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(i) => (&unsigned[..i], Some(&unsigned[i + 1..])),
        None => (unsigned, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    let exponent_ok = exponent.map_or(true, |e| { let e = e.strip_prefix(['+', '-']).unwrap_or(e); !e.is_empty() && digits(e) });
    if (whole.is_empty() && fraction.is_empty()) || !digits(whole) || !digits(fraction) || !exponent_ok { return f64::NAN; }
    text.parse().unwrap_or(f64::NAN)
}

/// `float(...)`: also accepts `inf`/`infinity` spellings in any case.
fn to_float(value: &Value) -> f64 {
    if let Value::Str(text) = value {
        let lower = text.trim().to_lowercase();
        let unsigned = lower.strip_prefix(['+', '-']).unwrap_or(&lower);
        if unsigned == "inf" || unsigned == "infinity" {
            return if lower.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY };
        }
    }
    to_number(value)
}

/// Exact integer from a decimal, hex, octal, or binary string for `longint(...)`.
fn parse_bigint(text: &str) -> RResult<Value> {
    let trimmed = text.trim();
    let invalid = || err(ErrorKind::SyntaxError, format!("cannot convert {text:?} to a longint"));
    if trimmed.is_empty() { return Ok(Value::Long(0)); }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = trimmed.strip_prefix(prefix) {
            return match i128::from_str_radix(digits, radix) {
                Ok(v) if !digits.starts_with(['+', '-']) => long_in_range(v),
                _ => invalid(),
            };
        }
    }
    let unsigned = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    if unsigned.is_empty() || !unsigned.chars().all(|c| c.is_ascii_digit()) { return invalid(); }
    match trimmed.parse::<i128>() {
        Ok(v) => long_in_range(v),
        Err(_) => range_err("longint overflow"),
    }
}
