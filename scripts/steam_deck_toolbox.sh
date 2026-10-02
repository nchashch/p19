#!/bin/sh

# No args: enter the toolbox interactively, same as before.
# With args: run that command inside the toolbox non-interactively and return — e.g.
# `scripts/steam_deck_toolbox.sh cargo build -p client --release`. Needed for anything that has to link
# against the toolbox's own (older) glibc rather than the host's — see AGENTS.md's "Commands"
# section for the full GLIBC-mismatch gotcha this avoids.
if [ "$#" -eq 0 ]; then
    toolbox enter steamrt4
else
    toolbox run -c steamrt4 -- "$@"
fi
