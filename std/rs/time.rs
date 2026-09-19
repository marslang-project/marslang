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
            std::thread::sleep(Duration::from_secs_f64(seconds));
            Ok(Value::Null)
        })
        .build()
}
