# Marslang overview and restart plan

Reviewed 2026-09-14. This is a review and proposed plan, not a language specification or an implementation change.

## Repository state

- Private repository: `Unknownuserfrommars/marslang`.
- `main` (`c6ccce6`) contains only the initial README. It is still GitHub's default branch.
- `py1` (`f9608a1`, 2026-03-23) contains the Python implementation, v0.4.1.
- `rs1` (`8896d53`, 2026-04-26) contains the Rust implementation, rs-0.1.3, including merged PR #10.
- The two remaining remote `codex/...` branches are ancestors of merged Rust work. No open issues or PRs, and no GitHub releases, were returned during this review.
- The local checkout is on `rs1`. Workspace `../CLAUDE.md` applies as agent instructions; neither implementation branch contains its own CLAUDE.md or AGENTS.md.

## Language identity

Marslang uses `.mrs` files, semicolon-terminated statements, braces, an `m` entry function, `func`, and `family` classes with `init` constructors and `me` for the current instance. Parameters use forms such as `float r;`; variables can have annotations such as `nums (array[int])`.

The Python branch documents `hot` as compile-time substitution, `fixed` as immutable binding, and `cold` as explicitly non-hot. Rust currently lowers both hot and fixed variables to JavaScript `const`, so those semantics are not equivalent. Both retain the `fasle` spelling as a compatibility alias.

The Python language reference is useful design history, but it should not be assumed to describe either implementation perfectly.

## Architecture and coverage

| Area | Python v0.4.1 | Rust rs-0.1.3 |
| --- | --- | --- |
| Front end | Lexer with positions, recursive-descent parser, expression precedence, dataclass AST | Line preprocessing and handwritten string parsing; separate lexer used by `lex` command |
| Execution | AST serialized into Python dictionaries; Python-hosted VM walks the structure | JavaScript emitter with a small embedded runtime; Node runs output |
| Variables and types | Type checks at several runtime boundaries; hot substitution and function inlining | Type annotations stored but not enforced; hot/fixed become `const` |
| Functions and families | Functions, methods, inheritance, hot functions | Block/expression functions, JS classes, constructor conversion |
| Control flow | if/elif/else, repeat, for/also, match/range, run/handle/then | if/elif/else and repeat; other constructs have no dedicated AST/lowering |
| Collections | arr/set/pair wrappers with Marslang methods and type restrictions | a/s/p produce native JS arrays, Set objects, and pair objects |
| Imports | Python `importlib` module loading | Relative CommonJS `require`; no recursive `.mrs` module compilation/export system |
| CLI | Compile, output selection, run, verbose diagnostics, Unix/Windows wrappers | Compile, lex, source-replay REPL |
| Existing tests | 10, including actual execution and CLI cases | 7, checking generated JS substrings |

Rust source map:

- `src/lib.rs`: compilation API, version constant, unit tests.
- `src/parser.rs`: actual parsing path. `compile_source_to_js` calls it directly.
- `src/ast.rs`: program, statement, declaration, and expression types.
- `src/eval.rs`: JS emitter, runtime prelude, limited scope tracking, entrypoint invocation.
- `src/main.rs`: command dispatch, files, REPL.
- `src/lexer.rs`: standalone token listing, not integrated into compilation.
- `src/grammar.pest`: unused grammar; Cargo has no Pest dependency.
- `stdlib.mrs`: min/max functions; not automatically loaded.

The Python pipeline is `compiler.py` → `lexer.py` → `parser.py` → `codegen.py` → generated Python runner → `runtime.py`. Its VM is a structured interpreter, not a bytecode engine. Generated runners still import the Marslang package.

## Verified behavior

Both existing suites passed: **Rust 7/7; Python 10/10**. Additional probes demonstrated these gaps in Rust:

