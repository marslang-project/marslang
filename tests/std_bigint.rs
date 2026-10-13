//! The bigint kind (src/bigint.rs) and the std.bigint package.

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

const FACTORIAL_100: &str = "93326215443944152681699238856266700490715968264381621468592963895217599993229915608941463976156518286253697920827223758251185210916864000000000000000000000000";

#[test]
fn factorial_of_100_and_two_to_the_200() {
    assert_eq!(FACTORIAL_100.len(), 158);
    executes(r#"
        takepkg std.bigint;
        func m{
            out(bigint.factorial(100));
            out(bigint.of(2) ** 200);
            // The operators alone give the same as the package's product tree.
            product = bigint.of(1);
            for (i = 2, i <= 1000, i++){ product = product * i; }
            out(product == bigint.factorial(1000), string(product).len());
        }"#,
        &format!("{FACTORIAL_100}\n1606938044258990275541962092341162602522202993782792835301376\ntrue 2568\n"));
}

#[test]
fn mixing_with_ints_and_longints_gives_bigints() {
    executes(r#"
        takepkg std.types;
        func m{
            a = bigint(10);
            out(a + 1, 1 + a, a - longint(3), longint(3) - a, a * 2, 2 * a);
            out(types.kind(a + 1), types.kind(1 + a), types.kind(longint(2) * a), types.kind(a - a));
            out(-a, +a, a ** 2, 2 ** bigint(100), types.kind(2 ** bigint(3)));
            // A result stays a bigint even when it is small.
            out(types.kind(bigint(2) - 1), types.kind(-bigint(0)));
            out(2147483647 + bigint(1), 9223372036854775807 + bigint(1));
        }"#,
        "11 11 7 -7 20 20\nbigint bigint bigint bigint\n-10 10 100 1267650600228229401496703205376 bigint\nbigint bigint\n2147483648 9223372036854775808\n");
}

#[test]
fn mixing_with_floats_needs_a_conversion() {
    for expression in ["bigint(1) + 1.0", "1.5 * bigint(2)", "bigint(4) / 2.0", "bigint(1) % 1.5", "bigint(4) ** 0.5", "2.0 ** bigint(3)"] {
        runtime_error(&format!("func m{{ out({expression}); }}"), "TypeError: mixed bigint/float arithmetic");
    }
    executes("func m{ out(float(bigint(3)) + 0.5, bigint(3) + bigint(2.0)); }", "3.5 5\n");
}

#[test]
fn comparisons_are_exact_against_every_kind() {
    executes(r#"
        func m{
            // 2^53 + 1 is the first integer a float cannot hold: rounding it would make these wrong.
            b = bigint(2) ** 53 + 1;
            out(b == 9007199254740992.0, b != 9007199254740992.0, b > 9007199254740992.0, b < 9007199254740994.0);
            out(9007199254740992.0 < b, 9007199254740994.0 > b, b == longint(9007199254740993), b == 9007199254740993);
            out(bigint(1) == 1, bigint(1) == 1.0, bigint(1) == longint(1), bigint(1) == "1");
            out(bigint(3) < 3.5, bigint(3) > 2.5, bigint(-3) < -2.5, bigint(-3) > -3.5, bigint(3) <= 3.0, bigint(3) >= 3.0);
            nan = float("nan");
            out(bigint(5) == nan, bigint(5) < nan, bigint(5) < float("inf"), bigint(5) > float("-inf"));
            huge = bigint(10) ** 400;
            out(huge > 1e308, huge < float("inf"), -huge < -1e308, bigint(2) ** 1023 == 2.0 ** 1023);
            out(bigint(10) ** 30 > bigint(10) ** 29, -(bigint(10) ** 30) < 5, bigint(7) > longint(-9223372036854775807));
            out(arr(bigint(3), 1, 2.5, longint(-4), bigint(10) ** 20).asort());
        }"#,
        "false true true true\ntrue true true true\ntrue true true false\ntrue true true true true true\nfalse false true true\ntrue true true true\ntrue true true\n[-4, 1, 2.5, 3, 100000000000000000000]\n");
    runtime_error("func m{ out(bigint(5) < \"a\"); }", "comparison requires matching types");
}

