//! `takepkg rs.hash;` - checksums and digests for `std.hash`, by the RustCrypto
//! crates (sha2, sha1, md-5, hmac) and crc32fast.
//!
//! Text is hashed as its UTF-8 bytes, and every digest comes back as lowercase
//! hex. A file is read in 64 KiB chunks, so one larger than memory, or a
//! binary file, can be hashed. `file` returns `[true, hex]` or
//! `[false, kind, message]`, and `std/hash.mars` raises the family `kind`
//! names, as `std/rs/file.rs` does for `std.file`.
//!
//! `same` is here rather than in Marslang because it must compare every byte
//! before answering. Marslang has no bitwise operators, and an interpreted
//! loop that stops at the first difference would show where the digests part.

use std::fs;
use std::io::{self, Read};

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

use crate::value::*;

/// The size of each read from a file. Memory use stays this large whatever
/// the file's size.
const CHUNK: usize = 64 * 1024;

pub fn package() -> Value {
    NativePackage::new("rs.hash")
        .function("sha256", 1, |a| Ok(text_digest(Sha256::new(), text(&a[0], "hash.sha256")?)))
        .function("sha512", 1, |a| Ok(text_digest(Sha512::new(), text(&a[0], "hash.sha512")?)))
        .function("sha1", 1, |a| Ok(text_digest(Sha1::new(), text(&a[0], "hash.sha1")?)))
        .function("md5", 1, |a| Ok(text_digest(md5::Md5::new(), text(&a[0], "hash.md5")?)))
        .function("crc32", 1, |a| Ok(text_digest(Crc32(crc32fast::Hasher::new()), text(&a[0], "hash.crc32")?)))
        .function("hmac_sha256", 2, |a| {
            let key = text(&a[0], "hash.hmac_sha256")?;
            let message = text(&a[1], "hash.hmac_sha256")?;
            // HMAC takes a key of any length, so this cannot fail.
            let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
            mac.update(message.as_bytes());
            Ok(Value::str(&hex(mac.finalize().into_bytes())))
        })
        .function("file", 2, |a| {
            let path = text(&a[0], "hash.file")?;
            match text(&a[1], "hash.file")? {
                "sha256" => Ok(file_digest(path, Sha256::new())),
                "sha512" => Ok(file_digest(path, Sha512::new())),
                "sha1" => Ok(file_digest(path, Sha1::new())),
                "md5" => Ok(file_digest(path, md5::Md5::new())),
                "crc32" => Ok(file_digest(path, Crc32(crc32fast::Hasher::new()))),
                other => range_err(format!(
                    "hash.file needs an algorithm of \"sha256\", \"sha512\", \"sha1\", \"md5\", or \"crc32\", got \"{other}\""
                )),
            }
        })
        .function("same", 2, |a| Ok(Value::Bool(same(text(&a[0], "hash.same")?, text(&a[1], "hash.same")?))))
        .build()
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

/// A hash that is fed bytes a piece at a time and then finished as hex.
trait Stream {
    fn feed(&mut self, bytes: &[u8]);
    fn finish(self) -> String;
}

impl Stream for Sha256 {
    fn feed(&mut self, bytes: &[u8]) { Digest::update(self, bytes) }
    fn finish(self) -> String { hex(Digest::finalize(self)) }
}

impl Stream for Sha512 {
    fn feed(&mut self, bytes: &[u8]) { Digest::update(self, bytes) }
    fn finish(self) -> String { hex(Digest::finalize(self)) }
}

impl Stream for Sha1 {
    fn feed(&mut self, bytes: &[u8]) { Digest::update(self, bytes) }
    fn finish(self) -> String { hex(Digest::finalize(self)) }
}

impl Stream for md5::Md5 {
    fn feed(&mut self, bytes: &[u8]) { Digest::update(self, bytes) }
    fn finish(self) -> String { hex(Digest::finalize(self)) }
}

/// CRC-32 in the same shape as the digests, which keeps the 32-bit value in
/// hex with eight digits.
struct Crc32(crc32fast::Hasher);

impl Stream for Crc32 {
    fn feed(&mut self, bytes: &[u8]) { self.0.update(bytes) }
    fn finish(self) -> String { format!("{:08x}", self.0.finalize()) }
}

fn text_digest(mut hash: impl Stream, text: &str) -> Value {
    hash.feed(text.as_bytes());
    Value::str(&hash.finish())
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The digest of a file, or the failure to read it.
fn file_digest(path: &str, mut hash: impl Stream) -> Value {
    match read_chunks(path, |bytes| hash.feed(bytes)) {
        Ok(()) => Value::array(vec![Value::Bool(true), Value::str(&hash.finish())]),
        Err(failure) => failure,
    }
}

/// Calls feed with each chunk of the file in turn, or returns the failure to
/// report. A directory is refused by name first, since opening one can
/// succeed on some systems and fail with a less useful error on others.
fn read_chunks(path: &str, mut feed: impl FnMut(&[u8])) -> Result<(), Value> {
    if fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(failure("IsDirectory", format!("{path}: is a directory")));
    }
    let mut file = fs::File::open(path).map_err(|error| io_failure(path, error))?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(read) => feed(&buffer[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(io_failure(path, error)),
        }
    }
}

fn failure(kind: &str, message: String) -> Value {
    Value::array(vec![Value::Bool(false), Value::str(kind), Value::str(&message)])
}

/// The kinds std.file's `_result` maps, worded as std.file words them, so the
/// message is the same whichever package reported the failure.
fn io_failure(path: &str, error: io::Error) -> Value {
    use io::ErrorKind::*;
    match error.kind() {
        NotFound => failure("NotFound", format!("{path}: no such file or directory")),
        PermissionDenied => failure("Permission", format!("{path}: permission denied")),
        IsADirectory => failure("IsDirectory", format!("{path}: is a directory")),
        NotADirectory => failure("NotDirectory", format!("{path}: a part of this path is a file, not a directory")),
        _ => failure("File", format!("{path}: {error}")),
    }
}

/// Whether the two strings are equal, comparing every byte whatever they hold.
/// Different lengths differ at once, so only equal-length strings are compared
/// byte by byte.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() { return false; }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b) {
        difference |= x ^ y;
    }
    std::hint::black_box(difference) == 0
}

#[cfg(test)]
mod tests {
    use super::same;

    #[test]
    fn same_compares_whole_strings() {
        assert!(same("", ""));
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
        assert!(!same("", "a"));
        assert!(!same("abc", "xbc"));
    }
}
