#!/bin/sh
set -eu

flags=""
want=""
for argument in "$@"; do
    if [ -n "$want" ]; then
        flags="$flags $want $argument"
        want=""
        continue
    fi
    case "$argument" in
        --) break ;;
        --release) flags="$flags --release" ;;
        --target | --manifest-path | --profile) want="$argument" ;;
        --target=* | --manifest-path=* | --profile=*) flags="$flags $argument" ;;
    esac
done

cargo build --bins --examples $flags
exec cargo test "$@"
