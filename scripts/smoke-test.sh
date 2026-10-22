#!/bin/sh
# Checks that mder starts and renders: opens README.md, waits for the
# rendered view to load, then quits. For CI, where nobody is watching.
#
#   scripts/smoke-test.sh PROGRAM [ARCH]
#
# PROGRAM is mder's program: target/universal/release/mder.app/Contents/MacOS/mder
# on the Mac, dist/linux/mder/mder on Linux, dist/mder/mder.exe on Windows
# (run from Git Bash). Linux needs a display: run it under xvfb-run.
# ARCH (arm64 or x86_64) is for the Mac: it runs that half of a universal
# program; x86_64 on Apple silicon needs Rosetta.
set -eu

cd "$(dirname "$0")/.."

program=$1
arch=${2:-}
log=$(mktemp "${TMPDIR:-/tmp}/mder-smoke.XXXXXX")
file="$PWD/README.md"

if [ -n "$arch" ]; then
    arch "-$arch" "$program" --log "$log" "$file" &
else
    "$program" --log "$log" "$file" &
fi
pid=$!

finish() {
    kill "$pid" 2>/dev/null || true
    echo "--- mder's log:"
    cat "$log"
    rm -f "$log"
}

for _ in $(seq 1 60); do
    if grep -q "page loaded" "$log" 2>/dev/null; then
        echo "mder ${arch:-} started and rendered README.md"
        finish
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "mder ${arch:-} quit before rendering" >&2
        finish
        exit 1
    fi
    sleep 1
done
echo "mder ${arch:-} didn't render README.md within 60 seconds" >&2
finish
exit 1