| Probe | Observed result |
| --- | --- |
| Bundled `hello.mrs` | Compiles, then fails with `TypeError: nums.iget is not a function` |
| `x = 1;` followed on another line by `x = 2;` | Emits two `let x` declarations; Node rejects duplicate binding |
| `x == 1;` as a statement after declaring x | Misclassified as a declaration; invalid JS |
| `out("https://example.com");` | Comment preprocessing truncates the string; invalid JS |
| `func m{ out(1); }` on one line | Compile error, although equivalent multiline code is accepted |
| Method returning `me.r * 2` | Emits unreplaced `me` in compound expression; ReferenceError |
| User-defined `out(int x;) => x + 10` called inside `slout(out(1))` | Calls builtin out; prints `1` and `undefined` instead of `11` |
| Unterminated `/*` comment | Accepted as an empty program |
| REPL lines `out("first");`, then `out("second");` | Prints first twice, because all source is re-executed each submission |

Root causes: declaration detection checks for any `=` before checking assignment operators; expressions often become raw text inside `Expr::Ident`; comment stripping ignores strings; function names are not added to emitter scopes. The `Binary` AST variant exists but the parser never constructs it.

The REPL preserves source, not runtime state. Invalid submissions stay in its buffer, and incomplete blocks produce errors while being entered. This behavior is documented in part, but limits interactive use.

The README promises `compiler.exe`, but Cargo's package is `marslang` and there is no explicit `[[bin]]` declaration. The actual build produced **marslang.exe**.

Python is a broader reference, but probes also found:

- `func m{ fixed x = 1; x = 2; out(x); }` prints `2`: redeclaration bypasses immutable-binding enforcement.
- `func m{ nums (array[int]) = arr(1); nums[0] = "oops"; out(nums[0]); }` prints `oops`: indexed writes bypass the array element check.
- `func m{ x = 1; if (true) { x = 2; } out(x); }` prints `1`: bare `=` creates a block-local declaration. The desired declaration/assignment rule needs to be explicit before porting behavior.

## Proposed next steps

### 1. Establish the language contract and development home

Proposed direction: continue Rust with the JS backend while using Python's docs, examples, and tests as reference material. Confirm intended semantics rather than copying all Python behavior.