#[test]
fn equal_numbers_are_one_map_key_and_set_element() {
    executes(r#"
        func m{
            d = map();
            d.set(1, "a");
            out(d.get(bigint(1)), d.has(longint(1)), d.has(1.0));
            d.set(bigint(1), "b");
            out(d.len(), d.get(1), d.keys());
            out(set(1, bigint(1), 1.0, longint(1)).len(), set(-0.0, bigint(0)).len());
            // Beyond longint: a float that holds the number exactly is the same key.
            out(set(1e20, bigint(10) ** 20).len(), set(1e20, bigint(10) ** 20 + 1).len());
            big = map();
            big.set(bigint(10) ** 30, "x");
            out(big.get(bigint("1000000000000000000000000000000")), big.has(1e30), big.has(bigint(10) ** 30 + 1));
            out(arr(1, bigint(2)).has(2.0), arr(1, bigint(2)).get(longint(2)));
            s = set(bigint(10) ** 25);
            s.remove(bigint(10) ** 25);
            out(s.len());
        }"#,
        "a true true\n1 b [1]\n1 1\n1 2\nx false false\ntrue 1\n0\n");
}

#[test]
fn conversions_to_and_from_bigint() {
    executes(r#"
        takepkg std.types;
        func m{
            out(int(bigint(5)), types.kind(int(bigint(5))), int(-(bigint(2) ** 31)));
            out(longint(bigint(2) ** 62), types.kind(longint(bigint(5))), longint(-(bigint(2) ** 63)));
            out(float(bigint(2) ** 70), float(bigint(-3)), types.kind(float(bigint(1))));
            out(float(bigint(2) ** 53 + 1), float(bigint(2) ** 1023) == 2.0 ** 1023);
            out(string(bigint(-42)), types.kind(string(bigint(1))));
            out(bigint(5), bigint(longint(5)), bigint(bigint(5)), bigint(5.0), bigint(-1e20), bigint(-0.0));
            out(bigint("-123"), bigint("007"), bigint("-0"), types.kind(bigint("1")));
            out(bigint("123456789012345678901234567890123456789012345678901234567890"));
        }"#,
        "5 int -2147483648\n4611686018427387904 longint -9223372036854775808\n1.1805916207174113e+21 -3 float\n9007199254740992 true\n-42 string\n5 5 5 5 -100000000000000000000 0\n-123 7 0 bigint\n123456789012345678901234567890123456789012345678901234567890\n");

    runtime_error("func m{ out(int(bigint(2) ** 31)); }", "RangeError: int overflow");
    runtime_error("func m{ out(longint(bigint(2) ** 63)); }", "RangeError: longint overflow");
    runtime_error("func m{ out(float(bigint(10) ** 309)); }", "RangeError: the bigint is too large for a float");
    for text in ["12x", "", " 1", "+1", "1_000", "0x10", "1.5", "--1", "١٢"] {
        runtime_error(&format!("func m{{ out(bigint(\"{text}\")); }}"), &format!("SyntaxError: cannot convert {text:?} to a bigint"));
    }
    for float in ["2.5", "float(\"inf\")", "float(\"nan\")"] {
        runtime_error(&format!("func m{{ out(bigint({float})); }}"), "RangeError: bigint() needs a whole, finite float");
    }
    runtime_error("func m{ out(bigint(null)); }", "TypeError: bigint() expects an int, longint, bigint, float, or string, got null");
    runtime_error("func m{ out(bigint(arr())); }", "got array");
    runtime_error("func m{ out(bigint()); }", "TypeError: bigint() expects 1 argument(s), got 0");
}

#[test]
fn text_display_has_every_digit() {
    executes(r#"
        func m{
            x = bigint(10) ** 25;
            out(arr(bigint(1), x), map().set(bigint(5), -x), pair(x, "x"));
            out(f"{x}", string(x), "n=" + string(bigint(-7)));
            slout(x);
            out("");
        }"#,
        "[1, 10000000000000000000000000] {5: -10000000000000000000000000} pair(10000000000000000000000000, \"x\")\n10000000000000000000000000 10000000000000000000000000 n=-7\n10000000000000000000000000\n");
}

