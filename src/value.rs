//! Runtime values of the Marslang interpreter.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::ast::FuncDecl;

pub(crate) const MAX_SAFE: i64 = 9_007_199_254_740_991;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    /// `int` kind. Literals may exceed 32 bits (up to 2^53-1), but checked
    /// arithmetic and `int` annotations enforce the signed 32-bit range.
    Int(i64),
    /// `longint` kind: signed 64-bit.
    Long(i64),
    /// `float` kind, retained even for integral values such as `1.0`.
    Float(f64),
    Str(Rc<str>),
    Array(Rc<Array>),
    Set(Rc<Set>),
    Map(Rc<Map>),
    Pair(Rc<Pair>),
    Instance(Rc<Instance>),
    Func(Rc<Callable>),
    Package(Rc<Package>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumKind { Int, Long, Float }

impl NumKind {
    pub fn name(self) -> &'static str {
        match self { NumKind::Int => "int", NumKind::Long => "longint", NumKind::Float => "float" }
    }
}

impl Value {
    pub fn str(text: &str) -> Value { Value::Str(Rc::from(text)) }

    pub fn num_kind(&self) -> Option<NumKind> {
        match self {
            Value::Int(_) => Some(NumKind::Int),
            Value::Long(_) => Some(NumKind::Long),
            Value::Float(_) => Some(NumKind::Float),
            _ => None,
        }
    }

    /// Numeric value as a double, for int and float kinds.
    pub fn as_f64(&self) -> Option<f64> {
        match self { Value::Int(i) => Some(*i as f64), Value::Float(f) => Some(*f), _ => None }
    }

    pub fn truth(&self) -> bool {
        match self {
            Value::Null | Value::Bool(false) => false,
            Value::Int(0) | Value::Long(0) => false,
            Value::Float(f) => *f != 0.0,
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null", Value::Bool(_) => "bool", Value::Int(_) => "int",
            Value::Long(_) => "longint", Value::Float(_) => "float", Value::Str(_) => "string",
            Value::Array(_) => "array", Value::Set(_) => "set", Value::Map(_) => "map",
            Value::Pair(_) => "pair", Value::Instance(_) => "instance", Value::Func(_) => "function",
            Value::Package(_) => "package",
        }
    }

