#!/bin/bash

# Script to help prepare a new release
# Usage: ./scripts/prepare-release.sh [patch|minor|major]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to bump version
bump_version() {
    local version=$1
    local bump_type=$2

    # Parse version components
    IFS='.' read -r major minor patch <<< "$version"

    case $bump_type in
        patch)
            patch=$((patch + 1))
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            ;;
        major)
            major=$((major + 1))
            minor=0
            patch=0
            ;;
    esac

    echo "$major.$minor.$patch"
}

# Get the last version from CHANGELOG.md
echo -e "${BLUE}Reading current version from CHANGELOG.md...${NC}"
LAST_VERSION=$(grep -E '## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md | head -1 | sed 's/## \[\(.*\)\].*/\1/')

if [ -z "$LAST_VERSION" ]; then
    echo -e "${RED}Error: No version found in CHANGELOG.md${NC}"
    echo -e "${YELLOW}Please ensure CHANGELOG.md contains at least one version entry.${NC}"
    exit 1
fi

echo -e "${GREEN}Current version: v$LAST_VERSION${NC}"

# Get bump type from argument or prompt
if [ -z "$1" ]; then
    echo ""
    echo -e "${YELLOW}What type of release is this?${NC}"
    echo "  1) patch (bug fixes, small changes)"
    echo "  2) minor (new features, backwards compatible)"
    echo "  3) major (breaking changes)"
    echo ""
    echo -n "Select release type [1-3]: "
    read -r RELEASE_CHOICE

    case $RELEASE_CHOICE in
        1)
            BUMP_TYPE="patch"
            ;;
        2)
            BUMP_TYPE="minor"
            ;;
        3)
            BUMP_TYPE="major"
            ;;
        *)
            echo -e "${RED}Error: Invalid choice${NC}"
            exit 1
            ;;
    esac
else
    # Accept patch, minor, major as command line argument
    if [[ "$1" =~ ^(patch|minor|major)$ ]]; then
        BUMP_TYPE=$1
    else
        echo -e "${RED}Error: Invalid argument. Use 'patch', 'minor', or 'major'${NC}"
        exit 1
    fi
fi

# Calculate new version
VERSION=$(bump_version "$LAST_VERSION" "$BUMP_TYPE")

echo -e "${GREEN}Bumping from v$LAST_VERSION to v$VERSION (${BUMP_TYPE} release)${NC}"

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
{print}
' CHANGELOG.md > "$TEMP_FILE"

mv "$TEMP_FILE" CHANGELOG.md

echo -e "${GREEN}CHANGELOG.md updated${NC}"

# Update package.json
echo -e "${BLUE}Updating package.json version to $VERSION...${NC}"
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" package.json
else
    # Linux
    sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" package.json
fi
echo -e "${GREEN}package.json updated${NC}"

# Update tauri.conf.json
echo -e "${BLUE}Updating tauri.conf.json version to $VERSION...${NC}"
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri_src/tauri.conf.json
else
    # Linux
    sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri_src/tauri.conf.json
fi
echo -e "${GREEN}tauri.conf.json updated${NC}"

# Update Cargo.toml
echo -e "${BLUE}Updating Cargo.toml version to $VERSION...${NC}"
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" tauri_src/Cargo.toml
else
    # Linux
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" tauri_src/Cargo.toml
fi
echo -e "${GREEN}Cargo.toml updated${NC}"

echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Review and update the CHANGELOG.md with your release notes under the [$VERSION] section"
echo "2. Commit all version changes:"
echo "   git add CHANGELOG.md package.json tauri_src/tauri.conf.json tauri_src/Cargo.toml"
echo "   git commit -m \"Prepare release v$VERSION\""
echo "3. Create a pull request against the main branch"
echo "4. Once the PR is merged, the GitHub Action will automatically:"
echo "   - Run tests"
echo "   - Build binaries for supported platforms"
echo "   - Create a GitHub release with the changelog notes"