#[test]
fn division_and_remainders_with_signs() {
    executes(r#"
        takepkg std.bigint = big;
        takepkg std.types;
        func m{
            out(bigint(7) / 2, bigint(-7) / 2, bigint(8) / 2, types.kind(bigint(8) / 2), types.kind(bigint(7) / 2));
            out(7 / bigint(2), longint(-8) / bigint(-2));
            out(bigint(-7) % 2, bigint(7) % -2, bigint(-7) % -2, bigint(6) % 3, types.kind(bigint(6) % 3));
            out(big.divide(-7, 2), big.remainder(-7, 2), big.divide(7, -2), big.remainder(7, -2));
            out(big.divide(-7, -2), big.remainder(-7, -2), big.divide(6, -3), big.remainder(6, -3), big.divide(7, 2));
            out(types.kind(big.divide(7, 2)), types.kind(big.remainder(7, 2)));
            // a == b * divide(a, b) + remainder(a, b), whatever the signs.
            ok = true;
            for (a, arr(bigint(10) ** 30 + 7, -(bigint(10) ** 30) - 7, 0, 5, -5)){
                for (b, arr(3, -3, bigint(10) ** 20 + 1, -(bigint(10) ** 20))){
                    if (a != b * big.divide(a, b) + big.remainder(a, b)){ ok = false; }
                    rest = big.remainder(a, b);
                    if (rest != 0 and (rest < 0) != (b < 0)){ ok = false; }
                }
            }
            out(ok);
            huge = bigint(10) ** 400;
            out(huge / bigint(10) ** 200 == bigint(10) ** 200, (huge + 1) / (huge * 3), 1 / huge);
        }"#,
        "3.5 -3.5 4 bigint float\n3.5 4\n-1 1 -1 0 bigint\n-4 1 -4 -1\n3 -1 -2 0 3\nbigint bigint\ntrue\ntrue 0.3333333333333333 0\n");
    for expression in ["bigint(1) / 0", "bigint(1) % 0", "1 / bigint(0)", "longint(1) % bigint(0)", "bigint(0) ** -1"] {
        runtime_error(&format!("func m{{ out({expression}); }}"), "RangeError: division by zero");
    }
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.divide(1, 0)); }", "RangeError: division by zero");
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.remainder(1, 0)); }", "RangeError: division by zero");
    runtime_error("func m{ out((bigint(10) ** 400 + 1) / 3); }", "RangeError: the quotient is not whole and is too large for a float");
}

#[test]
fn powers_and_their_size_limit() {
    executes(r#"
        takepkg std.types;
        func m{
            out(bigint(2) ** -2, bigint(-1) ** -3, types.kind(bigint(-1) ** -3), bigint(1) ** -5, bigint(10) ** -400);
            out(bigint(0) ** 0, bigint(0) ** 5, bigint(-1) ** 100000000000001, bigint(1) ** bigint("100000000000000000000000"));
            out(bigint(-2) ** 3, bigint(2) ** 0, types.kind(bigint(2) ** 0));
            // The largest power of two within the limit of 3321929 bits.
            out(bigint(2) ** 3321928 > 0);
        }"#,
        "0.25 -1 bigint 1 0\n1 0 -1 1\n-8 1 bigint\ntrue\n");
    for expression in [
        "bigint(2) ** 100000000000", "bigint(2) ** 3321929", "bigint(10) ** bigint(\"1000000000000000000000\")",
        "(bigint(2) ** 3321928) * 2", "(bigint(2) ** 2000000) * (bigint(2) ** 2000000)", "-(bigint(2) ** 3321928) - bigint(2) ** 3321928",
        "3 ** bigint(3000000)",
    ] {
        runtime_error(&format!("func m{{ out({expression}); }}"), "RangeError: bigint too large: the limit is 3321929 bits");
    }
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.pow(2, -1)); }", "RangeError: bigint.pow needs an exponent of 0 or more, got -1");
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.factorial(205023)); }", "RangeError: bigint.factorial(n) has more than a million digits");
}

