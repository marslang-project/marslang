//! std.secrets and std.uuid: the length and alphabet of every token, unbiased
//! whole numbers, choices and passwords with their errors, the constant-time
//! comparison, and UUID formats, versions, ordering, and checks.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

fn output(source: &str) -> String {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    stdout
}

fn executes(source: &str, expected: &str) {
    assert_eq!(output(source), expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

const LOWER_HEX: &str = "0123456789abcdef";
const PASSWORD_LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const PASSWORD_DIGITS: &str = "0123456789";
const PASSWORD_SYMBOLS: &str = "!@#$%^&*-_=+";

fn all_in(text: &str, alphabet: &str) -> bool {
    text.chars().all(|c| alphabet.contains(c))
}

/// The UUID's 8-4-4-4-12 layout, with lowercase hex everywhere else.
fn assert_layout(uuid: &str) {
    assert_eq!(uuid.len(), 36, "{uuid}");
    for (index, c) in uuid.chars().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            assert_eq!(c, '-', "{uuid}");
        } else {
            assert!(LOWER_HEX.contains(c), "{uuid}: {c} is not lowercase hex");
        }
    }
}

fn hex_digit(uuid: &str, index: usize) -> char {
    uuid.chars().nth(index).unwrap()
}

// ----- tokens -----

#[test]
fn token_hex_has_two_lowercase_hex_digits_per_byte() {
    let out = output(r##"
takepkg std.secrets;
func m{
    for (n, arr(0, 1, 5, 16, 33, 1000)){ out(secrets.token_hex(n)); }
}
"##);
    let tokens: Vec<&str> = out.lines().collect();
    let counts = [0, 1, 5, 16, 33, 1000];
    assert_eq!(tokens.len(), counts.len());
    for (token, n) in tokens.iter().zip(counts) {
        assert_eq!(token.len(), 2 * n, "token_hex({n}) gave {token}");
        assert!(all_in(token, LOWER_HEX), "{token}");
    }
}

#[test]
fn token_url_is_url_safe_base64_without_padding() {
    let out = output(r##"
takepkg std.secrets;
func m{
    for (n, arr(0, 1, 2, 3, 4, 5, 16, 33, 1000)){ out(secrets.token_url(n)); }
}
"##);
    let tokens: Vec<&str> = out.lines().collect();
    let counts = [0usize, 1, 2, 3, 4, 5, 16, 33, 1000];
    assert_eq!(tokens.len(), counts.len());
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    for (token, n) in tokens.iter().zip(counts) {
        // ceil(4n / 3) characters, and never the = padding or the + and / of plain base64.
        assert_eq!(token.len(), (4 * n + 2) / 3, "token_url({n}) gave {token}");
        assert!(all_in(token, alphabet), "{token}");
    }
}

#[test]
fn bytes_are_ints_from_0_to_255_in_the_requested_count() {
    executes(r##"
takepkg std.secrets;
takepkg std.types;
func m{
    out(secrets.bytes(0).len(), secrets.bytes(1).len(), secrets.bytes(65536).len());
    bad = 0;
    for (b, secrets.bytes(4000)){
        if (types.kind(b) != "int" or b < 0 or b > 255){ bad = bad + 1; }
    }
    out(bad);
}
"##, "0 1 65536\n0\n");
}

#[test]
fn bytes_use_the_whole_range() {
    // 4000 bytes: each value 0 to 255 is missing with probability about (255/256)^4000, so all appear.
    executes(r##"
takepkg std.secrets;
func m{
    seen = set();
    for (b, secrets.bytes(4000)){ seen.add(b); }
    out(seen.len());
}
"##, "256\n");
}

#[test]
fn tokens_and_bytes_reject_counts_out_of_range() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.bytes(-1)); }
"##, "secrets.bytes needs a count from 0 to 65536, got -1");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.bytes(65537)); }
"##, "secrets.bytes needs a count from 0 to 65536, got 65537");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.token_hex(-2)); }
"##, "secrets.token_hex needs a count from 0 to 65536, got -2");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.token_url(70000)); }
"##, "secrets.token_url needs a count from 0 to 65536, got 70000");
}

#[test]
fn bytes_reject_a_float_count() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.bytes(1.5)); }
"##, "TypeError");
}

// ----- whole numbers -----

