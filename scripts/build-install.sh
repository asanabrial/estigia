#!/bin/sh
# Rebuilds and performs a complete source installation of Estigia on macOS or
# Linux.
#
# Run from any location:
#
#   <checkout>/scripts/build-install.sh

set -eu

TOOLCHAIN='1.97.0'

say() { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 \
        || fail "'$1' is required but is not available on PATH. Install Rust from https://rustup.rs/ and run this script again."
}

need rustup
need cargo

# The checkout, not this script's own directory: the script lives in
# `scripts/` and everything below builds and installs the crate one level up.
REPOSITORY="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

# Cargo's install root, resolved once and absolutely: the directory the binary
# is installed into is the directory this script looks in. `CARGO_INSTALL_ROOT`
# and `CARGO_HOME` are honoured as Cargo honours them, and the fallback is
# Cargo's own default, `$HOME/.cargo` — not `$HOME`, which would install beside
# the binary this script exists to replace. Cargo's `install.root` config
# setting is deliberately not read: this script pins the root so the written
# path and the checked path agree, and a machine that sets `install.root`
# should set `CARGO_INSTALL_ROOT` for it. A relative root is taken from the
# caller's working directory, before any `cd`.
if [ -n "${CARGO_INSTALL_ROOT:-}" ]; then
    INSTALL_ROOT="$CARGO_INSTALL_ROOT"
elif [ -n "${CARGO_HOME:-}" ]; then
    INSTALL_ROOT="$CARGO_HOME"
elif [ -n "${HOME:-}" ]; then
    INSTALL_ROOT="$HOME/.cargo"
else
    fail "cannot tell where Cargo installs binaries: set HOME, CARGO_HOME or CARGO_INSTALL_ROOT."
fi
case "$INSTALL_ROOT" in
    /*) ;;
    *) INSTALL_ROOT="$PWD/$INSTALL_ROOT" ;;
esac

say "Installing Rust $TOOLCHAIN if needed"
rustup toolchain install "$TOOLCHAIN" --profile minimal \
    || fail "'rustup toolchain install $TOOLCHAIN --profile minimal' failed."

(
    cd "$REPOSITORY"
    say 'Compiling Estigia in release mode'
    rustup run "$TOOLCHAIN" cargo build --release --locked \
        || fail "'rustup run $TOOLCHAIN cargo build --release --locked' failed."

    say 'Reinstalling the compiled binary'
    rustup run "$TOOLCHAIN" cargo install --root "$INSTALL_ROOT" --path . --locked --force \
        || fail "'rustup run $TOOLCHAIN cargo install --root $INSTALL_ROOT --path . --locked --force' failed. Cargo's own output above says why; if it was a locked executable, close the agents using Estigia and run the script again."
)

ESTIGIA="$INSTALL_ROOT/bin/estigia"
[ -x "$ESTIGIA" ] && [ -f "$ESTIGIA" ] \
    || fail "Cargo reported success, but the installed executable was not found at '$ESTIGIA'."

say 'Performing the complete source-build installation'
"$ESTIGIA" --version || fail "'$ESTIGIA --version' failed."
"$ESTIGIA" update || fail "'$ESTIGIA update' failed."
"$ESTIGIA" setup --all --allow-source-build || fail "'$ESTIGIA setup --all --allow-source-build' failed."
"$ESTIGIA" status || fail "'$ESTIGIA status' failed."
"$ESTIGIA" doctor || fail "'$ESTIGIA doctor' failed."

say "Complete installation finished: $ESTIGIA"
