//! std.hash: digests checked against published test vectors, file hashing in
//! and across chunks, the std.file families and messages for failures, and the
//! constant-time comparison.

use std::path::PathBuf;

use md5::Md5;
use sha2::{Digest, Sha256};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    assert_eq!(stdout, expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

/// A file in the temporary directory, removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, bytes: &[u8]) -> TempFile {
        let path = std::env::temp_dir().join(format!("marslang-std-hash-{}-{name}", std::process::id()));
        std::fs::write(&path, bytes).expect("write temp file");
        TempFile(path)
    }

    /// The path as Marslang source: raw, so Windows backslashes stay as they are.
    fn literal(&self) -> String {
        let text = self.0.display().to_string();
        assert!(!text.contains('"'), "temp path with a quote: {text}");
        format!("r\"{text}\"")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn sha256_matches_the_published_vectors() {
    executes(r#"
        takepkg std.hash;
        func m{
            out(hash.sha256(""));
            out(hash.sha256("abc"));
        }"#,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n\
         ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n");
}

#[test]
fn sha512_matches_the_published_vectors() {
    executes(r#"
        takepkg std.hash;
        func m{
            out(hash.sha512(""));
            out(hash.sha512("abc"));
        }"#,
        "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e\n\
         ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f\n");
}

#[test]
fn sha1_matches_the_published_vectors() {
    executes(r#"
        takepkg std.hash;
        func m{
            out(hash.sha1(""));
            out(hash.sha1("abc"));
            out(hash.sha1("The quick brown fox jumps over the lazy dog"));
        }"#,
        "da39a3ee5e6b4b0d3255bfef95601890afd80709\n\
         a9993e364706816aba3e25717850c26c9cd0d89d\n\
         2fd4e1c67a2d28fced849ee1bb76e7391b93eb12\n");
}

#[test]
fn md5_matches_the_published_vectors() {
    executes(r#"
        takepkg std.hash;
        func m{
            out(hash.md5(""));
            out(hash.md5("abc"));
            out(hash.md5("The quick brown fox jumps over the lazy dog"));
        }"#,
        "d41d8cd98f00b204e9800998ecf8427e\n\
         900150983cd24fb0d6963f7d28e17f72\n\
         9e107d9d372bb6826bd81d3542a419d6\n");
}

#[test]
fn crc32_is_eight_hex_digits_with_leading_zeros() {
    executes(r#"
        takepkg std.hash;
        func m{
            out(hash.crc32("123456789"));
            out(hash.crc32(""));
            out(hash.crc32("abc"));
        }"#,
        "cbf43926\n00000000\n352441c2\n");
}

#[test]
fn hmac_sha256_matches_rfc_4231_test_cases_1_and_2() {
    // Test case 1: a 20-byte key of 0x0b, and the message "Hi There".
    let key = "\u{b}".repeat(20);
    executes(&format!(r#"
        takepkg std.hash;
        func m{{
            out(hash.hmac_sha256("{key}", "Hi There"));
            out(hash.hmac_sha256("Jefe", "what do ya want for nothing?"));
        }}"#),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7\n\
         5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843\n");
}

#[test]
fn non_ascii_text_is_hashed_as_its_utf8_bytes() {
    let text = "héllo, 世界 🌍";
    let expected = hex(Sha256::digest(text.as_bytes()));
    executes(&format!(r#"
        takepkg std.hash;
        func m{{ out(hash.sha256("{text}")); }}"#),
        &format!("{expected}\n"));
    // The same bytes in a file hash to the same digest.
    let file = TempFile::new("utf8.txt", text.as_bytes());
    executes(&format!(r#"
        takepkg std.hash;
        func m{{ out(hash.file({}, "sha256")); }}"#, file.literal()),
        &format!("{expected}\n"));
}

#[test]
fn a_file_hashes_to_the_digest_of_its_bytes_for_every_algorithm() {
    let file = TempFile::new("abc.bin", b"abc");
    executes(&format!(r#"
        takepkg std.hash;
        func m{{
            out(hash.file({0}, "sha256"));
            out(hash.file({0}, "sha512"));
            out(hash.file({0}, "sha1"));
            out(hash.file({0}, "md5"));
            out(hash.file({0}, "crc32"));
        }}"#, file.literal()),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n\
         ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f\n\
         a9993e364706816aba3e25717850c26c9cd0d89d\n\
         900150983cd24fb0d6963f7d28e17f72\n\
         352441c2\n");
}

#[test]
fn a_binary_file_with_bytes_that_are_not_utf8_hashes_exactly() {
    // Every byte value, a NUL, and bytes that could not begin a UTF-8 sequence.
    let bytes: Vec<u8> = (0..=255u8).chain([0x00, 0xFF, 0xC3, 0x28, 0xFE, 0x80]).collect();
    let file = TempFile::new("binary.dat", &bytes);
    let expected = hex(Sha256::digest(&bytes));
    executes(&format!(r#"
        takepkg std.hash;
        func m{{ out(hash.file({}, "sha256")); }}"#, file.literal()),
        &format!("{expected}\n"));
}

#[test]
fn a_file_larger_than_one_chunk_is_read_completely() {
    // Three chunks and a bit: the last read is short, and every byte counts.
    let bytes: Vec<u8> = (0..(3 * 64 * 1024 + 17)).map(|i| (i * 31 % 251) as u8).collect();
    let file = TempFile::new("large.bin", &bytes);
    executes(&format!(r#"
        takepkg std.hash;
        func m{{
            out(hash.file({0}, "sha256"));
            out(hash.file({0}, "crc32"));
        }}"#, file.literal()),
        &format!("{}\n{:08x}\n", hex(Sha256::digest(&bytes)), crc32fast::hash(&bytes)));
}

#[test]
fn a_file_of_exactly_one_chunk_hashes_correctly() {
    let bytes = vec![b'x'; 64 * 1024];
    let file = TempFile::new("exact.bin", &bytes);
    executes(&format!(r#"
        takepkg std.hash;
        func m{{ out(hash.file({}, "md5")); }}"#, file.literal()),
        &format!("{}\n", hex(Md5::digest(&bytes))));
}

#[test]
fn an_empty_file_hashes_to_the_digest_of_nothing() {
    let file = TempFile::new("empty.txt", b"");
    executes(&format!(r#"
        takepkg std.hash;
        func m{{ out(hash.file({}, "sha256")); }}"#, file.literal()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n");
}

#[test]
fn a_missing_file_raises_file_not_found() {
    let missing = std::env::temp_dir().join("marslang-std-hash-definitely-missing.bin");
    let _ = std::fs::remove_file(&missing);
    let source = format!(r#"
        takepkg std.hash;
        takepkg std.file;
        func m{{
            run{{ hash.file({}, "sha256"); }} handle(file.NotFoundError e){{
                out(lasterr().message.contains("no such file or directory"));
            }}
        }}"#, format!("r\"{}\"", missing.display()));
    executes(&source, "true\n");
}

#[test]
fn a_directory_raises_file_is_directory() {
    let dir = std::env::temp_dir();
    let source = format!(r#"
        takepkg std.hash;
        takepkg std.file;
        func m{{
            run{{ hash.file({}, "md5"); }} handle(file.IsDirectoryError e){{
                out("directory");
            }}
        }}"#, format!("r\"{}\"", dir.display()));
    executes(&source, "directory\n");
}

#[test]
fn a_file_failure_message_starts_with_the_path() {
    let missing = std::env::temp_dir().join(format!("marslang-std-hash-{}-absent.bin", std::process::id()));
    let _ = std::fs::remove_file(&missing);
    let source = format!(r#"
        takepkg std.hash;
        takepkg std.file;
        func m{{
            run{{ hash.file({}, "sha256"); }} handle(file.FileError e){{
                out(lasterr().message.starts_with({}));
            }}
        }}"#, format!("r\"{}\"", missing.display()), format!("r\"{}\"", missing.display()));
    executes(&source, "true\n");
}

#[test]
fn an_unknown_algorithm_raises_range_error_listing_them() {
    runtime_error(r#"
        takepkg std.hash;
        func m{ hash.file("notes.txt", "sha3"); }"#,
        r#"hash.file needs an algorithm of "sha256", "sha512", "sha1", "md5", or "crc32", got "sha3""#);
}

#[test]
fn an_unknown_algorithm_is_refused_before_the_file_is_read() {
    // The file is missing, yet the algorithm is what is reported.
    runtime_error(r#"
        takepkg std.hash;
        func m{ hash.file("no-such-file-anywhere.txt", "SHA256"); }"#,
        r#"got "SHA256""#);
}

#[test]
fn a_non_string_argument_is_a_type_error() {
    runtime_error(r#"
        takepkg std.hash;
        func m{ hash.sha256(5); }"#,
        "TypeError");
}

#[test]
fn same_compares_every_byte_and_is_false_for_other_lengths() {
    executes(r#"
        takepkg std.hash;
        func m{
            digest = hash.hmac_sha256("Jefe", "what do ya want for nothing?");
            out(hash.same(digest, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"));
            out(hash.same(digest, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3844"));
            out(hash.same(digest, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec384"));
            out(hash.same("", ""));
            out(hash.same("", "x"));
            out(hash.same("abc", "ABC"));
        }"#,
        "true\nfalse\nfalse\ntrue\nfalse\nfalse\n");
}

#[test]
fn same_compares_non_ascii_text_by_its_bytes() {
    executes(r#"
        takepkg std.hash;
        func m{ out(hash.same("héllo", "héllo"), hash.same("héllo", "hello")); }"#,
        "true false\n");
}