#[test]
fn big_literals_are_bigints() {
    executes(r#"
        takepkg std.types;
        hot H = 99999999999999999999;
        func m{
            x = 123456789012345678901234567890;
            out(x, types.kind(x), -123456789012345678901234567890, types.kind(-123456789012345678901234567890));
            out(types.kind(2147483647), types.kind(2147483648), types.kind(9223372036854775807), types.kind(9223372036854775808));
            out(types.kind(-9223372036854775808), types.kind(-9223372036854775809), -9223372036854775809);
            out(H + 1, 340282366920938463463374607431768211456 == bigint(2) ** 128);
        }"#,
        "123456789012345678901234567890 bigint -123456789012345678901234567890 bigint\nint longint longint bigint\nlongint bigint -9223372036854775809\n100000000000000000000 true\n");
    // Past the size limit a literal compiles, and raises when it is reached.
    runtime_error(&format!("func m{{ out({}); }}", "9".repeat(1_000_002)), "RangeError: bigint too large");
    // Editors are told the kind.
    let symbols = marslang::symbols("func m{ x = 99999999999999999999; y = bigint(5); }", std::path::Path::new(".")).expect("symbols");
    assert!(symbols.contains(r#""name":"x","kind":"variable","type":"bigint""#), "{symbols}");
    assert!(symbols.contains(r#""name":"y","kind":"variable","type":"bigint""#), "{symbols}");
}

#[test]
fn json_reads_and_writes_bigints() {
    executes(r#"
        takepkg std.json;
        takepkg std.types;
        func m{
            v = json.parse("123456789012345678901234567890");
            out(v, types.kind(v), json.stringify(v), json.parse(json.stringify(v)) == v);
            doc = json.parse("{\"id\": 123456789012345678901234567890, \"small\": 5, \"neg\": -99999999999999999999}");
            out(json.stringify(doc));
            out(types.kind(doc.get("small")), types.kind(doc.get("neg")), json.stringify(arr(bigint(1), bigint(10) ** 20)));
        }"#,
        "123456789012345678901234567890 bigint 123456789012345678901234567890 true\n{\"id\":123456789012345678901234567890,\"small\":5,\"neg\":-99999999999999999999}\nint bigint [1,100000000000000000000]\n");
    let digits = "9".repeat(1_000_002);
    runtime_error(&format!("takepkg std.json;\nfunc m{{ json.parse(\"{digits}\"); }}"), "RangeError: the JSON number \"9999999999");
}

#[test]
fn annotations_and_unions() {
    executes(r#"
        takepkg std.types;
        func twice(bigint n) => n * 2;
        func keep([int,longint,bigint] n) => types.kind(n);
        func m{
            x (bigint) = 5;
            out(x, types.kind(x));
            x = longint(7);
            out(types.kind(x));
            out(twice(21), types.kind(twice(21)), twice(bigint(10) ** 20));
            out(keep(1), keep(longint(1)), keep(bigint(1)), keep(99999999999999999999));
            xs (array[bigint]) = arr(1, longint(2));
            xs.add(3);
            out(xs, types.kind(xs[0]), types.kind(xs[2]));
            m2 (map[string,bigint]) = map();
            m2.set("a", 1);
            out(types.kind(m2.get("a")));
            y (bigint) = 2.0;
            out(types.kind(y));
        }"#,
        "5 bigint\nbigint\n42 bigint 200000000000000000000\nint longint bigint bigint\n[1, 2, 3] bigint bigint\nbigint\nbigint\n");
    runtime_error("func m{ y (bigint) = 2.5; }", "TypeError: expected bigint, got float");
    runtime_error("func m{ y (bigint) = \"5\"; }", "TypeError: expected bigint, got string");
    runtime_error("func f(int n) => n; func m{ f(bigint(1)); }", "TypeError: expected int, got bigint; convert it with int(...)");
    runtime_error("func f(longint n) => n; func m{ f(bigint(1)); }", "TypeError: expected exact longint, got bigint");
    runtime_error("func f(float n) => n; func m{ f(bigint(1)); }", "TypeError: expected float, got bigint");
    runtime_error("func m{ x (int) = bigint(1); }", "expected int, got bigint");
    // Converting the elements of a fixed array is changing it.
    runtime_error("func m{ fixed xs = arr(1); ys (array[bigint]) = xs; }", "cannot mutate fixed value");
}

#[test]
fn std_math_refuses_bigints_with_a_clear_type_error() {
    for call in ["math.abs(bigint(5))", "math.sqrt(bigint(5))", "math.gcd(bigint(4), 6)", "math.pow(bigint(2), 3)"] {
        runtime_error(&format!("takepkg std.math;\nfunc m{{ out({call}); }}"),
            "TypeError: a bigint does not match [int,longint,float]; convert it with int(...), longint(...), or float(...)");
    }
    // Only the conversions the parameter takes are suggested.
    runtime_error("func f([int,longint] n) => n; func m{ f(bigint(1)); }",
        "TypeError: a bigint does not match [int,longint]; convert it with int(...) or longint(...)");
    runtime_error("func f([float,string] n) => n; func m{ f(bigint(1)); }",
        "TypeError: a bigint does not match [float,string]; convert it with float(...)");
    runtime_error("func f([string,bool] n) => n; func m{ f(bigint(1)); }", "TypeError: value does not match [string,bool]");
}

#[test]
fn parse_and_digits_in_bases_2_16_and_36() {
    executes(r#"
        takepkg std.bigint = big;
        func m{
            out(big.parse("101", 2), big.parse("-11111111", 2), big.digits(5, 2), big.digits(-255, 2));
            out(big.parse("ff", 16), big.parse("FF", 16), big.parse("-DeadBeef", 16), big.digits(3735928559, 16));
            out(big.parse("zz", 36), big.parse("ZZ", 36), big.digits(1295, 36), big.digits(bigint(36) ** 10, 36));
            out(big.digits(0, 2), big.parse("000", 16), big.digits(bigint(2) ** 100, 16));
            x = big.factorial(50);
            out(big.parse(big.digits(x, 36), 36) == x, big.parse(big.digits(-x, 2), 2) == -x, big.parse(big.digits(x, 16), 16) == x);
            out(big.parse(big.digits(x, 7), 7) == x, big.parse(string(x), 10) == x);
        }"#,
        "5 -255 101 -11111111\n255 255 -3735928559 deadbeef\n1295 1295 zz 10000000000\n0 0 10000000000000000000000000\ntrue true true\ntrue true\n");
    for (call, message) in [
        ("big.parse(\"12\", 1)", "RangeError: bigint.parse needs a base from 2 to 36, got 1"),
        ("big.digits(5, 37)", "RangeError: bigint.digits needs a base from 2 to 36, got 37"),
        ("big.parse(\"2\", 2)", "SyntaxError: \"2\" is not a number in base 2: its digits are 0 to 1"),
        ("big.parse(\"0x1f\", 16)", "SyntaxError: \"0x1f\" is not a number in base 16: its digits are 0 to 9 and a to f"),
        ("big.parse(\"\", 10)", "SyntaxError: \"\" is not a number in base 10"),
        ("big.parse(\" 1\", 10)", "SyntaxError: \" 1\" is not a number in base 10"),
        ("big.parse(\"-\", 10)", "SyntaxError: \"-\" is not a number in base 10"),
        ("big.parse(5, 10)", "TypeError: expected string"),
    ] {
        runtime_error(&format!("takepkg std.bigint = big;\nfunc m{{ out({call}); }}"), message);
    }
}

#[test]
fn the_rest_of_the_package() {
    executes(r#"
        takepkg std.bigint;
        takepkg std.types;
        func m{
            out(bigint.gcd(12, 18), bigint.gcd(-12, 18), bigint.gcd(0, 0), bigint.gcd(0, -5), types.kind(bigint.gcd(12, 18)));
            out(bigint.gcd(bigint.factorial(30), bigint.of(2) ** 100), bigint.lcm(4, 6), bigint.lcm(-4, 6), bigint.lcm(0, 5));
            out(bigint.abs(-5), bigint.abs(bigint.of(-10) ** 30), types.kind(bigint.abs(5)));
            out(bigint.sign(-3), bigint.sign(0), bigint.sign(bigint.of(10) ** 30), types.kind(bigint.sign(1)));
            out(bigint.pow(3, 100), bigint.pow(bigint.of(-2), 3), bigint.pow(0, 0), types.kind(bigint.pow(2, 2)));
            out(bigint.sqrt(0), bigint.sqrt(1), bigint.sqrt(15), bigint.sqrt(16), bigint.sqrt(bigint.of(10) ** 100));
            x = bigint.factorial(100);
            s = bigint.sqrt(x);
            out(s * s <= x and (s + 1) * (s + 1) > x);
            out(bigint.factorial(0), bigint.factorial(1), bigint.factorial(20), types.kind(bigint.factorial(0)));
            out(bigint.of("5") + 1, types.kind(bigint.of(5)));
        }"#,
        "6 6 0 5 bigint\n67108864 12 12 0\n5 1000000000000000000000000000000 bigint\n-1 0 1 bigint\n515377520732011331036461129765621272702107522001 -8 1 bigint\n0 1 3 4 100000000000000000000000000000000000000000000000000\ntrue\n1 1 2432902008176640000 bigint\n6 bigint\n");
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.sqrt(-4)); }", "RangeError: bigint.sqrt needs a number of 0 or more, got -4");
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint.factorial(-1)); }", "RangeError: bigint.factorial needs n of 0 or more, got -1");
    // Importing the package takes the built-in's name; calling it says what to do.
    runtime_error("takepkg std.bigint;\nfunc m{ out(bigint(5)); }", "TypeError: bigint here is the package std.bigint, which takes the built-in's name: use bigint.of(x)");
}

