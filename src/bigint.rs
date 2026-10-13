//! `bigint`: whole numbers of any size, the fourth numeric kind.
//!
//! A bigint is a `num_bigint::BigInt` behind an `Rc`, so `Value` stays the size
//! it was and copying one only counts a reference. It holds no other values,
//! so the cycle collector never registers it, as it never registers a string.
//!
//! The int, longint, and float paths of the interpreter come first and are
//! unchanged: what is here runs only when an operand is a bigint, and int or
//! longint overflow still raises rather than turning into a bigint. A result
//! stays a bigint even when it is small.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::rc::Rc;

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};

use crate::value::*;

/// The most bits a bigint may have: every whole number of up to a million
/// decimal digits fits, and numbers this size are still quick to work with.
/// Without a limit, `bigint(2) ** 100000000000` would run until memory ran out.
pub const MAX_BITS: u64 = 3_321_929;

/// The bigint `n`, or a RangeError when it passes the size limit.
pub fn value(n: BigInt) -> RResult<Value> {
    if n.bits() > MAX_BITS { return too_large(); }
    Ok(Value::Big(Rc::new(n)))
}

pub fn too_large<T>() -> RResult<T> {
    range_err(format!("bigint too large: the limit is {MAX_BITS} bits, about a million digits"))
}

/// The exact value of an int, longint, or bigint, borrowed when it is a bigint.
fn integer(value: &Value) -> Option<Cow<'_, BigInt>> {
    match value {
        Value::Int(i) | Value::Long(i) => Some(Cow::Owned(BigInt::from(*i))),
        Value::Big(b) => Some(Cow::Borrowed(b.as_ref())),
        _ => None,
    }
}

// ----- arithmetic -----

/// `+ - * / % **` when either operand is a bigint and both are numbers. An int
/// or longint on the other side joins exactly; a float must be converted first,
/// as with longint, since neither kind holds the other.
pub fn arithmetic(op: &str, a: &Value, b: &Value) -> RResult<Value> {
    let (Some(x), Some(y)) = (integer(a), integer(b)) else {
        return type_err("mixed bigint/float arithmetic; convert one side first, with float(...) or bigint(...)");
    };
    let (x, y) = (x.as_ref(), y.as_ref());
    match op {
        "+" => value(x + y),
        "-" => value(x - y),
        "*" => {
            // The product has at least this many bits: refuse it before working it out.
            if !x.is_zero() && !y.is_zero() && x.bits() + y.bits() - 1 > MAX_BITS { return too_large(); }
            value(x * y)
        }
        "/" => divide(x, y),
        // Truncating, as for ints: the remainder has the dividend's sign.
        "%" if y.is_zero() => range_err("division by zero"),
        "%" => Ok(Value::Big(Rc::new(x % y))),
        "**" => power(x, y),
        _ => type_err(format!("{op} is not an arithmetic operator")),
    }
}

/// `/`, as for ints: the exact quotient when it is whole, otherwise the
/// nearest float.
fn divide(x: &BigInt, y: &BigInt) -> RResult<Value> {
    if y.is_zero() { return range_err("division by zero"); }
    let (quotient, rest) = x.div_rem(y);
    if rest.is_zero() { return Ok(Value::Big(Rc::new(quotient))); }
    let result = ratio_to_f64(x, y);
    if result.is_infinite() {
        return range_err("the quotient is not whole and is too large for a float; bigint.divide gives its whole part");
    }
    Ok(Value::Float(result))
}

/// `**`. A negative exponent is `1 / base ** -exponent`, as `/` gives it: a
/// float, except that 1 and -1 stay bigints and 0 raises division by zero.
fn power(base: &BigInt, exponent: &BigInt) -> RResult<Value> {
    let unit = base.magnitude().is_one();
    if exponent.is_negative() {
        if base.is_zero() { return range_err("division by zero"); }
        if !unit {
            // A base beyond a float's range is infinity here, and its result 0.
            let (base, exponent) = (base.to_f64().unwrap_or(f64::NAN), exponent.to_f64().unwrap_or(f64::NAN));
            return Ok(Value::Float(float_pow(base, exponent)));
        }
    }
    if base.is_zero() || unit {
        // 0, 1, and -1 stay small whatever the exponent.
        let result = if exponent.is_zero() { BigInt::one() }
            else if base.is_zero() { BigInt::zero() }
            else if base.is_negative() && exponent.is_odd() { -BigInt::one() }
            else { BigInt::one() };
        return Ok(Value::Big(Rc::new(result)));
    }
    // Any other base at least doubles at each step, so the result's size is
    // known before working it out: exponent * log2(|base|) bits.
    let Some(exponent) = exponent.to_u32().filter(|&e| u64::from(e) <= MAX_BITS) else { return too_large() };
    if f64::from(exponent) * log2(base.magnitude()) > MAX_BITS as f64 + 1.0 { return too_large(); }
    value(base.pow(exponent))
}

