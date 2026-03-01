#!/usr/bin/env bash
#
# Download and install the PDFium shared library.
#
# Pre-built binary from https://github.com/bblanchon/pdfium-binaries.
# Installed to /usr/local/lib so Pdfium::bind_to_system_library() works.
#
# Usage:
#   ./scripts/install-pdfium.sh                      # default: linux-x64
#   ./scripts/install-pdfium.sh linux-arm64           # override platform
#   PDFIUM_PLATFORM=linux-arm64 ./scripts/install-pdfium.sh

set -euo pipefail

PLATFORM="${1:-${PDFIUM_PLATFORM:-linux-x64}}"
URL="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-${PLATFORM}.tgz"

echo "Installing PDFium (${PLATFORM})..."

curl -fsSL "$URL" | tar xz -C /tmp
mv /tmp/lib/libpdfium.so /usr/local/lib/
ldconfig
rm -rf /tmp/include /tmp/lib /tmp/*.cmake /tmp/LICENSE /tmp/VERSION /tmp/args.gn

echo "PDFium installed to /usr/local/lib/libpdfium.so"
