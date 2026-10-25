#!/bin/sh
# Merge a service's per-architecture images into the tags an operator pulls.
#
# Each architecture is built and pushed on its own native runner as
# `<image>:<version>-amd64` / `-arm64`; this combines them into one manifest
# so `docker pull <image>:<version>` resolves to the host's architecture.
#
#   publish_manifest.sh <image> <version>
#
#     image    ghcr.io/<owner>/elide-<service>
#     version  the tag without its leading v, e.g. 0.1.0 or 0.1.0-rc.1
#
# A release also moves `latest`; a prerelease does not. A release candidate
# must not become the image someone gets by default, least of all once the
# release it precedes exists — nothing moves `latest` back. The test is the
# `-` SemVer uses to introduce a prerelease, which is the same rule
# release.yml applies when deciding whether to mark the GitHub Release a
# prerelease. The two must agree: an image tagged `latest` beside a Release
# marked prerelease is a contradiction a reader cannot resolve.
#
# Lives here rather than inline in the workflow so it can be run against a
# local registry and is covered by tests — the prerelease rule is the kind of
# thing that looks obviously right and silently inverts.
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <image> <version>" >&2
    exit 2
fi

image="$1"
version="$2"

if [ -z "$image" ] || [ -z "$version" ]; then
    echo "both <image> and <version> are required, and neither may be empty" >&2
    exit 2
fi

# `latest` only for a release. Printed rather than silently skipped, so a run
# log says which rule applied.
tags="$version"
case "$version" in
    *-*) echo "$version is a prerelease; leaving :latest alone." ;;
    *) tags="$tags latest" ;;
esac

for tag in $tags; do
    echo "Creating $image:$tag"
    docker buildx imagetools create \
        --tag "$image:$tag" \
        "$image:$version-amd64" \
        "$image:$version-arm64"
done
