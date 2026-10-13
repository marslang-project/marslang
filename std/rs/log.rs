//! `takepkg rs.log;` - writing lines to standard error, for `std.log`.
//!
//! Only the write is native: Marslang has no way to reach standard error.
//! Levels, timestamps, files, and the Logger family are in `std/log.mars`.
//! Standard error is unbuffered by default; the flush keeps a line from
//! waiting behind the next one if the process ends straight after.

use std::io::Write;

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.log")
        // The text and a newline, to standard error, flushed. Returns null.
        .function("write", 1, |args| match &args[0] {
            Value::Str(text) => {
                // A closed or broken standard error is ignored: a logger that
                // fails the program it reports on is worse than a lost line.
                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(text.as_bytes());
                let _ = stderr.write_all(b"\n");
                let _ = stderr.flush();
                Ok(Value::Null)
            }
            other => type_err(format!("log.write expects a string, got {}", other.type_name())),
        })
        .build()
}
