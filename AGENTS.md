# Marslang

## Workspace

- Canonical checkout: `G:\Marslang` on Windows, `/mnt/g/Marslang` in WSL.
- `/home/kevib/vibepanel_projects/marslang` is a compatibility symlink to this checkout.
- This directory is an independent Git repository. Preserve existing uncommitted work.

## Development direction

Read `docs/PROJECT_REVIEW.md` for the review and staged roadmap. Continue the Rust-to-JavaScript bootstrap compiler, stabilize the core with execution tests, and then work toward a compiler written in Marslang. Proposed language syntax in the roadmap is not implemented syntax.

## Validation

Windows Cargo and Node can operate on this checkout without the old WSL UNC working-directory issue. From WSL, the available Windows tools are:

```bash
/mnt/c/Users/Kevin/.cargo/bin/cargo.exe test
/mnt/c/Users/Kevin/.cargo/bin/cargo.exe run -- compile hello.mrs -o hello.js
'/mnt/c/Program Files/nodejs/node.exe' hello.js
```

The bundled example is covered by a Node execution test and prints `3` and `78.53975`. Run the full Rust suite, including `tests/execution.rs`; generated-text assertions alone do not establish correctness. Set `MARSLANG_NODE` to the Node executable if it is not on PATH. Use a separate target directory if switching between Windows and WSL-native Rust toolchains.
