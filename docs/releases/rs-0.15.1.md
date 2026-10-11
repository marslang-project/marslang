# rs-0.15.1 — Security fixes

Fixes found by a security review of the interpreter, its installer, and its
release process. Everyone on rs-0.15.0 should update: writing a file could make
a private one readable by every user of the machine.

## file.write no longer exposes private files

`file.write` replaces a file all at once, through a temporary file renamed over
the old one. That temporary file took default permissions, so on Linux and
macOS a private file such as a `600` `.env` full of tokens became `644`,
readable by everyone, after a single write. `json.save`, `env.write`,
`toml.write`, `ini.write`, and `csv.write` all write through it.

The replacement now takes the old file's permissions, and its owner where the
system allows. Writing to a link updates the file it points to and leaves the
link in place, where before the link was replaced by a plain file.

## Temporary files cannot be redirected

The temporary file had a guessable name and was opened in a way that follows
links, so another user of the same machine could plant a link where it would
go and make your program overwrite a different file. It is now created
exclusively, under an unguessable name, and never reuses anything already
there. `file.with_temp_dir` folders are unguessable too, and on Linux and macOS
only you can open them.

## Deeply nested files no longer crash the interpreter

A file with tens of thousands of nested parentheses overflowed the stack in
`marslang check` and `marslang symbols`, which the VS Code extension runs on
every file you open. One expression may now nest 1,000 levels deep and chain
10,000 operators in a row; beyond that, compiling stops with an error that says
so, and compiling runs on the interpreter's large stack. No real program comes
near either limit.

## Safer installs

- `install.sh` runs only once it has downloaded completely, so a connection
  dropped in the middle of `curl ... | sh` runs nothing instead of half of the
  steps. It also refuses any download that is not HTTPS.
- Every archive now carries a signed build attestation, proving it was built by
  the release workflow from this repository and not changed since:

  ```bash
  gh attestation verify marslang-rs-0.15.1-x86_64-unknown-linux-musl.tar.gz --repo marslang-project/marslang
  ```

- The release workflow's build jobs, which run third-party build scripts, can
  no longer change the repository or its releases; only the final publishing
  job can.

## Validation

`cargo test` passes 154 tests on Linux (14 unit, 139 execution, 1 memory),
including new ones for permissions, links, private temporary folders, and the
nesting limits, and 152 on Windows, where the Unix permission tests do not
apply. Every push runs them on Windows, macOS, and Linux on x86_64 and ARM64,
and runs the static Linux binary inside Alpine, Debian 11, Ubuntu 20.04, Rocky
Linux 8, and Amazon Linux 2.
