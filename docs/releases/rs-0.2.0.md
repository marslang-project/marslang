# rs-0.2.0

Rust bootstrap compiler release, 2026-09-16. Backend: JavaScript running on Node.

## Breaking syntax change

Function and method declarations now use comma-separated parameters with no
trailing semicolon:

```mars
func add(int a, int b) => a + b;
```

Migrate `func add(int a; int b;)` to the form above. Old semicolon signatures
are rejected with a migration diagnostic. Commas inside bracketed parameter
types remain part of the type. Trailing commas are not accepted. Zero-parameter
forms and statement-ending semicolons are unchanged.

## Included

- Updated parser, reference grammar, bundled examples, and syntax fixtures.
- Node execution tests plus malformed-parameter and nested-type parser checks.
- Language contract derived from the original prompts and later design decisions.
- Feature inventory, project review, and staged self-hosting roadmap.

## Validation and limitations

`cargo test`: 15 passing tests, with 8 known regression cases explicitly ignored.
Node must be on PATH, or selected through `MARSLANG_NODE`.

The broader language additions are documented targets, not implemented features:
decorators (`std.Decorator`), maps/deque, additional loops, library packages,
wildcard/relative module handling, and copy/immutability semantics remain pending.

Existing core gaps remain, including the bundled example's collection runtime
failure, hot substitution, compound `me` expressions, and line-oriented parsing.
See [LANGUAGE_CORE.md](../LANGUAGE_CORE.md) and
[LIMITATIONS.md](../LIMITATIONS.md) before relying on unsupported syntax.
