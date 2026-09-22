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

enum MethodTarget { Method(Rc<Function>), Value(Value) }

/// A container restriction from a matched annotation, applied after matching.
#[derive(Clone)]
struct Restriction { target: Value, args: Rc<[String]>, unit: usize }

/// Union choices made while planning a type check, for depth-first retries: the
/// option index chosen at each union reached, in order, and its option count.
#[derive(Default)]
struct Choices { forced: Vec<usize>, taken: Vec<(usize, usize)> }

/// Plans retried with different union choices before a type check gives up.
const MAX_TYPE_ATTEMPTS: usize = 64;

/// The computed result of restricting one container: its new restriction list
/// and converted contents, written only once the whole plan has succeeded.
struct Staged { target: Value, restrictions: Vec<(Rc<[String]>, usize)>, contents: Contents }

enum Contents {
    Array(Vec<Value>),
    Set(IndexMap<Key, Value>),
    Map(IndexMap<Key, (Value, Value)>),
    Pair(Value, Value),
}

/// Globals and declarations of one source unit (the program or a bundled package).
struct Unit {
    /// Package name, or `None` for the program itself.
    name: Option<String>,
    globals: RefCell<HashMap<String, Value>>,
    /// Imported packages by their source alias, for `alias.Family` annotations.
    imports: RefCell<HashMap<String, Value>>,
    functions: HashMap<String, Rc<Function>>,
    families: HashMap<String, Rc<Family>>,
}

struct Env {
    unit: usize,
    /// The family whose method is running, for private/subclass access checks.
    owner: Option<Rc<Family>>,
    /// `None` while running top-level statements, which declare globals.
    locals: Option<Rc<Frame>>,
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
    /// Flush after every `out`/`slout` (when output is a terminal).
    interactive: bool,
    input: Input,
    depth: usize,
    /// Loaded packages by key.
    packages: HashMap<String, Value>,
    /// Built-in error families by name: `Error` and the kinds inheriting from it.
    error_families: HashMap<&'static str, Rc<Family>>,
    /// The error value raised by `err` that is currently propagating, with the
    /// serial number its `RuntimeError` carries.
    raised: Option<(u64, Value)>,
    next_serial: u64,
    /// The most recently handled error, returned by `lasterr()`.
    last_error: Option<Value>,
    /// The family whose method is running (`None` in free functions and top-level
    /// code), for checking private/subclass access when a bound method is called.
    caller: Option<Rc<Family>>,
}

type Exec = RResult<Flow>;

impl<'o> Interp<'o> {
    pub fn new(out: &'o mut dyn Write, input: InputSource, interactive: bool) -> Self {
        Interp { units: Vec::new(), out, interactive, input: Input { source: input, lines: None, line: 0 },
            depth: 0, packages: HashMap::new(), error_families: error_families(), raised: None, next_serial: 0,
            last_error: None, caller: None }
    }

    /// Run a resolved program: load its packages (each after its own imports),
    /// execute top-level statements in order, then call `m` when it is declared.
    /// Afterwards every value the run created is released, including cycles.
    pub(crate) fn run(&mut self, program: &Program, packages: &[LoadedPackage]) -> RResult<()> {
        let result = self.run_program(program, packages);
        self.units.clear();
        self.packages.clear();
        self.raised = None;
        self.last_error = None;
        crate::gc::collect();
        result
    }

    fn run_program(&mut self, program: &Program, packages: &[LoadedPackage]) -> RResult<()> {
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
        if let Some(entry) = entry { self.call_function(&entry, None, Vec::new(), None)?; }
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
        Ok(Value::package(package.name.clone(), members))
    }

    fn load_unit(&mut self, program: &Program, name: Option<String>) -> RResult<usize> {
        let index = self.units.len();
        let mut functions = HashMap::new();
        let mut decls = HashMap::new();
        for item in &program.items {
            match item {
                Item::Func(f) => { functions.insert(f.name.clone(), Rc::new(Function { decl: std::sync::Arc::new(f.clone()), package: index, owner: None })); }
                Item::Family(f) => { decls.insert(f.name.clone(), f); }
                _ => {}
            }
        }
        // Parents outside this file: a built-in error family, or `alias.Family`
        // from a package this file imports (packages load before their importers).
        let imports: HashMap<String, Value> = program.items.iter().filter_map(|item| match item {
            Item::Import(import) => {
                let package = import.key.as_ref().and_then(|key| self.packages.get(key))?;
                Some((display_name(import.alias.as_deref()?).to_string(), package.clone()))
            }
            _ => None,
        }).collect();
        let errors = &self.error_families;
        let outside = |parent: &str| -> Option<Rc<Family>> {
            match parent.split_once('.') {
                Some((alias, name)) => match imports.get(alias) {
                    Some(Value::Package(package)) => match package.members.get(name) {
                        Some(Value::Func(f)) => match f.as_ref() { Callable::Family(family) => Some(family.clone()), _ => None },
                        _ => None,
                    },
                    _ => None,
                },
                None => errors.get(parent).cloned(),
            }
        };
        let mut families = HashMap::new();
        for name in decls.keys() { build_family(name, &decls, &mut families, &outside, index, &mut Vec::new())?; }
        self.units.push(Unit { name, globals: RefCell::new(HashMap::new()), imports: RefCell::new(HashMap::new()), functions, families });
        Ok(index)
    }