/// log2 of a magnitude of any size, from its leading 64 bits.
fn log2(n: &BigUint) -> f64 {
    let shift = n.bits().saturating_sub(64);
    let top = (n >> shift).to_u64().unwrap_or(u64::MAX);
    (top as f64).log2() + shift as f64
}

pub fn negate(b: &BigInt) -> Value { Value::Big(Rc::new(-b)) }

/// `n / d` as the nearest float, ties to even, as IEEE division rounds, even
/// when `n` and `d` are far beyond a float's range. `d` is not zero; a quotient
/// beyond the largest float is infinity.
pub fn ratio_to_f64(n: &BigInt, d: &BigInt) -> f64 {
    let negative = n.is_negative() != d.is_negative();
    let (n, d) = (n.magnitude(), d.magnitude());
    let magnitude = if n.is_zero() { 0.0 } else {
        // Scaled so the whole quotient has 65 or 66 bits: the 53 a float keeps,
        // and the rest, with whether anything remains, to round by.
        let shift = 65 - (n.bits() as i64 - d.bits() as i64);
        let (quotient, rest) = if shift >= 0 { (n << shift as u64).div_rem(d) } else { n.div_rem(&(d << shift.unsigned_abs())) };
        scale(quotient.to_u128().expect("at most 66 bits"), !rest.is_zero(), -shift)
    };
    if negative { -magnitude } else { magnitude }
}

/// `(m + f) * 2^exponent` rounded to the nearest float, where `m` has at least
/// 54 bits and `0 < f < 1` exactly when `inexact`.
fn scale(m: u128, inexact: bool, exponent: i64) -> f64 {
    let length = 128 - i64::from(m.leading_zeros());
    let top = exponent + length - 1;
    if top > 1023 { return f64::INFINITY; }
    // A float keeps 53 bits, and fewer below 2^-1022, where it is subnormal.
    let keep = if top >= -1022 { 53 } else { 53 - (-1022 - top) };
    if keep < 0 { return 0.0; }
    let drop = length - keep;
    let mut kept = m >> drop;
    let rest = m & ((1u128 << drop) - 1);
    let half = 1u128 << (drop - 1);
    if rest > half || (rest == half && (inexact || kept & 1 == 1)) { kept += 1; }
    // kept * 2^e is a float exactly, unless rounding carried it past the largest.
    let e = exponent + drop;
    let power = if e >= -1022 { f64::from_bits(((e + 1023) as u64) << 52) } else { f64::from_bits(1u64 << (e + 1074)) };
    kept as f64 * power
}

// ----- comparing and keys -----

/// The exact order of two numbers when either is a bigint; `None` against
/// NaN, or when either is not a number.
pub fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Big(x), Value::Big(y)) => Some(x.cmp(y)),
        (Value::Big(x), Value::Int(i) | Value::Long(i)) => Some(compare_small(x, *i)),
        (Value::Int(i) | Value::Long(i), Value::Big(y)) => Some(compare_small(y, *i).reverse()),
        (Value::Big(x), Value::Float(f)) => compare_float(x, *f),
        (Value::Float(f), Value::Big(y)) => compare_float(y, *f).map(Ordering::reverse),
        _ => None,
    }
}

fn compare_small(x: &BigInt, i: i64) -> Ordering {
    match x.to_i64() {
        Some(x) => x.cmp(&i),
        None if x.is_negative() => Ordering::Less,
        None => Ordering::Greater,
    }
}

/// Exact, never by rounding the bigint to a float: the float's whole part is
/// an integer exactly, and its fraction only matters on a tie.
fn compare_float(x: &BigInt, f: f64) -> Option<Ordering> {
    if f.is_nan() { return None; }
    if f.is_infinite() { return Some(if f > 0.0 { Ordering::Less } else { Ordering::Greater }); }
    let whole = f.floor();
    let floor = BigInt::from_f64(whole).expect("a finite float");
    Some(x.cmp(&floor).then(if f > whole { Ordering::Less } else { Ordering::Equal }))
}

