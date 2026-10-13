//! `takepkg rs.bigint;` - what `std.bigint` cannot do quickly in Marslang:
//! integer square roots, and reading and writing numbers in other bases. The
//! bigint kind itself, with its operators, is built into the interpreter
//! (`src/bigint.rs`); everything else in `std/bigint.mars` is Marslang.

use std::rc::Rc;

use num_traits::Signed;

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.bigint")
        // The whole part of the square root of a bigint of 0 or more.
        .function("sqrt", 1, |args| match &args[0] {
            Value::Big(n) if n.is_negative() => range_err(format!("bigint.sqrt needs a number of 0 or more, got {n}")),
            Value::Big(n) => Ok(Value::Big(Rc::new(n.sqrt()))),
            other => type_err(format!("rs.bigint.sqrt expects a bigint, got {}", other.type_name())),
        })
        // The number written in text in a base from 2 to 36, with - in front
        // of a negative one; letters stand for digits from 10, in either case.
        .function("parse", 2, |args| {
            let base = base(&args[1], "bigint.parse")?;
            let Value::Str(text) = &args[0] else {
                return type_err(format!("bigint.parse expects a string, got {}", args[0].type_name()));
            };
            let (negative, written) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text.as_ref()),
            };
            let digits: Option<Vec<u8>> = written.chars().map(|c| c.to_digit(base).map(|d| d as u8)).collect();
            match digits {
                Some(digits) if !digits.is_empty() => match crate::bigint::from_digits(&digits, base, negative) {
                    Some(n) => Ok(Value::Big(Rc::new(n))),
                    None => crate::bigint::too_large(),
                },
                _ => err(ErrorKind::SyntaxError, format!("{} is not a number in base {base}: {}", crate::bigint::quote(text), digits_hint(base))),
            }
        })
        // A bigint's digits in a base from 2 to 36, letters in lower case.
        .function("digits", 2, |args| {
            let base = base(&args[1], "bigint.digits")?;
            match &args[0] {
                Value::Big(n) => Ok(Value::str(&n.to_str_radix(base))),
                other => type_err(format!("rs.bigint.digits expects a bigint, got {}", other.type_name())),
            }
        })
        .build()
}

fn base(value: &Value, name: &str) -> RResult<u32> {
    match value {
        Value::Int(n) if (2..=36).contains(n) => Ok(*n as u32),
        Value::Int(n) => range_err(format!("{name} needs a base from 2 to 36, got {n}")),
        other => type_err(format!("{name} needs an int base from 2 to 36, got {}", other.type_name())),
    }
}

/// Which characters are digits in `base`, for a parse error.
fn digits_hint(base: u32) -> String {
    let last = std::char::from_digit(base - 1, base).expect("a digit of its base");
    match base {
        2..=10 => format!("its digits are 0 to {last}, with - in front of a negative number"),
        _ => format!("its digits are 0 to 9 and a to {last}, in either case, with - in front of a negative number"),
    }
}