    fn run_top_level(&mut self, program: &Program, unit: usize) -> RResult<()> {
        let mut env = Env { unit, owner: None, locals: None, me: None };
        for item in &program.items {
            match item {
                Item::Import(import) => {
                    let package = import.key.as_ref().and_then(|key| self.packages.get(key)).cloned()
                        .ok_or_else(|| RuntimeError::new(ErrorKind::Error, format!("package '{}' is not loaded", import.module)))?;
                    let alias = import.alias.clone().unwrap_or_default();
                    self.units[unit].imports.borrow_mut().insert(display_name(&alias).to_string(), package.clone());
                    self.units[unit].globals.borrow_mut().insert(alias, package);
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
        if let Some(value) = env.locals.as_ref().and_then(|frame| frame.lookup(name)) { return Ok(value); }
        if name == "me" {
            return env.me.clone().ok_or_else(|| RuntimeError::new(ErrorKind::Error, "'me' is only available inside family methods"));
        }
        let unit = &self.units[env.unit];
        if let Some(value) = unit.globals.borrow().get(name) { return Ok(value.clone()); }
        if let Some(function) = unit.functions.get(name) { return Ok(Value::Func(Rc::new(Callable::Func(function.clone())))); }
        if let Some(family) = unit.families.get(name).or_else(|| self.error_families.get(name)) {
            return Ok(Value::Func(Rc::new(Callable::Family(family.clone()))));
        }
        if let Some(builtin) = Builtin::lookup(name) { return Ok(Value::Func(Rc::new(Callable::Builtin(builtin)))); }
        err(ErrorKind::Error, format!("'{}' is used before it is initialized", display_name(name)))
    }

    fn declare(&self, env: &mut Env, name: &str, value: Value) {
        match &env.locals {
            Some(frame) => { frame.values.borrow_mut().insert(name.to_string(), value); }
            None => { self.units[env.unit].globals.borrow_mut().insert(name.to_string(), value); }
        }
    }

    fn assign(&self, env: &mut Env, name: &str, value: Value) {
        // The nearest frame that has the name, which for a closure may be the
        // frame of the function around it.
        let value = match &env.locals {
            Some(frame) => match frame.assign(name, value) { Ok(()) => return, Err(value) => value },
            None => value,
        };
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
        // Statement boundaries are safe points: no container is borrowed here.
        if crate::gc::due() { crate::gc::collect(); }
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
            Stmt::Func { name, decl } => {
                let closure = self.closure(decl, env);
                self.declare(env, name, closure);
            }
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
            Stmt::Run { body, handlers, then_block } => return self.exec_run(body, handlers, then_block.as_deref(), env),
        }
        Ok(Flow::Next)
    }

    /// `run{} handle(...){} ... then{}`: the first handler whose error family
    /// matches runs; `then` always runs last, even while an error propagates or
    /// the block exits early. An error, `ret`, `break`, or `continue` inside
    /// `then` replaces the pending outcome.
    fn exec_run(&mut self, body: &[Stmt], handlers: &[Handler], then_block: Option<&[Stmt]>, env: &mut Env) -> Exec {
        let mut outcome = self.exec_block(body, env);
        if let Err(error) = outcome {
            let value = self.error_value(&error);
            outcome = match self.find_handler(handlers, &value, env.unit) {
                Ok(Some(handler)) => {
                    self.last_error = Some(value.clone());
                    if let Some(name) = &handler.name { self.declare(env, name, value); }
                    self.exec_block(&handler.body, env)
                }
                Ok(None) => Err(self.propagate(error, value)),
                Err(problem) => Err(problem),
            };
        }
        if let Some(block) = then_block {
            let in_flight = self.raised.take();
            match self.exec_block(block, env)? {
                Flow::Next => self.raised = in_flight,
                flow => return Ok(flow),
            }
        }
        outcome
    }

    /// The error value for a caught error: the one `err` raised, or a new
    /// instance of the built-in family for an interpreter error.
    fn error_value(&mut self, error: &RuntimeError) -> Value {
        if let Some((serial, _)) = &self.raised {
            if error.serial != 0 && *serial == error.serial { return self.raised.take().unwrap().1; }
        }
        let value = Value::new_instance(self.error_families[error.kind.name()].clone(), Meta::default());
        if let Value::Instance(o) = &value { o.fields.borrow_mut().insert(Rc::from("message"), Value::str(&error.message)); }
        value
    }

    /// Keep an unhandled error's value attached while it continues outward.
    fn propagate(&mut self, mut error: RuntimeError, value: Value) -> RuntimeError {
        self.next_serial += 1;
        error.serial = self.next_serial;
        self.raised = Some((error.serial, value));
        error
    }

    fn find_handler<'h>(&self, handlers: &'h [Handler], value: &Value, unit: usize) -> RResult<Option<&'h Handler>> {
        let Value::Instance(instance) = value else { return Ok(None) };
        for handler in handlers {
            for ty in &handler.types {
                let family = match self.family_named(ty, unit) {
                    Some(family) if family.error_kind.is_some() => family,
                    Some(_) => return type_err(format!("{ty} is not an error family; error families inherit from Error")),
                    None => return type_err(format!("unknown error type {ty}")),
                };
                if descends(&instance.family, &family) { return Ok(Some(handler)); }
            }
        }
        Ok(None)
    }

