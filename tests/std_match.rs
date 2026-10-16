//! `match`: the statement and expression forms, every kind of pattern,
//! guards, bindings and their scope, control flow inside arms, and the
//! compile and runtime errors, with their lines.

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

fn compile_error(source: &str, expected: &str) {
    let error = marslang::compile(source).expect_err(&format!("expected a compile error: {source}"));
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

fn report(source: &str) -> String {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    result.expect_err("expected a runtime error").report()
}

const SHAPES: &str = r#"
family Shape{ func init(){ me.kind = "shape"; } }
family Circle(Shape){
    func init(float r){ me.r = r; }
    func size() => me.r * 2;
}
family Square(Shape){ func init(){ me.side = 1; } }
"#;

#[test]
fn literal_patterns_compare_with_equality() {
    executes(r#"
        func name(any x){
            match (x){
                1 => out("one");
                2, 3 => out("two or three");
                -4 => out("minus four");
                2.5 => out("two and a half");
                "quit" => out("quit");
                true => out("yes");
                false => out("no");
                null => out("nothing");
                _ => out("other");
            }
        }
        func m{
            name(1); name(1.0); name(bigint(1)); name(longint(1));
            name(2); name(3.0); name(-4); name(2.5);
            name("quit"); name("Quit"); name(true); name(false); name(null);
            name("1"); name(0); name(arr());
        }
    "#, "one\none\none\none\ntwo or three\ntwo or three\nminus four\ntwo and a half\nquit\nother\nyes\nno\nnothing\nother\nother\nother\n");
}

#[test]
fn big_and_long_literals_match() {
    executes(r#"
        func m{
            for (x, arr(3000000000, bigint("100000000000000000000"), 2147483647, -2147483648)){
                match (x){
                    3000000000 => out("long");
                    100000000000000000000 => out("big");
                    2147483647, -2147483648 => out("int edges");
                }
            }
        }
    "#, "long\nbig\nint edges\nint edges\n");
}

#[test]
fn the_first_matching_arm_runs_and_no_match_does_nothing() {
    executes(r#"
        func m{
            match (5){
                int n => out("first");
                5 => out("second");
            }
            match ("x"){
                1 => out("never");
            }
            out("done");
        }
    "#, "first\ndone\n");
}

#[test]
fn the_subject_is_evaluated_once() {
    executes(r#"
        func next(array log){ log.add(1); ret log.len(); }
        func m{
            log = arr();
            match (next(log)){
                5 => out("five");
                2 => out("two");
                1 => out("one");
            }
            out(log.len());
        }
    "#, "one\n1\n");
}

#[test]
fn type_patterns_match_the_exact_kind() {
    executes(&format!("{SHAPES}{}", r#"
        func kind(any x){
            match (x){
                int n => out(f"int {n}");
                longint n => out(f"longint {n}");
                bigint n => out(f"bigint {n}");
                float n => out(f"float {n}");
                string s => out("string " + s);
                bool b => out(f"bool {b}");
                array a => out(f"array {a.len()}");
                set s => out("set");
                map d => out("map");
                pair p => out("pair");
                date d => out("date");
                datetime d => out("datetime");
                duration d => out("duration");
                Circle c => out(f"circle {c.size()}");
                Shape s => out("shape");
                null => out("null");
                any v => out("something else");
            }
        }
        func m{
            kind(1); kind(longint(1)); kind(bigint(1)); kind(1.0); kind("a"); kind(true);
            kind(arr(1, 2)); kind(set()); kind(map()); kind(pair(1, 2));
            kind(date("2024-01-02")); kind(datetime("2024-01-02 03:04:05Z")); kind(duration("1h"));
            kind(Circle(1.5)); kind(Square()); kind(Shape()); kind(null); kind(kind);
        }
    "#), "int 1\nlongint 1\nbigint 1\nfloat 1\nstring a\nbool true\narray 2\nset\nmap\npair\ndate\ndatetime\nduration\ncircle 3\nshape\nshape\nnull\nsomething else\n");
}

#[test]
fn type_patterns_do_not_convert() {
    // float matches only floats, unlike a float annotation, which converts an int.
    executes(r#"
        func m{
            match (2){
                float f => out("float");
                int i => out("int");
            }
            match (2.0){
                int i => out("int");
                float f => out("float");
            }
        }
    "#, "int\nfloat\n");
}

#[test]
fn unions_and_bare_types() {
    executes(r#"
        func m{
            for (x, arr(1, 2.5, "s", true)){
                match (x){
                    [int, float] n => out(f"number {n}");
                    string => out("a string");
                    [bool] => out("a bool");
                }
            }
        }
    "#, "number 1\nnumber 2.5\na string\na bool\n");
}

#[test]
fn families_from_packages_and_error_families() {
    executes(r#"
        takepkg std.containers;
        takepkg std.Error;
        func m{
            s = containers.stack();
            match (s){
                containers.queue q => out("queue");
                containers.stack t => out("stack");
            }
            run{ x = 1 / 0; } handle(Error.Base e){
                match (e){
                    Error.TypeError t => out("type");
                    Error.RangeError r => out("range: " + r.message);
                }
                match (e){
                    TypeError => out("bare type");
                    RangeError => out("bare range");
                }
            }
        }
    "#, "stack\nrange: division by zero\nbare range\n");
}

#[test]
fn guards_follow_any_pattern() {
    executes(r#"
        func size(any x){
            match (x){
                int n if (n > 100) => out(f"big {n}");
                int n if (n < 0) => out(f"negative {n}");
                1, 2 if (x == 2) => out("two");
                _ if (x == "skip") => out("skipped");
                "a" if (false) => out("never");
                _ => out(f"other {x}");
            }
        }
        func m{ size(500); size(-3); size(1); size(2); size("skip"); size("a"); size(5); }
    "#, "big 500\nnegative -3\nother 1\ntwo\nskipped\nother a\nother 5\n");
}

#[test]
fn a_guard_runs_only_when_its_pattern_matches() {
    executes(r#"
        func check(string tag) { out("guard " + tag); ret true; }
        func m{
            match ("x"){
                1 if (check("one")) => out("one");
                string s if (check(s)) => out("string");
                _ if (check("rest")) => out("rest");
            }
        }
    "#, "guard x\nstring\n");
}

#[test]
fn bindings_are_local_to_their_arm() {
    executes(r#"
        func m{
            n = "outer";
            match (42){
                int n => { out(n); n = n + 1; out(n); }
            }
            out(n);
            match ("s"){
                string n if (n == "s") => out("guard sees " + n);
            }
            match (7){
                int k => { inner = k * 2; out(inner); }
            }
            match (1){
                int value => { f = func(int x) => x + value; out(f(10)); }
            }
        }
    "#, "42\n43\nouter\nguard sees s\n14\n11\n");
    // A name bound in one arm is unknown outside it.
    compile_error("func m{\n    match (1){\n        int k => out(k);\n    }\n    out(k);\n}", "line 5: unknown name 'k'");
    compile_error("func m{\n    match (1){\n        int k => out(k);\n        _ => out(k);\n    }\n}", "line 4: unknown name 'k'");
}

#[test]
fn ret_break_and_continue_inside_arms() {
    executes(r#"
        func sign(int n){
            match (n){
                0 => ret "zero";
                int x if (x < 0) => { ret "negative"; }
            }
            ret "positive";
        }
        func m{
            out(sign(0), sign(-1), sign(5));
            for (i, arr(1, 2, 3, 4, 5)){
                match (i){
                    2 => continue;
                    4 => { out("stop"); break; }
                    _ => out(i);
                }
            }
            i = 0;
            while (true){
                i++;
                match (i % 3){
                    0 => break;
                }
            }
            out(i);
        }
    "#, "zero negative positive\n1\n3\nstop\n3\n");
    compile_error("func m{\n    match (1){\n        1 => break;\n    }\n}", "break/continue must be inside a loop");
}

#[test]
fn arm_bodies_can_be_any_statement() {
    executes(r#"
        func m{
            match (2){
                2 => if (true){ out("if in arm"); } else { out("no"); }
            }
            match (1){
                1 => for (x, arr(1, 2)){ out(x); }
            }
            match (3){
                3 => total = 3 + 4;
            }
            match (1){ 1 => out("one line"); _ => out("other"); }
            match (5){ _ => out("last arm without a semicolon") }
        }
    "#, "if in arm\n1\n2\none line\nlast arm without a semicolon\n");
}

#[test]
fn matches_nest() {
    executes(r#"
        func both(any a, any b){
            match (a){
                int x => match (b){
                    int y => out(f"ints {x} {y}");
                    _ => out(f"int {x} and something");
                }
                string s => {
                    match (b){
                        string t => out(s + t);
                    }
                }
            }
        }
        func m{ both(1, 2); both(1, "z"); both("a", "b"); }
    "#, "ints 1 2\nint 1 and something\nab\n");
}

#[test]
fn match_at_the_top_level() {
    executes("x = 3;\nmatch (x){\n    int n => out(n * 2);\n}\nfunc m{ out(\"m\"); }", "6\nm\n");
}

#[test]
fn match_expressions() {
    executes(r#"
        func label(any x) => match (x){
            1 => "one";
            2, 3 => "few";
            int n if (n > 100) => f"big {n}";
            string s => "text " + s;
            _ => "other"
        };
        func m{
            out(label(1), label(3), label(500), label("a"), label(7.5));
            x = 2;
            word = match (x){ 1 => "one"; _ => "other" };
            out(word);
            out(match (x){ 2 => "two"; _ => "?" }, "inline");
            typed(string) = match (x){
                2 => "typed";
                _ => "?";
            };
            out(typed);
            total = 10 + match (x){
                2 => 5;
                _ => 0;
            };
            out(total);
            total += match (x){ 2 => 1; _ => 0 };
            out(total);
            nested = match (x){
                2 => match ("b"){
                    "a" => "2a";
                    "b" => "2b";
                };
                _ => "?";
            };
            out(nested);
            out(match (x){ 2 => arr(1, 2); _ => arr() }.len());
        }
    "#, "one few big 500 text a other\nother\ntwo inline\ntyped\n15\n16\n2b\n2\n");
    executes(r#"
        func pick(int x){
            ret match (x){
                1 => "one";
                _ => "other";
            };
        }
        func m{
            out(pick(1), pick(2));
            match (1){
                1 => word = match ("b"){
                    "b" => "assigned in an arm";
                };
            }
        }
    "#, "one other\n");
}

#[test]
fn arms_with_arrows_commas_and_comments_in_their_text() {
    executes(r#"
        family Box{
            func init(any v){ me.v = v; }
            func show(){
                match (me.v){
                    "=>" => out("arrow");                // a comment
                    "if", "a,b" => out("tricky");
                    /* a block comment */ int n if (n == me.v) => out(f"int {n}");
                }
            }
        }
        func m{
            Box("=>").show(); Box("if").show(); Box("a,b").show(); Box(3).show();
            f = func(int x) => match (x){ 1 => "one"; _ => "many" };
            out(f(1), f(2));
            out(arr(match (1){ 1 => "in an array" }));
        }
    "#, "arrow\ntricky\ntricky\nint 3\none many\n[\"in an array\"]\n");
}

#[test]
fn a_match_expression_without_a_matching_arm_raises() {
    runtime_error("func m{ out(match (7){ 1 => \"one\" }); }", "RangeError: no match arm matches 7 (an int)");
    runtime_error("func m{ x = match (\"q\"){ 1 => \"one\" }; }", "RangeError: no match arm matches \"q\" (a string)");
    // The statement form does nothing instead.
    executes("func m{ match (7){ 1 => out(\"one\"); } out(\"ok\"); }", "ok\n");
}

#[test]
fn match_is_a_keyword() {
    compile_error("func m{ match = 1; }", "'match' is a keyword");
    compile_error("func m{ x = match; }", "'match' is a keyword");
    compile_error("func match(int x) => x;\nfunc m{}", "'match' is a keyword and cannot name a function");
    compile_error("func f(int match) => match;\nfunc m{}", "'match' is a keyword and cannot name a parameter");
    compile_error("func m{ for (match, arr(1)){ } }", "'match' is a keyword and cannot name a loop variable");
    compile_error("func m{ match(int) = 1; }", "'match' is a keyword");
    compile_error("family match{}\nfunc m{}", "'match' is a keyword and cannot name a family");
    // Names that merely start with match are ordinary, and so is a field named match.
    executes("family Re{ func init(){ me.match = 1; } }\nfunc m{ matches = 2; matcher = Re(); out(matches + matcher.match); }", "3\n");
}

#[test]
fn compile_errors_name_their_lines() {
    compile_error("func m{\n    match (1){\n    }\n}", "line 2: this match has no arms");
    compile_error("func m{\n    match (1){\n        1 => out(1);\n        2 out(2);\n    }\n}", "line 4: a match arm needs '=>'");
    compile_error("func m{\n    match (1){\n        _ => out(1);\n        2 => out(2);\n    }\n}",
        "line 4: this match arm can never run: the _ arm before it matches every value");
    compile_error("func m{\n    match (1){\n        any v => out(1);\n        2 => out(2);\n    }\n}",
        "line 4: this match arm can never run: the any arm before it");
    compile_error("func m{\n    match (1){\n        1 => out(1);\n        2, 1.0 => out(2);\n    }\n}",
        "line 4: 1 appears twice in this match: it is already matched on line 3");
    compile_error("func m{\n    match (1){\n        \"a\", \"a\" => out(1);\n    }\n}", "line 3: \"a\" appears twice");
    compile_error("func m{\n    match (1){\n        1 => out(1);\n        Circle c => out(2);\n    }\n}",
        "line 4: unknown type 'Circle' in a match pattern");
    compile_error("takepkg std.math;\nfunc m{\n    match (1){\n        math.Thing t => out(2);\n    }\n}",
        "line 4: unknown type 'math.Thing' in a match pattern");
    compile_error("func m{\n    match (1){\n        Function f => out(2);\n    }\n}", "line 3: Function is a parameter type");
    compile_error("func m{\n    match (1){\n        array[int] a => out(2);\n    }\n}", "line 3: 'array[int]' is not a type");
    compile_error("func m{\n    match (1){\n        int, float => out(2);\n    }\n}", "line 3: only literals can be listed with commas");
    compile_error("func m{\n    match (1){\n        1, x => out(2);\n    }\n}", "line 3: 'x' is not a literal");
    compile_error("func m{\n    match (1){\n        int n if n > 1 => out(2);\n    }\n}", "line 3: a match guard is written if (condition)");
    compile_error("func m{\n    match (1){\n        1 =>\n            out(1);\n    }\n}", "line 3: a match arm needs a body after '=>'");
    compile_error("func m{\n    match 1 {\n        1 => out(1);\n    }\n}", "line 2: a match is written match (value)");
    compile_error("func m{\n    match (1){\n        1 => out(1);\n", "unterminated");
    compile_error("func m{\n    x = match (1){\n        1 => 2;\n        1 => 3;\n    };\n}", "line 4: 1 appears twice");
    compile_error("func m{ x = match (1){ }; }", "this match has no arms");
    compile_error("func m{ x = match (1){ _ => 1; 2 => 3 }; }", "this match arm can never run");
    // A repeated literal is allowed after an arm with a guard.
    executes("func m{\n    match (1){\n        1 if (false) => out(\"guarded\");\n        1 => out(\"plain\");\n    }\n}", "plain\n");
}

#[test]
fn runtime_errors_in_arms_keep_their_lines() {
    assert_eq!(report("func m{\n    match (1){\n        2 => out(2);\n        1 => out(1 / 0);\n    }\n}"),
        "RangeError: division by zero\n    at m (line 4)");
    assert_eq!(report("func m{\n    match (1){\n        1 => {\n            out(\"in\");\n            x = 1 / 0;\n        }\n    }\n}"),
        "RangeError: division by zero\n    at m (line 5)");
    assert_eq!(report("func m{\n    match (1){\n        int n if (n / 0 > 1) => out(1);\n    }\n}"),
        "RangeError: division by zero\n    at m (line 3)");
    assert_eq!(report("func m{\n    x = match (1){\n        2 => 0;\n        1 => 1 / 0;\n    };\n}"),
        "RangeError: division by zero\n    at m (line 4)");
    assert_eq!(report("func f(int x) => match (x){\n    1 => x / 0;\n    _ => 0;\n};\nfunc m{\n    f(1);\n}"),
        "RangeError: division by zero\n    at f (line 2)\n    at m (line 6)");
    assert_eq!(report("func m{\n    out(match (3){ 1 => 1; _ => 1 / 0 });\n}"),
        "RangeError: division by zero\n    at m (line 2)");
    assert_eq!(report("func m{\n    x = match (3){\n        1 => 1;\n    };\n}"),
        "RangeError: no match arm matches 3 (an int)\n    at m (line 2)");
}

#[test]
fn symbols_list_bound_names_with_their_types() {
    let symbols = marslang::symbols("func m{\n    match (1){\n        int n => out(n);\n        [string, bool] s => out(s);\n        _ => out(0);\n    }\n}",
        std::path::Path::new(".")).expect("symbols");
    assert!(symbols.contains(r#"{"name":"n","kind":"match","type":"int","inferred":false,"scope":"m"}"#), "{symbols}");
    assert!(symbols.contains(r#"{"name":"s","kind":"match","type":"string | bool","inferred":false,"scope":"m"}"#), "{symbols}");
    let symbols = marslang::symbols("func m{\n    word = match (1){ 1 => \"a\"; _ => \"b\" };\n}", std::path::Path::new(".")).expect("symbols");
    assert!(symbols.contains(r#"{"name":"word","kind":"variable","type":"string","inferred":true,"scope":"m"}"#), "{symbols}");
}

#[test]
fn literal_matching_is_not_slower_than_an_if_chain() {
    // Both run the same 20,000 dispatches over eight literals; the match
    // evaluates its subject once and compares without making values.
    let arms = (1..=8).map(|n| format!("        {n} => hits += {n};\n")).collect::<String>();
    let chain = (1..=8).map(|n| format!("{}(i % 9 == {n}){{ hits += {n}; }}", if n == 1 { "if " } else { " elif " })).collect::<String>();
    let with_match = format!("func m{{\n    hits = 0;\n    for (i = 0, i < 20000, i++){{\n    match (i % 9){{\n{arms}    }}\n    }}\n    out(hits);\n}}");
    let with_if = format!("func m{{\n    hits = 0;\n    for (i = 0, i < 20000, i++){{\n        {chain}\n    }}\n    out(hits);\n}}");
    // The best of three runs, so a busy machine does not decide it.
    let time = |source: &str| {
        let mut best = None;
        let mut output = String::new();
        for _ in 0..3 {
            let compiled = marslang::compile(source).expect("compile failed");
            let started = std::time::Instant::now();
            let (out, result) = marslang::run_captured(compiled, "");
            let elapsed = started.elapsed();
            result.expect("runtime error");
            output = out;
            best = Some(best.map_or(elapsed, |b: std::time::Duration| b.min(elapsed)));
        }
        (output, best.unwrap())
    };
    let (match_out, match_time) = time(&with_match);
    let (if_out, if_time) = time(&with_if);
    assert_eq!(match_out, if_out);
    // Generous; it is typically faster.
    assert!(match_time < if_time * 2, "match took {match_time:?}, the if chain {if_time:?}");
}

/// Every example in the match section of docs/api/language.md runs and prints
/// what the comments on its `out` lines say.
#[test]
fn the_documented_examples_print_what_their_comments_say() {
    let doc = include_str!("../docs/api/language.md").replace("\r\n", "\n");
    let section = doc.split("\n## match\n").nth(1).expect("a match section").split("\n## ").next().unwrap();
    let mut checked = 0;
    for block in section.split("```mars\n").skip(1) {
        let code = block.split("```").next().unwrap();
        let expected: String = code.lines()
            .filter(|line| line.contains("out(") && line.contains("// "))
            .map(|line| format!("{}\n", line.rsplit("// ").next().unwrap().trim()))
            .collect();
        executes(code, &expected);
        checked += 1;
    }
    assert_eq!(checked, 3);
}