/// The map and set key of a bigint, equal to the key of any int, longint, or
/// float it equals.
pub fn key(b: &Rc<BigInt>) -> Key {
    if let Some(i) = b.to_i64() { return Key::Integer(i); }
    // Beyond longint, a float equals a bigint only when it holds it exactly:
    // at most 53 significant bits, below 2^1024.
    let bits = b.bits();
    if bits <= 1024 && bits - b.trailing_zeros().unwrap_or(0) <= 53 {
        if let Some(f) = b.to_f64() { return Key::Num(f.to_bits()); }
    }
    Key::Big(b.clone())
}

// ----- making and converting -----

/// `bigint(x)`: the exact integer an int, longint, bigint, whole float, or
/// string of decimal digits stands for.
pub fn construct(arg: &Value) -> RResult<Value> {
    match arg {
        Value::Int(i) | Value::Long(i) => Ok(Value::Big(Rc::new(BigInt::from(*i)))),
        Value::Big(_) => Ok(arg.clone()),
        // Every finite whole float is an integer exactly; nothing is rounded.
        Value::Float(f) if f.is_finite() && f.fract() == 0.0 => Ok(Value::Big(Rc::new(BigInt::from_f64(*f).expect("a finite float")))),
        Value::Float(f) => range_err(format!("bigint() needs a whole, finite float, got {}; round it first", format_float(*f))),
        Value::Str(text) => {
            let (negative, digits) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text.as_ref()),
            };
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return err(ErrorKind::SyntaxError, format!(
                    "cannot convert {} to a bigint: write decimal digits, with - in front of a negative number", quote(text)));
            }
            match from_decimal(digits, negative) {
                Some(n) => Ok(Value::Big(Rc::new(n))),
                None => too_large(),
            }
        }
        other => type_err(format!("bigint() expects an int, longint, bigint, float, or string, got {}", other.type_name())),
    }
}

/// The number written in decimal `digits`, all ASCII digits; `None` when it
/// passes the size limit.
pub fn from_decimal(digits: &str, negative: bool) -> Option<BigInt> {
    let values: Vec<u8> = digits.bytes().map(|c| c - b'0').collect();
    from_digits(&values, 10, negative)
}

/// The number whose digits in `radix` are `values`, most significant first,
/// each below `radix`; `None` when it passes the size limit.
pub fn from_digits(values: &[u8], radix: u32, negative: bool) -> Option<BigInt> {
    let start = values.iter().position(|&d| d != 0).unwrap_or(values.len());
    let significant = &values[start..];
    // k digits make at least (k - 1) * log2(radix) + 1 bits: refuse a number
    // that is plainly too large before reading it.
    if significant.len() > 1 && (significant.len() - 1) as f64 * f64::from(radix).log2() >= MAX_BITS as f64 {
        return None;
    }
    let magnitude = if radix.is_power_of_two() {
        BigUint::from_radix_be(significant, radix)?
    } else {
        read_digits(significant, radix, &mut std::collections::HashMap::new())
    };
    let sign = if negative { Sign::Minus } else { Sign::Plus };
    (magnitude.bits() <= MAX_BITS).then(|| BigInt::from_biguint(sign, magnitude))
}

/// Digits in a radix that is not a power of two. num-bigint reads them one
/// machine word at a time, which takes time growing with the square of their
/// number: over a second for a million digits, and JSON may come from anyone.
/// Reading each half and joining them, `high * radix^low_digits + low`, lets
/// the multiplications use num-bigint's fast algorithms instead.
fn read_digits(digits: &[u8], radix: u32, powers: &mut std::collections::HashMap<usize, BigUint>) -> BigUint {
    const DIRECT: usize = 2_000;
    if digits.len() <= DIRECT {
        return BigUint::from_radix_be(digits, radix).expect("digits below their radix");
    }
    let low_digits = digits.len() / 2;
    let (high, low) = digits.split_at(digits.len() - low_digits);
    let high = read_digits(high, radix, powers);
    let low = read_digits(low, radix, powers);
    let power = powers.entry(low_digits).or_insert_with(|| BigUint::from(radix).pow(low_digits as u32));
    high * &*power + low
}

/// `int(b)`.
pub fn to_int(b: &BigInt) -> RResult<Value> {
    b.to_i32().map(|i| Value::Int(i64::from(i))).ok_or(()).or_else(|_| range_err("int overflow: the bigint does not fit in an int"))
}