#[test]
fn int_and_longint_overflow_still_raises_and_never_promotes() {
    for (source, message) in [
        ("out(2147483647 + 1);", "int overflow"),
        ("x = 2147483647; x = x * 2;", "int overflow"),
        ("out(2 ** 31);", "int overflow"),
        ("out(-(-2147483648));", "int overflow"),
        ("out(longint(9223372036854775807) + 1);", "longint overflow"),
        ("out(longint(2) ** 64);", "longint overflow"),
        ("out(-9223372036854775807 - 2);", "longint overflow"),
    ] {
        runtime_error(&format!("func m{{ {source} }}"), message);
    }
}

#[test]
fn truth_durations_and_other_packages() {
    executes(r#"
        takepkg std.algorithms;
        takepkg std.stats;
        takepkg std.time;
        takepkg std.types;
        func m{
            if (bigint(0)){ out("zero is true"); } else { out("zero is false"); }
            out(not bigint(0), not bigint(5), bigint(3) and true);
            out(time.hours(1) * bigint(2) == time.hours(2), time.hours(4) / bigint(2) == time.hours(2));
            out(types.kind(bigint(1)), types.is_number(bigint(1)));
            // Packages that take any number take bigints too.
            out(stats.mean(arr(bigint(10) ** 20, 0)), algorithms.sort_by(arr(bigint(3), 1, 2.5), func(any x) => x));
        }"#,
        "zero is false\ntrue false true\ntrue true\nbigint true\n50000000000000000000 [1, 2.5, 3]\n");
    runtime_error("takepkg std.stats;\nfunc m{ out(stats.mean(arr(bigint(10) ** 400))); }", "RangeError: the bigint is too large for a float");
}

#[test]
fn values_stay_the_size_they_were() {
    // The bigint is boxed, so adding it did not make every value larger.
    assert!(std::mem::size_of::<marslang::value::Value>() <= 24, "{}", std::mem::size_of::<marslang::value::Value>());
}

/// Every example in docs/api/std-bigint.md runs and prints what the comments
/// on its `out` lines say.
#[test]
fn the_documented_examples_print_what_their_comments_say() {
    let doc = include_str!("../docs/api/std-bigint.md").replace("\r\n", "\n");
    let mut checked = 0;
    for block in doc.split("```mars\n").skip(1) {
        let code = block.split("```").next().unwrap();
        if !code.contains("func m") { continue; }
        let expected: String = code.lines()
            .filter(|line| line.contains("out(") && line.contains("// "))
            .map(|line| format!("{}\n", line.rsplit("// ").next().unwrap().trim()))
            .collect();
        executes(code, &expected);
        checked += 1;
    }
    assert!(checked >= 7, "only {checked} examples found");
}
