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
fn single_line_entry_function() {
    executes("func m{ out(1); }", "1\n");
}

#[test]
fn comment_markers_inside_strings() {
    executes("func m{\n out(\"https://example.com/*text*/\");\n}", "https://example.com/*text*/\n");
}

#[test]
fn compound_me_expression() {
    executes("family Box{\n func init(int value){\n me.value = value;\n }\n func doubled(){\n ret me.value * 2;\n }\n}\nfunc m{\n b = Box(7);\n out(b.doubled());\n}", "14\n");
}

#[test]
fn canonical_sorted_example() {
    executes("fixed hot pi (float) = 3.14159;\nfunc area(float r) => pi * r * r;\nfunc m{\n nums (array[int]) = arr(3,1,4,1,5);\n nums.asort();\n out(nums.iget(0));\n out(area(5));\n}", "1\n78.53975\n");
}

#[test]
fn array_pop_removes_first_and_lpop_removes_last() {
    executes("func m{\n nums = arr(1,2,3,4);\n nums.pop();\n nums.lpop();\n out(nums.iget(0));\n out(nums.iget(1));\n}", "2\n3\n");
}

#[test]
fn array_sort_is_descending() {
    executes("func m{\n nums = arr(1,3,2);\n nums.sort();\n out(nums.iget(0));\n}\n", "3\n");
}

#[test]
fn unterminated_comment_has_diagnostic() {
    let diagnostic = marslang::compile_source_to_js("/* unfinished")
        .expect_err("unterminated comment must be rejected");
    assert!(diagnostic.to_lowercase().contains("comment"), "{diagnostic}");
    assert!(diagnostic.contains("1:1"), "expected opening location 1:1: {diagnostic}");
}

#[test]
fn hot_reassignment_is_rejected() {
    let diagnostic = marslang::compile_source_to_js("func m{\n hot x = 5;\n x = 6;\n}")
        .expect_err("hot reassignment must be rejected");
    assert!(diagnostic.contains("x"), "diagnostic must identify binding: {diagnostic}");
}

