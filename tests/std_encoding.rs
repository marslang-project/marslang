//! Tests for std.encoding (std/encoding.mars with std/rs/encoding.rs).

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

/// Wrap one or more statements in a program that has the package.
fn program(body: &str) -> String {
    format!("takepkg std.encoding;\nfunc m{{\n{body}\n}}\n")
}
fn runs(body: &str, expected: &str) { executes(&program(body), expected); }
fn fails(body: &str, expected: &str) { runtime_error(&program(body), expected); }

#[test]
fn rfc4648_vectors() {
    let vectors = [
        ("", "", "", ""),
        ("f", "Zg==", "Zg", "66"),
        ("fo", "Zm8=", "Zm8", "666f"),
        ("foo", "Zm9v", "Zm9v", "666f6f"),
        ("foob", "Zm9vYg==", "Zm9vYg", "666f6f62"),
        ("fooba", "Zm9vYmE=", "Zm9vYmE", "666f6f6261"),
        ("foobar", "Zm9vYmFy", "Zm9vYmFy", "666f6f626172"),
    ];
    for (text, b64, b64url, hex) in vectors {
        runs(
            &format!(r#"out(encoding.base64("{text}") + "|" + encoding.base64_url("{text}") + "|" + encoding.hex("{text}"));"#),
            &format!("{b64}|{b64url}|{hex}\n"),
        );
        runs(
            &format!(
                r#"out(encoding.from_base64("{b64}") == "{text}", encoding.from_base64_url("{b64url}") == "{text}", encoding.from_hex("{hex}") == "{text}");"#
            ),
            "true true true\n",
        );
    }
}

#[test]
fn non_ascii_text_round_trips() {
    runs(
        r#"
        t = "héllo 中文 😀";
        out(encoding.hex(t));
        out(encoding.base64(t));
        out(encoding.from_base64(encoding.base64(t)) == t);
        out(encoding.from_base64_url(encoding.base64_url(t)) == t);
        out(encoding.from_hex(encoding.hex(t)) == t);
        out(encoding.from_bytes(encoding.bytes(t)) == t);
        out(encoding.from_url(encoding.url(t)) == t);
        "#,
        "68c3a96c6c6f20e4b8ade6968720f09f9880\naMOpbGxvIOS4reaWhyDwn5iA\ntrue\ntrue\ntrue\ntrue\ntrue\n",
    );
}

#[test]
fn base64_alphabets_and_padding() {
    // 0xFB 0xFF is "+/8=" standard and "-_8" url-safe.
    runs(
        r#"
        out(encoding.base64_bytes(arr(251, 255)));
        out(encoding.from_base64_bytes("+/8="), encoding.from_base64_bytes("+/8"));
        out(encoding.from_base64_url("fn4_") == "~~?", encoding.from_base64("fn4/") == "~~?");
        "#,
        "+/8=\n[251, 255] [251, 255]\ntrue true\n",
    );
    // from_base64_url and from_base64 cannot be checked on bytes, but url is
    // reachable for text: "~~" is 0x7E 0x7E = "fn4".
    runs(r#"out(encoding.base64_url("~~?"), encoding.base64("~~?"));"#, "fn4_ fn4/\n");
}

#[test]
fn base64_decoding_is_lenient_about_padding_and_whitespace() {
    runs(
        r#"
        out(encoding.from_base64("Zm8"), encoding.from_base64("Zm8="), encoding.from_base64("Zg"), encoding.from_base64("Zg=="));
        out(encoding.from_base64("Zm9v\r\nYmFy\n"));
        out(encoding.from_base64("  Zm9v YmFy\t"));
        out(encoding.from_base64_url("Zm9vYg=="), encoding.from_base64_url("Zm9v\nYg"));
        "#,
        "fo fo f f\nfoobar\nfoobar\nfoob foob\n",
    );
}

#[test]
fn bytes_arrays() {
    runs(
        r#"
        out(encoding.bytes(""));
        out(encoding.bytes("hé"));
        out(encoding.bytes("中"));
        out(encoding.bytes("😀"));
        out(encoding.from_bytes(arr(104, 195, 169)));
        out(encoding.from_bytes(arr()) == "");
        "#,
        "[]\n[104, 195, 169]\n[228, 184, 173]\n[240, 159, 152, 128]\nhé\ntrue\n",
    );
}

#[test]
fn binary_data_goes_through_base64_bytes() {
    runs(
        r#"
        data = arr(0, 1, 2, 253, 254, 255, 128);
        text = encoding.base64_bytes(data);
        out(text);
        back = encoding.from_base64_bytes(text);
        out(back, string(back) == string(data));
        out("[" + encoding.base64_bytes(arr()) + "]", encoding.from_base64_bytes(""));
        out(encoding.base64_bytes(arr(0, 255, 16)), encoding.from_base64_bytes("AP8Q"));
        out(encoding.base64_bytes(encoding.bytes("foobar")));
        "#,
        "AAEC/f7/gA==\n[0, 1, 2, 253, 254, 255, 128] true\n[] []\nAP8Q [0, 255, 16]\nZm9vYmFy\n",
    );
}

#[test]
fn url_encoding() {
    runs(
        r#"
        out(encoding.url("AZaz09-._~"));
        out(encoding.url("a b"));
        out(encoding.url("a/b?c=d&e#f"));
        out(encoding.url(":/?#[]@!$&'()*+,;=% \"<>"));
        out(encoding.url("é中😀"));
        out(encoding.url(""));
        "#,
        "AZaz09-._~\na%20b\na%2Fb%3Fc%3Dd%26e%23f\n%3A%2F%3F%23%5B%5D%40%21%24%26%27%28%29%2A%2B%2C%3B%3D%25%20%22%3C%3E\n%C3%A9%E4%B8%AD%F0%9F%98%80\n\n",
    );
}

#[test]
fn url_decoding_keeps_plus() {
    runs(
        r#"
        out(encoding.from_url("a%20b%2fc%2Fd"));
        out(encoding.from_url("a+b"));
        out(encoding.from_url("%C3%A9%E4%B8%AD"));
        out(encoding.from_url("plain~text"));
        out(encoding.from_url("é"));
        "#,
        "a b/c/d\na+b\né中\nplain~text\né\n",
    );
}

#[test]
fn query_building() {
    runs(
        r#"
        out(encoding.query(map().set("a", 1).set("b", "two words")));
        out(encoding.query(map()));
        out(encoding.query(map().set("k&v", "x=y").set("é", true).set("n", null)));
        out(encoding.query(map().set("z", 1).set("a", 2)));
        "#,
        "a=1&b=two%20words\n\nk%26v=x%3Dy&%C3%A9=true&n=null\nz=1&a=2\n",
    );
}

#[test]
fn query_parsing() {
    runs(
        r#"
        out(encoding.from_query("a=1&b=two%20words"));
        out(encoding.from_query("?q=two+words&page=2"));
        out(encoding.from_query("a=1+2%2B3"));
        out(encoding.from_query("flag&empty=&x=1"));
        out(encoding.from_query("a=1&a=2&b=3&a=4"));
        out(encoding.from_query(""), encoding.from_query("?"), encoding.from_query("&&"));
        out(encoding.from_query("a=1&&b=2&"));
        out(encoding.from_query("k=a=b"));
        out(encoding.from_query("%C3%A9=%E4%B8%AD"));
        out(encoding.from_query("=v"));
        "#,
        concat!(
            "{\"a\": \"1\", \"b\": \"two words\"}\n",
            "{\"q\": \"two words\", \"page\": \"2\"}\n",
            "{\"a\": \"1 2+3\"}\n",
            "{\"flag\": \"\", \"empty\": \"\", \"x\": \"1\"}\n",
            "{\"a\": \"4\", \"b\": \"3\"}\n",
            "{} {} {}\n",
            "{\"a\": \"1\", \"b\": \"2\"}\n",
            "{\"k\": \"a=b\"}\n",
            "{\"é\": \"中\"}\n",
            "{\"\": \"v\"}\n",
        ),
    );
}

#[test]
fn query_round_trip() {
    runs(
        r#"
        original = map().set("name", "Ada Lovelace").set("note", "a+b=c&d").set("emoji", "😀");
        back = encoding.from_query(encoding.query(original));
        out(string(back) == string(original), back.get("note"), back.keys());
        "#,
        "true a+b=c&d [\"name\", \"note\", \"emoji\"]\n",
    );
}

#[test]
fn errors_name_the_function_and_position() {
    let cases = [
        (r#"encoding.from_base64("ab!d");"#, r#"encoding.from_base64: character 3 is "!" which is not in the base64 alphabet"#),
        (r#"encoding.from_base64("ab-d");"#, "use encoding.from_base64_url"),
        (r#"encoding.from_base64_url("ab+d");"#, "use encoding.from_base64"),
        (r#"encoding.from_base64_url("ab/d");"#, "encoding.from_base64_url: character 3"),
        (r#"encoding.from_base64_bytes("ab d?");"#, "encoding.from_base64_bytes: character 5"),
        (r#"encoding.from_base64("Zm9vY");"#, "5 base64 characters is not a valid length"),
        (r#"encoding.from_base64("Zg=a");"#, r#"character 4 is "a" after the = padding"#),
        (r#"encoding.from_base64("Zg===");"#, "the = padding is wrong"),
        (r#"encoding.from_base64("/w==");"#, "not valid UTF-8 text; byte 1 is invalid (for binary data, use encoding.from_base64_bytes)"),
        (r#"encoding.from_base64_url("_w");"#, "use encoding.from_base64_bytes"),
        (r#"encoding.from_hex("abc");"#, "encoding.from_hex: odd number of hex digits (3)"),
        (r#"encoding.from_hex("6g");"#, r#"encoding.from_hex: character 2 is "g" which is not a hex digit"#),
        (r#"encoding.from_hex("c3");"#, "encoding.from_hex: the decoded bytes are not valid UTF-8 text; byte 1"),
        (r#"encoding.from_bytes(arr(104, 256));"#, "encoding.from_bytes: byte 2 is 256, which is not from 0 to 255"),
        (r#"encoding.from_bytes(arr(-1));"#, "byte 1 is -1"),
        (r#"encoding.from_bytes(arr(255));"#, "encoding.from_bytes: the decoded bytes are not valid UTF-8"),
        (r#"encoding.base64_bytes(arr(1, 300));"#, "encoding.base64_bytes: byte 2 is 300"),
        (r#"encoding.from_url("ab%4");"#, r#"encoding.from_url: bad percent-escape at character 3 ("%4")"#),
        (r#"encoding.from_url("%");"#, "bad percent-escape at character 1"),
        (r#"encoding.from_url("%zz");"#, r#"("%zz")"#),
        (r#"encoding.from_url("%FF");"#, "encoding.from_url: the decoded bytes are not valid UTF-8"),
        (r#"encoding.from_query("a=1&b=%zz");"#, "encoding.from_query: bad percent-escape at character 7"),
        (r#"encoding.from_query("?%");"#, "encoding.from_query: bad percent-escape at character 2"),
        (r#"encoding.from_query("%zz=1");"#, "bad percent-escape at character 1"),
    ];
    for (body, expected) in cases {
        fails(body, expected);
    }
}

#[test]
fn errors_are_syntax_errors_and_wrong_types_are_type_errors() {
    fails(r#"encoding.from_hex("zz");"#, "SyntaxError");
    fails(r#"encoding.from_base64("!");"#, "SyntaxError");
    fails(r#"encoding.from_url("%");"#, "SyntaxError");
    fails(r#"encoding.from_bytes(arr(256));"#, "RangeError");
    fails(r#"encoding.base64_bytes(arr(-5));"#, "RangeError");
    fails(r#"encoding.from_bytes(arr("a"));"#, "TypeError");
    fails(r#"encoding.base64_bytes(arr(1.5));"#, "TypeError");
    fails(r#"encoding.base64(5);"#, "TypeError");
    fails(r#"encoding.from_base64(5);"#, "TypeError");
    fails(r#"encoding.hex(5);"#, "TypeError");
    fails(r#"encoding.url(5);"#, "TypeError");
    fails(r#"encoding.bytes(5);"#, "TypeError");
    fails(r#"encoding.from_bytes("abc");"#, "TypeError");
    fails(r#"encoding.query("a=1");"#, "TypeError");
    fails(r#"encoding.from_query(arr());"#, "TypeError");
}

#[test]
fn trailing_bits_and_odd_but_valid_input() {
    // Non-zero bits left over in the last character are accepted.
    runs(r#"out(encoding.from_base64("Zh"));"#, "f\n");
    runs(r#"out(encoding.from_hex("") == "", encoding.from_hex("4A4a"));"#, "true JJ\n");
}