#[test]
fn int_below_stays_in_range_and_is_roughly_uniform() {
    let out = output(r##"
takepkg std.secrets;
func m{
    for (i = 0, i < 6000, i++){ out(secrets.int_below(6)); }
}
"##);
    let mut counts = [0usize; 6];
    for line in out.lines() {
        let value: usize = line.parse().expect("int_below printed a non-number");
        assert!(value < 6, "int_below(6) gave {value}");
        counts[value] += 1;
    }
    assert_eq!(counts.iter().sum::<usize>(), 6000);
    // Expected 1000 of each; a standard deviation is about 29, so 850 to 1150 is over five.
    for (value, count) in counts.iter().enumerate() {
        assert!((850..=1150).contains(count), "value {value} came up {count} times in 6000");
    }
}

#[test]
fn int_below_one_is_always_zero() {
    executes(r##"
takepkg std.secrets;
func m{
    total = 0;
    for (i = 0, i < 200, i++){ total = total + secrets.int_below(1); }
    out(total);
}
"##, "0\n");
}

#[test]
fn int_below_gives_an_int_for_an_int_and_a_longint_for_a_longint() {
    executes(r##"
takepkg std.secrets;
takepkg std.types;
func m{
    out(types.kind(secrets.int_below(10)));
    out(types.kind(secrets.int_below(10000000000)));
    big = secrets.int_below(9223372036854775807);
    out(types.kind(big), big >= 0, big < 9223372036854775807);
}
"##, "int\nlongint\nlongint true true\n");
}

#[test]
fn int_below_handles_counts_past_32_bits() {
    // Counts past 32 bits need more bytes per draw; every draw must still land in range.
    executes(r##"
takepkg std.secrets;
func m{
    limit = 10000000000;
    ok = true;
    for (i = 0, i < 200, i++){
        value = secrets.int_below(limit);
        if (value < 0 or value >= limit){ ok = false; }
    }
    out(ok);
}
"##, "true\n");
}

#[test]
fn int_below_rejects_zero_and_negative_counts() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.int_below(0)); }
"##, "secrets.int_below needs a count above 0, got 0");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.int_below(-3)); }
"##, "secrets.int_below needs a count above 0, got -3");
}

// ----- choices and passwords -----

#[test]
fn choice_picks_only_from_the_items() {
    let out = output(r##"
takepkg std.secrets;
func m{
    for (i = 0, i < 300, i++){ out(secrets.choice(arr("a", "b", "c"))); }
    out(secrets.choice(arr(7)));
}
"##);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 301);
    let seen: HashSet<&str> = lines[..300].iter().copied().collect();
    assert_eq!(seen, HashSet::from(["a", "b", "c"]));
    assert!(lines[..300].iter().all(|line| ["a", "b", "c"].contains(line)));
    assert_eq!(lines[300], "7");
}

#[test]
fn choice_rejects_an_empty_array() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.choice(arr())); }
"##, "secrets.choice needs at least one item");
}

#[test]
fn password_has_the_requested_length_and_alphabet() {
    let out = output(r##"
takepkg std.secrets;
func m{
    for (n, arr(1, 2, 3, 4, 5, 8, 12, 16, 40, 1024)){
        for (i = 0, i < 20, i++){ out(secrets.password(n)); }
    }
}
"##);
    let lengths = [1usize, 2, 3, 4, 5, 8, 12, 16, 40, 1024];
    let alphabet = format!("{PASSWORD_LETTERS}{PASSWORD_DIGITS}{PASSWORD_SYMBOLS}");
    let passwords: Vec<&str> = out.lines().collect();
    assert_eq!(passwords.len(), lengths.len() * 20);
    for (index, password) in passwords.iter().enumerate() {
        let length = lengths[index / 20];
        assert_eq!(password.chars().count(), length, "password of length {length} was {password}");
        assert!(all_in(password, &alphabet), "{password}");
    }
}

fn assert_every_class(password: &str) {
    assert!(password.chars().any(|c| c.is_ascii_lowercase()), "no lowercase: {password}");
    assert!(password.chars().any(|c| c.is_ascii_uppercase()), "no uppercase: {password}");
    assert!(password.chars().any(|c| c.is_ascii_digit()), "no digit: {password}");
    assert!(password.chars().any(|c| PASSWORD_SYMBOLS.contains(c)), "no symbol: {password}");
}

#[test]
fn password_from_length_4_has_every_class() {
    // Every password of length 4 or more has a lowercase letter, an uppercase letter, a digit, and a symbol.
    let out = output(r##"
takepkg std.secrets;
func m{
    for (n, arr(4, 5, 6, 8, 12, 40)){
        for (i = 0, i < 100, i++){ out(secrets.password(n)); }
    }
    for (i = 0, i < 3, i++){ out(secrets.password(1024)); }
}
"##);
    let passwords: Vec<&str> = out.lines().collect();
    assert_eq!(passwords.len(), 6 * 100 + 3);
    for password in &passwords {
        assert_every_class(password);
    }
    assert_eq!(passwords[600..].iter().map(|p| p.len()).collect::<Vec<_>>(), [1024, 1024, 1024]);
}

#[test]
fn password_rejects_lengths_out_of_range() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.password(0)); }
"##, "secrets.password needs a length from 1 to 1024, got 0");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.password(1025)); }
"##, "secrets.password needs a length from 1 to 1024, got 1025");
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.password(-4)); }
"##, "secrets.password needs a length from 1 to 1024, got -4");
}

// ----- comparison -----

