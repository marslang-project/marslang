//! `takepkg rs.random;` - the random number generator under `std.random`.
//!
//! Only the generator itself is native: Marslang has no unsigned or wrapping
//! arithmetic to run one, and it is called for every number. Everything built
//! from its numbers (choices, shuffles, samples, weights, the normal
//! distribution) is Marslang, in `std/random.mars`.
//!
//! The generator is xoshiro256++ (Blackman and Vigna, 2019), Julia's default
//! and the one behind Rust's `rand::rngs::SmallRng`: 256 bits of state, a
//! period of 2^256 - 1, fast, and passing BigCrush and PractRand. Its state is four longints in a Marslang array that each generator
//! keeps, which these functions update in place. Seeds expand into a state
//! through SplitMix64, as its authors recommend. It is not for passwords or
//! keys: its output can be predicted from enough earlier output.

use std::hash::{BuildHasher, Hasher};

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.random")
        // A new state from a seed: null for an unpredictable one, or an int,
        // longint, or string for the same numbers on every run.
        .function("state", 1, |args| Ok(Value::array(words(seed(&args[0])?).into_iter().map(Value::Long).collect())))
        // Start an existing state over from a seed.
        .function("reseed", 2, |args| {
            let state = state_array(&args[0])?;
            let words = words(seed(&args[1])?);
            let mut items = state.items.borrow_mut();
            for (slot, word) in items.iter_mut().zip(words) { *slot = Value::Long(word); }
            Ok(Value::Null)
        })
        // A float from 0.0 up to, never including, 1.0, with all 53 bits of
        // precision a float has.
        .function("float", 1, |args| {
            let next = step(&args[0])?;
            Ok(Value::Float((next >> 11) as f64 * (1.0 / (1u64 << 53) as f64)))
        })
        // A whole number from low to high, both included, every value equally
        // likely. An int when both ends are ints, otherwise a longint.
        .function("between", 3, |args| {
            let (low, high, long) = match (&args[1], &args[2]) {
                (Value::Int(low), Value::Int(high)) => (*low, *high, false),
                (Value::Int(low) | Value::Long(low), Value::Int(high) | Value::Long(high)) => (*low, *high, true),
                (low, high) => return type_err(format!(
                    "random.int_between expects ints or longints, got {} and {}", low.type_name(), high.type_name()
                )),
            };
            if low > high { return range_err(format!("random.int_between needs low <= high, got {low} and {high}")); }
            // The number of values, wrapping to 0 for all 2^64 longints.
            let span = (high.wrapping_sub(low) as u64).wrapping_add(1);
            let offset = if span == 0 { step(&args[0])? } else { below(&args[0], span)? };
            let value = low.wrapping_add(offset as i64);
            Ok(if long { Value::Long(value) } else { Value::Int(value) })
        })
        .build()
}

/// A number from 0 up to `span` (exclusive) with no bias, by Lemire's method
/// ("Fast Random Integer Generation in an Interval", 2019): one
/// multiplication, and a division only in the rare case that a draw must be
/// rejected because it would favour some values.
fn below(state: &Value, span: u64) -> RResult<u64> {
    let mut product = step(state)? as u128 * span as u128;
    if (product as u64) < span {
        let threshold = span.wrapping_neg() % span;
        while (product as u64) < threshold {
            product = step(state)? as u128 * span as u128;
        }
    }
    Ok((product >> 64) as u64)
}

/// The next 64 bits from a state array, advancing it (xoshiro256++).
fn step(state: &Value) -> RResult<u64> {
    let array = state_array(state)?;
    let mut items = array.items.borrow_mut();
    let mut s = [0u64; 4];
    for (word, item) in s.iter_mut().zip(items.iter()) {
        match item {
            Value::Long(bits) | Value::Int(bits) => *word = *bits as u64,
            _ => return type_err("a random generator's state must hold four longints"),
        }
    }
    let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
    let t = s[1] << 17;
    s[2] ^= s[0];
    s[3] ^= s[1];
    s[1] ^= s[2];
    s[0] ^= s[3];
    s[2] ^= t;
    s[3] = s[3].rotate_left(45);
    for (item, word) in items.iter_mut().zip(s) { *item = Value::Long(word as i64); }
    Ok(result)
}

fn state_array(value: &Value) -> RResult<&std::rc::Rc<Array>> {
    match value {
        Value::Array(array) if array.items.borrow().len() == 4 => Ok(array),
        _ => type_err("a random generator's state must be an array of four longints"),
    }
}

/// The 64-bit seed a seed value stands for.
fn seed(value: &Value) -> RResult<u64> {
    match value {
        Value::Null => Ok(entropy()),
        Value::Int(n) | Value::Long(n) => Ok(*n as u64),
        // FNV-1a over the UTF-8 bytes: fixed forever, unlike Rust's own hashers,
        // so a string seed gives the same numbers on every version and platform.
        Value::Str(text) => Ok(text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
        })),
        other => type_err(format!("a random seed must be an int, longint, string, or null, got {}", other.type_name())),
    }
}

/// Four state words from one seed by SplitMix64, which never yields the
/// all-zero state that xoshiro cannot leave.
fn words(mut seed: u64) -> [i64; 4] {
    let mut next = || {
        seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as i64
    };
    [next(), next(), next(), next()]
}

/// 64 unpredictable bits: SipHash under keys the operating system's random
/// source supplied, which differ for every `RandomState`, over the time.
fn entropy() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        hasher.write_u128(now.as_nanos());
    }
    hasher.write_u32(std::process::id());
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_of(words: [u64; 4]) -> Value {
        Value::array(words.iter().map(|w| Value::Long(*w as i64)).collect())
    }

    #[test]
    fn matches_the_reference_xoshiro256plusplus() {
        // The first outputs of the reference C implementation from state 1, 2, 3, 4.
        let state = state_of([1, 2, 3, 4]);
        let outputs: Vec<u64> = (0..4).map(|_| step(&state).unwrap()).collect();
        assert_eq!(outputs, [41943041, 58720359, 3588806011781223, 3591011842654386]);
    }

    #[test]
    fn splitmix_matches_its_reference() {
        // SplitMix64 from seed 0, as in the reference implementation.
        assert_eq!(words(0)[0] as u64, 0xe220_a839_7b1d_cdaf);
    }

    #[test]
    fn below_is_in_range_and_reaches_both_ends() {
        let state = state_of([5, 6, 7, 8]);
        let mut seen = [false; 3];
        for _ in 0..1000 {
            let n = below(&state, 3).unwrap();
            assert!(n < 3);
            seen[n as usize] = true;
        }
        assert_eq!(seen, [true; 3]);
    }
}
