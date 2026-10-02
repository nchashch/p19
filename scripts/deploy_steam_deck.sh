#!/bin/sh
# Builds a release `client` binary inside the steamrt4 toolbox (the correct glibc baseline for
# the Deck/Steam Runtime — see AGENTS.md's "Commands" section) and stages it plus its `assets/`
# directory under `steam_deck_staging/`, ready for SteamOS Devkit Client's Title Upload:
#   Local Folder   = steam_deck_staging/
#   Start Command  = ./client
#
# "Clean isolated run": a plain host-side `cargo build`/`cargo run` for this same target
# silently taints cargo's cached intermediate objects with the *host's* (newer) glibc, and a
# bare `rm` of the final binary is not enough to undo that — cargo can still decide only a
# relink (not a full recompile) is needed and reuse the tainted objects. `cargo clean -p client
# --release`, run *inside* the toolbox before building, forces a real recompile against the
# toolbox's own (older) glibc. Run this any time you're not sure whether the last release build
# of `client` happened on the host or inside the toolbox — see AGENTS.md's GLIBC-mismatch note
# for the full story (`GLIBC_2.4x not found` on the Deck otherwise).
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
STAGING_DIR="$REPO_ROOT/steam_deck_staging"

cd "$REPO_ROOT"

echo "==> Cleaning client's release artifacts inside the steamrt4 toolbox"
./scripts/steam_deck_toolbox.sh cargo clean -p client --release

echo "==> Building client --release inside the steamrt4 toolbox"
./scripts/steam_deck_toolbox.sh cargo build -p client --release

echo "==> Staging binary + assets at $STAGING_DIR"
mkdir -p "$STAGING_DIR"
cp target/release/client "$STAGING_DIR/client"
rm -rf "$STAGING_DIR/assets"
cp -r client/assets "$STAGING_DIR/assets"

if command -v objdump >/dev/null 2>&1; then
    max_glibc=$(objdump -T "$STAGING_DIR/client" | grep -oE 'GLIBC_[0-9.]+' | sed 's/GLIBC_//' | sort -Vu | tail -1)
    if [ -n "$max_glibc" ]; then
        echo "==> Highest GLIBC symbol version linked: $max_glibc"
        # Known toolbox baseline is ~2.41 (see AGENTS.md) — anything higher means this build
        # linked against the HOST's glibc, not the toolbox's, and will likely fail on the Deck
        # with "GLIBC_2.4x not found".
        highest=$(printf '%s\n%s\n' "$max_glibc" "2.41" | sort -V | tail -1)
        if [ "$highest" = "$max_glibc" ] && [ "$max_glibc" != "2.41" ]; then
            echo "WARNING: GLIBC_$max_glibc exceeds the toolbox's known baseline (~2.41)." >&2
            echo "         This binary was likely linked against the HOST's glibc, not the toolbox's." >&2
            echo "         It may fail on the Deck with 'GLIBC_2.4x not found'." >&2
        fi
    fi
else
    echo "NOTE: objdump not found — skipping GLIBC baseline sanity check." >&2
fi

echo "==> Done. Staged at $STAGING_DIR"
echo "    In SteamOS Devkit Client's Title Upload: Local Folder = $STAGING_DIR, Start Command = ./client"