#[test]
fn same_compares_whole_strings() {
    executes(r##"
takepkg std.secrets;
func m{
    out(secrets.same("abc", "abc"), secrets.same("abc", "abd"));
    out(secrets.same("abc", "ab"), secrets.same("", ""), secrets.same("a", "A"));
}
"##, "true false\nfalse true false\n");
}

#[test]
fn same_rejects_a_non_string() {
    runtime_error(r##"
takepkg std.secrets;
func m{ out(secrets.same(1, "1")); }
"##, "TypeError");
}

// ----- UUID v4 -----

#[test]
fn v4_has_the_layout_version_and_variant() {
    let out = output(r##"
takepkg std.uuid;
func m{
    for (i = 0, i < 1000, i++){ out(uuid.v4()); }
}
"##);
    let ids: Vec<&str> = out.lines().collect();
    assert_eq!(ids.len(), 1000);
    for id in &ids {
        assert_layout(id);
        assert_eq!(hex_digit(id, 14), '4', "version digit of {id}");
        assert!("89ab".contains(hex_digit(id, 19)), "variant digit of {id}");
    }
}

#[test]
fn a_thousand_v4s_are_all_distinct() {
    let out = output(r##"
takepkg std.uuid;
func m{
    ids = set();
    for (i = 0, i < 1000, i++){ ids.add(uuid.v4()); }
    out(ids.len());
}
"##);
    assert_eq!(out, "1000\n");
}

// ----- UUID v7 -----

#[test]
fn v7_has_the_layout_version_variant_and_current_time() {
    let out = output(r##"
takepkg std.uuid;
func m{
    for (i = 0, i < 50, i++){ out(uuid.v7()); }
}
"##);
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    let ids: Vec<&str> = out.lines().collect();
    assert_eq!(ids.len(), 50);
    for id in &ids {
        assert_layout(id);
        assert_eq!(hex_digit(id, 14), '7', "version digit of {id}");
        assert!("89ab".contains(hex_digit(id, 19)), "variant digit of {id}");
        let prefix: String = id[..8].to_string() + &id[9..13];
        let millis = i64::from_str_radix(&prefix, 16).unwrap();
        assert!((millis - now_ms).abs() < 60_000, "{id} has time {millis}, now is {now_ms}");
    }
}

#[test]
fn v7_values_increase_across_separated_calls() {
    let out = output(r##"
takepkg std.uuid;
takepkg std.time;
func m{
    for (i = 0, i < 5, i++){
        a = uuid.v7();
        time.sleep(0.002);
        b = uuid.v7();
        out(a < b);
    }
}
"##);
    assert_eq!(out, "true\ntrue\ntrue\ntrue\ntrue\n");
}

// ----- checks -----

#[test]
fn valid_accepts_uuids_in_either_case() {
    executes(r##"
takepkg std.uuid;
func m{
    out(uuid.valid("5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b13"));
    out(uuid.valid("5B1E9C2A-7D40-4F3E-9A61-0C8D2E7F4B13"));
    out(uuid.valid("5b1E9c2A-7d40-4F3e-9a61-0C8d2e7f4B13"));
    out(uuid.valid(uuid.nil()));
    out(uuid.valid(uuid.v4()), uuid.valid(uuid.v7()));
}
"##, "true\ntrue\ntrue\ntrue\ntrue true\n");
}

#[test]
fn valid_rejects_malformed_text() {
    executes(r##"
takepkg std.uuid;
func m{
    out(uuid.valid(""));
    out(uuid.valid("5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b1"));
    out(uuid.valid("5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b133"));
    out(uuid.valid("5b1e9c2a7d404f3e9a610c8d2e7f4b13"));
    out(uuid.valid("5b1e9c2a_7d40-4f3e-9a61-0c8d2e7f4b13"));
    out(uuid.valid("5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b1g"));
    out(uuid.valid("{5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b13}"));
    out(uuid.valid(" 5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b13"));
    out(uuid.valid("5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b1 "));
}
"##, "false\nfalse\nfalse\nfalse\nfalse\nfalse\nfalse\nfalse\nfalse\n");
}

#[test]
fn valid_rejects_a_non_string() {
    runtime_error(r##"
takepkg std.uuid;
func m{ out(uuid.valid(5)); }
"##, "TypeError");
}

#[test]
fn version_reads_the_version_digit() {
    executes(r##"
takepkg std.uuid;
func m{
    out(uuid.version(uuid.v4()), uuid.version(uuid.v7()));
    out(uuid.version("c232ab00-9414-11ec-b3c8-9e6bdeced846"));
    out(uuid.version("C232AB00-9414-11EC-B3C8-9E6BDECED846"));
    out(uuid.version("00000000-0000-a000-8000-000000000000"));
    out(uuid.version(uuid.nil()));
}
"##, "4 7\n1\n1\n10\n0\n");
}

#[test]
fn version_rejects_text_that_is_not_a_uuid() {
    runtime_error(r##"
takepkg std.uuid;
func m{ out(uuid.version("not-a-uuid")); }
"##, "SyntaxError: uuid.version needs a UUID in 8-4-4-4-12 hex form, got not-a-uuid");
    runtime_error(r##"
takepkg std.uuid;
func m{ out(uuid.version("")); }
"##, "SyntaxError");
}

#[test]
fn nil_is_all_zeros() {
    executes(r##"
takepkg std.uuid;
func m{ out(uuid.nil()); }
"##, "00000000-0000-0000-0000-000000000000\n");
}
