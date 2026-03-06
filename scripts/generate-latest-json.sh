#!/bin/bash

# Generates latest.json for the Tauri updater plugin.
#
# Usage:
#   ./scripts/generate-latest-json.sh <version> <changelog> <sig_file> <output_file>
#
# Arguments:
#   version     - Release version (e.g. "0.5.0")
#   changelog   - Release notes text
#   sig_file    - Path to the .sig file for the .app.tar.gz
#   output_file - Path to write latest.json (e.g. "release-assets/latest.json")
#
# The download URL is derived from the version using the GitHub Releases convention.

set -e

REPO_URL="https://github.com/ethanbond64/proveall"

VERSION="$1"
CHANGELOG="$2"
SIG_FILE="$3"
OUTPUT_FILE="$4"

if [ -z "$VERSION" ] || [ -z "$SIG_FILE" ] || [ -z "$OUTPUT_FILE" ]; then
  echo "Usage: $0 <version> <changelog> <sig_file> <output_file>"
  exit 1
fi

if [ ! -f "$SIG_FILE" ]; then
  echo "Error: Signature file not found: $SIG_FILE"
  exit 1
fi

SIGNATURE=$(cat "$SIG_FILE")
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
DOWNLOAD_URL="${REPO_URL}/releases/download/v${VERSION}/ProveAll_${VERSION}_aarch64.app.tar.gz"

# Use jq if available, otherwise build JSON manually
if command -v jq &> /dev/null; then
  jq -n \
    --arg version "$VERSION" \
    --arg notes "$CHANGELOG" \
    --arg pub_date "$PUB_DATE" \
    --arg signature "$SIGNATURE" \
    --arg url "$DOWNLOAD_URL" \
    '{
      version: $version,
      notes: $notes,
      pub_date: $pub_date,
      platforms: {
        "darwin-aarch64": {
          signature: $signature,
          url: $url
        }
      }
    }' > "$OUTPUT_FILE"
else
  # Escape JSON strings manually (newlines and quotes)
  ESCAPED_NOTES=$(printf '%s' "$CHANGELOG" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}' | sed 's/\\n$//')
  ESCAPED_SIG=$(printf '%s' "$SIGNATURE" | sed 's/\\/\\\\/g; s/"/\\"/g')

  cat > "$OUTPUT_FILE" <<ENDJSON
{
  "version": "${VERSION}",
  "notes": "${ESCAPED_NOTES}",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": "${ESCAPED_SIG}",
      "url": "${DOWNLOAD_URL}"
    }
  }
}
ENDJSON
fi

echo "Generated $OUTPUT_FILE for v${VERSION}"
