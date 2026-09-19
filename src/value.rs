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
    /// Each entry is one applied collection annotation's type arguments.
    pub restrictions: RefCell<Vec<Rc<[String]>>>,
}

impl Meta {
    pub fn inherit(&self) -> Meta {
        Meta { frozen: Cell::new(false), restrictions: RefCell::new(self.restrictions.borrow().clone()) }
    }
}

#[derive(Default)]
pub struct Array { pub items: RefCell<Vec<Value>>, pub meta: Meta }
#[derive(Default)]
pub struct Set { pub items: RefCell<IndexMap<Key, Value>>, pub meta: Meta }
#[derive(Default)]
pub struct Map { pub items: RefCell<IndexMap<Key, (Value, Value)>>, pub meta: Meta }
pub struct Pair { pub first: RefCell<Value>, pub second: RefCell<Value>, pub meta: Meta }
pub struct Instance { pub family: Rc<Family>, pub fields: RefCell<IndexMap<Rc<str>, Value>>, pub meta: Meta }

impl Value {
    pub fn array(items: Vec<Value>) -> Value {
        Value::Array(Rc::new(Array { items: RefCell::new(items), meta: Meta::default() }))
    }
    pub fn pair(first: Value, second: Value) -> Value {
        Value::Pair(Rc::new(Pair { first: RefCell::new(first), second: RefCell::new(second), meta: Meta::default() }))
    }
}

/// A user function or method, bound to the package whose globals it reads.
pub struct Function { pub decl: FuncDecl, pub package: usize }

pub struct Family {
    pub name: String,
    pub parent: Option<Rc<Family>>,
    pub methods: HashMap<String, Rc<Function>>,
}

impl Family {
    pub fn method(&self, name: &str) -> Option<Rc<Function>> {
        self.methods.get(name).cloned().or_else(|| self.parent.as_ref()?.method(name))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Builtin {
    Out, Slout, In, Inln, Arr, Set, Pair, Map, Int, Long, Float, String,
}

impl Builtin {
    pub fn lookup(name: &str) -> Option<Builtin> {
        Some(match name {
            "out" => Builtin::Out, "slout" => Builtin::Slout, "in" => Builtin::In, "inln" => Builtin::Inln,
            "arr" | "a" => Builtin::Arr, "set" | "s" => Builtin::Set, "pair" | "p" => Builtin::Pair,
            "map" | "dict" => Builtin::Map, "int" => Builtin::Int, "longint" => Builtin::Long,
            "float" => Builtin::Float, "string" => Builtin::String,
            _ => return None,
        })
    }
}

pub enum Callable {
    Func(Rc<Function>),
    Method(Value, Rc<Function>),
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

    pub fn build(self) -> Value { Value::Package(Rc::new(Package { name: self.name, members: self.members })) }
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
            Callable::Method(..) => self as *const Callable as usize,
        })
    }

    pub fn name(&self) -> String {
        match self {
            Callable::Func(f) | Callable::Method(_, f) => f.decl.name.clone(),
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

#[derive(Clone, Debug)]
pub struct RuntimeError { pub kind: ErrorKind, pub message: String }

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub type RResult<T> = Result<T, RuntimeError>;

pub fn err<T>(kind: ErrorKind, message: impl Into<String>) -> RResult<T> {
    Err(RuntimeError { kind, message: message.into() })
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
        Value::Func(f) => { let _ = write!(out, "<func {}>", f.name()); }
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
