#!/bin/bash

# Generates latest.json for the Tauri updater plugin.
#
# Usage:
#   ./scripts/generate-latest-json.sh <version> <changelog> <output_file> \
#       <updater_platform> <sig_file> [<updater_platform> <sig_file> ...]
#
# Arguments:
#   version          - Release version (e.g. "0.5.0")
#   changelog        - Release notes text
#   output_file      - Path to write latest.json
#   updater_platform - Tauri updater platform key (e.g. "darwin-aarch64")
#   sig_file         - Path to the .sig file for that platform's .app.tar.gz
#
# Platform/sig_file pairs can be repeated for multi-platform releases.
# Download URLs are derived from the version and platform using GitHub Releases conventions.

set -e

REPO_URL="https://github.com/ethanbond64/proveall"

VERSION="$1"
CHANGELOG="$2"
OUTPUT_FILE="$3"
shift 3 || true

if [ -z "$VERSION" ] || [ -z "$OUTPUT_FILE" ]; then
  echo "Usage: $0 <version> <changelog> <output_file> <platform> <sig_file> [<platform> <sig_file> ...]"
  exit 1
fi

# Collect platform entries from remaining args (pairs of platform + sig_file)
PLATFORMS=()
SIG_FILES=()
while [ $# -ge 2 ]; do
  PLATFORMS+=("$1")
  SIG_FILES+=("$2")
  shift 2
done

if [ ${#PLATFORMS[@]} -eq 0 ]; then
  echo "Error: At least one <platform> <sig_file> pair is required"
  exit 1
fi

PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Derive arch label from updater platform key (e.g. "darwin-aarch64" -> "aarch64")
get_arch() {
  echo "$1" | cut -d'-' -f2
}

if command -v jq &> /dev/null; then
  # Build the base object
  JSON=$(jq -n \
    --arg version "$VERSION" \
    --arg notes "$CHANGELOG" \
    --arg pub_date "$PUB_DATE" \
    '{ version: $version, notes: $notes, pub_date: $pub_date, platforms: {} }')

  # Add each platform entry
  for i in "${!PLATFORMS[@]}"; do
    PLATFORM="${PLATFORMS[$i]}"
    SIG_FILE="${SIG_FILES[$i]}"

    if [ ! -f "$SIG_FILE" ]; then
      echo "Error: Signature file not found: $SIG_FILE"
      exit 1
    fi

    ARCH=$(get_arch "$PLATFORM")
    SIGNATURE=$(cat "$SIG_FILE")
    DOWNLOAD_URL="${REPO_URL}/releases/download/v${VERSION}/ProveAll_${VERSION}_${ARCH}.app.tar.gz"

    JSON=$(echo "$JSON" | jq \
      --arg platform "$PLATFORM" \
      --arg signature "$SIGNATURE" \
      --arg url "$DOWNLOAD_URL" \
      '.platforms[$platform] = { signature: $signature, url: $url }')
  done

  echo "$JSON" > "$OUTPUT_FILE"
else
  # Manual JSON construction (no jq)
  # Build platform entries
  PLATFORM_JSON=""
  for i in "${!PLATFORMS[@]}"; do
    PLATFORM="${PLATFORMS[$i]}"
    SIG_FILE="${SIG_FILES[$i]}"

    if [ ! -f "$SIG_FILE" ]; then
      echo "Error: Signature file not found: $SIG_FILE"
      exit 1
    fi

    ARCH=$(get_arch "$PLATFORM")
    SIGNATURE=$(cat "$SIG_FILE" | sed 's/\\/\\\\/g; s/"/\\"/g')
    DOWNLOAD_URL="${REPO_URL}/releases/download/v${VERSION}/ProveAll_${VERSION}_${ARCH}.app.tar.gz"

    [ -n "$PLATFORM_JSON" ] && PLATFORM_JSON="${PLATFORM_JSON},"
    PLATFORM_JSON="${PLATFORM_JSON}
    \"${PLATFORM}\": {
      \"signature\": \"${SIGNATURE}\",
      \"url\": \"${DOWNLOAD_URL}\"
    }"
  done

  ESCAPED_NOTES=$(printf '%s' "$CHANGELOG" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}' | sed 's/\\n$//')

  cat > "$OUTPUT_FILE" <<ENDJSON
{
  "version": "${VERSION}",
  "notes": "${ESCAPED_NOTES}",
  "pub_date": "${PUB_DATE}",
  "platforms": {${PLATFORM_JSON}
  }
}
ENDJSON
fi

echo "Generated $OUTPUT_FILE for v${VERSION} (${PLATFORMS[*]})"