#[test]
fn duplicate_parameters_and_invalid_function_names_are_rejected() {
    for (source, diagnostic) in [
        ("func f(int x, int x) => x;", "duplicate parameter"),
        ("func (int x) => x;", "invalid function name"),
        ("func 123(int x) => x;", "invalid function name"),
        ("func bad name(int x) => x;", "invalid function name"),
    ] {
        let error = marslang::compile_source_to_js(source).expect_err(source);
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn comparisons_and_quoted_equals_are_not_declarations() {
    executes("func m{\n x = 1;\n x == 1;\n x != 2;\n x <= 2;\n x >= 0;\n \"é=1\";\n out(\"a = b\");\n out(x);\n}", "a = b\n1\n");
}

#[test]
fn user_function_shadows_builtin_even_before_declaration() {
    executes("func m{\n slout(out(1));\n}\nfunc out(int x) => x + 10;", "11");
}

#[test]
fn comments_preserve_escaped_quotes_and_ignore_fake_openers() {
    executes(r#"// /* this is not a block comment
func m{
    x /* comment */ = 3;
    out("say \"// hello\""); // /* ignored too
    out("C:\\"); /* closed */
    out(x);
}
"#, "say \"// hello\"\nC:\\\n3\n");
    let error = marslang::compile_source_to_js("// ignored\n  /* open").unwrap_err();
    assert!(error.contains("2:3"), "{error}");
    assert!(marslang::compile_source_to_js("out(\"unfinished);").is_err());
}

fn runtime_error(source: &str, expected: &str) {
    let js = marslang::compile_source_to_js(source).expect("compile failed");
    let node = std::env::var_os("MARSLANG_NODE").unwrap_or_else(|| "node".into());
    let result = Command::new(node).args(["-e", &js]).output().expect("Node required");
    assert!(!result.status.success(), "expected runtime error: {source}");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains(expected), "expected {expected}, got {stderr}");
}

#[test]
fn scope_assignment_and_explicit_shadowing() {
    executes("x = 1; func change { x = 2; } func m { change(); out(x); if (true) { x = 3; cold x = x + 1; out(x); x = 5; out(x); } out(x); x (int) = 10; if (true) { x = 11; } out(x); }", "2\n4\n5\n3\n11\n");
    assert!(marslang::compile_source_to_js("func m{ cold x=1; cold x=2; }").unwrap_err().contains("duplicate local"));
    assert!(marslang::compile_source_to_js("func m{ out(missing); }").unwrap_err().contains("unknown name"));
}

#[test]
fn fixed_aliases_and_deep_copy() {
    runtime_error("func m{ a1=arr(1); fixed a2=a1; a1.add(2); }", "cannot mutate fixed value");
    runtime_error("func m{ nested=arr(1); fixed a1=arr(nested); nested.add(2); }", "cannot mutate fixed value");
    runtime_error("func m{ p1=pair(1,2); fixed p2=p1; p1.first=3; }", "cannot mutate fixed value");
    assert!(marslang::compile_source_to_js("func m{ fixed x=5; y=x; y=6; }").unwrap_err().contains("fixed/hot"));
    executes("func m{ fixed x=5; y=x.copy(); y=6; out(y); fixed a1=arr(arr(1)); a2=a1.copy(); a2.iget(0).add(2); out(a1.iget(0).len()); out(a2.iget(0).len()); }", "6\n1\n2\n");
}

#[test]
fn identity_equality_and_short_circuiting() {
    executes("func fail => 1 / 0; func m{ a1=arr(1); a2=a1; out(a1==a2); out(a1==arr(1)); out(1==\"1\"); out(false and fail()); out(true or fail()); out(not null); out(2 + 3 * 4); out(2 ** 3 ** 2); }", "true\nfalse\nfalse\nfalse\ntrue\ntrue\n14\n512\n");
}

#[test]
fn checked_integer_values_and_mutations() {
    runtime_error("func m{ x (int)=2147483647; x=x+1; }", "int overflow");
    runtime_error("func m{ x (int)=2147483647; out(x+1-1); }", "int overflow");
    runtime_error("func m{ xs (array[int])=arr(1); ys=xs; ys.add(\"bad\"); }", "expected int");
    runtime_error("func f(int x){ out(x); } func m{ f(2147483648); }", "int overflow");
    runtime_error("func m{ x (longint)=9223372036854775807; x=x+1; }", "longint overflow");
    executes("func m{ x (longint)=9223372036854775807; out(x); x=x-1; out(x); }", "9223372036854775807\n9223372036854775806\n");
}

#[test]
fn maps_membership_order_and_missing_values() {
    executes("func m{ d=map(); d.set(\"b\",2); d.set(\"a\",1); d.set(\"b\",3); out(d.len()); out(d.has(\"b\")); out(d.get(\"missing\")); for (key,d){ slout(key); out(d.get(key)); } out(d.keys().iget(0)); out(d.values().iget(0)); other=dict(); out(other.is_empty()); }", "2\ntrue\nnull\nb3\na1\nb\n3\ntrue\n");
    runtime_error("func m{ d (map[string,int])=map(); d.set(\"x\",\"bad\"); }", "expected int");
    runtime_error("func m{ d=map(); fixed alias=d; d.set(\"x\",1); }", "cannot mutate fixed value");
}

#[test]
fn while_foreach_break_continue_and_loop_scope() {
    executes("func m{ i=0; total=0; while(i<6){ i+=1; if(i==2){continue;} if(i==5){break;} total=total+i; } out(total); item=99; for(item,arr(1,2,3)){ if(item==2){continue;} out(item); } out(item); for(value,set(3,1,3)){ out(value); } }", "8\n1\n3\n99\n3\n1\n");
    for source in ["func m{break;}", "func m{if(true){continue;}}"] {
        assert!(marslang::compile_source_to_js(source).unwrap_err().contains("inside a loop"));
    }
}

#[test]
fn reusable_family_data_models() {
    executes("family Token{ func init(string kind,string text){me.kind=kind;me.text=text;} func is(string kind)=>me.kind==kind; } func m{ tokens=arr(Token(\"name\",\"x\"),Token(\"number\",\"5\")); for(token,tokens){if(token.is(\"name\")){out(token.text);}} }", "x\n");
}

#[test]
fn repeat_count_is_evaluated_once_and_nested_loops_do_not_capture_names() {
    executes("func m{ n=2; __i=9; repeat n { n=0; repeat 2 {out(__i);} } }", "9\n9\n9\n9\n");
    runtime_error("func m{repeat -1 {out(1);}}", "repeat count");
}

#[test]
fn three_part_for_updates_on_continue_and_supports_also() {
    executes("func m{ total=0; for(i=0,i<5,i++){if(i==2){continue;} total+=i;} out(total); for((i=0 also n=3),i<3,(i++ also n--)){out(i+n);} for(i=0,i<9,i++){if(i==2){break;} out(i);} }", "8\n3\n3\n3\n0\n1\n");
    assert!(marslang::compile_source_to_js("func m{for(i=0,i<2,i++){} out(i);}").is_err());
    assert!(marslang::compile_source_to_js("func m{for((i=0,n=0),i<2,i++){}}").is_err());
}

#[test]
fn collection_iteration_uses_a_snapshot() {
    executes("func m{ xs=arr(1,2); for(x,xs){xs.add(9);out(x);} out(xs.len()); d=map();d.set(\"a\",1);for(k,d){d.set(\"b\",2);out(k);} out(d.len()); }", "1\n2\n4\na\n2\n");
}

#[test]
fn hot_constants_substitute_and_keep_numeric_checks() {
    executes("hot x (int)=2; hot y=x+3; func m{out(y*2);}", "10\n");
    assert!(marslang::compile_source_to_js("func m{ x=1; hot y=x; }").unwrap_err().contains("compile-time constant"));
    runtime_error("func m{ x (int)=-2147483648; out(-x); }", "int overflow");
}

#[test]
fn copy_preserves_cycles_and_shared_structure_without_fixed_state() {
    executes("func m{ xs=arr(); xs.add(xs); fixed root=xs; ys=root.copy(); out(ys==ys.iget(0)); ys.add(1); out(root.len()); out(ys.len()); child=arr(1); tree=arr(child,child); cloned=tree.copy(); out(cloned.iget(0)==cloned.iget(1)); out(cloned.iget(0)==child); }", "true\n1\n2\ntrue\nfalse\n");
}

#[test]
fn bundled_example_executes() {
    executes(include_str!("../hello.mrs"), "3\n78.53975\n");
}

#[test]
fn large_integer_literals_do_not_round_before_type_checks() {
    executes("func f(longint n){out(n);} func m{f(9223372036854775807);f(-9223372036854775808);}", "9223372036854775807\n-9223372036854775808\n");
    runtime_error("func m{out(9223372036854775808);}", "longint overflow");
    runtime_error("func m{out(9007199254740991+1);}", "unsafe integer arithmetic");
}

#[test]
fn float_arithmetic_does_not_use_integer_overflow_rules() {
    executes("func m{ x (float)=9007199254740991; out(x*2); out(5.0/2); }", "18014398509481982\n2.5\n");
}

#[test]
fn conditions_and_grouped_headers_respect_quoted_delimiters() {
    executes("func m{if(\")\"==\")\"){out(1);} for(i=0,i<1,i++){out(\"a also b\");} if(\"\"){out(2);} if(arr()){out(3);} while(null){out(99);} }", "1\na also b\n2\n3\n");
}

#[test]
fn unicode_length_and_iteration_count_grapheme_clusters() {
    executes("func m{out(\"中\".len());out(\"あ\".len());out(\"ع\".len());out(\"عَ\".len());out(\"é\".len());out(\"😀\".len());out(\"👨‍👩‍👧‍👦\".len());out(\"🇨🇳\".len());out(\"👍🏽\".len());out(\"\".len()); for(ch,\"中é👨‍👩‍👧‍👦عَ\"){out(ch);out(ch.len());}}",
        "1\n1\n1\n1\n1\n1\n1\n1\n1\n0\n中\n1\né\n1\n👨‍👩‍👧‍👦\n1\nعَ\n1\n");
}

#[test]
fn unicode_length_dispatch_preserves_collection_and_family_methods() {
    executes("family Counter{func len(int offset)=>offset+1;} func m{out(arr(1,2).len());out(set(1,1).len());out(map().len());c=Counter();out(c.len(4));}", "2\n1\n0\n5\n");
    runtime_error("func m{out(\"x\".len(1));}", "takes no arguments");
}

#[test]
fn string_reverse_returns_a_new_string_and_preserves_graphemes() {
    executes("func m{ s=\"中é👨‍👩‍👧‍👦عَ🇨🇳👍🏽\"; reversed=s.reverse(); out(reversed); out(s); out(\"\".reverse()); out(\"ABCDE\".reverse()); fixed f=\"ab\"; out(f.reverse()); out(f); }",
        "👍🏽🇨🇳عَ👨‍👩‍👧‍👦é中\n中é👨‍👩‍👧‍👦عَ🇨🇳👍🏽\n\nEDCBA\nba\nab\n");
    runtime_error("func m{out(\"x\".reverse(1));}", "string.reverse() takes no arguments");
}

#[test]
fn string_reverse_dispatch_preserves_other_reverse_methods() {
    executes("family Counter{func reverse(int offset)=>offset+1;} func m{c=Counter();out(c.reverse(4)); a=arr(1,2); a.reverse(); out(a.iget(0)); out(a.iget(1));}", "5\n2\n1\n");
}

#[test]
fn named_reverse_is_restricted_to_the_third_slicing_argument() {
    for method in ["slice", "lenslice"] {
        let expr = marslang::compile_source_to_js(&format!("func m{{ text=\"ABCDE\"; out(text.{method}(0,3,reverse=true)); }}"));
        assert!(expr.is_ok(), "{expr:?}");
    }
    for source in [
        "f(reverse=true)", "text.reverse(reverse=true)",
        "text.lenslice(start=0,3)", "text.lenslice(0,reverse=true)",
        "text.lenslice(0,3,reverse=true,reverse=false)",
        "text.lenslice(0,3,reverse=true,1)", "text.lenslice(0,3,reverse=)",
    ] {
        let program = format!("func f(int x)=>x; func m{{text=\"ABCDE\"; {source};}}");
        assert!(marslang::compile_source_to_js(&program).is_err(), "accepted {source}");
    }
}

#[test]
fn unicode_slicing_preserves_order_and_whole_characters() {
    executes(r#"func m{
        s="A中é👨‍👩‍👧‍👦عَ🇨🇳👍🏽Z";
        out(s.lenslice(1,3)); out(s.slice(1,4));
        out(s.lenslice(1,3,reverse=true)); out(s.slice(1,4,reverse=true));
        out("ABCDE".lenslice(0,3,reverse=true));
        out("ABCDE".lenslice(1,3,reverse=true));
        out("ABCDE".lenslice(0,3,reverse=false));
        out("ABCDE".slice(0,3,true)); out(s);
        n (longint)=1; out(s.lenslice(n,1));
    }"#, "中é👨‍👩‍👧‍👦\n中é👨‍👩‍👧‍👦\nعَ🇨🇳👍🏽\nعَ🇨🇳👍🏽\nCDE\nBCD\nABC\nCDE\nA中é👨‍👩‍👧‍👦عَ🇨🇳👍🏽Z\n中\n");
}

#[test]
fn slicing_accepts_empty_ranges_and_exact_boundaries() {
    executes(r#"func m{
        for(back,arr(false,true)){
            out("ABC".lenslice(3,0,reverse=back).len());
            out("ABC".slice(3,3,reverse=back).len());
            out("".lenslice(0,0,reverse=back).len());
            out("".slice(0,0,reverse=back).len());
            out("ABC".lenslice(0,3,reverse=back));
            out("ABC".slice(0,3,reverse=back));
            out(arr().lenslice(0,0,reverse=back).len());
            out(arr(1).slice(1,1,reverse=back).len());
        }
    }"#, "0\n0\n0\n0\nABC\nABC\n0\n0\n0\n0\n0\n0\nABC\nABC\n0\n0\n");
}

#[test]
fn slicing_rejects_invalid_bounds_with_named_error() {
    for receiver in ["\"ABC\"", "arr(1,2,3)"] {
        for method in ["slice", "lenslice"] {
            for args in ["-1,1", "0,-1", "4,0", "4,4", "0,4", "1,4", "9223372036854775807,0"] {
                for reverse in ["false", "true"] {
                    runtime_error(&format!("func m{{out({receiver}.{method}({args},reverse={reverse}));}}"),
                        "OutOfBoundsError: slice");
                }
            }
        }
        runtime_error(&format!("func m{{out({receiver}.slice(2,1));}}"), "OutOfBoundsError: slice");
        runtime_error(&format!("func m{{out({receiver}.lenslice(3,1));}}"), "OutOfBoundsError: slice");
    }
}

#[test]
fn slicing_validates_argument_types_and_arity() {
    for receiver in ["\"ABC\"", "arr(1,2,3)"] {
        for method in ["slice", "lenslice"] {
            for args in ["", "0", "0,1,true,false", "0.5,1", "0,1.5", "\"0\",1", "true,1", "0,1,1", "0,1,null"] {
                runtime_error(&format!("func m{{out({receiver}.{method}({args}));}}"), "TypeError:");
            }
        }
    }
}

#[test]
fn array_slices_preserve_types_and_share_elements() {
    executes(r#"func m{
        a (array[int])=arr(1,2,3,4,5);
        b=a.lenslice(0,3,reverse=true); for(x,b){out(x);} out(a.len());
        c=a.slice(1,4,reverse=true); for(x,c){out(x);}
        child=arr(1); parent=arr(child); cut=parent.lenslice(0,1);
        out(cut==parent); out(cut.iget(0)==child);
        fixed f=arr(1,2); fresh=f.lenslice(0,1); fresh.add(3); out(fresh.len());
    }"#, "3\n4\n5\n5\n2\n3\n4\nfalse\ntrue\n2\n");
    runtime_error("func m{a (array[int])=arr(1,2);b=a.lenslice(0,1);b.add(\"bad\");}", "expected int");
    runtime_error("func m{fixed a=arr(arr(1));b=a.slice(0,1);b.iget(0).add(2);}", "cannot mutate fixed");
}

#[test]
fn slicing_dispatch_evaluates_once_and_preserves_family_methods() {
    executes(r#"calls=0;
        func text{calls=calls+1;ret "ABCDE";}
        func flag{calls=calls+10;ret true;}
        family Window{func slice(int a,int b)=>a+b;func lenslice(int a,int b)=>a*b;}
        func m{out(text().lenslice(0,3,reverse=flag()));out(calls);
            w=Window();out(w.slice(2,3));out(w.lenslice(2,3));}
    "#, "CDE\n11\n5\n6\n");
}

#[test]
fn public_api_string_example_executes() {
    executes(include_str!("../docs/api/examples/strings.mrs"), "CDE\nEDCBA\nABCDE\n3\né👨‍👩‍👧‍👦\n0\n");
}