/// `longint(b)`.
pub fn to_long(b: &BigInt) -> RResult<Value> {
    b.to_i64().map(Value::Long).ok_or(()).or_else(|_| range_err("longint overflow: the bigint does not fit in a longint"))
}

/// `float(b)`: the nearest float, or RangeError beyond the largest one rather
/// than infinity.
pub fn to_float(b: &BigInt) -> RResult<Value> { nearest_float(b).map(Value::Float) }

/// The nearest float to `b`, ties to even; RangeError beyond the largest float.
pub fn nearest_float(b: &BigInt) -> RResult<f64> {
    match b.to_f64() {
        Some(f) if f.is_finite() => Ok(f),
        _ => range_err("the bigint is too large for a float, whose largest value is about 1.8e308"),
    }
}

/// `text` quoted for an error message, shortened when it is long.
pub fn quote(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(40).collect();
    if chars.next().is_some() { format!("{:?}", format!("{head}...")) } else { format!("{text:?}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(text: &str) -> BigInt { text.parse().unwrap() }

    #[test]
    fn quotients_round_like_ieee_division() {
        // Operands a float holds exactly: the hardware's division is the reference.
        for (n, d) in [(1i64, 3i64), (2, 3), (-7, 2), (10, 4), (1 << 53, 3), ((1 << 53) + 1, 2), (123456789, -1000), (-1, -7)] {
            assert_eq!(ratio_to_f64(&BigInt::from(n), &BigInt::from(d)), n as f64 / d as f64, "{n} / {d}");
        }
        // Far beyond a float's range on both sides, the quotient is still exact.
        let huge = big("1").clone() << 5000u32;
        assert_eq!(ratio_to_f64(&(&huge * 3), &(&huge * 2)), 1.5);
        assert_eq!(ratio_to_f64(&huge, &(&huge * 3)), 1.0 / 3.0);
        // Overflow, underflow, and the smallest subnormal.
        assert_eq!(ratio_to_f64(&(BigInt::one() << 1100u32), &BigInt::from(3)), f64::INFINITY);
        assert_eq!(ratio_to_f64(&BigInt::one(), &(BigInt::one() << 1074u32)), f64::from_bits(1));
        assert_eq!(ratio_to_f64(&BigInt::one(), &(BigInt::one() << 1075u32)), 0.0);
        assert_eq!(ratio_to_f64(&BigInt::from(3), &(BigInt::one() << 1076u32)), f64::from_bits(1));
        // 2^-1030 is subnormal: 2^-1074 * 2^44. (powi is no reference: on
        // Linux it works out 1 / 2^1030, which overflows, and gives 0.)
        assert_eq!(ratio_to_f64(&BigInt::one(), &(BigInt::one() << 1030u32)), f64::from_bits(1 << 44));
    }

    #[test]
    fn halfway_quotients_round_to_even() {
        // 2^53 + 1 is halfway between two floats; 2^53 + 3 too, rounding up.
        let two53 = BigInt::one() << 53u32;
        assert_eq!(ratio_to_f64(&((&two53 + 1) * 4), &BigInt::from(4)), 9007199254740992.0);
        assert_eq!(ratio_to_f64(&((&two53 + 3) * 4), &BigInt::from(4)), 9007199254740996.0);
        // Just above halfway goes up.
        assert_eq!(ratio_to_f64(&((&two53 + 1) * 4 + 1), &BigInt::from(4)), 9007199254740994.0);
    }

    #[test]
    fn the_cycle_collector_never_tracks_a_bigint() {
        crate::gc::collect();
        let before = crate::gc::live();
        let values: Vec<Value> = (0..100).map(|i| construct(&Value::Int(i)).unwrap()).collect();
        for value in &values { crate::gc::track(value); }
        assert_eq!(crate::gc::live(), before);
        // Inside a container it is just a value the container holds.
        let array = Value::array(values);
        assert_eq!(crate::gc::live(), before + 1);
        drop(array);
        crate::gc::collect();
        assert_eq!(crate::gc::live(), before);
    }

    #[test]
    fn keys_match_the_float_or_longint_they_equal() {
        let at = |text: &str| key(&Rc::new(big(text)));
        assert!(at("5") == Key::of(&Value::Int(5)));
        assert!(at("100000000000000000000") == Key::of(&Value::Float(1e20)));
        assert!(at("-100000000000000000000") == Key::of(&Value::Float(-1e20)));
        assert!(matches!(at("100000000000000000001"), Key::Big(_)));
    }
}