    /// Identity of a heap object, used for container equality, keys, and copying.
    pub fn object_id(&self) -> Option<usize> {
        Some(match self {
            Value::Array(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Set(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Map(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Pair(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Instance(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Package(o) => Rc::as_ptr(o) as *const () as usize,
            Value::Func(f) => return f.identity(),
            _ => return None,
        })
    }

    /// Container metadata (fixed state and element restrictions).
    pub fn meta(&self) -> Option<&Meta> {
        match self {
            Value::Array(o) => Some(&o.meta), Value::Set(o) => Some(&o.meta),
            Value::Map(o) => Some(&o.meta), Value::Pair(o) => Some(&o.meta),
            Value::Instance(o) => Some(&o.meta), _ => None,
        }
    }
}

/// Equality: numbers compare by exact value across int, longint, and float;
/// strings by contents; objects by identity.
pub fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        _ if a.num_kind().is_some() && b.num_kind().is_some() => compare_numbers(a, b) == Some(std::cmp::Ordering::Equal),
        _ => match (a.object_id(), b.object_id()) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

/// Exact ordering of two numbers of any kinds; `None` when either is NaN.
pub fn compare_numbers(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(x) | Value::Long(x), Value::Int(y) | Value::Long(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Int(i) | Value::Long(i), Value::Float(f)) => compare_integer_float(*i, *f),
        (Value::Float(f), Value::Int(i) | Value::Long(i)) => compare_integer_float(*i, *f).map(|o| o.reverse()),
        _ => None,
    }
}

fn compare_integer_float(i: i64, f: f64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if f.is_nan() { return None; }
    // 2^63 is exactly representable; every i64 lies in [-2^63, 2^63).
    if f >= 9_223_372_036_854_775_808.0 { return Some(Ordering::Less); }
    if f < -9_223_372_036_854_775_808.0 { return Some(Ordering::Greater); }
    let whole = f.floor();
    Some(i.cmp(&(whole as i64)).then(if f > whole { Ordering::Less } else { Ordering::Equal }))
}

/// Hash key consistent with `equal`: equal numbers of any kind share a key,
/// -0 equals 0, and NaN keys equal each other.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Key { Null, Bool(bool), Integer(i64), Num(u64), Str(Rc<str>), Obj(usize) }

impl Key {
    pub fn of(value: &Value) -> Key {
        match value {
            Value::Null => Key::Null,
            Value::Bool(b) => Key::Bool(*b),
            Value::Int(i) | Value::Long(i) => Key::Integer(*i),
            Value::Float(f) if f.fract() == 0.0 && *f >= -9_223_372_036_854_775_808.0 && *f < 9_223_372_036_854_775_808.0 => {
                Key::Integer(*f as i64)
            }
            Value::Float(f) => Key::Num(if f.is_nan() { f64::NAN.to_bits() } else { f.to_bits() }),
            Value::Str(s) => Key::Str(s.clone()),
            other => Key::Obj(other.object_id().unwrap_or(0)),
        }
    }
}

#[derive(Default)]
pub struct Meta {
    pub frozen: Cell<bool>,
    /// Each entry is one applied collection annotation's type arguments, with
    /// the unit (program or package) whose names they refer to.
    pub restrictions: RefCell<Vec<(Rc<[String]>, usize)>>,
}

impl Meta {
    pub fn inherit(&self) -> Meta {
        Meta { frozen: Cell::new(false), restrictions: RefCell::new(self.restrictions.borrow().clone()) }
    }
}

pub struct Array { pub items: RefCell<Vec<Value>>, pub meta: Meta }
pub struct Set { pub items: RefCell<IndexMap<Key, Value>>, pub meta: Meta }
pub struct Map { pub items: RefCell<IndexMap<Key, (Value, Value)>>, pub meta: Meta }
pub struct Pair { pub first: RefCell<Value>, pub second: RefCell<Value>, pub meta: Meta }
pub struct Instance { pub family: Rc<Family>, pub fields: RefCell<IndexMap<Rc<str>, Value>>, pub meta: Meta }

/// Constructors for values that can hold other values. They register the new
/// object with the cycle collector (`crate::gc`), so always create them here.
impl Value {
    fn tracked(self) -> Value {
        crate::gc::track(&self);
        self
    }
    pub fn array(items: Vec<Value>) -> Value { Value::new_array(items, Meta::default()) }
    pub fn new_array(items: Vec<Value>, meta: Meta) -> Value {
        Value::Array(Rc::new(Array { items: RefCell::new(items), meta })).tracked()
    }
    pub fn new_set(items: IndexMap<Key, Value>, meta: Meta) -> Value {
        Value::Set(Rc::new(Set { items: RefCell::new(items), meta })).tracked()
    }
    pub fn new_map(items: IndexMap<Key, (Value, Value)>, meta: Meta) -> Value {
        Value::Map(Rc::new(Map { items: RefCell::new(items), meta })).tracked()
    }
    pub fn pair(first: Value, second: Value) -> Value { Value::new_pair(first, second, Meta::default()) }
    pub fn new_pair(first: Value, second: Value, meta: Meta) -> Value {
        Value::Pair(Rc::new(Pair { first: RefCell::new(first), second: RefCell::new(second), meta })).tracked()
    }
    pub fn new_instance(family: Rc<Family>, meta: Meta) -> Value {
        Value::Instance(Rc::new(Instance { family, fields: RefCell::new(IndexMap::new()), meta })).tracked()
    }
    /// A method bound to its receiver.
    pub fn method(receiver: Value, function: Rc<Function>) -> Value {
        Value::Func(Rc::new(Callable::Method(receiver, function))).tracked()
    }
    /// A function closing over the frame it was made in, and `me` there.
    pub fn closure(function: Rc<Function>, frame: Option<Rc<Frame>>, me: Option<Value>) -> Value {
        if let Some(frame) = &frame { crate::gc::track_frame(frame); }
        Value::Func(Rc::new(Callable::Closure(function, frame, me))).tracked()
    }
    pub fn package(name: String, members: IndexMap<String, Value>) -> Value {
        Value::Package(Rc::new(Package { name, members })).tracked()
    }
}

/// The local variables of one call. A closure keeps the frame it was made in,
/// so frames are shared, and each has the frame of the function it is nested in
/// as its parent. Names are unique after resolving, so a lookup walks outward
/// without any risk of finding a different variable of the same name.
pub struct Frame {
    pub values: RefCell<HashMap<String, Value>>,
    pub parent: RefCell<Option<Rc<Frame>>>,
    /// Registered with the cycle collector: done once a closure captures it.
    pub tracked: Cell<bool>,
}

impl Frame {
    pub fn new(values: HashMap<String, Value>, parent: Option<Rc<Frame>>) -> Rc<Frame> {
        Rc::new(Frame { values: RefCell::new(values), parent: RefCell::new(parent), tracked: Cell::new(false) })
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.values.borrow().get(name) { return Some(value.clone()); }
        self.parent.borrow().as_ref().and_then(|parent| parent.lookup(name))
    }

    /// Set `name` in the nearest frame that has it, or hand the value back when none does.
    pub fn assign(&self, name: &str, value: Value) -> Result<(), Value> {
        if let Some(slot) = self.values.borrow_mut().get_mut(name) {
            *slot = value;
            return Ok(());
        }
        match self.parent.borrow().as_ref() {
            Some(parent) => parent.assign(name, value),
            None => Err(value),
        }
    }
}

/// A user function or method, bound to the package whose globals it reads.
pub struct Function {
    pub decl: std::sync::Arc<FuncDecl>,
    pub package: usize,
    /// The family declaring this method; `None` for a free function.
    pub owner: Option<std::rc::Weak<Family>>,
}

pub struct Family {
    pub name: String,
    pub parent: Option<Rc<Family>>,
    pub methods: HashMap<String, Rc<Function>>,
    /// For `Error` and every family inheriting from it: the nearest built-in
    /// error kind. `None` for families that are not errors.
    pub error_kind: Option<ErrorKind>,
}

impl Family {
    pub fn method(&self, name: &str) -> Option<Rc<Function>> {
        self.methods.get(name).cloned().or_else(|| self.parent.as_ref()?.method(name))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Builtin {
    Out, Slout, In, Inln, Arr, Set, Pair, Map, Int, Long, Float, String, Err, LastErr,
}

impl Builtin {
    pub fn lookup(name: &str) -> Option<Builtin> {
        Some(match name {
            "out" => Builtin::Out, "slout" => Builtin::Slout, "in" => Builtin::In, "inln" => Builtin::Inln,
            "arr" | "a" => Builtin::Arr, "set" | "s" => Builtin::Set, "pair" | "p" => Builtin::Pair,
            "map" | "dict" => Builtin::Map, "int" => Builtin::Int, "longint" => Builtin::Long,
            "float" => Builtin::Float, "string" => Builtin::String,
            "err" => Builtin::Err, "lasterr" => Builtin::LastErr,
            _ => return None,
        })
    }
}

pub enum Callable {
    Func(Rc<Function>),
    Method(Value, Rc<Function>),
    /// An anonymous or nested function: the frame it closes over (none at top
    /// level) and `me`, when it was made inside a method.
    Closure(Rc<Function>, Option<Rc<Frame>>, Option<Value>),
    Family(Rc<Family>),
    Builtin(Builtin),
    Native(Rc<NativeFn>),
}

/// A function implemented in Rust by a native package (`std/rs/*.rs`).
pub struct NativeFn { pub name: String, pub arity: usize, pub imp: fn(&[Value]) -> RResult<Value> }

/// Builder for native packages: `NativePackage::new("rs.x").function(...).build()`.
pub struct NativePackage { name: String, members: IndexMap<String, Value> }

impl NativePackage {
    pub fn new(name: &str) -> Self { NativePackage { name: name.to_string(), members: IndexMap::new() } }

    pub fn function(mut self, name: &str, arity: usize, imp: fn(&[Value]) -> RResult<Value>) -> Self {
        let function = NativeFn { name: format!("{}.{name}", self.name), arity, imp };
        self.members.insert(name.to_string(), Value::Func(Rc::new(Callable::Native(Rc::new(function)))));
        self
    }

    pub fn value(mut self, name: &str, value: Value) -> Self {
        self.members.insert(name.to_string(), value);
        self
    }

    pub fn build(self) -> Value { Value::package(self.name, self.members) }
}

/// `**` for floats: like `powf`, except that a NaN exponent and `(±1) ** ±inf`
/// produce NaN.
pub fn float_pow(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) { f64::NAN } else { base.powf(exponent) }
}

impl Callable {
    fn identity(&self) -> Option<usize> {
        Some(match self {
            Callable::Func(f) => Rc::as_ptr(f) as usize,
            Callable::Family(f) => Rc::as_ptr(f) as usize,
            Callable::Native(f) => Rc::as_ptr(f) as usize,
            Callable::Builtin(b) => *b as usize,
            Callable::Method(..) | Callable::Closure(..) => self as *const Callable as usize,
        })
    }

    pub fn name(&self) -> String {
        match self {
            Callable::Func(f) | Callable::Method(_, f) | Callable::Closure(f, ..) => f.decl.name.clone(),
            Callable::Family(f) => f.name.clone(),
            Callable::Builtin(b) => format!("{b:?}").to_lowercase(),
            Callable::Native(f) => f.name.clone(),
        }
    }
}

/// A loaded package (`takepkg`): read-only named members.
pub struct Package { pub name: String, pub members: IndexMap<String, Value> }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind { Error, TypeError, RangeError, OutOfBoundsError, SyntaxError }

impl ErrorKind {
    pub const ALL: [ErrorKind; 5] = [ErrorKind::Error, ErrorKind::TypeError, ErrorKind::RangeError,
        ErrorKind::OutOfBoundsError, ErrorKind::SyntaxError];
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::Error => "Error", ErrorKind::TypeError => "TypeError", ErrorKind::RangeError => "RangeError",
            ErrorKind::OutOfBoundsError => "OutOfBoundsError", ErrorKind::SyntaxError => "SyntaxError",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeError {
    /// The built-in kind; for a user error family, the kind it inherits from.
    pub kind: ErrorKind,
    pub message: String,
    /// Name of the user error family raised with `err`, such as `ParseError`.
    pub family: Option<String>,
    /// Identifies the error value raised by `err` while it propagates (0 otherwise).
    pub(crate) serial: u64,
    /// Set by `std.cli.exit(code)`: not an error but a request to stop with
    /// that status. No handler catches it; `then` blocks still run.
    pub exit: Option<i32>,
}

/// Stop the program with an exit status (`std.cli.exit`).
pub fn exit_request(code: i32) -> RuntimeError {
    RuntimeError { kind: ErrorKind::Error, message: format!("exit {code}"), exit: Some(code), ..Default::default() }
}

thread_local! {
    /// The command line of the program running on this thread: the path of its
    /// file as given, then the arguments after it. Each run has its own thread.
    static COMMAND_LINE: RefCell<(String, Vec<String>)> = const { RefCell::new((String::new(), Vec::new())) };
}

pub(crate) fn set_command_line(program: String, args: Vec<String>) {
    COMMAND_LINE.with(|line| *line.borrow_mut() = (program, args));
}

pub fn program_path() -> String {
    COMMAND_LINE.with(|line| line.borrow().0.clone())
}

pub fn program_args() -> Vec<String> {
    COMMAND_LINE.with(|line| line.borrow().1.clone())
}

impl RuntimeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        RuntimeError { kind, message: message.into(), ..Default::default() }
    }
}

impl Default for ErrorKind {
    fn default() -> Self { ErrorKind::Error }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = self.family.as_deref().unwrap_or(self.kind.name());
        write!(f, "{name}: {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub type RResult<T> = Result<T, RuntimeError>;

pub fn err<T>(kind: ErrorKind, message: impl Into<String>) -> RResult<T> {
    Err(RuntimeError { kind, message: message.into(), ..Default::default() })
}
pub fn type_err<T>(message: impl Into<String>) -> RResult<T> { err(ErrorKind::TypeError, message) }
pub fn range_err<T>(message: impl Into<String>) -> RResult<T> { err(ErrorKind::RangeError, message) }

/// Format a float the way the language prints numbers: shortest round-trip
/// digits, plain notation for exponents in [-7, 21), scientific otherwise.
pub fn format_float(value: f64) -> String {
    if value.is_nan() { return "NaN".into(); }
    if value.is_infinite() { return if value > 0.0 { "Infinity".into() } else { "-Infinity".into() }; }
    if value == 0.0 { return "0".into(); }
    let sci = format!("{:e}", value.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().unwrap() + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        if k == 1 { format!("{digits}e{sign}{}", e.abs()) }
        else { format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs()) }
    };
    if value < 0.0 { format!("-{body}") } else { body }
}

/// `string(value)` conversion; also used by `slout`.
pub fn to_display_string(value: &Value) -> String {
    match value {
        Value::Str(s) => s.to_string(),
        Value::Float(f) => format_float(*f),
        other => inspect(other),
    }
}

/// `out` formatting: like `string`, but negative zero prints as `-0`.
pub fn to_output_string(value: &Value) -> String {
    match value {
        Value::Float(f) if *f == 0.0 && f.is_sign_negative() => "-0".into(),
        other => to_display_string(other),
    }
}

/// Human-readable rendering of any value. Container formatting is not a stable
/// serialization format.
pub fn inspect(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out, &mut Vec::new(), false);
    out
}

fn write_value(value: &Value, out: &mut String, seen: &mut Vec<usize>, nested: bool) {
    use std::fmt::Write;
    if let Some(id) = value.object_id() {
        if matches!(value, Value::Array(_) | Value::Set(_) | Value::Map(_) | Value::Pair(_) | Value::Instance(_)) {
            if seen.contains(&id) { out.push_str("<cycle>"); return; }
            seen.push(id);
        }
    }
    let list = |items: Vec<Value>, out: &mut String, seen: &mut Vec<usize>| {
        for (i, item) in items.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            write_value(item, out, seen, true);
        }
    };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => { let _ = write!(out, "{b}"); }
        Value::Int(i) | Value::Long(i) => { let _ = write!(out, "{i}"); }
        Value::Float(f) if *f == 0.0 && f.is_sign_negative() => out.push_str("-0"),
        Value::Float(f) => out.push_str(&format_float(*f)),
        Value::Str(s) if nested => { let _ = write!(out, "{:?}", s.as_ref()); }
        Value::Str(s) => out.push_str(s),
        Value::Array(a) => {
            out.push('[');
            list(a.items.borrow().clone(), out, seen);
            out.push(']');
        }
        Value::Set(s) => {
            out.push_str("set(");
            list(s.items.borrow().values().cloned().collect(), out, seen);
            out.push(')');
        }
        Value::Map(m) => {
            out.push('{');
            let entries: Vec<_> = m.items.borrow().values().cloned().collect();
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_value(k, out, seen, true);
                out.push_str(": ");
                write_value(v, out, seen, true);
            }
            out.push('}');
        }
        Value::Pair(p) => {
            out.push_str("pair(");
            let (first, second) = (p.first.borrow().clone(), p.second.borrow().clone());
            list(vec![first, second], out, seen);
            out.push(')');
        }
        Value::Instance(o) if o.family.error_kind.is_some() => {
            // Errors print as `Name: message`. The message is formatted with the same
            // visited set, since it can be any value, including the error itself.
            let _ = write!(out, "{}: ", o.family.name);
            let message = o.fields.borrow().get("message").cloned();
            if let Some(message) = message { write_value(&message, out, seen, false); }
        }
        Value::Instance(o) => {
            out.push_str(&o.family.name);
            let fields: Vec<_> = o.fields.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            if fields.is_empty() { out.push_str(" {}"); } else {
                out.push_str(" { ");
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    let _ = write!(out, "{k}: ");
                    write_value(v, out, seen, true);
                }
                out.push_str(" }");
            }
        }
        Value::Func(f) => match f.as_ref() {
            Callable::Family(family) => { let _ = write!(out, "<family {}>", family.name); }
            _ => { let _ = write!(out, "<func {}>", f.name()); }
        },
        Value::Package(p) => { let _ = write!(out, "<package {}>", p.name); }
    }
    if matches!(value, Value::Array(_) | Value::Set(_) | Value::Map(_) | Value::Pair(_) | Value::Instance(_)) {
        seen.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::format_float;

    #[test]
    fn an_error_whose_message_contains_itself_prints_without_recursing() {
        use super::*;
        // Runs on the test thread's ordinary stack, where unbounded recursion would abort.
        let family = Rc::new(Family { name: "Error".into(), parent: None, methods: HashMap::new(), error_kind: Some(ErrorKind::Error) });
        let error = Value::new_instance(family, Meta::default());
        let Value::Instance(o) = &error else { unreachable!() };
        o.fields.borrow_mut().insert(Rc::from("message"), error.clone());
        assert_eq!(to_display_string(&error), "Error: <cycle>");
        let list = Value::array(vec![error.clone()]);
        o.fields.borrow_mut().insert(Rc::from("message"), list);
        assert_eq!(to_display_string(&error), "Error: [<cycle>]");
        o.fields.borrow_mut().clear();
    }

    #[test]
    fn floats_print_like_the_bootstrap_backend() {
        for (value, text) in [
            (1.0, "1"), (78.53975, "78.53975"), (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"), (1e20, "100000000000000000000"), (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"), (-2.5, "-2.5"), (18014398509481982.0, "18014398509481982"),
            (f64::INFINITY, "Infinity"), (f64::NAN, "NaN"), (123456789012.5, "123456789012.5"),
        ] {
            assert_eq!(format_float(value), text);
        }
    }
}
