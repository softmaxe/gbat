#!/bin/sh
# Record the current build and connected mouse without replaying sample output.
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
cd "$project_root"

for dependency in cargo vhs ffmpeg; do
    if ! command -v "$dependency" >/dev/null 2>&1; then
        printf 'Required command not found: %s\n' "$dependency" >&2
        exit 1
    fi
done

cargo build --release --locked
binary="$project_root/target/release/gbat"
version=$("$binary" --version)
printf 'Recording %s\n' "$version"
# Fail before recording if the mouse cannot provide a real battery reading.
"$binary"

work_dir=$(mktemp -d "$project_root/target/record-demo.XXXXXX")
trap 'rm -r "$work_dir"' EXIT
trap 'exit 1' HUP INT TERM
candidate="$work_dir/demo.gif"

# Render to a temporary file so a failed recording keeps the published GIF.
awk -v output="$candidate" '
    /^[[:space:]]*Output[[:space:]]/ {
        if (!replaced) {
            print "Output \"" output "\""
            replaced = 1
        }
        next
    }
    { print }
    END { if (!replaced) exit 1 }
' demo.tape > "$work_dir/demo.tape"
vhs "$work_dir/demo.tape"

test -s "$candidate"
ffmpeg -hide_banner -loglevel error -xerror -i "$candidate" -f null -
cp "$candidate" "$project_root/assets/demo.gif"
printf 'Recorded %s in %s\n' "$version" "$project_root/assets/demo.gif"
