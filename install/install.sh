#!/bin/sh
# Install the Marslang interpreter on Linux or macOS.
#
#   curl -fsSL https://marslang.kevin-z.com/install.sh | sh
#   curl -fsSL https://marslang.kevin-z.com/install.sh | sh -s -- --version rs-0.9.0 --use std,ext
#
# Downloads a released binary, verifies its checksum, and installs it under the
# user's home directory. Nothing is compiled, so no Rust toolchain is needed,
# and nothing outside $HOME is written, so no root access is needed.
set -eu

repo="marslang-project/marslang"
version="latest"
use="std"
install_dir="${MARSLANG_HOME:-$HOME/.marslang}"
pkg_dir="${MARSLANG_PKGS:-$HOME/marslang_pkgs}"
edit_path=yes

usage() {
    cat >&2 <<'USAGE'
Usage: install.sh [options]
  --version <tag>   release to install, such as rs-0.9.0 (default: latest)
  --use <sets>      comma-separated package sets: std, ext (default: std)
  --dir <path>      where the interpreter goes (default: ~/.marslang)
  --pkg-dir <path>  where installed packages go (default: ~/marslang_pkgs)
  --no-path         do not touch your shell profile
USAGE
    exit 2
}

while [ $# -gt 0 ]; do
    case $1 in
        --version) version=${2:?--version needs a tag}; shift 2 ;;
        --use) use=${2:?--use needs a list}; shift 2 ;;
        --dir) install_dir=${2:?--dir needs a path}; shift 2 ;;
        --pkg-dir) pkg_dir=${2:?--pkg-dir needs a path}; shift 2 ;;
        --no-path) edit_path=no; shift ;;
        -h|--help) usage ;;
        *) echo "unknown option: $1" >&2; usage ;;
    esac
done

step() { printf '==> %s\n' "$1"; }
die() { printf 'error: %s\n' "$1" >&2; exit 1; }

wants_ext=no
for set in $(echo "$use" | tr ',' ' '); do
    case $set in
        std) ;;
        ext) wants_ext=yes ;;
        *) die "unknown package set '$set'; use any of: std, ext" ;;
    esac
done

command -v curl >/dev/null 2>&1 || die "curl is required"

case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) die "unsupported architecture '$(uname -m)'; build from source instead" ;;
esac
# Linux releases are static musl binaries, which run on every distribution;
# releases before rs-0.15.0 only have glibc ones, used when musl is missing.
case "$(uname -s)" in
    Linux) targets="$arch-unknown-linux-musl $arch-unknown-linux-gnu" ;;
    Darwin) targets="$arch-apple-darwin" ;;
    *) die "unsupported system '$(uname -s)'; build from source instead" ;;
esac

if [ "$version" = latest ]; then
    step "Looking up the latest release"
    version=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" 2>/dev/null |
        sed -n 's/.*"tag_name"[ ]*:[ ]*"\([^"]*\)".*/\1/p' | head -n 1) ||
        die "could not read the latest release of $repo; pass --version rs-X.Y.Z, or check that the repository is public"
    [ -n "$version" ] || die "could not read the latest release of $repo; pass --version rs-X.Y.Z"
fi

# A mirror or a local copy can be used instead of the GitHub release.
base="${MARSLANG_DOWNLOAD_BASE:-https://github.com/$repo/releases/download/$version}"
staging=$(mktemp -d "${TMPDIR:-/tmp}/marslang-install.XXXXXX")
trap 'rm -rf "$staging"' EXIT INT TERM

# SHA256SUMS lists every archive of the release, so it also says which exist.
curl -fsSL "$base/SHA256SUMS" -o "$staging/SHA256SUMS" || die "the release has no SHA256SUMS file: $base/SHA256SUMS"
archive=""
expected=""
for target in $targets; do
    candidate="marslang-$version-$target.tar.gz"
    expected=$(awk -v name="$candidate" '$2 == name || $2 == "*"name { print $1 }' "$staging/SHA256SUMS" | head -n 1)
    if [ -n "$expected" ]; then archive="$candidate"; break; fi
done
[ -n "$archive" ] || die "release $version has no archive for this system ($targets)"

step "Downloading $archive"
curl -fsSL "$base/$archive" -o "$staging/$archive" || die "no such release asset: $base/$archive"

step "Verifying the checksum"
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$staging/$archive" | cut -d' ' -f1)
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$staging/$archive" | cut -d' ' -f1)
else
    die "neither sha256sum nor shasum is available to verify the download"
fi
[ "$actual" = "$expected" ] || die "checksum mismatch for $archive: expected $expected, got $actual"

step "Installing to $install_dir"
tar -xzf "$staging/$archive" -C "$staging"
binary=$(find "$staging" -type f -name marslang -perm -u+x | head -n 1)
[ -n "$binary" ] || die "$archive does not contain a marslang binary"
mkdir -p "$install_dir/bin" "$pkg_dir"
install -m 755 "$binary" "$install_dir/bin/marslang"

[ "$wants_ext" = yes ] && printf 'warning: extension packages are not published yet; only the standard library was installed\n' >&2

if [ "$edit_path" = yes ]; then
    profile=""
    case "${SHELL:-}" in
        */zsh) profile="$HOME/.zshrc" ;;
        */bash) [ -f "$HOME/.bash_profile" ] && profile="$HOME/.bash_profile" || profile="$HOME/.bashrc" ;;
        */fish) profile="$HOME/.config/fish/config.fish" ;;
        *) [ -f "$HOME/.profile" ] && profile="$HOME/.profile" ;;
    esac
    line="export PATH=\"$install_dir/bin:\$PATH\""
    case "$profile" in *config.fish) line="fish_add_path $install_dir/bin" ;; esac
    if [ -n "$profile" ] && ! grep -Fqs "$install_dir/bin" "$profile" 2>/dev/null; then
        step "Adding $install_dir/bin to your PATH in $profile"
        mkdir -p "$(dirname "$profile")"
        printf '\n# marslang\n%s\n' "$line" >> "$profile"
    fi
fi
if [ "$pkg_dir" != "$HOME/marslang_pkgs" ]; then
    printf 'note: set MARSLANG_PKGS=%s in your environment so packages there are found\n' "$pkg_dir"
fi

printf '\n%s is installed.\n' "$("$install_dir/bin/marslang" --version)"
printf '  interpreter: %s\n' "$install_dir/bin/marslang"
printf '  packages:    %s\n\n' "$pkg_dir"
printf 'Open a new shell, then run:  marslang hello.mars\n'
printf 'Documentation: https://marslang.kevin-z.com\n'
