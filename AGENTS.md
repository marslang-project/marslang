# Marslang

## Workspace

- Canonical checkout: `G:\Marslang` on Windows, `/mnt/g/Marslang` in WSL.
- `/home/kevib/vibepanel_projects/marslang` is a compatibility symlink to this checkout.
- This directory is an independent Git repository. Preserve existing uncommitted work.

## Development direction

Read `docs/PROJECT_REVIEW.md` for the review and staged roadmap. Decision 2026-09-19: the JavaScript backend is deleted; Marslang is an interpreted language run by the Rust tree-walking interpreter (`src/interp.rs`). Stabilize the core with execution tests, then work toward tooling written in Marslang. Proposed language syntax in the roadmap is not implemented syntax. Do not reintroduce a JavaScript/Node dependency.

## Validation

Only Rust is required; Node is no longer used. From Windows:

```bash
cargo test
cargo run -- hello.mars
```

From WSL, use the Windows toolchain (`/mnt/c/Users/Kevin/.cargo/bin/cargo.exe test`) or the
WSL-native one with a separate target directory
(`/home/kevib/.cargo/bin/cargo test --target-dir target/wsl`).

The bundled example prints `3` and `78.53975`. `tests/execution.rs` runs each program in the
interpreter and asserts its output or runtime error; run the full suite, not only unit tests.
