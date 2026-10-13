// std.terminal, and the way std.format measures coloured text.
//
// Colours are decided from the real standard output and environment, which
// differ from machine to machine, so most tests switch them with
// terminal.set_colors. The few that read the environment run the interpreter
// as a child process, whose output is a pipe, not a terminal.

use std::process::{Command, Stdio};

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

/// A program that imports std.terminal and std.format, with colours forced on or off.
fn with_colors(on: bool, body: &str) -> String {
    format!("takepkg std.terminal;\ntakepkg std.format;\nfunc m{{\n    terminal.set_colors({on});\n{body}\n}}")
}

#[test]
fn styles_wrap_text_in_sgr_codes_and_a_reset() {
    executes(&with_colors(true, r#"
        out(terminal.bold("x"));
        out(terminal.dim("x"));
        out(terminal.italic("x"));
        out(terminal.underline("x"));
    "#), "\x1b[1mx\x1b[0m\n\x1b[2mx\x1b[0m\n\x1b[3mx\x1b[0m\n\x1b[4mx\x1b[0m\n");
}

#[test]
fn each_colour_has_its_code() {
    executes(&with_colors(true, r#"
        out(terminal.red("x"), terminal.green("x"), terminal.yellow("x"), terminal.blue("x"));
        out(terminal.magenta("x"), terminal.cyan("x"), terminal.gray("x"));
    "#),
        "\x1b[31mx\x1b[0m \x1b[32mx\x1b[0m \x1b[33mx\x1b[0m \x1b[34mx\x1b[0m\n\
         \x1b[35mx\x1b[0m \x1b[36mx\x1b[0m \x1b[90mx\x1b[0m\n");
}

#[test]
fn color_by_name_is_the_same_as_the_function() {
    executes(&with_colors(true, r#"
        for (name, arr("red", "green", "yellow", "blue", "magenta", "cyan", "gray")){
            out(terminal.color("t", name));
        }
        out(terminal.color("t", "cyan") == terminal.cyan("t"));
    "#),
        "\x1b[31mt\x1b[0m\n\x1b[32mt\x1b[0m\n\x1b[33mt\x1b[0m\n\x1b[34mt\x1b[0m\n\
         \x1b[35mt\x1b[0m\n\x1b[36mt\x1b[0m\n\x1b[90mt\x1b[0m\ntrue\n");
}

#[test]
fn rgb_is_a_24_bit_colour() {
    executes(&with_colors(true, r#"
        out(terminal.rgb("o", 255, 165, 0));
        out(terminal.rgb("k", 0, 0, 0));
    "#), "\x1b[38;2;255;165;0mo\x1b[0m\n\x1b[38;2;0;0;0mk\x1b[0m\n");
}

#[test]
fn anything_can_be_styled_as_text() {
    executes(&with_colors(true, r#"
        out(terminal.green(42));
        out(terminal.bold(arr(1, 2)));
        out(terminal.red(null));
    "#), "\x1b[32m42\x1b[0m\n\x1b[1m[1, 2]\x1b[0m\n\x1b[31mnull\x1b[0m\n");
}

#[test]
fn styles_nest() {
    executes(&with_colors(true, r#"
        out(terminal.bold(terminal.red("x")));
        out(terminal.underline(terminal.rgb("x", 1, 2, 3)));
    "#), "\x1b[1m\x1b[31mx\x1b[0m\x1b[0m\n\x1b[4m\x1b[38;2;1;2;3mx\x1b[0m\x1b[0m\n");
}

#[test]
fn a_style_resumes_after_one_inside_it_ends() {
    // The inner reset would end the outer red too; red starts again after it.
    executes(&with_colors(true, r#"
        out(terminal.red("a" + terminal.bold("b") + "c"));
    "#), "\x1b[31ma\x1b[1mb\x1b[0m\x1b[31mc\x1b[0m\n");
}

#[test]
fn colours_off_give_plain_text() {
    executes(&with_colors(false, r#"
        out(terminal.bold("a"), terminal.dim("b"), terminal.italic("c"), terminal.underline("d"));
        out(terminal.red("e"), terminal.green("f"), terminal.yellow("g"), terminal.blue("h"));
        out(terminal.magenta("i"), terminal.cyan("j"), terminal.gray("k"));
        out(terminal.color("l", "red"), terminal.rgb("m", 1, 2, 3));
        out(terminal.bold(terminal.red(7)));
        out(terminal.colors());
    "#), "a b c d\ne f g h\ni j k\nl m\n7\nfalse\n");
}

#[test]
fn set_colors_overrides_and_colors_reports() {
    executes(&with_colors(true, r#"
        out(terminal.colors());
        terminal.set_colors(false);
        out(terminal.colors(), terminal.red("x"));
        out(terminal.set_colors(true));
        out(terminal.colors());
    "#), "true\nfalse x\nnull\ntrue\n");
}

#[test]
fn strip_removes_escape_sequences() {
    executes(&with_colors(true, r#"
        styled = terminal.bold(terminal.red("Done")) + " " + terminal.rgb("ok", 1, 2, 3);
        out(terminal.strip(styled));
        out(terminal.strip(terminal.underline("a" + terminal.cyan("b") + "c")) == "abc");
        out(terminal.strip(chr(27) + "[2J" + chr(27) + "[H" + chr(27) + "[?25lhi" + chr(27) + "[1;5;7m"));
        out(terminal.strip("plain"));
        out(terminal.strip(""));
        out(terminal.strip(12));
    "#), "Done ok\ntrue\nhi\nplain\n\n12\n");
}

#[test]
fn strip_leaves_what_is_not_a_whole_escape_sequence() {
    executes(&with_colors(true, r#"
        out(terminal.strip("a" + chr(27) + "b[31m"));
        out(terminal.strip("tail" + chr(27) + "[31"));
        out(terminal.strip("[31m not an escape"));
    "#), "a\x1bb[31m\ntail\x1b[31\n[31m not an escape\n");
}

#[test]
fn strip_undoes_colours_for_any_text() {
    executes(&with_colors(true, r#"
        for (text, arr("", "x", "two words", "你好", "tab\there", "a\nb")){
            out(terminal.strip(terminal.bold(terminal.green(text))) == text);
        }
    "#), "true\ntrue\ntrue\ntrue\ntrue\ntrue\n");
}

#[test]
fn clear_prints_the_sequence_only_when_colours_are_on() {
    executes(&with_colors(true, "out(terminal.clear());"), "\x1b[2J\x1b[Hnull\n");
    executes(&with_colors(false, "out(terminal.clear());"), "null\n");
}

#[test]
fn progress_fills_from_0_to_100_percent() {
    executes(&with_colors(false, r#"
        out(terminal.progress(0, 10));
        out(terminal.progress(0.0, 10));
        out(terminal.progress(0.5, 20));
        out(terminal.progress(1, 10));
        out(terminal.progress(1.0, 10));
        out(terminal.progress(0.1, 10));
    "#),
        "[----------]   0%\n[----------]   0%\n[##########----------]  50%\n\
         [##########] 100%\n[##########] 100%\n[#---------]  10%\n");
}

#[test]
fn progress_with_odd_widths_rounds_the_bar_down() {
    executes(&with_colors(false, r#"
        out(terminal.progress(0.5, 7));
        out(terminal.progress(0.5, 1));
        out(terminal.progress(1, 1));
        out(terminal.progress(0, 1));
        out(terminal.progress(0.99, 3));
        out(terminal.progress(0.34, 3));
        out(terminal.progress(0.29, 100).len());
    "#), "[###----]  50%\n[-]  50%\n[#] 100%\n[-]   0%\n[##-]  99%\n[#--]  34%\n107\n");
}

#[test]
fn progress_percent_is_right_aligned_in_four_characters() {
    executes(&with_colors(false, r#"
        for (f, arr(0, 0.05, 0.5, 1)){
            line = terminal.progress(f, 4);
            out(line.slice(line.len() - 4, line.len()) + "|");
        }
    "#), "  0%|\n  5%|\n 50%|\n100%|\n");
}

#[test]
fn progress_is_never_coloured_and_never_full_before_the_end() {
    executes(&with_colors(true, r#"
        out(terminal.progress(0.999, 10));
        out(terminal.progress(0.29, 10));
    "#), "[#########-]  99%\n[##--------]  29%\n");
}

#[test]
fn progress_rejects_fractions_outside_0_to_1() {
    for fraction in ["-0.1", "1.01", "2", "-1", "float(\"nan\")", "float(\"inf\")"] {
        runtime_error(&format!("takepkg std.terminal;\nfunc m{{ terminal.progress({fraction}, 10); }}"), "RangeError");
    }
    runtime_error("takepkg std.terminal;\nfunc m{ terminal.progress(1.5, 10); }", "fraction from 0 to 1, got 1.5");
}

#[test]
fn progress_rejects_a_width_below_1() {
    runtime_error("takepkg std.terminal;\nfunc m{ terminal.progress(0.5, 0); }", "width of at least 1, got 0");
    runtime_error("takepkg std.terminal;\nfunc m{ terminal.progress(0.5, -3); }", "RangeError");
}

#[test]
fn an_unknown_colour_name_lists_the_known_ones() {
    runtime_error(
        "takepkg std.terminal;\nfunc m{ terminal.color(\"x\", \"purple\"); }",
        "does not know the colour \"purple\"; the colours are red, green, yellow, blue, magenta, cyan, gray",
    );
    // Checked whether or not colours are on.
    runtime_error(&with_colors(false, "terminal.color(\"x\", \"Red\");"), "RangeError");
    runtime_error(&with_colors(true, "terminal.color(\"x\", \"\");"), "RangeError");
}

#[test]
fn rgb_values_must_be_from_0_to_255() {
    for call in ["rgb(\"x\", 256, 0, 0)", "rgb(\"x\", 0, -1, 0)", "rgb(\"x\", 0, 0, 1000)"] {
        runtime_error(&with_colors(true, &format!("terminal.{call};")), "RangeError");
        runtime_error(&with_colors(false, &format!("terminal.{call};")), "RangeError");
    }
    runtime_error(&with_colors(true, "terminal.rgb(\"x\", 300, 0, 0);"), "from 0 to 255, got 300");
}

#[test]
fn the_terminal_has_a_positive_size() {
    executes(&format!("takepkg std.types;\n{}", with_colors(false, r#"
        c = terminal.columns();
        r = terminal.rows();
        out(c > 0, r > 0);
        out(types.kind(c), types.kind(r));
        out(types.kind(terminal.is_terminal()));
    "#)), "true true\nint int\nbool\n");
}

// ----- std.format measures only the visible text -----

#[test]
fn format_width_ignores_escape_sequences() {
    executes(&with_colors(true, r#"
        out(format.width(terminal.red("Ada")));
        out(format.width(terminal.bold(terminal.rgb("你好", 1, 2, 3))));
        out(format.width(terminal.red("Ada")) == format.width("Ada"));
        out(format.width(chr(27) + "[2J" + chr(27) + "[H"));
        out(format.width(chr(27) + "[31"));
    "#), "3\n4\ntrue\n0\n4\n");
}

#[test]
fn format_left_right_and_center_pad_coloured_text_to_its_visible_width() {
    executes(&with_colors(true, r#"
        out(format.left(terminal.red("ab"), 5) + "|");
        out(format.right(terminal.red("ab"), 5) + "|");
        out(format.center(terminal.red("ab"), 6) + "|");
    "#),
        "\x1b[31mab\x1b[0m   |\n   \x1b[31mab\x1b[0m|\n  \x1b[31mab\x1b[0m  |\n");
}

#[test]
fn format_table_lines_up_coloured_cells() {
    executes(&with_colors(true, r#"
        out(format.table(arr(
            arr(terminal.bold("Name"), terminal.bold("Age")),
            arr("Ada", 36),
            arr(terminal.red("Grace"), 85),
            arr(terminal.rgb("你好", 1, 2, 3), 7)
        )));
    "#),
        "\x1b[1mName\x1b[0m   \x1b[1mAge\x1b[0m\n\
         Ada     36\n\
         \x1b[31mGrace\x1b[0m   85\n\
         \x1b[38;2;1;2;3m你好\x1b[0m     7\n");
}

#[test]
fn a_coloured_table_is_the_plain_one_with_codes_added() {
    executes(&with_colors(true, r#"
        plain = format.table(arr(arr("Name", "City"), arr("Ada", "London"), arr("Grace", "New York")));
        coloured = format.table(arr(
            arr(terminal.bold("Name"), terminal.bold("City")),
            arr(terminal.green("Ada"), "London"),
            arr(terminal.red("Grace"), terminal.cyan("New York"))
        ));
        out(terminal.strip(coloured) == plain);
    "#), "true\n");
}

// ----- the environment, seen from a program run as a child process -----

/// Run a program with the interpreter, with these variables set and the
/// colour and size ones otherwise removed; its output is a pipe.
fn run_program(source: &str, env: &[(&str, &str)]) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let file = std::env::temp_dir().join(format!("marslang-terminal-{}-{}.mars", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&file, source).expect("write the program");
    let mut command = Command::new(env!("CARGO_BIN_EXE_marslang"));
    command.arg(&file).stdin(Stdio::null());
    for name in ["NO_COLOR", "FORCE_COLOR", "COLUMNS", "LINES"] { command.env_remove(name); }
    for (name, value) in env { command.env(name, value); }
    let output = command.output().expect("run marslang");
    let _ = std::fs::remove_file(&file);
    assert!(output.status.success(), "marslang failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("utf-8 output").replace("\r\n", "\n")
}

const SHOW_COLORS: &str = "takepkg std.terminal;\nfunc m{ out(terminal.colors(), terminal.is_terminal(), terminal.bold(\"x\")); }";
const SHOW_SIZE: &str = "takepkg std.terminal;\nfunc m{ out(terminal.columns(), terminal.rows()); }";

#[test]
fn piped_output_has_no_colours() {
    assert_eq!(run_program(SHOW_COLORS, &[]), "false false x\n");
}

#[test]
fn no_color_keeps_them_off_and_force_color_turns_them_on() {
    assert_eq!(run_program(SHOW_COLORS, &[("NO_COLOR", "1")]), "false false x\n");
    assert_eq!(run_program(SHOW_COLORS, &[("FORCE_COLOR", "1")]), "true false \x1b[1mx\x1b[0m\n");
    // Asked for in so many words, it wins over NO_COLOR.
    assert_eq!(run_program(SHOW_COLORS, &[("FORCE_COLOR", "1"), ("NO_COLOR", "1")]), "true false \x1b[1mx\x1b[0m\n");
}

#[test]
fn empty_environment_variables_count_as_unset() {
    assert_eq!(run_program(SHOW_COLORS, &[("FORCE_COLOR", "")]), "false false x\n");
    assert_eq!(run_program(SHOW_COLORS, &[("NO_COLOR", ""), ("FORCE_COLOR", "")]), "false false x\n");
}

#[test]
fn set_colors_beats_the_environment() {
    let program = "takepkg std.terminal;\nfunc m{ terminal.set_colors(false); out(terminal.colors(), terminal.bold(\"x\")); }";
    assert_eq!(run_program(program, &[("FORCE_COLOR", "1")]), "false x\n");
}

#[test]
fn size_falls_back_to_columns_and_lines_then_80_by_24() {
    assert_eq!(run_program(SHOW_SIZE, &[]), "80 24\n");
    assert_eq!(run_program(SHOW_SIZE, &[("COLUMNS", "132"), ("LINES", "50")]), "132 50\n");
    // Each one alone; nonsense is ignored.
    assert_eq!(run_program(SHOW_SIZE, &[("COLUMNS", "100")]), "100 24\n");
    assert_eq!(run_program(SHOW_SIZE, &[("COLUMNS", "wide"), ("LINES", "0")]), "80 24\n");
    assert_eq!(run_program(SHOW_SIZE, &[("COLUMNS", "-5"), ("LINES", " 30 ")]), "80 30\n");
}
