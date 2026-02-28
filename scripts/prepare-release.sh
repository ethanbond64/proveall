#!/bin/bash

# Script to help prepare a new release
# Usage: ./scripts/prepare-release.sh [version]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get version from argument or prompt
if [ -z "$1" ]; then
    echo -e "${YELLOW}Enter the new version number (e.g., 1.0.0):${NC}"
    read -r VERSION
else
    VERSION=$1
fi

# Validate version format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format. Please use semantic versioning (e.g., 1.0.0)${NC}"
    exit 1
fi

echo -e "${GREEN}Preparing release v$VERSION...${NC}"

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo -e "${RED}Error: You have uncommitted changes. Please commit or stash them first.${NC}"
    exit 1
fi

# Update CHANGELOG.md
echo -e "${GREEN}Updating CHANGELOG.md...${NC}"

# Get today's date
DATE=$(date +%Y-%m-%d)

# Create temp file
TEMP_FILE=$(mktemp)

# Process CHANGELOG.md
awk -v version="$VERSION" -v date="$DATE" '
/## \[Unreleased\]/ {
    print $0
    print ""
    print "## [" version "] - " date
    next
}
/^\[Unreleased\]:/ {
    print "[Unreleased]: https://github.com/ethanbond64/proveall/compare/v" version "...HEAD"
    print "[" version "]: https://github.com/ethanbond64/proveall/compare/v0.1.0...v" version
    next
}
{print}
' CHANGELOG.md > "$TEMP_FILE"

mv "$TEMP_FILE" CHANGELOG.md

echo -e "${GREEN}CHANGELOG.md updated${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Review and update the CHANGELOG.md with your release notes under the [$VERSION] section"
echo "2. Commit the changes: git add CHANGELOG.md && git commit -m \"Prepare release v$VERSION\""
echo "3. Create a pull request to main branch"
echo "4. Once the PR is merged, the GitHub Action will automatically:"
echo "   - Run tests"
echo "   - Build binaries for supported platforms"
echo "   - Create a GitHub release with the changelog notes"