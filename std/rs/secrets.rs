//! `takepkg rs.secrets;` - the operating system's random bytes for `std.secrets`.
//!
//! The only native function is `bytes(n)`: n random bytes from getrandom, the
//! operating system's cryptographically secure source (getrandom(2) on Linux,
//! BCryptGenRandom on Windows, getentropy on macOS). Everything built from
//! those bytes, such as hex and URL-safe tokens, unbiased integers, choices,
//! and passwords, is Marslang in `std/secrets.mars`.

use crate::value::*;

/// Most bytes one call returns: 64 KiB, far more than any key or token needs.
const LIMIT: i64 = 65536;

pub fn package() -> Value {
    NativePackage::new("rs.secrets")
        // n random bytes, each an int from 0 to 255, for n from 0 to 65536.
        .function("bytes", 1, |args| {
            let count = match &args[0] {
                Value::Int(n) | Value::Long(n) if (0..=LIMIT).contains(n) => *n as usize,
                Value::Int(n) | Value::Long(n) => {
                    return range_err(format!("secrets.bytes needs a count from 0 to {LIMIT}, got {n}"));
                }
                other => {
                    return type_err(format!("secrets.bytes needs an int or longint count, got {}", other.type_name()));
                }
            };
            match random(count) {
                Ok(data) => Ok(Value::array(data.into_iter().map(|byte| Value::Int(byte as i64)).collect())),
                Err(error) => err(ErrorKind::Error, format!("the operating system's random source failed: {error}")),
            }
        })
        .build()
}

/// `count` bytes from the operating system's random source.
fn random(count: usize) -> Result<Vec<u8>, getrandom::Error> {
    let mut buffer = vec![0u8; count];
    getrandom::getrandom(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::random;

    #[test]
    fn gives_the_requested_count() {
        assert_eq!(random(0).unwrap().len(), 0);
        assert_eq!(random(16).unwrap().len(), 16);
        assert_eq!(random(65536).unwrap().len(), 65536);
    }

    #[test]
    fn differs_between_calls() {
        // 32 random bytes repeating exactly is a 2^-256 chance.
        assert_ne!(random(32).unwrap(), random(32).unwrap());
    }
}
