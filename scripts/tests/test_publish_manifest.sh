#!/bin/sh
# Tests for publish_manifest.sh, which decides whether a tag moves `latest`.
#
# `docker buildx imagetools create` is stubbed by putting a fake `docker` on
# PATH that records its arguments, so the rule is exercised without a registry.
# What matters is which tags get created: a prerelease moving `latest` is the
# failure this guards, and it is invisible until after a release ships.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
script="$here/../publish_manifest.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# A `docker` that logs its arguments instead of talking to a registry.
mkdir -p "$tmp/bin"
cat > "$tmp/bin/docker" <<'STUB'
#!/bin/sh
echo "$@" >> "$DOCKER_CALLS"
STUB
chmod +x "$tmp/bin/docker"
export PATH="$tmp/bin:$PATH"

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

# Which tags did a run create?
created_tags() {
    sed -n 's/.*--tag \([^ ]*\).*/\1/p' "$DOCKER_CALLS" | sed 's/.*://' | tr '\n' ' ' | sed 's/ $//'
}

run() {
    DOCKER_CALLS="$tmp/calls"
    export DOCKER_CALLS
    : > "$DOCKER_CALLS"
    sh "$script" "$@" > "$tmp/out" 2>&1
}

# --- a release moves latest -------------------------------------------------

run ghcr.io/example/elide-ner 0.1.0
check "a release creates the version tag and latest" "0.1.0 latest" "$(created_tags)"

run ghcr.io/example/elide-ner 2.0.0
check "a major release also moves latest" "2.0.0 latest" "$(created_tags)"

# --- a prerelease does not --------------------------------------------------

run ghcr.io/example/elide-ner 0.1.0-rc.1
check "a release candidate creates only its own tag" "0.1.0-rc.1" "$(created_tags)"

run ghcr.io/example/elide-ner 1.2.3-beta
check "a beta creates only its own tag" "1.2.3-beta" "$(created_tags)"

run ghcr.io/example/elide-ner 1.0.0-alpha.2
check "a numbered alpha creates only its own tag" "1.0.0-alpha.2" "$(created_tags)"

# The reason should be in the log, not silently skipped.
run ghcr.io/example/elide-ner 0.1.0-rc.2
if grep -q "is a prerelease" "$tmp/out"; then
    echo "ok   a prerelease says why latest was left alone"
else
    echo "FAIL a prerelease should log why latest was left alone" >&2
    failures=$((failures + 1))
fi

# --- the per-architecture sources ------------------------------------------

run ghcr.io/example/elide-ocr 0.1.0
sources=$(sed -n 's/.*--tag [^ ]* //p' "$DOCKER_CALLS" | head -1)
check "merges the two per-architecture tags" \
    "ghcr.io/example/elide-ocr:0.1.0-amd64 ghcr.io/example/elide-ocr:0.1.0-arm64" \
    "$sources"

# --- usage ------------------------------------------------------------------

if sh "$script" only-one-arg >/dev/null 2>&1; then
    echo "FAIL a missing version should be rejected" >&2
    failures=$((failures + 1))
else
    echo "ok   a missing version is rejected"
fi

if sh "$script" "" 0.1.0 >/dev/null 2>&1; then
    echo "FAIL an empty image should be rejected" >&2
    failures=$((failures + 1))
else
    echo "ok   an empty image is rejected"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "all checks passed"
