//! `takepkg rs.time;` - clocks and sleeping for `std.time`.

use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::value::*;

/// Reference point for `monotonic`, fixed on first use.
static START: OnceLock<Instant> = OnceLock::new();

pub fn package() -> Value {
    START.get_or_init(Instant::now);
    NativePackage::new("rs.time")
        // Seconds since 1970-01-01 UTC, from the system clock.
        .function("now", 0, |_| {
            let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
            Ok(Value::Float(since.as_secs_f64()))
        })
        // Seconds from a fixed point; never goes backwards.
        .function("monotonic", 0, |_| Ok(Value::Float(START.get_or_init(Instant::now).elapsed().as_secs_f64())))
        .function("sleep", 1, |args| {
            let seconds = match &args[0] {
                Value::Float(f) => *f,
                _ => return type_err("rs.time.sleep expects a float"),
            };
            if !seconds.is_finite() || seconds < 0.0 { return range_err("sleep needs a finite, nonnegative number of seconds"); }
            let Ok(duration) = Duration::try_from_secs_f64(seconds) else {
                return range_err("sleep duration is too long");
            };
            std::thread::sleep(duration);
            Ok(Value::Null)
        })
        // The calendar kinds: today's date and this moment, in the local zone.
        .function("today", 0, |_| Ok(crate::date::today()))
        .function("now_datetime", 0, |_| Ok(crate::date::now()))
        // A length of time from a number of units, of any numeric kind.
        .function("length", 2, |args| {
            let unit = match &args[1] { Value::Float(unit) => *unit, _ => return type_err("rs.time.length expects a float unit") };
            let amount = match &args[0] {
                Value::Int(n) | Value::Long(n) => *n as f64,
                Value::Float(f) if f.is_finite() => *f,
                Value::Float(_) => return range_err("a duration needs a finite number"),
                other => return type_err(format!("a duration needs a number, not {}", other.type_name())),
            };
            crate::date::seconds(amount * unit).map(crate::date::duration_value)
        })
        .build()
}
