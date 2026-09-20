# rs-0.9.0 — Installation and the user package directory

Marslang installs without a Rust toolchain, and packages installed for a user
are importable from every program that user writes.

## Install

`install.ps1` and `install.sh` download the archive for the platform they run
on, verify it against the release's `SHA256SUMS`, put `marslang` in a per-user
directory, and add that directory to `PATH`. Nothing is compiled and nothing
outside the home directory is written, so neither script needs administrator or
root rights.

```powershell
irm https://marslang.kevin-z.com/install.ps1 | iex
```

```sh
curl -fsSL https://marslang.kevin-z.com/install.sh | sh
```

Both take the release to install (`-Version` / `--version`), the package sets to
install (`-Use` / `--use`, from `std` and `ext`), the install directory, and a
switch to leave `PATH` alone. `std` is built into the interpreter; `ext` is
accepted, reports that extension packages are not published yet, and installs
nothing.

[.github/workflows/release.yml](../../.github/workflows/release.yml) builds the
archives when an `rs-*` tag is pushed: Windows x86-64, Linux x86-64 and
AArch64, and macOS on both architectures. It runs the tests for every target it
can execute, writes `SHA256SUMS` across the archives, and uploads them to that
tag's release.

## The user package directory

A package that the program's own directory does not provide is now looked for in
the user's package directory: `MARSLANG_PKGS`, or `marslang_pkgs` in the home
directory. `marslang pkgs` prints the directory in use.

```mars
takepkg greet;            // a program file, or a package installed for you
```

The program's directory is searched first, so a file beside the program always
wins over an installed package of the same name, and installing something later
cannot change a program that already works. The two never mix inside one
package: a package found in the user directory runs its parents' `init.mars`
files from there as well. A failed import lists every file that was looked for,
in both directories.

## Validation

`cargo test` passes 112 tests (13 unit, 98 execution, 1 memory) on Windows Rust
and on WSL-native Rust.