Write a small specification covering declaration versus reassignment, shadowing, fixed binding versus deep immutability, hot evaluation, type checking, collection names/methods, boolean operators, and the canonical cleanup keyword (`then` versus the Rust README's `now_do`). Decide whether a/s/p are preferred spellings or aliases of arr/set/pair.

Make the real implementation discoverable from main, or select the intended default branch as a separate repository-maintenance action. Reconcile the CLI binary name and README parser description. No remote settings were changed in this review.

Acceptance: a newcomer can find the working branch and build/run the documented example using accurate instructions.

### 2. Deliver a small, dependable core in separate milestones

Follow-up decision, 2026-09-15: keep Rust with the JavaScript backend as the bootstrap implementation, and work toward a compiler written in Marslang later. Complete these steps separately rather than treating front-end replacement, name resolution, and runtime coverage as one change.

1. Define core declaration/reassignment and scope rules, operator precedence, and fixed-binding behavior. Specify numeric behavior, equality, and truthiness instead of inheriting JavaScript behavior accidentally. Mark unresolved features as deferred.
2. Add executable regression cases and basic CI. Assert program output, exit status, and diagnostics, rather than generated JavaScript substrings alone.
3. Replace the front end with a single lexer/parser and structured expressions with source spans. The existing lexer also needs escaped strings, comments, operators, and lexical errors; connecting it as-is is insufficient.
4. Add consistent name resolution and the collection operations required by the bundled example.

Start with a small end-to-end corpus: example execution, repeated assignment, comparisons, strings containing comments, compound `me` expressions, and shadowing. Tests should execute generated JS and assert output or diagnostics.

Replace line heuristics with one active token/grammar parser. A token-based recursive-descent parser with precedence handling would fit the existing small AST, but Pest is also viable if deliberately adopted. Preserve source spans and report malformed or unsupported input before emission.

Add name resolution to distinguish declarations from assignments and resolve functions, builtins, and family constructors consistently. Implement the collection API needed by the bundled example.

Acceptance: the bundled example prints `3` and `78.53975`; reassignment works according to the chosen rules; malformed source fails with source locations; all observed regressions have execution coverage.

### 3. Restore intentional feature coverage

Port for/also, match/range, and run/handle/then in small steps. Add runtime collection and scalar checks, then hot evaluation/inlining according to the specification. Define module loading and exports before extending takepkg.

Use shared language examples with explicit expected results. Where Python and Rust disagree, decide expected semantics; do not treat Python output as automatically correct.

### 4. Improve interactive use and distribution

Support multiline input and recovery from invalid submissions in the REPL; choose whether it should retain a persistent runtime or remain explicitly a replay tool. Add accurate help/version/run commands, Linux and Windows CI, and release builds. Keep Node dependency clear while targeting JS.

Consider a native or bytecode backend after the language behavior and conformance corpus are stable.

### 5. Bootstrap a compiler written in Marslang (self-hosting)

Self-hosting means implementing the compiler in the language it compiles. It does not require a native backend: a Marslang compiler can produce JavaScript and run under Node throughout this process.

Start by documenting the basic syntax and type rules in a small language specification, backed by executable examples. A specification or typedef file alone cannot make the language self-hosting; the bootstrap implementation must parse and implement those constructs first.

Before starting the compiler port, provide the subset needed to represent and process source code:

- Strings with indexing/slicing and a defined character/offset model; arrays and key/value maps.
- Records or equivalent family-based data structures, plus a way to distinguish token and AST node kinds. Type aliases (`typedef`-like declarations) are useful, but their spelling is not decided here; tagged variants can be added if justified by the compiler's needs.
- Functions, recursion, iteration, mutable local state, and predictable scope/type behavior.
- Modules and exports, file reading/writing, command-line arguments, and error reporting through a small documented host runtime.

Port incrementally: token/source-location types and lexer first, then AST and parser, name resolution, and JavaScript emission. Run the same conformance corpus against the Rust and Marslang implementations, using specified results as the authority. Keep the Rust bootstrap usable until the self-hosted compiler passes those tests.

Bootstrap stages:

1. Stage 0: the Rust compiler compiles the Marslang compiler source to JavaScript (stage 1).
2. Stage 1: Node runs that generated compiler to compile the same Marslang compiler source again (stage 2).
3. Stage 2: run the conformance corpus and compile the compiler again (stage 3). Require equivalent behavior; once emission is deterministic, compare stage 2 and stage 3 output byte-for-byte as an additional check.

Acceptance: a documented clean bootstrap command builds these stages, stage 2 passes the core corpus, and subsequent bootstrapping is reproducible. Self-hosting is a later milestone, not a prerequisite for fixing the current compiler. Full hot-function evaluation/inlining, a native backend, and broad Python feature parity need not precede it.

## Current development location (2026-09-15)

The follow-up moves the working repository to `G:\Marslang`, accessible in WSL as `/mnt/g/Marslang`, with a compatibility link at `/home/kevib/vibepanel_projects/marslang`. The original review environment below is historical. Use Windows Cargo/Node on this Windows drive, or separately installed WSL-native tools; keep build artifacts separate if switching toolchains.

## Review environment and reproducibility

The repository was cloned into its own workspace subdirectory using the existing Windows gh authentication. No login settings were changed.

WSL has Python 3 but no native Cargo/Node or pytest. Existing Windows tools were sufficient. Windows Cargo's incremental locks failed on the WSL UNC filesystem; putting build artifacts on Windows and disabling incremental compilation worked:

```bash
/mnt/c/Users/Kevin/.cargo/bin/cargo.exe test --config build.incremental=false --target-dir C:/Users/Kevin/AppData/Local/Temp/marslang-review-target
```

The Python branch was exported to `/tmp/marslang-py1-review`, where `python.exe -m pytest -q` passed. Rust execution probes are in `/tmp/marslang-rs-review`; they used the built marslang.exe and node.exe. These temporary paths are review aids, not committed test infrastructure.

Only this review document was added to the checkout. No compiler fixes, commits, pushes, or remote repository changes were made.