    /// `err(Family, message)` raises a new error; `err(error)` raises a caught one again.
    fn raise(&mut self, args: Vec<Value>) -> RResult<Value> {
        let usage = "err expects an error family and a message, such as err(Error, \"message\"), or a caught error";
        let value = match args.as_slice() {
            [Value::Func(f)] | [Value::Func(f), _] => match f.as_ref() {
                Callable::Family(family) if family.error_kind.is_some() => {
                    let message = args.get(1).map(to_display_string).unwrap_or_default();
                    let value = Value::new_instance(family.clone(), Meta::default());
                    if let Value::Instance(o) = &value { o.fields.borrow_mut().insert(Rc::from("message"), Value::str(&message)); }
                    value
                }
                Callable::Family(family) => return type_err(format!("{} is not an error family; error families inherit from Error", family.name)),
                _ => return type_err(usage),
            },
            [Value::Instance(o)] if o.family.error_kind.is_some() => args[0].clone(),
            _ => return type_err(usage),
        };
        let Value::Instance(instance) = &value else { unreachable!() };
        let kind = instance.family.error_kind.unwrap();
        let builtin = Rc::ptr_eq(&instance.family, &self.error_families[kind.name()]);
        let message = instance.fields.borrow().get("message").map(to_display_string).unwrap_or_default();
        let error = RuntimeError { kind, message, family: (!builtin).then(|| instance.family.name.clone()), serial: 0 };
        Err(self.propagate(error, value))
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
                self.typed(value, ty, env.unit)?
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
                self.get_member(&object, field, env.owner.as_ref())?
            }
            Expr::Index { object, index } => {
                let object = self.eval(object, env)?;
                let index = self.eval(index, env)?;
                match &object {
                    Value::Str(text) => character_at(text, &index)?,
                    Value::Array(array) => {
                        let items = array.items.borrow();
                        items[whole_index(&index, items.len(), "array")?].clone()
                    }
                    other => return type_err(format!("{} {} cannot be indexed; strings and arrays can", article(other.type_name()), other.type_name())),
                }
            }
            Expr::Lambda(decl) => self.closure(decl, env),
            Expr::Call { callee, args } => return self.eval_call(callee, args, env),
        })
    }

    /// A function made where it stands: it keeps the current frame, `me`, and
    /// the family whose method is running, so it can reach what the code
    /// around it can, including that family's private methods.
    fn closure(&self, decl: &std::sync::Arc<FuncDecl>, env: &Env) -> Value {
        let function = Rc::new(Function { decl: decl.clone(), package: env.unit, owner: env.owner.as_ref().map(Rc::downgrade) });
        Value::closure(function, env.locals.clone(), env.me.clone())
    }

    fn eval_args(&mut self, args: &[Expr], env: &mut Env) -> RResult<Vec<Value>> {
        args.iter().map(|arg| self.eval(arg, env)).collect()
    }

    fn eval_call(&mut self, callee: &Expr, args: &[Expr], env: &mut Env) -> RResult<Value> {
        match callee {
            Expr::Member { object, field } => {
                let object = self.eval(object, env)?;
                if field == "copy" && args.is_empty() { return Ok(deep_copy(&object, &mut HashMap::new())); }
                // Select the target before evaluating arguments, so their side
                // effects cannot replace it. Built-in string and collection
                // methods are fixed by the receiver's type.
                let target = self.method_target(&object, field, env.owner.as_ref())?;
                let args = self.eval_args(args, env)?;
                match target {
                    Some(MethodTarget::Method(method)) => self.call_function(&method, Some(object), args, None),
                    Some(MethodTarget::Value(function)) => self.call_value(&function, args),
                    None => match &object {
                        Value::Str(text) => Ok(string_method(text, field, &args)?.expect("string methods always resolve")),
                        Value::Array(array) => self.array_method(&object, array, field, args),
                        Value::Set(set) => self.set_method(&object, set, field, args),
                        Value::Map(map) => self.map_method(&object, map, field, args),
                        _ => unreachable!("method_target resolves every other receiver"),
                    },
                }
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
            Callable::Func(f) => self.call_function(f, None, args, None),
            Callable::Method(receiver, f) => {
                // A bound method taken out of its family keeps its restriction:
                // check against whoever calls it now, not where it was taken.
                check_access(f, self.caller.as_ref())?;
                self.call_function(f, Some(receiver.clone()), args, None)
            }
            Callable::Closure(f, frame, me) => self.call_function(f, me.clone(), args, frame.clone()),
            Callable::Family(family) => self.construct(family, args),
            Callable::Builtin(builtin) => self.call_builtin(*builtin, args),
            Callable::Native(function) => {
                if args.len() != function.arity {
                    return type_err(format!("{} expects {} argument(s), got {}", function.name, function.arity, args.len()));
                }
                // A bug in a native package must not take down the interpreter:
                // a panic becomes an ordinary, catchable error.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (function.imp)(&args)))
                    .unwrap_or_else(|panic| {
                        let detail = panic.downcast_ref::<&str>().map(|s| s.to_string())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic".into());
                        err(ErrorKind::Error, format!("internal error in {}: {detail}", function.name))
                    })
            }
        }
    }

    /// `parent` is the frame a closure closes over; its variables stay visible.
    fn call_function(&mut self, function: &Rc<Function>, me: Option<Value>, args: Vec<Value>, parent: Option<Rc<Frame>>) -> RResult<Value> {
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
            locals.insert(param.name.clone(), self.typed(arg, &param.ty, function.package)?);
        }
        let owner = function.owner.as_ref().and_then(|owner| owner.upgrade());
        let previous_caller = std::mem::replace(&mut self.caller, owner.clone());
        let mut env = Env { unit: function.package, owner, locals: Some(Frame::new(locals, parent)), me };
        self.depth += 1;
        let result = match &decl.body {
            FuncBody::Expr(e) => self.eval(e, &mut env),
            FuncBody::Block(body) => self.exec_block(body, &mut env).map(|flow| match flow {
                Flow::Return(value) => value,
                _ => Value::Null,
            }),
        };
        self.depth -= 1;
        self.caller = previous_caller;
        result
    }

    fn construct(&mut self, family: &Rc<Family>, args: Vec<Value>) -> RResult<Value> {
        let instance = Value::new_instance(family.clone(), Meta::default());
        match family.method("init") {
            Some(init) => { self.call_function(&init, Some(instance.clone()), args, None)?; }
            None if family.error_kind.is_some() => {
                let name = &family.name;
                return type_err(format!("{name} is an error family: raise it with err({name}, message) instead of calling it"));
            }
            None if !args.is_empty() => return type_err(format!("{} has no init and takes no arguments", family.name)),
            None => {}
        }
        Ok(instance)
    }

    /// What `object.name(...)` calls. `None` means a built-in string or
    /// collection method, dispatched after the arguments are evaluated.
    fn method_target(&self, object: &Value, name: &str, caller: Option<&Rc<Family>>) -> RResult<Option<MethodTarget>> {
        Ok(Some(match object {
            Value::Instance(instance) => {
                // A field holding a function takes precedence over a method.
                if let Some(field) = instance.fields.borrow().get(name) { return Ok(Some(MethodTarget::Value(field.clone()))); }
                match instance.family.method(name) {
                    Some(method) => { check_access(&method, caller)?; MethodTarget::Method(method) }
                    None => return type_err(format!("{} has no method '{name}'", instance.family.name)),
                }
            }
            Value::Package(_) | Value::Pair(_) => MethodTarget::Value(self.get_member(object, name, caller)?),
            Value::Str(_) | Value::Array(_) | Value::Set(_) | Value::Map(_) => return Ok(None),
            other => return type_err(format!("{} has no method '{name}'", other.type_name())),
        }))
    }

    fn get_member(&self, object: &Value, field: &str, caller: Option<&Rc<Family>>) -> RResult<Value> {
        match object {
            Value::Instance(instance) => {
                if let Some(value) = instance.fields.borrow().get(field) { return Ok(value.clone()); }
                match instance.family.method(field) {
                    Some(method) => { check_access(&method, caller)?; Ok(Value::method(object.clone(), method)) }
                    None => type_err(format!("{} has no field '{field}'", instance.family.name)),
                }
            }
            Value::Pair(pair) => match field {
                "first" => Ok(pair.first.borrow().clone()),
                "second" => Ok(pair.second.borrow().clone()),
                _ => type_err(format!("pair has no field '{field}'")),
            },
            Value::Package(package) => package.members.get(field).cloned()
                .ok_or_else(|| RuntimeError::new(ErrorKind::TypeError, format!("{} has no member '{field}'", package.name))),
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
        self.out.write_all(text.as_bytes()).or_else(|e| err(ErrorKind::Error, format!("failed to write output: {e}")))?;
        if self.interactive { self.flush()?; }
        Ok(())
    }

    fn flush(&mut self) -> RResult<()> {
        self.out.flush().or_else(|e| err(ErrorKind::Error, format!("failed to write output: {e}")))
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
            Builtin::In => {
                arity(0)?;
                // Show any prompt before blocking on input.
                self.flush()?;
                Ok(Value::str(&self.input.read_all()))
            }
            Builtin::Inln => {
                arity(0)?;
                self.flush()?;
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
                let mut items = IndexMap::new();
                for item in args { items.entry(Key::of(&item)).or_insert(item); }
                Ok(Value::new_set(items, Meta::default()))
            }
            Builtin::Pair => {
                arity(2)?;
                let mut args = args.into_iter();
                Ok(Value::pair(args.next().unwrap(), args.next().unwrap()))
            }
            Builtin::Map => { arity(0)?; Ok(Value::new_map(IndexMap::new(), Meta::default())) }
            Builtin::Int => {
                arity(1)?;
                let number = to_number(&args[0]);
                let value = if number.is_finite() && number.fract() == 0.0 { Value::Int(number as i64) } else { Value::Float(number) };
                // Conversion requires an exact integer: 2.5 is rejected, not truncated.
                if let Value::Float(_) = value { return type_err("expected int"); }
                self.typed(value, "int", 0)
            }
            Builtin::Long => {
                arity(1)?;
                let value = match &args[0] {
                    Value::Str(text) => parse_bigint(text)?,
                    other => other.clone(),
                };
                self.typed(value, "longint", 0)
            }
            Builtin::Float => { arity(1)?; Ok(Value::Float(to_float(&args[0]))) }
            Builtin::String => { arity(1)?; Ok(Value::str(&to_display_string(&args[0]))) }
            Builtin::Err => {
                if args.is_empty() || args.len() > 2 { return type_err(format!("err() expects 1 or 2 argument(s), got {}", args.len())); }
                self.raise(args)
            }
            Builtin::LastErr => { arity(0)?; Ok(self.last_error.clone().unwrap_or(Value::Null)) }
        }
    }

    // ----- collections -----

    fn array_method(&mut self, object: &Value, array: &Rc<Array>, name: &str, args: Vec<Value>) -> RResult<Value> {
        let arity = |n: usize| method_arity("array", name, &args, n);
        match name {
            "slice" | "lenslice" => {
                let items = slice_items(&array.items.borrow(), &args, name == "lenslice")?;
                return Ok(Value::new_array(items, array.meta.inherit()));
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
                let mut checked = self.check_parts(&map.meta, vec![(args[0].clone(), 0), (args[1].clone(), 1)])?.into_iter();
                let (key, value) = (checked.next().unwrap(), checked.next().unwrap());
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
    fn check(&mut self, meta: &Meta, value: Value, part: usize) -> RResult<Value> {
        Ok(self.check_parts(meta, vec![(value, part)])?.remove(0))
    }

    /// Check several parts of one insertion (a map key and value) as a single
    /// operation: if any part fails, nothing is converted or restricted.
    fn check_parts(&mut self, meta: &Meta, parts: Vec<(Value, usize)>) -> RResult<Vec<Value>> {
        let restrictions = meta.restrictions.borrow().clone();
        let jobs = parts.into_iter()
            .map(|(value, part)| (value, restrictions.iter().map(|(args, unit)| (args[part].clone(), *unit)).collect()))
            .collect();
        self.conform(jobs)
    }

    /// Check or convert a value against a (whitespace-free) type annotation
    /// written in `unit`. A collection annotation changes its container (it adds
    /// a restriction and converts the elements); those changes are applied only
    /// once the whole annotation, including every nested part, has matched.
    pub fn typed(&mut self, value: Value, ty: &str, unit: usize) -> RResult<Value> {
        Ok(self.conform(vec![(value, vec![(ty.to_string(), unit)])])?.remove(0))
    }

    /// Check values against their annotations as one operation. All annotations
    /// are planned together; if the combined plan cannot be applied, union choices
    /// are retried depth-first. Container changes are committed only when every
    /// check succeeds, so a failure leaves all values unchanged.
    fn conform(&mut self, jobs: Vec<(Value, Vec<(String, usize)>)>) -> RResult<Vec<Value>> {
        let mut forced = Vec::new();
        let mut first_error = None;
        for _ in 0..MAX_TYPE_ATTEMPTS {
            let mut choices = Choices { forced: std::mem::take(&mut forced), taken: Vec::new() };
            let mut pending = Vec::new();
            let outcome = self.plan_jobs(&jobs, &mut pending, &mut choices)
                .and_then(|values| Ok((values, self.stage(&pending)?)));
            match outcome {
                Ok((values, staged)) => {
                    Self::commit(staged);
                    return Ok(values);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                    // Try the next option at the most recent union that has one.
                    let Some(k) = choices.taken.iter().rposition(|&(chosen, count)| chosen + 1 < count) else { break };
                    forced = choices.taken[..k].iter().map(|&(chosen, _)| chosen).collect();
                    forced.push(choices.taken[k].0 + 1);
                }
            }
        }
        Err(first_error.expect("at least one attempt ran"))
    }

    fn plan_jobs(&mut self, jobs: &[(Value, Vec<(String, usize)>)], pending: &mut Vec<Restriction>,
                 choices: &mut Choices) -> RResult<Vec<Value>> {
        let mut values = Vec::with_capacity(jobs.len());
        for (value, types) in jobs {
            let mut value = value.clone();
            for (ty, unit) in types { value = self.plan_type(value, ty, *unit, pending, choices)?; }
            values.push(value);
        }
        Ok(values)
    }

    /// Check `value` against `ty` without changing any container: scalars are
    /// returned converted, and container restrictions to apply go to `pending`.
    fn plan_type(&mut self, value: Value, ty: &str, unit: usize, pending: &mut Vec<Restriction>,
                 choices: &mut Choices) -> RResult<Value> {
        if let Some(inner) = ty.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            let options = split_types(inner);
            // Prefer the actual numeric kind before trying compatible conversions.
            if let Some(kind) = value.num_kind() {
                if options.iter().any(|o| o == kind.name()) { return self.plan_type(value, kind.name(), unit, pending, choices); }
            }
            // Plans change nothing, so a failed alternative leaves nothing behind.
            // An alternative only matches if it can be applied together with what
            // is already planned (restrictions may meet on shared containers).
            // The choice is recorded so a later conflict can retry the next option.
            let point = choices.taken.len();
            let start = choices.forced.get(point).copied().unwrap_or(0);
            for (index, option) in options.iter().enumerate().skip(start) {
                choices.taken.push((index, options.len()));
                let mut attempt = Vec::new();
                if let Ok(result) = self.plan_type(value.clone(), option, unit, &mut attempt, choices) {
                    let compatible = attempt.is_empty() || {
                        let mut combined = pending.clone();
                        combined.extend(attempt.iter().cloned());
                        self.stage(&combined).is_ok()
                    };
                    if compatible {
                        pending.append(&mut attempt);
                        return Ok(result);
                    }
                }
                choices.taken.truncate(point);
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
            "any" => Ok(value),
            // Parameter types for passing code around: func apply(Function f).
            "Function" => match &value {
                Value::Func(f) if !matches!(f.as_ref(), Callable::Family(_)) => Ok(value),
                _ => type_err(format!("expected a function, got {}", value.type_name())),
            },
            "Family" => match &value {
                Value::Func(f) if matches!(f.as_ref(), Callable::Family(_)) => Ok(value),
                _ => type_err(format!("expected a family, got {}", value.type_name())),
            },
            _ => match collection_type(ty) {
                Some((kind, args)) => self.plan_collection(value, ty, kind, args, unit, pending, choices),
                None => self.instance_of(value, ty, unit),
            },
        }
    }

    fn plan_collection(&mut self, value: Value, ty: &str, kind: &str, args: Option<&str>, unit: usize,
                       pending: &mut Vec<Restriction>, choices: &mut Choices) -> RResult<Value> {
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
        let parts: Vec<(Value, usize)> = match &value {
            Value::Array(a) => a.items.borrow().iter().map(|v| (v.clone(), 0)).collect(),
            Value::Set(s) => s.items.borrow().values().map(|v| (v.clone(), 0)).collect(),
            Value::Map(m) => m.items.borrow().values().flat_map(|(k, v)| [(k.clone(), 0), (v.clone(), 1)]).collect(),
            Value::Pair(p) => vec![(p.first.borrow().clone(), 0), (p.second.borrow().clone(), 1)],
            _ => unreachable!(),
        };
        for (item, part) in parts {
            let converted = self.plan_type(item.clone(), &args[part], unit, pending, choices)?;
            // Converting a fixed container's numeric kind would be a mutation.
            if frozen && converted.num_kind() != item.num_kind() { mutable(meta)?; }
        }
        pending.push(Restriction { target: value.clone(), args, unit });
        Ok(value)
    }

    /// Compute the complete outcome of a plan without changing anything: for
    /// every container it restricts (grouped by identity, so aliases combine),
    /// the full restriction list and the converted contents. Fails if any
    /// element cannot satisfy every restriction on its container, or if a fixed
    /// container's numeric kinds would change.
    fn stage(&mut self, pending: &[Restriction]) -> RResult<Vec<Staged>> {
        let mut order: Vec<usize> = Vec::new();
        let mut groups: HashMap<usize, (Value, Vec<(Rc<[String]>, usize)>)> = HashMap::new();
        for restriction in pending {
            let id = restriction.target.object_id().unwrap();
            let entry = groups.entry(id).or_insert_with(|| {
                order.push(id);
                let existing = restriction.target.meta().unwrap().restrictions.borrow().clone();
                (restriction.target.clone(), existing)
            });
            entry.1.push((restriction.args.clone(), restriction.unit));
        }
        let mut staged = Vec::with_capacity(order.len());
        for id in order {
            let (target, restrictions) = groups.remove(&id).unwrap();
            let frozen = target.meta().unwrap().frozen.get();
            let convert = |this: &mut Self, value: Value, part: usize| -> RResult<Value> {
                let mut converted = value.clone();
                for (args, unit) in &restrictions {
                    converted = this.plan_type(converted, &args[part], *unit, &mut Vec::new(), &mut Choices::default())?;
                }
                // The result must satisfy every restriction as it is: int and longint
                // restrictions on one container, for example, cannot both hold.
                for (args, unit) in &restrictions {
                    let again = this.plan_type(converted.clone(), &args[part], *unit, &mut Vec::new(), &mut Choices::default())?;
                    if again.num_kind() != converted.num_kind() {
                        return type_err(format!("value cannot satisfy both {} and the container's other restrictions", args[part]));
                    }
                }
                if frozen && converted.num_kind() != value.num_kind() { mutable(target.meta().unwrap())?; }
                Ok(converted)
            };
            let contents = match &target {
                Value::Array(a) => {
                    let items = a.items.borrow().clone();
                    Contents::Array(items.into_iter().map(|v| convert(self, v, 0)).collect::<RResult<_>>()?)
                }
                Value::Set(s) => {
                    let items: Vec<Value> = s.items.borrow().values().cloned().collect();
                    let mut rebuilt = IndexMap::new();
                    for v in items { let v = convert(self, v, 0)?; rebuilt.entry(Key::of(&v)).or_insert(v); }
                    Contents::Set(rebuilt)
                }
                Value::Map(m) => {
                    let entries: Vec<(Value, Value)> = m.items.borrow().values().cloned().collect();
                    let mut rebuilt = IndexMap::new();
                    for (k, v) in entries {
                        let (k, v) = (convert(self, k, 0)?, convert(self, v, 1)?);
                        rebuilt.insert(Key::of(&k), (k, v));
                    }
                    Contents::Map(rebuilt)
                }
                Value::Pair(p) => {
                    let (first, second) = (p.first.borrow().clone(), p.second.borrow().clone());
                    Contents::Pair(convert(self, first, 0)?, convert(self, second, 1)?)
                }
                _ => unreachable!("restrictions only target containers"),
            };
            staged.push(Staged { target, restrictions, contents });
        }
        Ok(staged)
    }

    /// Write staged outcomes. Nothing here can fail, so a plan applies entirely.
    fn commit(staged: Vec<Staged>) {
        for Staged { target, restrictions, contents } in staged {
            *target.meta().unwrap().restrictions.borrow_mut() = restrictions;
            match (&target, contents) {
                (Value::Array(a), Contents::Array(items)) => *a.items.borrow_mut() = items,
                (Value::Set(s), Contents::Set(items)) => *s.items.borrow_mut() = items,
                (Value::Map(m), Contents::Map(items)) => *m.items.borrow_mut() = items,
                (Value::Pair(p), Contents::Pair(first, second)) => {
                    *p.first.borrow_mut() = first;
                    *p.second.borrow_mut() = second;
                }
                _ => unreachable!("staged contents match their container"),
            }
        }
    }

    /// A family annotation: the family declared in `unit` (or `alias.Family` from
    /// a package it imports), matching that family or any family inheriting from it.
    fn instance_of(&self, value: Value, ty: &str, unit: usize) -> RResult<Value> {
        let Some(family) = self.family_named(ty, unit) else { return type_err(format!("unknown type {ty}")) };
        match &value {
            Value::Instance(instance) if descends(&instance.family, &family) => Ok(value),
            _ => type_err(format!("expected {ty}")),
        }
    }

    /// The family a type name refers to in `unit`: declared there, a built-in
    /// error family, or `alias.Family` from an imported package.
    fn family_named(&self, ty: &str, unit: usize) -> Option<Rc<Family>> {
        match ty.split_once('.') {
            Some((alias, name)) => match self.units[unit].imports.borrow().get(alias) {
                Some(Value::Package(package)) => match package.members.get(name) {
                    Some(Value::Func(f)) => match f.as_ref() { Callable::Family(family) => Some(family.clone()), _ => None },
                    _ => None,
                },
                _ => None,
            },
            None => self.units[unit].families.get(ty).or_else(|| self.error_families.get(ty)).cloned(),
        }
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
                outside: &dyn Fn(&str) -> Option<Rc<Family>>, unit: usize, visiting: &mut Vec<String>) -> RResult<Rc<Family>> {
    if let Some(family) = built.get(name) { return Ok(family.clone()); }
    if visiting.iter().any(|v| v == name) { return type_err(format!("family '{name}' inherits from itself")); }
    let decl = decls[name];
    visiting.push(name.to_string());
    let parent = match &decl.extends {
        Some(parent) if decls.contains_key(parent) => Some(build_family(parent, decls, built, outside, unit, visiting)?),
        Some(parent) => match outside(parent) {
            Some(family) => Some(family),
            None => return type_err(format!("family '{name}' extends unknown family '{parent}'")),
        },
        None => None,
    };
    visiting.pop();
    let error_kind = parent.as_ref().and_then(|p| p.error_kind);
    // Each method keeps a weak link to its family, for access checks.
    let family = Rc::new_cyclic(|owner| {
        let methods = decl.methods.iter()
            .map(|m| (m.name.clone(), Rc::new(Function { decl: std::sync::Arc::new(m.clone()), package: unit, owner: Some(owner.clone()) })))
            .collect();
        Family { name: name.to_string(), parent, methods, error_kind }
    });
    built.insert(name.to_string(), family.clone());
    Ok(family)
}

/// Enforce `@Decorator.private` (callers must be methods of the declaring
/// family) and `@Decorator.subclass` (or of a family inheriting from it).
fn check_access(method: &Rc<Function>, caller: Option<&Rc<Family>>) -> RResult<()> {
    let access = method.decl.access;
    if access == Access::Public { return Ok(()); }
    let Some(owner) = method.owner.as_ref().and_then(|owner| owner.upgrade()) else { return Ok(()) };
    let allowed = match (access, caller) {
        (Access::Private, Some(caller)) => Rc::ptr_eq(caller, &owner),
        (Access::Subclass, Some(caller)) => descends(caller, &owner),
        _ => false,
    };
    if allowed { return Ok(()); }
    let name = &method.decl.name;
    match access {
        Access::Private => type_err(format!("{name} is private to {}", owner.name)),
        _ => type_err(format!("{name} is only available to {} and families inheriting from it", owner.name)),
    }
}

/// `Error` and the built-in kinds that inherit from it.
fn error_families() -> HashMap<&'static str, Rc<Family>> {
    let base = Rc::new(Family { name: "Error".into(), parent: None, methods: HashMap::new(), error_kind: Some(ErrorKind::Error) });
    let mut families = HashMap::from([("Error", base.clone())]);
    for kind in &ErrorKind::ALL[1..] {
        let family = Family { name: kind.name().into(), parent: Some(base.clone()), methods: HashMap::new(), error_kind: Some(*kind) };
        families.insert(kind.name(), Rc::new(family));
    }
    families
}

/// Whether `family` is `ancestor` or inherits from it.
fn descends(family: &Rc<Family>, ancestor: &Rc<Family>) -> bool {
    let mut current = Some(family.clone());
    while let Some(candidate) = current {
        if Rc::ptr_eq(&candidate, ancestor) { return true; }
        current = candidate.parent.clone();
    }
    false
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
                        .ok_or_else(|| RuntimeError { kind: ErrorKind::RangeError, message: "longint overflow".into(), ..Default::default() })?;
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
    use crate::package::native::rs_string as shared;
    let no_args = |method: &str| if args.is_empty() { Ok(()) } else { type_err(format!("string.{method}() takes no arguments")) };
    let count = |n: usize| if args.len() == n { Ok(()) } else { type_err(format!("string.{name}() takes {n} argument(s), got {}", args.len())) };
    let strings = |parts: Vec<&str>| Value::array(parts.into_iter().map(Value::str).collect());
    Ok(Some(match name {
        "len" => { no_args("len")?; Value::Int(text.graphemes(true).count() as i64) }
        "reverse" => { no_args("reverse")?; Value::str(&text.graphemes(true).rev().collect::<String>()) }
        "slice" | "lenslice" => {
            let graphemes: Vec<&str> = text.graphemes(true).collect();
            Value::str(&slice_items(&graphemes, args, name == "lenslice")?.concat())
        }
        "iget" => { count(1)?; character_at(text, &args[0])? }
        // split() splits at runs of whitespace; split(sep) at each separator.
        "split" => match args.len() {
            0 => strings(text.split_whitespace().collect()),
            1 => strings(shared::split(text, text_arg(args, 0, name)?)?),
            n => return type_err(format!("string.split() takes 0 or 1 argument(s), got {n}")),
        },
        "lines" => { no_args("lines")?; strings(text.lines().collect()) }
        // strip() removes whitespace; strip(chars) removes any of those characters.
        "strip" | "lstrip" | "rstrip" => {
            let (start, end) = (name != "rstrip", name != "lstrip");
            match args.len() {
                0 => Value::str(match (start, end) {
                    (true, true) => text.trim(),
                    (true, false) => text.trim_start(),
                    _ => text.trim_end(),
                }),
                1 => Value::str(&strip_characters(text, text_arg(args, 0, name)?, start, end)),
                n => return type_err(format!("string.{name}() takes 0 or 1 argument(s), got {n}")),
            }
        }
        "upper" => { no_args("upper")?; Value::str(&text.to_uppercase()) }
        "lower" => { no_args("lower")?; Value::str(&text.to_lowercase()) }
        "replace" => { count(2)?; Value::str(&shared::replace(text, text_arg(args, 0, name)?, text_arg(args, 1, name)?)?) }
        "find" | "rfind" => {
            count(1)?;
            Value::Int(shared::find(text, text_arg(args, 0, name)?, name == "rfind").map_or(-1, |i| i as i64))
        }
        "contains" => { count(1)?; Value::Bool(shared::find(text, text_arg(args, 0, name)?, false).is_some()) }
        "starts_with" => { count(1)?; Value::Bool(shared::starts_with(text, text_arg(args, 0, name)?)) }
        "ends_with" => { count(1)?; Value::Bool(shared::ends_with(text, text_arg(args, 0, name)?)) }
        _ => return type_err(format!("string has no method '{name}'")),
    }))
}

fn text_arg<'a>(args: &'a [Value], i: usize, method: &str) -> RResult<&'a str> {
    match &args[i] {
        Value::Str(text) => Ok(text),
        other => type_err(format!("string.{method}() expects a string, got {}", other.type_name())),
    }
}

