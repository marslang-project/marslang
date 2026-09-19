//! `takepkg rs.math;` - floating-point primitives from the platform math library.
//! `std.math` builds its public API on these in Marslang (`std/math.mars`);
//! arguments must already be floats.

use crate::value::*;

pub fn package() -> Value {
    let mut package = NativePackage::new("rs.math")
        // Sign bit, which distinguishes -0.0 from 0.0.
        .function("signbit", 1, |args| Ok(Value::Bool(float(&args[0], "signbit")?.is_sign_negative())))
        .function("atan2", 2, |args| Ok(Value::Float(float(&args[0], "atan2")?.atan2(float(&args[1], "atan2")?))))
        .function("pow", 2, |args| Ok(Value::Float(float_pow(float(&args[0], "pow")?, float(&args[1], "pow")?))));
    macro_rules! unary {
        ($($name:literal => $f:expr),* $(,)?) => {
            $(package = package.function($name, 1, |args| Ok(Value::Float($f(float(&args[0], $name)?))));)*
        };
    }
    unary! {
        "sqrt" => f64::sqrt, "cbrt" => f64::cbrt, "exp" => f64::exp, "expm1" => f64::exp_m1,
        "ln" => f64::ln, "log1p" => f64::ln_1p, "log2" => f64::log2, "log10" => f64::log10,
        "sin" => f64::sin, "cos" => f64::cos, "tan" => f64::tan,
        "asin" => f64::asin, "acos" => f64::acos, "atan" => f64::atan,
        "sinh" => f64::sinh, "cosh" => f64::cosh, "tanh" => f64::tanh,
    }
    package.build()
}

fn float(value: &Value, name: &str) -> RResult<f64> {
    match value {
        Value::Float(f) => Ok(*f),
        _ => type_err(format!("rs.math.{name} expects float arguments")),
    }
}
