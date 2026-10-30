#!/bin/sh
# Tests for check_compose_version.sh, which stops a release whose compose file
# still pulls an older version's images.
#
# Each case writes a small compose file and .env.example and runs the check
# against them. What matters is the exit status: a mismatch that passes is the
# failure this guards, and it is invisible until someone pulls the release.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
script="$here/../check_compose_version.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failures=0

check() {
    description="$1"
    expected="$2"
    actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "ok   $description"
    else
        echo "FAIL $description" >&2
        echo "       expected: $expected" >&2
        echo "       actual:   $actual" >&2
        failures=$((failures + 1))
    fi
}

# compose <default>... : a compose file with one image line per default.
compose() {
    : > "$tmp/compose.yaml"
    for default in "$@"; do
        echo "    image: ghcr.io/example/elide-ner:\${ELIDE_VERSION:-$default}" >> "$tmp/compose.yaml"
    done
}

example() {
    printf '# Image tag to run.\nELIDE_VERSION=%s\n' "$1" > "$tmp/.env.example"
}

# Exit status of the check for a version, as "pass" or "fail".
run() {
    if sh "$script" "$1" "$tmp/compose.yaml" "$tmp/.env.example" > /dev/null 2>&1; then
        echo pass
    else
        echo fail
    fi
}

compose 0.1.0-rc.7 0.1.0-rc.7
example 0.1.0-rc.7
check "matching defaults pass" pass "$(run 0.1.0-rc.7)"

compose 0.1.0-rc.4 0.1.0-rc.4
example 0.1.0-rc.4
check "a forgotten bump fails" fail "$(run 0.1.0-rc.7)"

compose 0.1.0-rc.7 0.1.0-rc.4
example 0.1.0-rc.7
check "one stale service fails" fail "$(run 0.1.0-rc.7)"

compose 0.1.0-rc.7
example 0.1.0-rc.4
check "a stale .env.example fails" fail "$(run 0.1.0-rc.7)"

compose 0.1.0-rc.7
example 0.1.0-rc.7
check "a prefix of the version is not the version" fail "$(run 0.1.0-rc.70)"

: > "$tmp/compose.yaml"
example 0.1.0-rc.7
check "a compose file with no default fails" fail "$(run 0.1.0-rc.7)"

check "the repository's own files match their default" pass "$(
    sh "$script" "$(sed -n 's/^ELIDE_VERSION=//p' "$here/../../docker/.env.example")" \
        > /dev/null 2>&1 && echo pass || echo fail
)"

if [ "$failures" -ne 0 ]; then
    echo "$failures failed" >&2
    exit 1
fi
