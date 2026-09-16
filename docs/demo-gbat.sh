#!/bin/sh
# Replay the battery reading from the original README demo without HID access.
# Forward version and other arguments to the current build.
set -eu

if [ "$#" -eq 0 ]; then
    printf 'Battery: 90%%\n'
else
    exec "${GBAT_DEMO_BINARY:?Set GBAT_DEMO_BINARY to the current gbat build}" "$@"
fi
