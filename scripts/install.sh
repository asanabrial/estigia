#!/bin/sh
# Installs Estigia on Linux or macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.sh | sh
#
# Downloads the release archive for this machine, checks it against the
# published SHA-256 sums, and puts the binary somewhere on the path. Nothing is
# installed if the checksum does not match.
#
# This script and install.ps1 no longer share any logic — they only download and
# verify. `curl | sh` and `irm | iex` are different worlds, and mirroring the
# real work by hand across both was how issue-flow ended up with 488 and 505
# lines that had to be kept in step.
#
# Plain POSIX sh on purpose: this has to run before anything is installed, on
# whatever the machine happens to have.

set -eu

REPO="asanabrial/estigia"
# A user-owned directory by default, so no sudo.
INSTALL_DIR="${ESTIGIA_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${ESTIGIA_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || fail "this needs $1, which is not installed"
}

# Every refusal below names something that can actually be run. Six build
# targets have to exist before anyone can install, and until they all do, the
# fallback that always works is a source build — so it is named in the refusal
# rather than left for the reader to think of.
source_build() {
    printf 'cargo install --git https://github.com/%s' "$REPO"
}

target_triple() {
    kernel="$(uname -s)"
    machine="$(uname -m)"
    case "$kernel" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        # Git Bash, MSYS2 and Cygwin all run on Windows, where the release is a
        # zip and the PATH lives in the registry. Send them to the right script
        # rather than to a build they did not ask for.
        MINGW*|MSYS*|CYGWIN*)
            fail "on Windows, run instead:
    irm https://raw.githubusercontent.com/$REPO/main/scripts/install.ps1 | iex" ;;
        *) fail "no prebuilt Estigia for $kernel; build from source with '$(source_build)'" ;;
    esac
    case "$machine" in
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) fail "no prebuilt Estigia for $machine; build from source with '$(source_build)'" ;;
    esac
    printf '%s-%s' "$arch" "$os"
}

resolve_version() {
    if [ "$VERSION" != "latest" ]; then
        printf '%s' "$VERSION"
        return
    fi
    # The redirect from /releases/latest names the tag, which avoids depending
    # on the API and its rate limit.
    resolved=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
        "https://github.com/$REPO/releases/latest" 2>/dev/null | sed 's|.*/tag/||')
    [ -n "$resolved" ] || fail "could not work out the latest version; set ESTIGIA_VERSION"
    printf '%s' "$resolved"
}

need curl
need tar
need uname

TARGET="$(target_triple)"
VERSION="$(resolve_version)"
PACKAGE="estigia-$VERSION-$TARGET"
ARCHIVE="$PACKAGE.tar.gz"
# Overridable so an internal mirror can serve the same layout, and so the
# verification path can be exercised without publishing anything.
BASE="${ESTIGIA_BASE_URL:-https://github.com/$REPO/releases/download/$VERSION}"

say "Estigia $VERSION for $TARGET"

TEMP="$(mktemp -d)"
# Leave nothing behind, including on failure.
trap 'rm -rf "$TEMP"' EXIT INT TERM

say "  downloading"
curl -fsSL "$BASE/$ARCHIVE" -o "$TEMP/$ARCHIVE" \
    || fail "no release archive at $BASE/$ARCHIVE; build from source with '$(source_build)'"
# One sum per archive, which is what the release workflow publishes: it runs
# `shasum -a 256 <archive> > <archive>.sha256` per target, and there has never
# been an aggregate listing. Both installers asked for `SHA256SUMS` and refused
# when it was missing — fail-closed, correct, and about a release that was
# complete: the first person to run the documented one-liner would have been
# told no checksums were published for a version that had them all.
#
# It also removes the parsing. A listing has to be searched for the right line,
# and these two had already drifted on how: one allowed for the `*` that
# `sha256sum` writes in binary mode and the other did not.
curl -fsSL "$BASE/$ARCHIVE.sha256" -o "$TEMP/$ARCHIVE.sha256" \
    || fail "no checksum published for $ARCHIVE; refusing to install unverified"

say "  verifying"
expected=$(awk '{print $1}' "$TEMP/$ARCHIVE.sha256")
[ -n "$expected" ] || fail "$ARCHIVE.sha256 carries no checksum"
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$TEMP/$ARCHIVE" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$TEMP/$ARCHIVE" | awk '{print $1}')
else
    fail "this needs sha256sum or shasum to verify the download"
fi
[ "$actual" = "$expected" ] || fail "checksum mismatch: expected $expected, got $actual"

# The sum says the bytes are the ones published. It says nothing about who
# published them, because whoever could replace the archive could replace the
# sum beside it. The provenance answers that second question: it is signed by
# the workflow run that built the archive, with an identity nobody holds.
#
# Checked only when `gh` is here. Making it mandatory would mean an installer
# that refuses to work without the GitHub CLI, on the machines least likely to
# have it — and a check that stops people installing gets removed, not fixed.
# When it IS here and the archive fails it, that is a hard stop: a present tool
# reporting a bad signature is not an inconclusive answer.
if command -v gh >/dev/null 2>&1; then
    say "  checking provenance"
    if gh attestation verify "$TEMP/$ARCHIVE" --repo "$REPO" >/dev/null 2>&1; then
        say "  provenance: signed by the $REPO release workflow"
    elif gh auth status >/dev/null 2>&1; then
        fail "provenance check FAILED for $ARCHIVE — the bytes match the published sum but
    nothing proves the $REPO workflow built them. Refusing to install."
    else
        say "  provenance: not checked (gh is not logged in)"
    fi
fi

say "  extracting"
tar -xzf "$TEMP/$ARCHIVE" -C "$TEMP"
CANDIDATE="$TEMP/$PACKAGE/estigia"
say "  recording candidate lifecycle"
if ! "$CANDIDATE" __record-install; then
    fail "candidate lifecycle admission failed; refusing to replace the installed executable"
fi

say "  installing to $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
install -m 755 "$CANDIDATE" "$INSTALL_DIR/estigia" 2>/dev/null \
    || { cp "$CANDIDATE" "$INSTALL_DIR/estigia" && chmod 755 "$INSTALL_DIR/estigia"; }

say ""
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        say "Installed. Run 'estigia setup --all' to register it in your agents."
        ;;
    *)
        say "Installed, but $INSTALL_DIR is not on your PATH."
        say "Add this to your shell profile:"
        say ""
        say "    export PATH=\"\$PATH:$INSTALL_DIR\""
        say ""
        say "Then run 'estigia setup --all' to register it in your agents."
        ;;
esac
