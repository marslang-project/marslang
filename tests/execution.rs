use std::process::Command;

fn executes(source: &str, expected: &str) {
    let js = marslang::compile_source_to_js(source).expect("Marslang compilation failed");
    let node = std::env::var_os("MARSLANG_NODE").unwrap_or_else(|| "node".into());
    let result = Command::new(node)
        .arg("-e")
        .arg(&js)
        .output()
        .expect("Node is required: put node on PATH or set MARSLANG_NODE");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "Node failed: {}\n{stderr}\n{js}", result.status);
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    let stdout = String::from_utf8(result.stdout).expect("stdout must be UTF-8");
    assert_eq!(stdout.replace("\r\n", "\n"), expected);
}

#[test]
fn expression_function_and_typed_parameters() {
    executes("func add(int a, int b) => a + b;\nfunc m{\n out(add(2,3));\n}", "5\n");
}

#[test]
fn family_init_me_and_method_call() {
    executes("family Box{\n func init(int value){\n me.value = value;\n }\n func get(){\n ret me.value;\n }\n}\nfunc m{\n b = Box(7);\n out(b.get());\n}", "7\n");
}

#[test]
fn false_typo_alias_and_single_line_output() {
    executes("func m{\n slout('value=');\n out(fasle);\n}", "value=false\n");
}

#[test]
fn repeat_and_condition() {
    executes("func m{\n repeat 2 {\n if (true) {\n out(3);\n }\n }\n}", "3\n3\n");
}

#[test]
fn comma_parameters_in_block_function_and_constructor() {
    executes("family Sum{\n func init(int a, int b){\n me.total = a + b;\n }\n func value(){\n ret me.total;\n }\n}\nfunc add(int a, int b){\n ret a + b;\n}\nfunc m{\n s = Sum(2,3);\n out(add(s.value(),4));\n}", "9\n");
}

#[test]
fn bracketed_type_commas_do_not_split_parameters() {
    let program = marslang::parser::parse_program(
        "func choose([int, float] value, pair[int, pair[string, int]] data) => value;",
    ).expect("bracketed parameter types should parse");
    let marslang::ast::Item::Func(function) = &program.items[0] else {
        panic!("expected function");
    };
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].ty, "[int, float]");
    assert_eq!(function.params[0].name, "value");
    assert_eq!(function.params[1].ty, "pair[int, pair[string, int]]");
    assert_eq!(function.params[1].name, "data");
}

#[test]
fn semicolon_parameters_are_rejected_with_migration_hint() {
    for source in [
        "func add(int a; int b;) => a + b;",
        "func add(int a, int b;) => a + b;",
        "func one(int a;) => a;",
    ] {
        let error = marslang::compile_source_to_js(source).expect_err(source);
        assert!(error.contains("commas, not semicolons"), "{error}");
    }
}

#[test]
fn malformed_parameter_lists_are_rejected() {
    for source in [
        "func f(int a int b) => a;",
        "func f(, int a) => a;",
        "func f(int a,, int b) => a;",
        "func f(int a,) => a;",
        "func f([int, float value) => value;",
        "func f(int] value) => value;",
        "func f(int a => a;",
    ] {
        assert!(marslang::compile_source_to_js(source).is_err(), "accepted: {source}");
    }
}

#[test]
#[ignore = "parser currently requires line-oriented block layout"]
fn single_line_entry_function() {
    executes("func m{ out(1); }", "1\n");
}

#[test]
#[ignore = "comment preprocessing currently truncates string contents"]
fn comment_markers_inside_strings() {
    executes("func m{\n out(\"https://example.com/*text*/\");\n}", "https://example.com/*text*/\n");
}

#[test]
#[ignore = "compound expressions currently retain raw me references"]
fn compound_me_expression() {
    executes("family Box{\n func init(int value){\n me.value = value;\n }\n func doubled(){\n ret me.value * 2;\n }\n}\nfunc m{\n b = Box(7);\n out(b.doubled());\n}", "14\n");
}

#[test]
#[ignore = "canonical arr constructor and collection methods are not implemented"]
fn canonical_sorted_example() {
    executes("fixed hot pi (float) = 3.14159;\nfunc area(float r) => pi * r * r;\nfunc m{\n nums (array[int]) = arr(3,1,4,1,5);\n nums.asort();\n out(nums.iget(0));\n out(area(5));\n}", "1\n78.53975\n");
}

#[test]
#[ignore = "canonical arr constructor and collection methods are not implemented"]
fn array_pop_removes_first_and_lpop_removes_last() {
    executes("func m{\n nums = arr(1,2,3,4);\n nums.pop();\n nums.lpop();\n out(nums.iget(0));\n out(nums.iget(1));\n}", "2\n3\n");
}

#[test]
#[ignore = "canonical arr constructor and collection methods are not implemented"]
fn array_sort_is_descending() {
    executes("func m{\n nums = arr(1,3,2);\n nums.sort();\n out(nums.iget(0));\n}\n", "3\n");
}

#[test]
#[ignore = "lexer/parser must reject unterminated comments with source locations"]
fn unterminated_comment_has_diagnostic() {
    let diagnostic = marslang::compile_source_to_js("/* unfinished")
        .expect_err("unterminated comment must be rejected");
    assert!(diagnostic.to_lowercase().contains("comment"), "{diagnostic}");
    assert!(diagnostic.contains("1:1"), "expected opening location 1:1: {diagnostic}");
}

#[test]
#[ignore = "hot reassignment must be rejected before JavaScript emission"]
fn hot_reassignment_is_rejected() {
    let diagnostic = marslang::compile_source_to_js("func m{\n hot x = 5;\n x = 6;\n}")
        .expect_err("hot reassignment must be rejected");
    assert!(diagnostic.contains("x"), "diagnostic must identify binding: {diagnostic}");
}
