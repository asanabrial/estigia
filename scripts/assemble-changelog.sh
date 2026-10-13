#!/bin/sh
set -eu

usage() {
    echo "usage: scripts/assemble-changelog.sh <version> [<changelog> [<changes-dir>]]" >&2
    exit 2
}

[ $# -ge 1 ] && [ $# -le 3 ] || usage

root=$(cd "$(dirname "$0")/.." && pwd)
version=${1#[vV]}
changelog=${2:-$root/CHANGELOG.md}
changes=${3:-$root/changes}

[ -f "$changelog" ] || { echo "no changelog at $changelog" >&2; exit 1; }
[ -d "$changes" ] || { echo "nothing to assemble: no $changes"; exit 0; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

: > "$work/names"
for path in "$changes"/*; do
    [ -e "$path" ] || continue
    name=$(basename "$path")
    case "$name" in
        [0-9]*-*.md) [ -f "$path" ] && printf '%s\n' "$name" >> "$work/names" && continue ;;
    esac
    echo "$path is not a fragment: a file here is named <issue>-<slug>.md" >&2
    exit 1
done

sort -t- -k1,1nr -k2 "$work/names" > "$work/sorted"
[ -s "$work/sorted" ] || { echo "nothing to assemble: $changes holds no fragment"; exit 0; }

: > "$work/bundle"
while IFS= read -r name; do
    first=$(head -n 1 "$changes/$name")
    case "$first" in
        "section: "?*) section=${first#section: } ;;
        *) echo "$changes/$name does not open with a line 'section: <heading>'" >&2; exit 1 ;;
    esac
    status=0
    tail -n +2 "$changes/$name" | awk -v section="$section" '
        !started && /^[ \t]*$/ { next }
        { started = 1; lines[++count] = $0 }
        END {
            while (count > 0 && lines[count] ~ /^[ \t]*$/) count--
            if (count == 0 || lines[1] !~ /^- /) exit 3
            print "@@" section
            for (i = 1; i <= count; i++) print lines[i]
        }' >> "$work/bundle" || status=$?
    if [ "$status" -ne 0 ]; then
        echo "$changes/$name holds no entry: after the section line it must open with a '- ' bullet" >&2
        exit 1
    fi
done < "$work/sorted"

awk -v version="$version" '
    function opens(heading, wanted,   rest, size, next_character) {
        rest = heading
        sub(/^\[/, "", rest)
        sub(/^[vV]/, "", rest)
        size = length(wanted)
        if (substr(rest, 1, size) != wanted) return 0
        next_character = substr(rest, size + 1, 1)
        return next_character == "" || next_character !~ /[0-9A-Za-z.+-]/
    }
    function flush() {
        if (pending) {
            printf "%s\n\n", entries[section]
            delete entries[section]
            pending = 0
        }
    }
    FNR == NR {
        if ($0 ~ /^@@/) { current = substr($0, 3); next }
        entries[current] = (current in entries) ? entries[current] "\n" $0 : $0
        next
    }
    /^## / {
        flush()
        inside = opens(substr($0, 4), version)
        matched += inside
        section = ""
    }
    /^### / {
        flush()
        section = ""
        if (inside) {
            section = substr($0, 5)
            sub(/[ \t\r]+$/, "", section)
            pending = (section in entries)
        }
    }
    /^- / && pending {
        printf "%s\n", entries[section]
        delete entries[section]
        pending = 0
    }
    { print }
    END {
        flush()
        left = ""
        for (key in entries) left = left "\n  " key
        if (matched != 1) {
            print "expected exactly one heading opening " version ", found " matched+0 > "/dev/stderr"
            exit 1
        }
        if (left != "") {
            print "no section of " version " carries a heading for:" left > "/dev/stderr"
            exit 1
        }
    }' "$work/bundle" "$changelog" > "$work/assembled"

cat "$work/assembled" > "$changelog"
while IFS= read -r name; do rm -f "$changes/$name"; done < "$work/sorted"
echo "assembled $(wc -l < "$work/sorted" | tr -d ' ') fragment(s) into $changelog under $version"
