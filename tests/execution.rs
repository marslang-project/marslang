fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result {
        panic!("runtime error: {error}\noutput so far:\n{stdout}");
    }
    assert_eq!(stdout, expected);
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
        let error = marslang::compile(source).expect_err(source);
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
        assert!(marslang::compile(source).is_err(), "accepted: {source}");
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
    let diagnostic = marslang::compile("/* unfinished")
        .expect_err("unterminated comment must be rejected");
    assert!(diagnostic.to_lowercase().contains("comment"), "{diagnostic}");
    assert!(diagnostic.contains("1:1"), "expected opening location 1:1: {diagnostic}");
}

#[test]
fn hot_reassignment_is_rejected() {
    let diagnostic = marslang::compile("func m{\n hot x = 5;\n x = 6;\n}")
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
        let error = marslang::compile(source).expect_err(source);
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
    let error = marslang::compile("// ignored\n  /* open").unwrap_err();
    assert!(error.contains("2:3"), "{error}");
    assert!(marslang::compile("out(\"unfinished);").is_err());
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

#[test]
fn scope_assignment_and_explicit_shadowing() {
    executes("x = 1; func change { x = 2; } func m { change(); out(x); if (true) { x = 3; cold x = x + 1; out(x); x = 5; out(x); } out(x); x (int) = 10; if (true) { x = 11; } out(x); }", "2\n4\n5\n3\n11\n");
    assert!(marslang::compile("func m{ cold x=1; cold x=2; }").unwrap_err().contains("duplicate local"));
    assert!(marslang::compile("func m{ out(missing); }").unwrap_err().contains("unknown name"));
}

#[test]
fn fixed_aliases_and_deep_copy() {
    runtime_error("func m{ a1=arr(1); fixed a2=a1; a1.add(2); }", "cannot mutate fixed value");
    runtime_error("func m{ nested=arr(1); fixed a1=arr(nested); nested.add(2); }", "cannot mutate fixed value");
    runtime_error("func m{ p1=pair(1,2); fixed p2=p1; p1.first=3; }", "cannot mutate fixed value");
    assert!(marslang::compile("func m{ fixed x=5; y=x; y=6; }").unwrap_err().contains("fixed/hot"));
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
        assert!(marslang::compile(source).unwrap_err().contains("inside a loop"));
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
    assert!(marslang::compile("func m{for(i=0,i<2,i++){} out(i);}").is_err());
    assert!(marslang::compile("func m{for((i=0,n=0),i<2,i++){}}").is_err());
}

#[test]
fn collection_iteration_uses_a_snapshot() {
    executes("func m{ xs=arr(1,2); for(x,xs){xs.add(9);out(x);} out(xs.len()); d=map();d.set(\"a\",1);for(k,d){d.set(\"b\",2);out(k);} out(d.len()); }", "1\n2\n4\na\n2\n");
}

#[test]
fn hot_constants_substitute_and_keep_numeric_checks() {
    executes("hot x (int)=2; hot y=x+3; func m{out(y*2);}", "10\n");
    assert!(marslang::compile("func m{ x=1; hot y=x; }").unwrap_err().contains("compile-time constant"));
    runtime_error("func m{ x (int)=-2147483648; out(-x); }", "int overflow");
}

#[test]
fn copy_preserves_cycles_and_shared_structure_without_fixed_state() {
    executes("func m{ xs=arr(); xs.add(xs); fixed root=xs; ys=root.copy(); out(ys==ys.iget(0)); ys.add(1); out(root.len()); out(ys.len()); child=arr(1); tree=arr(child,child); cloned=tree.copy(); out(cloned.iget(0)==cloned.iget(1)); out(cloned.iget(0)==child); }", "true\n1\n2\ntrue\nfalse\n");
}

#[test]
fn bundled_example_executes() {
    executes(include_str!("../hello.mars"), "3\n78.53975\n");
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
        let expr = marslang::compile(&format!("func m{{ text=\"ABCDE\"; out(text.{method}(0,3,reverse=true)); }}"));
        assert!(expr.is_ok(), "{expr:?}");
    }
    for source in [
        "f(reverse=true)", "text.reverse(reverse=true)",
        "text.lenslice(start=0,3)", "text.lenslice(0,reverse=true)",
        "text.lenslice(0,3,reverse=true,reverse=false)",
        "text.lenslice(0,3,reverse=true,1)", "text.lenslice(0,3,reverse=)",
    ] {
        let program = format!("func f(int x)=>x; func m{{text=\"ABCDE\"; {source};}}");
        assert!(marslang::compile(&program).is_err(), "accepted {source}");
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
    executes(include_str!("../docs/api/examples/strings.mars"), "CDE\nEDCBA\nABCDE\n3\né👨‍👩‍👧‍👦\n0\n");
}

#[test]
fn dynamic_variables_change_type_but_annotations_and_fixed_still_hold() {
    executes(r#"takepkg std.math;
        func identity([int,longint,float] value)=>value;
        func m{
            x=5; x="hello"; out(x); x=1.0;
            out(math.min(identity(x),2.0));
            x=longint(3); out(math.max(identity(x),longint(2)));
            y (float)=1; out(math.min(y,2.0));
        }"#, "hello\n1\n3\n1\n");
    runtime_error("func m{x (int)=5;x=\"hello\";}","expected int");
    assert!(marslang::compile("func m{fixed x=5;x=\"hello\";}").unwrap_err().contains("cannot reassign"));
    assert!(marslang::compile("func m{x := 5;}").is_err());
}

#[test]
fn std_math_algorithms_preserve_numeric_kinds_and_bounds() {
    executes(r#"takepkg std.math;
        func m{
            out(math.min(3,2)); out(math.max(-3,-2)); out(math.abs(-7));
            out(math.clamp(-1,0,5)); out(math.clamp(6,0,5)); out(math.clamp(3,0,5));
            out(math.clamp(2,2,2));
            out(math.min(math.abs(-1.0),2.0));
            out(math.max(math.clamp(2.0,0.0,1.0),0.0));
            out(math.abs(longint(-9223372036854775807)));
            out(math.clamp(longint(3),longint(1),longint(2)));
            f=math.min; out(f(4,5));
        }"#, "2\n-2\n7\n0\n5\n3\n2\n1\n1\n9223372036854775807\n2\n4\n");
}

#[test]
fn std_math_checks_types_arity_and_invalid_ranges() {
    for expression in ["math.min(1,1.0)","math.max(1,longint(1))", "math.clamp(1.0,0.0,2)",
        "math.abs(true)", "math.abs(\"1\")", "math.abs(null)", "math.min(1)",
        "math.abs(1,2)", "math.max(arr(),arr())"] {
        runtime_error(&format!("takepkg std.math;func m{{out({expression});}}"),"TypeError:");
    }
    runtime_error("takepkg std.math;func m{out(math.clamp(1,3,2));}","lower bound exceeds upper bound");
    runtime_error("takepkg std.math;func m{out(math.abs(-2147483648));}","int overflow");
    runtime_error("takepkg std.math;func m{out(math.abs(-9223372036854775808));}","longint overflow");
    runtime_error("takepkg std.math;func m{out(math.abs(float(\"NaN\")));}","requires finite float");
}

#[test]
fn numeric_kind_survives_returns_containers_fields_and_copy() {
    executes(r#"takepkg std.math;
        func identity([int,longint,float] v)=>v;
        family Box{func init([int,longint,float] v){me.value=v;}func get=>me.value;}
        func m{
            v=identity(1.0); a=arr(v); d=map();d.set("v",v);p=pair(v,v);b=Box(v);
            for(x,arr(a.copy().iget(0),d.copy().get("v"),p.copy().first,b.copy().get())){
                out(math.min(x,2.0));
            }
            fixed f=1.0; out(math.min(f.copy(),2.0));
            hot h (float)=1; out(math.min(h,2.0));
            a2 (array[float])=arr(1);out(math.min(a2.iget(0),2.0));
            p2 (pair[float,float])=pair(1,2);out(math.min(p2.first,2.0));
            s2 (set[float])=set(1);for(x,s2){out(math.min(x,2.0));}
            d2 (map[float,float])=map();d2.set(1,2);for(k,d2){out(math.min(k,d2.get(k)));}
        }"#, "1\n1\n1\n1\n1\n1\n1\n1\n1\n1\n");
    runtime_error(r#"takepkg std.math;func identity([int,longint,float] v)=>v;
        func m{out(math.min(identity(1.0),1));}"#, "requires matching numeric types");
}

#[test]
fn float_tracking_preserves_scalar_equality_truth_and_collection_keys() {
    executes(r#"takepkg std.math;func m{
        out(1.0==1.0);out(1.0==1);out(1.0=="1");out(not 0.0);
        a=arr(1.0,2.0,1.0);out(a.has(1.0));out(a.get(1.0));out(a.rget(1.0));
        a.change(1.0,3.0);out(a.iget(0));
        s=set(1.0,1.0,2.0);out(s.len());out(s.has(1.0));
        for(x,s.copy()){out(math.min(x,3.0));}
        d=map();d.set(1.0,2.0);d.set(1.0,3.0);out(d.len());out(d.get(1.0));
        for(k,d.copy()){out(math.min(k,d.get(k)));}
        out(d.remove(1.0));out(s.remove(1.0));
        out(math.min(1.0+1.0,3.0));out(math.min(4.0/2.0,3.0));
        out(math.min(-(-1.0),2.0));
    }"#, "true\ntrue\nfalse\ntrue\ntrue\n0\n2\n3\n2\ntrue\n1\n2\n1\n3\n1\ntrue\ntrue\n2\n2\n1\n");
}

#[test]
fn bundled_math_imports_are_isolated_aliased_and_deduplicated() {
    executes(r#"takepkg std.math;takepkg std.math;
        takepkg std.math = calc;takepkg std.math = out;
        func min(int a,int b)=>99;
        func m{slout(math.min(2,3));slout(calc.max(2,3));slout(out.abs(-4));slout(min(1,2));}
    "#, "23499");
    runtime_error("takepkg std.math;func m{math.min=1;}", "TypeError:");
    assert!(marslang::compile("takepkg std.math = *;").unwrap_err().contains("wildcard"));
    assert!(marslang::compile("takepkg std.file;").unwrap_err().contains("not implemented"));
    assert!(marslang::compile("takepkg std.math = bad-name;").unwrap_err().contains("alias"));
    assert!(marslang::compile("takepkg std.math;math=1;").unwrap_err().contains("cannot reassign"));
}

#[test]
fn arithmetic_uses_runtime_numeric_kind_through_dynamic_boundaries() {
    executes(r#"takepkg std.math;
        func inc([int,longint,float] v)=>v+1;
        func m{
            i (int)=1; x=1.0; out(math.min(i+x,3.0));
            out(math.min(inc(x),3.0));
            x=2; out(math.min(inc(x),4));
            out(math.min(5/2,3.0));out(math.min(5.0-3.0,3.0));
        }"#, "2\n2\n3\n2.5\n2\n");
    runtime_error("func inc([int,longint,float] v)=>v+1;func m{out(inc(2147483647));}","int overflow");
    runtime_error("func m{x=1.0;y=longint(1);out(x+y);}","mixed longint/float");
}

#[test]
fn fixed_numeric_containers_keep_their_value_kinds() {
    executes(r#"takepkg std.math;func m{
        fixed p=pair(1.0,2.0);q (pair[float,float])=p;
        out(math.min(q.first,q.second));
        fixed a=arr(1.0);b (array[float])=a;out(math.min(b.iget(0),2.0));
    }"#, "1\n1\n");
    for (value, ty) in [("arr(1)","array[float]"),("set(1)","set[float]"),
        ("pair(1,2)","pair[float,float]")] {
        runtime_error(&format!("func m{{fixed a={value};b ({ty})=a;}}"), "cannot mutate fixed");
    }
}

#[test]
fn math_constants_are_float_values_and_read_only() {
    executes(r#"takepkg std.math;func m{
        out(math.PI);out(math.E);out(math.TAU);out(math.SQRT2);out(math.LN2);out(math.LN10);
        out(math.min(math.PI,4.0));out(math.TAU==math.PI*2.0);
    }"#, "3.141592653589793\n2.718281828459045\n6.283185307179586\n1.4142135623730951\n0.6931471805599453\n2.302585092994046\n3.141592653589793\ntrue\n");
    runtime_error("takepkg std.math;func m{math.PI=3.0;}","TypeError:");
}

#[test]
fn float_infinity_conversion_and_classification_without_a_keyword() {
    executes(r#"takepkg std.math;func m{
        inf=7;out(inf);
        for(x,arr(float("inf"),float("-inf"),float(" +INF "),float("Infinity"))){
            out(math.is_inf(x));out(math.is_finite(x));out(math.is_nan(x));
        }
        n=float("nan");out(math.is_nan(n));out(math.is_inf(n));out(math.is_finite(n));
        out(math.is_finite(1.0));out(math.is_finite(-0.0));
        out(float("-inf")<0.0);out(float("inf")>0.0);
    }"#, "7\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\ntrue\ntrue\ntrue\ntrue\n");
    assert!(marslang::compile("func m{out(inf);}").unwrap_err().contains("unknown name"));
    runtime_error("takepkg std.math;func m{out(math.is_finite(1));}","requires float");
}

#[test]
fn math_sign_helpers_and_signed_zero_contract() {
    executes(r#"takepkg std.math;func m{
        out(math.sign(-3));out(math.sign(0));out(math.sign(2.0));out(math.sign(longint(-5)));
        out(math.sign(-0.0));out(math.signbit(-0.0));out(math.signbit(0.0));
        out(math.signbit(math.copysign(1.0,-0.0)));out(math.copysign(-2,3));
        out(math.copysign(-2147483648,-1));out(math.copysign(longint(-9223372036854775808),longint(-1)));
        out(math.signbit(math.abs(-0.0)));
        out(math.signbit(math.min(-0.0,0.0)));out(math.signbit(math.min(0.0,-0.0)));
        out(math.signbit(math.max(-0.0,0.0)));out(math.signbit(math.max(0.0,-0.0)));
        out(math.signbit(math.clamp(-0.0,-1.0,1.0)));
    }"#, "-1\n0\n1\n-1\n0\ntrue\nfalse\ntrue\n2\n-2147483648\n-9223372036854775808\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\n");
    runtime_error("takepkg std.math;func m{out(math.copysign(-2147483648,1));}","int overflow");
    runtime_error("takepkg std.math;func m{out(math.copysign(longint(-9223372036854775808),longint(1)));}","longint overflow");
}

#[test]
fn math_interpolation_extrapolation_and_angles() {
    executes(r#"takepkg std.math;func m{
        out(math.lerp(0.0,10.0,0.25));out(math.lerp(0.0,10.0,2.0));
        out(math.inverse_lerp(0.0,10.0,2.5));out(math.inverse_lerp(10.0,0.0,2.5));
        out(math.remap(5.0,0.0,10.0,0.0,100.0));out(math.remap(20.0,0.0,10.0,0.0,100.0));
        out(math.step(2.0,1.0));out(math.step(2.0,2.0));out(math.step(2.0,3.0));
        out(math.abs(math.radians(180.0)-math.PI)<1e-14);
        out(math.abs(math.degrees(math.PI)-180.0)<1e-12);
    }"#, "2.5\n20\n0.25\n0.75\n50\n200\n0\n1\n1\ntrue\ntrue\n");
    runtime_error("takepkg std.math;func m{out(math.inverse_lerp(1.0,1.0,2.0));}","division by zero");
    runtime_error("takepkg std.math;func m{out(math.remap(2.0,1.0,1.0,0.0,10.0));}","division by zero");
    runtime_error("takepkg std.math;func m{out(math.inverse_lerp(-1e308,1e308,0.0));}","non-finite intermediate");
}

#[test]
fn math_rounding_uses_half_even_and_retains_negative_zero() {
    executes(r#"takepkg std.math;func m{
        for(x,arr(2.5,3.5,-2.5,-3.5,2.49,2.51,4503599627370496.0)){out(math.round(x));}
        out(math.floor(-1.2));out(math.ceil(-1.2));out(math.trunc(-1.2));
        out(math.signbit(math.round(-0.5)));out(math.signbit(math.round(-0.1)));
        out(math.signbit(math.trunc(-0.1)));out(math.signbit(math.ceil(-0.1)));
        out(math.min(math.round(2.5),3.0));
    }"#, "2\n4\n-2\n-4\n2\n3\n4503599627370496\n-2\n-1\n-1\ntrue\ntrue\ntrue\ntrue\n2\n");
}

#[test]
fn math_powers_roots_and_stable_hypot() {
    executes(r#"takepkg std.math;func m{
        out(math.sqrt(9.0));out(math.cbrt(-8.0));out(math.hypot(3.0,4.0));
        out(math.pow(2,10));out(math.pow(-2,31));out(math.pow(longint(-2),longint(63)));
        out(math.pow(longint(1),longint(9223372036854775807)));out(math.pow(0,0));
        out(math.pow(2.0,-2.0));out(math.min(math.pow(2.0,3.0),10.0));
        out(math.abs(math.hypot(1e308,1e308)/1e308-math.SQRT2)<1e-14);
        out(math.abs(math.hypot(1e-300,1e-300)/1e-300-math.SQRT2)<1e-14);
    }"#, "3\n-2\n5\n1024\n-2147483648\n-9223372036854775808\n1\n1\n0.25\n8\ntrue\ntrue\n");
    for expression in ["math.pow(2,31)","math.pow(longint(2),longint(63))", "math.pow(2,2147483647)"] {
        runtime_error(&format!("takepkg std.math;func m{{out({expression});}}"),"overflow");
    }
    runtime_error("takepkg std.math;func m{out(math.pow(2,-1));}","nonnegative");
}

#[test]
fn math_logs_exponentials_and_trig() {
    executes(r#"takepkg std.math;func m{
        out(math.exp(0.0));out(math.exp2(3.0));out(math.ln(1.0));
        out(math.log2(8.0));out(math.log10(100.0));out(math.log(8.0,2.0));
        out(math.abs(math.expm1(1e-16)-1e-16)<1e-30);
        out(math.abs(math.log1p(1e-16)-1e-16)<1e-30);
        out(math.sin(0.0));out(math.cos(0.0));out(math.tan(0.0));
        out(math.asin(0.0));out(math.acos(1.0));out(math.atan(0.0));
        out(math.abs(math.atan2(1.0,0.0)-math.PI/2.0)<1e-14);
        out(math.abs(math.atan2(-1.0,-1.0)+math.PI*0.75)<1e-14);
        out(math.sinh(0.0));out(math.cosh(0.0));out(math.tanh(0.0));
    }"#, "1\n8\n0\n3\n2\n3\ntrue\ntrue\n0\n1\n0\n0\n0\n0\ntrue\ntrue\n0\n1\n0\n");
}

#[test]
fn math_domains_nonfinite_inputs_and_results_raise_errors() {
    for expression in ["math.sqrt(-1.0)","math.ln(0.0)","math.ln(-1.0)",
        "math.log2(0.0)","math.log10(-1.0)","math.log1p(-1.0)","math.log1p(-2.0)",
        "math.asin(2.0)","math.acos(-2.0)","math.log(1.0,1.0)","math.log(0.0,2.0)",
        "math.log(2.0,-1.0)","math.pow(-1.0,0.5)","math.pow(0.0,-1.0)",
        "math.exp(1000.0)","math.exp2(1024.0)","math.cosh(1000.0)",
        "math.hypot(1.7e308,1.7e308)","math.degrees(1e308)",
        "math.min(float(\"nan\"),1.0)","math.max(1.0,float(\"nan\"))",
        "math.clamp(1.0,0.0,float(\"nan\"))", "math.sign(float(\"inf\"))",
        "math.floor(float(\"inf\"))", "math.sin(float(\"-inf\"))"] {
        runtime_error(&format!("takepkg std.math;func m{{out({expression});}}"),"RangeError:");
    }
}

#[test]
fn expanded_math_checks_float_types_arity_and_first_class_calls() {
    for (name,arity) in [("floor",1),("ceil",1),("round",1),("trunc",1),("sqrt",1),("cbrt",1),
        ("hypot",2),("exp",1),("exp2",1),("ln",1),("log2",1),("log10",1),("log",2),
        ("expm1",1),("log1p",1),("sin",1),("cos",1),("tan",1),("asin",1),("acos",1),
        ("atan",1),("atan2",2),("sinh",1),("cosh",1),("tanh",1),("radians",1),("degrees",1),
        ("lerp",3),("inverse_lerp",3),("remap",5),("step",2),("is_nan",1),("is_inf",1),("is_finite",1)] {
        let args=vec!["1";arity].join(",");
        runtime_error(&format!("takepkg std.math;func m{{math.{name}({args});}}"),"requires float");
        runtime_error(&format!("takepkg std.math;func m{{math.{name}();}}"),"expects");
    }
    executes("takepkg std.math = calc;func m{f=calc.sqrt;out(f(4.0));g=calc.round;out(g(2.5));}","2\n2\n");
    runtime_error("takepkg std.math;func m{out(math.pow(2.0,2));}","matching numeric types");
    runtime_error("takepkg std.math;func m{out(math.copysign(2,2.0));}","matching numeric types");
}

#[test]
fn numbers_compare_by_value_across_kinds() {
    executes(r#"func m{
        out(longint(5) < 10); out(1 == longint(1)); out(longint(2) < 2.5); out(1.0 == longint(1));
        out(longint(9007199254740993) == 9007199254740992.0); out(longint(-3) >= -3);
        out(set(1, longint(1), 1.0).len());
        d=map(); d.set(longint(2),"a"); out(d.get(2)); out(d.get(2.0));
        out(arr(longint(7)).has(7)); out(arr(3, longint(1), 2.5).asort().iget(0));
    }"#, "true\ntrue\ntrue\ntrue\nfalse\ntrue\n1\na\na\ntrue\n1\n");
}

/// Write package files under a fresh temporary directory and return it.
fn package_dir(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("marslang-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, source) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    dir
}

fn run_file(path: &std::path::Path) -> (String, Result<(), marslang::RuntimeError>) {
    marslang::run_captured(marslang::compile_file(path).expect("compile failed"), "")
}

#[test]
fn takepkg_loads_marslang_package_files() {
    let dir = package_dir("packages", &[
        ("main.mars", "takepkg util;\ntakepkg shapes.circle = c;\nfunc m{ out(util.twice(21)); out(util.LIMIT); out(c.area(c.Circle(2.0))); out(util.count()); }"),
        ("util.mars", "fixed LIMIT (int) = 10;\nloads = 0;\nloads = loads + 1;\nfunc twice(int x) => x * 2;\nfunc count => loads;\nfunc _hidden => 1;"),
        ("shapes/circle.mars", "takepkg std.math;\ntakepkg util;\nfamily Circle{ func init(float r){ me.r = r; } }\nfunc area(Circle shape) => math.PI * shape.r * shape.r + util.count() - 1;"),
    ]);
    let (out, result) = run_file(&dir.join("main.mars"));
    result.expect("runtime error");
    // util is imported twice but loaded once: its top-level code ran one time.
    assert_eq!(out, "42\n10\n12.566370614359172\n1\n");

    std::fs::write(dir.join("main.mars"), "takepkg util;\nfunc m{ util._hidden(); }").unwrap();
    let error = run_file(&dir.join("main.mars")).1.unwrap_err().to_string();
    assert!(error.contains("has no member '_hidden'"), "{error}");

    std::fs::write(dir.join("main.mars"), "takepkg util;\nfunc m{ util.LIMIT = 3; }").unwrap();
    assert!(run_file(&dir.join("main.mars")).1.unwrap_err().to_string().contains("read-only"));
}

#[test]
fn takepkg_reports_missing_circular_and_native_packages() {
    let dir = package_dir("package-errors", &[
        ("a.mars", "takepkg b;\nfunc f => 1;"),
        ("b.mars", "takepkg a;\nfunc g => 2;"),
        ("main.mars", "takepkg a;\nfunc m{}"),
    ]);
    let error = marslang::compile_file(&dir.join("main.mars")).unwrap_err();
    assert!(error.contains("circular package import: a -> b -> a"), "{error}");
    let error = marslang::compile("takepkg nowhere.to_be_found;").unwrap_err();
    assert!(error.contains("package 'nowhere.to_be_found' not found"), "{error}");
    let error = marslang::compile("takepkg rs.math;").unwrap_err();
    assert!(error.contains("only available to standard packages"), "{error}");
    assert!(marslang::compile("takepkg bad-name;").is_err());
}

#[test]
fn takepkg_packages_init_files_and_relative_imports() {
    let dir = package_dir("python-packages", &[
        ("main.mars", "takepkg app;\ntakepkg app.core.calc;\nfunc m{ out(app.VERSION); out(calc.total(2)); out(app.describe()); }"),
        // A directory with init.mars is a package; its init may import its own children.
        ("app/init.mars", "takepkg .core.calc;\nslout(\"init app;\");\nfixed VERSION (string) = \"1.0\";\nfunc describe => \"app with \" + string(calc.total(0));"),
        ("app/helpers.mars", "func base => 100;"),
        // Directories without init.mars are plain folders on the path.
        ("app/core/calc.mars", "takepkg .offset;\ntakepkg ..helpers;\nfunc total(int x) => helpers.base() + offset.OFFSET + x;"),
        ("app/core/offset.mars", "fixed OFFSET (int) = 10;"),
    ]);
    let (out, result) = run_file(&dir.join("main.mars"));
    result.expect("runtime error");
    // app/init runs once, before main, even though calc is imported twice.
    assert_eq!(out, "init app;1.0\n112\napp with 110\n");

    for (source, message) in [
        ("takepkg .helpers;\nfunc m{}", "has no parent package"),
        ("takepkg app.core.calc;\ntakepkg app.missing;\nfunc m{}", "package 'app.missing' not found"),
        ("takepkg app.init;\nfunc m{}", "'init' is reserved"),
    ] {
        std::fs::write(dir.join("main.mars"), source).unwrap();
        let error = marslang::compile_file(&dir.join("main.mars")).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    // From module app.helpers (package app), two dots would leave the top-level package.
    std::fs::write(dir.join("app/helpers.mars"), "takepkg ..other;\nfunc base => 1;").unwrap();
    std::fs::write(dir.join("main.mars"), "takepkg app.helpers;\nfunc m{}").unwrap();
    let error = marslang::compile_file(&dir.join("main.mars")).unwrap_err();
    assert!(error.contains("beyond the top-level package 'app'"), "{error}");
}

#[test]
fn collection_keeps_reachable_cycles_and_temporaries_intact() {
    // 30,000 garbage cycles force several collections while live cycles are held
    // by a global, a local, a parameter, and an unfinished call's argument list.
    executes(r#"
        kept = arr();
        family Node{ func init(){ me.self_ref = me; me.cb = me.get; } func get => 1; }
        func churn(int n){ total = 0; repeat n { g = arr(); g.add(g); node = Node(); total = total + node.cb(); } ret total; }
        func cyc(){ c = arr(); c.add(c); ret c; }
        func hold(array c) => churn(15000) + c.len();
        func m{
            kept.add(kept);
            local = cyc();
            out(churn(15000));
            both = pair(cyc(), churn(15000));
            out(both.first.iget(0) == both.first);
            out(hold(cyc()));
            out(kept.iget(0) == kept, local.iget(0) == local, kept.len());
        }"#, "15000\ntrue\n15001\ntrue true 1\n");
}

#[test]
fn family_annotations_accept_descendants_and_compare_declarations() {
    executes("family Parent{ func init(int x){ me.x=x; } }\nfamily Child(Parent){}\nfamily Grandchild(Child){}\nfunc read(Parent p) => p.x;\nfunc m{ out(read(Child(7))); out(read(Grandchild(8))); items (array[Parent]) = arr(Child(1)); out(items.len()); }", "7\n8\n1\n");
    runtime_error("family Parent{}\nfamily Other{}\nfunc read(Parent p) => 1;\nfunc m{ read(Other()); }", "expected Parent");
    runtime_error("func read(Missing p) => 1;\nfunc m{ read(1); }", "unknown type Missing");

    let dir = package_dir("family-identity", &[
        ("left.mars", "family Box{ func init(int x){ me.x=x; } }\nfunc read(Box b) => b.x;"),
        ("right.mars", "family Box{ func init(int x){ me.x=x; } }"),
        ("main.mars", "takepkg left;\ntakepkg right;\nfunc take(right.Box b) => b.x;\nfunc m{ out(left.read(left.Box(4))); out(take(right.Box(5))); left.read(right.Box(9)); }"),
    ]);
    let (out, result) = run_file(&dir.join("main.mars"));
    assert_eq!(out, "4\n5\n");
    assert!(result.unwrap_err().to_string().contains("expected Box"));
}

#[test]
fn failed_union_alternatives_leave_values_unchanged() {
    executes(r#"func choose([pair[array[longint],int],pair[array[int],string]] value) => value;
        func m{
            a=arr(1); value=pair(a,"ok"); out(choose(value).second);
            // The first alternative would have made the element a longint.
            a.add(2); out(a.iget(0) == 1, a.len());
            b (array[int]) = a; b.add(3); out(b.len());
        }"#, "ok\ntrue 2\n3\n");
    runtime_error("func choose([pair[array[longint],int],pair[array[int],string]] value) => value;\nfunc m{ a=arr(1); choose(pair(a,\"ok\")); a.add(\"x\"); }", "expected int");
}

#[test]
fn member_call_target_is_chosen_before_arguments() {
    executes("family Holder{ func init{ me.f=first; } }\nfunc first(int x) => 1;\nfunc second(int x) => 2;\nfunc swap(Holder h){ h.f=second; ret 0; }\nfunc m{ h=Holder(); out(h.f(swap(h))); out(h.f(0)); }", "1\n2\n");
}

#[test]
fn run_handle_catches_builtin_and_custom_errors() {
    executes(r#"
        family ParseError(Error){}
        family BadArgument(TypeError){}
        func parse(string text){
            if (text == ""){ err(ParseError, "empty input"); }
            ret text;
        }
        func m{
            run{ x = 1 / 0; } handle(RangeError e){ out(e.message); }
            run{ parse(""); } handle(ParseError e){ out(e.message); out(e); }
            run{ int("x"); } handle(Error e){ out(e); }
            run{ err(BadArgument, "custom kind"); } handle(TypeError e){ out(e); }
            run{ arr(1).iget(5); } handle(TypeError, RangeError){ out("no name"); }
            run{ "x" + 1; } handle([RangeError, TypeError] e){ out(e.message); }
        }"#,
        "division by zero\nempty input\nParseError: empty input\nTypeError: expected int\nBadArgument: custom kind\nno name\narithmetic requires numbers\n");
}

#[test]
fn first_matching_handler_wins_and_unmatched_errors_propagate() {
    executes(r#"
        family ParseError(Error){}
        func inner{
            run{ err(ParseError, "deep"); } handle(RangeError){ out("wrong handler"); }
        }
        func m{
            run{ err(ParseError, "first"); } handle(ParseError){ out("specific"); } handle(Error){ out("general"); }
            run{ inner(); } handle(ParseError e){ out("outer caught " + e.message); }
            run{
                run{ err(ParseError, "again"); } handle(Error e){ err(e); }
            } handle(ParseError e){ out("rethrown " + e.message); }
        }"#, "specific\nouter caught deep\nrethrown again\n");
    runtime_error("family ParseError(Error){}\nfunc m{ run{ err(ParseError, \"lost\"); } handle(RangeError){} }", "ParseError: lost");
}

#[test]
fn then_always_runs() {
    executes(r#"
        func early(){
            run{ ret "returned"; } then{ out("then after ret"); }
        }
        func m{
            run{ out("body"); } then{ out("then after body"); }
            run{ err(Error, "x"); } handle(Error){ out("handled"); } then{ out("then after handle"); }
            out(early());
            for(i=0,i<3,i++){
                run{ if(i==1){ break; } } then{ out("then " + string(i)); }
            }
            run{
                run{ err(RangeError, "passes through"); } then{ out("then while propagating"); }
            } handle(RangeError e){ out(e.message); }
        }"#,
        "body\nthen after body\nhandled\nthen after handle\nthen after ret\nreturned\nthen 0\nthen 1\nthen while propagating\npasses through\n");
    runtime_error("func m{ run{ err(Error, \"first\"); } then{ err(RangeError, \"from then\"); } }", "RangeError: from then");
}

#[test]
fn lasterr_returns_the_most_recently_handled_error() {
    executes(r#"func m{
        out(lasterr());
        run{ err(Error, "one"); } handle(Error){ out(lasterr().message); }
        run{ 1 / 0; } handle(RangeError){}
        out(lasterr());
    }"#, "null\none\nRangeError: division by zero\n");
}

#[test]
fn err_and_handle_reject_non_error_families() {
    runtime_error("family Plain{}\nfunc m{ err(Plain, \"x\"); }", "Plain is not an error family");
    runtime_error("func m{ err(\"just text\"); }", "err expects an error family");
    runtime_error("family Plain{}\nfunc m{ run{ err(Error, \"x\"); } handle(Plain){} }", "Plain is not an error family");
    runtime_error("func m{ run{ err(Error, \"x\"); } handle(Missing){} }", "unknown error type Missing");
    for source in ["func m{ run{ out(1); } }", "func m{ run{} handle([Error]){} }", "func m{ run{} handle(){} }"] {
        assert!(marslang::compile(source).is_err(), "accepted {source}");
    }
}

#[test]
fn packages_raise_errors_that_callers_can_handle() {
    let dir = package_dir("package-errors-handle", &[
        ("shapes.mars", "family ShapeError(Error){}\nfunc area(float r){ if (r < 0.0){ err(ShapeError, \"negative radius\"); } ret r * r; }"),
        ("main.mars", "takepkg shapes;\nfunc m{ run{ shapes.area(-1.0); } handle(shapes.ShapeError e){ out(e); } }"),
    ]);
    let (out, result) = run_file(&dir.join("main.mars"));
    result.expect("runtime error");
    assert_eq!(out, "ShapeError: negative radius\n");
}

#[test]
fn std_containers_stack_queue_deque_and_priority_queue() {
    executes(r#"takepkg std.containers;
        func m{
            s = containers.stack();
            s.push(1).push(2).push(3);
            out(s.pop(), s.peek(), s.len(), s.items());
            q = containers.queue();
            repeat 20 { q.push("x"); }
            q.push("last");
            repeat 20 { q.pop(); }
            out(q.pop(), q.pop(), q.is_empty());
            d = containers.deque();
            for (i=0, i<10, i++){ d.push_back(i); d.push_front(-i); }
            out(d.pop_front(), d.pop_back(), d.peek_front(), d.peek_back(), d.len());
            out(d.items().slice(0, 3));
            pq = containers.priority_queue();
            pq.push("low", 5).push("urgent", 1).push("first normal", 3).push("second normal", 3).push("mid", 2.5);
            order = arr();
            while (not pq.is_empty()){ order.add(pq.pop()); }
            out(order);
            out(pq.pop(), pq.peek());
        }"#,
        "3 2 2 [1, 2]\nlast null true\n-9 9 -8 8 18\n[-8, -7, -6]\n[\"urgent\", \"mid\", \"first normal\", \"second normal\", \"low\"]\nnull null\n");
    runtime_error("takepkg std.containers;\nfunc m{ fixed s = containers.stack(); s.push(1); }", "cannot mutate fixed value");
}

#[test]
fn std_math_integer_helpers_keep_integer_kinds() {
    executes(r#"takepkg std.math;
        func m{
            out(math.gcd(12, 18), math.gcd(-12, 18), math.gcd(0, 0), math.gcd(longint(12), longint(8)));
            out(math.lcm(4, 6), math.lcm(-4, 6), math.lcm(0, 5));
            out(math.is_even(4), math.is_odd(-3), math.is_even(longint(7)));
            out(math.div_floor(7, 2), math.div_floor(-7, 2), math.div_floor(7, -2), math.div_floor(-8, 2));
            out(math.div_ceil(7, 2), math.div_ceil(-7, 2), math.div_ceil(8, 2));
            out(math.factorial(0), math.factorial(12), math.factorial(longint(20)));
            out(math.perm(5, 2), math.perm(3, 5), math.comb(5, 2), math.comb(52, 5), math.comb(3, 5));
            out(math.comb(longint(66), longint(33)));
            out(math.max(math.gcd(12, 18), 1));
        }"#,
        "6 6 0 4\n12 12 0\ntrue true false\n3 -4 -4 -4\n4 -3 4\n1 479001600 2432902008176640000\n20 0 10 2598960 0\n7219428434016265740\n6\n");
    runtime_error("takepkg std.math;\nfunc m{ math.factorial(13); }", "int overflow");
    runtime_error("takepkg std.math;\nfunc m{ math.factorial(-1); }", "requires nonnegative");
    runtime_error("takepkg std.math;\nfunc m{ math.gcd(1.0, 2.0); }", "requires int or longint");
    runtime_error("takepkg std.math;\nfunc m{ math.gcd(1, longint(2)); }", "matching numeric types");
    runtime_error("takepkg std.math;\nfunc m{ math.div_floor(1, 0); }", "division by zero");
}

#[test]
fn any_annotation_accepts_every_value() {
    executes("family Box{}\nfunc show(any value) => value;\nfunc m{ out(show(1), show(\"s\"), show(null), show(arr(1)), show(Box())); items (array[any]) = arr(1, \"a\"); out(items); }",
        "1 s null [1] Box {}\n[1, \"a\"]\n");
}

#[test]
fn std_types_inspects_values() {
    executes(r#"takepkg std.types;
        family Shape{}
        family Circle(Shape){}
        func m{
            out(types.kind(1), types.kind(longint(1)), types.kind(1.5), types.kind("s"), types.kind(null), types.kind(arr()), types.kind(Circle()));
            out(types.family_name(Circle()), types.family_name(3));
            out(types.is_instance(Circle(), Shape), types.is_instance(Shape(), Circle), types.is_instance(1, Shape));
            out(types.is_number(2.5), types.is_number("2"));
        }"#,
        "int longint float string null array instance\nCircle null\ntrue false false\ntrue false\n");
}

#[test]
fn std_strings_search_split_and_pad_by_character() {
    executes(r#"takepkg std.strings;
        func m{
            out(strings.split("a,b,,c", ","), strings.split_whitespace("  one  two\tthree "), strings.lines("x\r\ny"));
            out(strings.join(arr("a", "b", "c"), "-"), strings.trim("  hi  "), strings.trim_start("  hi"), strings.trim_end("hi  "));
            out(strings.starts_with("marslang", "mars"), strings.ends_with("marslang", "lang"), strings.contains("marslang", "sl"));
            out(strings.find("中é👨‍👩‍👧‍👦x", "x"), strings.find("abc", "z"), strings.rfind("abcabc", "b"));
            out(strings.replace("a-b-c", "-", "+"), strings.upper("straße"), strings.lower("ÉCOLE"));
            out(strings.repeated("ab", 3), strings.pad_start("7", 3, "0"), strings.pad_end("中", 3, "."), strings.pad_start("long", 2, " "));
            s = "x"; out(string(5) + s);
        }"#,
        "[\"a\", \"b\", \"\", \"c\"] [\"one\", \"two\", \"three\"] [\"x\", \"y\"]\na-b-c hi hi hi\ntrue true true\n3 -1 4\na+b+c STRASSE école\nababab 007 中.. long\n5x\n");
    runtime_error("takepkg std.strings;\nfunc m{ strings.split(\"a\", \"\"); }", "separator must not be empty");
    runtime_error("takepkg std.strings;\nfunc m{ strings.join(arr(1), \",\"); }", "array of strings");
    runtime_error("takepkg std.strings;\nfunc m{ strings.pad_start(\"a\", 3, \"ab\"); }", "one character");
}

#[test]
fn std_time_clocks_and_sleep() {
    executes(r#"takepkg std.time;
        func m{
            start = time.monotonic();
            time.sleep(0.02);
            elapsed = time.monotonic() - start;
            out(elapsed >= 0.02, elapsed < 5.0, time.now() > 1700000000.0);
            time.sleep(0);
        }"#, "true true true\n");
    runtime_error("takepkg std.time;\nfunc m{ time.sleep(-1); }", "nonnegative");
}

#[test]
fn oversized_sleep_and_cyclic_error_messages_are_catchable() {
    executes(r#"takepkg std.time;
        func m{
            run{ time.sleep(1e30); } handle(RangeError e){ out(e); } then{ out("cleanup"); }
            run{
                run{ err(Error, "original"); } handle(Error e){ e.message = arr(e); out(e); err(e); }
            } handle(Error again){ out("rethrown", again); }
        }"#, "RangeError: sleep duration is too long\ncleanup\nError: [<cycle>]\nrethrown Error: [<cycle>]\n");
    runtime_error("func m{ run{ err(Error, \"x\"); } handle(Error e){ e.message = e; err(e); } }", "Error: Error: <cycle>");
}

#[test]
fn union_alternatives_with_shared_containers_apply_atomically() {
    executes(r#"takepkg std.types;
        func choose([pair[array[longint],array[int]],pair[array[int],array[int]]] value) => value;
        func m{
            a=arr(1);
            out(choose(pair(a,a)));
            out(a, types.kind(a.iget(0)));
            a.add(2); out(a.len());
            // A whole annotation that cannot hold changes nothing.
            b=arr(1);
            run{ x (pair[array[longint],array[int]]) = pair(b,b); } handle(TypeError e){ out(e); }
            out(types.kind(b.iget(0))); b.add("still unrestricted"); out(b.len());
        }"#, "pair([1], [1])\n[1] int\n2\nTypeError: expected int\nint\n2\n");
}

#[test]
fn gcd_handles_minimum_integers() {
    executes(r#"takepkg std.math;
        func m{
            out(math.gcd(-2147483648, 1), math.gcd(-2147483648, -2147483648 + 2), math.gcd(longint("-9223372036854775808"), longint(2)));
            out(math.lcm(-4, 6), math.comb(10, 3));
        }"#, "1 2 2\n12 120\n");
    runtime_error("takepkg std.math;\nfunc m{ math.gcd(-2147483648, 0); }", "int overflow");
    runtime_error("takepkg std.math;\nfunc m{ math.lcm(-2147483648, 1); }", "int overflow");
}

#[test]
fn priority_queue_items_are_in_pop_order_and_leave_the_queue_unchanged() {
    executes(r#"takepkg std.containers;
        func m{
            q = containers.priority_queue();
            q.push("c", 3).push("a", 1).push("b1", 2).push("b2", 2);
            out(q.items(), q.len(), q.peek());
            out(q.pop(), q.items());
            out(containers.priority_queue().items());
        }"#, "[\"a\", \"b1\", \"b2\", \"c\"] 4 a\na [\"b1\", \"b2\", \"c\"]\n[]\n");
}

#[test]
fn string_searches_match_whole_characters_only() {
    executes(r#"takepkg std.strings;
        func m{
            text = "e\u0301x";
            out(text.len(), strings.find(text, "\u0301"), strings.rfind(text, "\u0301"), strings.find(text, "x"));
            out(strings.contains(text, "e"), strings.contains(text, "e\u0301"), strings.starts_with(text, "e"), strings.ends_with(text, "x"));
            out(strings.replace(text, "e", "a"), strings.replace("aXbXc", "X", "-"), strings.split("a\r\nb", "\n").len(), strings.lines("a\r\nb").len());
            i = strings.find("中é👨‍👩‍👧‍👦x中", "中"); j = strings.rfind("中é👨‍👩‍👧‍👦x中", "中");
            out(i, j, "中é👨‍👩‍👧‍👦x中".lenslice(j, 1));
            out(strings.find("abc", ""), strings.rfind("abc", ""), strings.split("a,,b", ","));
        }"#,
        "2 -1 -1 1\nfalse true false true\ne\u{301}x a-b-c 1 2\n0 4 中\n0 3 [\"a\", \"\", \"b\"]\n");
}

#[test]
fn decorator_private_and_subclass_control_who_can_call_methods() {
    let program = |body: &str| format!(r#"takepkg std.Decorator;
        family Account{{
            func init(){{ me.balance = 0; }}
            @Decorator.private
            func _audit(string action) => "audit " + action;
            @Decorator.subclass
            func _limit() => 100;
            func deposit(int amount){{ me.balance = me.balance + amount; ret me._audit("deposit"); }}
        }}
        family Savings(Account){{
            func limit_twice() => me._limit() * 2;
            func sneak() => me._audit("sneak");
        }}
        family Other{{
            func peek(Account a) => a._limit();
        }}
        func m{{ {body} }}"#);
    executes(&program("a = Account(); out(a.deposit(5), a.balance); s = Savings(); out(s.limit_twice(), s.deposit(1));"),
        "audit deposit 5\n200 audit deposit\n");
    for (body, message) in [
        ("Account()._audit(\"x\");", "_audit is private to Account"),
        ("f = Account()._audit;", "_audit is private to Account"),
        ("Savings().sneak();", "_audit is private to Account"),
        ("Account()._limit();", "_limit is only available to Account and families inheriting from it"),
        ("Other().peek(Account());", "_limit is only available to Account"),
    ] {
        runtime_error(&program(body), message);
    }
}

#[test]
fn decorators_need_the_package_and_a_known_marker() {
    executes("takepkg std.Decorator = D;\nfamily Box{\n @D.private func _x() => 1;\n func x() => me._x();\n}\nfunc m{ out(Box().x()); }", "1\n");
    for (source, message) in [
        ("family Box{\n @Decorator.private\n func _x() => 1;\n}", "needs `takepkg std.Decorator;`"),
        ("takepkg std.Decorator;\nfamily Box{\n @Decorator.secret\n func _x() => 1;\n}", "unknown decorator @Decorator.secret"),
        ("takepkg std.Decorator;\nfamily Box{\n @Decorator.static\n func x() => 1;\n}", "@Decorator.static is not implemented yet"),
        ("takepkg std.Decorator;\nfamily Box{\n @Decorator.private @Decorator.subclass\n func x() => 1;\n}", "cannot be both private and subclass"),
        ("takepkg std.Decorator;\nfamily Box{\n @Decorator.private\n func init(){}\n}", "constructors are always public"),
        ("takepkg std.Decorator;\n@Decorator.private\nfunc x() => 1;", "only supported on family methods"),
        ("takepkg std.Decorator;\nfamily Box{\n @Decorator.private\n}", "must be followed by a func"),
        ("takepkg std.Decorator;\nfamily Box{\n @private\n func x() => 1;\n}", "invalid decorator"),
    ] {
        let error = marslang::compile(source).expect_err(source);
        assert!(error.contains(message), "{source}: {error}");
    }
    runtime_error("takepkg std.containers;\nfunc m{ containers.deque()._slot(0); }", "_slot is private to deque");
}
