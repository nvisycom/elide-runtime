#!/bin/sh
# Fail unless the compose defaults name the version being released.
#
# Usage: check_compose_version.sh <version> [compose.yaml] [.env.example]
#
#     version  the tag without its leading v, e.g. 0.1.0 or 0.1.0-rc.1
#
# docker/compose.yaml and the images it runs are one versioned unit: the file
# sets the settings, cache layout and warmers that only images built from the
# same commit understand. Its `${ELIDE_VERSION:-...}` default is therefore the
# release the file ships in, and .env.example repeats it. Run on a tag before
# anything is published, so a forgotten bump fails the release instead of
# shipping a compose file that pulls older images. rc.5 and rc.6 both shipped
# with the default still at rc.4.
set -eu

version="$1"
root=$(cd "$(dirname "$0")/.." && pwd)
compose="${2:-$root/docker/compose.yaml}"
env_example="${3:-$root/docker/.env.example}"

# Every default the compose file gives ELIDE_VERSION, one per line, deduplicated.
# Single-quoted on purpose: the pattern is the literal `${...}` in the file.
# shellcheck disable=SC2016
defaults=$(grep -o '\${ELIDE_VERSION:-[^}]*}' "$compose" \
    | sed 's/^\${ELIDE_VERSION:-//; s/}$//' | sort -u)
example=$(sed -n 's/^ELIDE_VERSION=//p' "$env_example")

status=0
if [ -z "$defaults" ]; then
    echo "$compose gives ELIDE_VERSION no default." >&2
    status=1
elif [ "$defaults" != "$version" ]; then
    echo "$compose defaults ELIDE_VERSION to:" >&2
    echo "$defaults" | sed 's/^/    /' >&2
    echo "but this release is $version. Set every default to $version." >&2
    status=1
fi
if [ "$example" != "$version" ]; then
    echo "$env_example sets ELIDE_VERSION=$example, but this release is $version." >&2
    status=1
fi
[ "$status" -eq 0 ] && echo "compose defaults match $version."
exit "$status"