/// `text` without any of the characters in `chars` at the start and/or end.
fn strip_characters(text: &str, chars: &str, start: bool, end: bool) -> String {
    let remove: std::collections::HashSet<&str> = chars.graphemes(true).collect();
    let all: Vec<&str> = text.graphemes(true).collect();
    let mut kept = &all[..];
    while start && kept.first().is_some_and(|c| remove.contains(c)) { kept = &kept[1..]; }
    while end && kept.last().is_some_and(|c| remove.contains(c)) { kept = &kept[..kept.len() - 1]; }
    kept.concat()
}

/// "a" or "an", for the kind names in messages.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

/// Character `index` of `text`, counting whole characters from 0.
fn character_at(text: &str, index: &Value) -> RResult<Value> {
    let characters: Vec<&str> = text.graphemes(true).collect();
    Ok(Value::str(characters[whole_index(index, characters.len(), "string")?]))
}

/// A position from 0 to `len - 1`; anything else is an OutOfBoundsError, and
/// a value that is not a whole number a TypeError.
fn whole_index(value: &Value, len: usize, what: &str) -> RResult<usize> {
    let index = match value {
        Value::Int(i) | Value::Long(i) => *i,
        Value::Float(f) if f.fract() == 0.0 && f.is_finite() => *f as i64,
        other => return type_err(format!("{what} indexes must be whole numbers, got {}", other.type_name())),
    };
    if index < 0 || index as usize >= len {
        let unit = if what == "string" { "characters" } else { "elements" };
        return err(ErrorKind::OutOfBoundsError, format!("index {index} is out of range for {} {what} of {len} {unit}", article(what)));
    }
    Ok(index as usize)
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
    let copy = match value {
        Value::Array(_) => Value::new_array(Vec::new(), meta),
        Value::Set(_) => Value::new_set(IndexMap::new(), meta),
        Value::Map(_) => Value::new_map(IndexMap::new(), meta),
        Value::Pair(_) => Value::new_pair(Value::Null, Value::Null, meta),
        Value::Instance(o) => Value::new_instance(o.family.clone(), meta),
        _ => return value.clone(),
    };
    // Register the copy before its contents, so cycles and shared parts map to it.
    seen.insert(id, copy.clone());
    match (value, &copy) {
        (Value::Array(a), Value::Array(c)) => {
            let items: Vec<Value> = a.items.borrow().clone();
            let items = items.iter().map(|v| deep_copy(v, seen)).collect();
            *c.items.borrow_mut() = items;
        }
        (Value::Set(s), Value::Set(c)) => {
            let items: Vec<Value> = s.items.borrow().values().cloned().collect();
            for v in items {
                let v = deep_copy(&v, seen);
                c.items.borrow_mut().entry(Key::of(&v)).or_insert(v);
            }
        }
        (Value::Map(m), Value::Map(c)) => {
            let entries: Vec<(Value, Value)> = m.items.borrow().values().cloned().collect();
            for (k, v) in entries {
                let (k, v) = (deep_copy(&k, seen), deep_copy(&v, seen));
                c.items.borrow_mut().insert(Key::of(&k), (k, v));
            }
        }
        (Value::Pair(p), Value::Pair(c)) => {
            let (first, second) = (p.first.borrow().clone(), p.second.borrow().clone());
            *c.first.borrow_mut() = deep_copy(&first, seen);
            *c.second.borrow_mut() = deep_copy(&second, seen);
        }
        (Value::Instance(o), Value::Instance(c)) => {
            let fields: Vec<(Rc<str>, Value)> = o.fields.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (k, v) in fields {
                let v = deep_copy(&v, seen);
                c.fields.borrow_mut().insert(k, v);
            }
        }
        _ => unreachable!("copy has the same kind as the original"),
    }
    copy
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
