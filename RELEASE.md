# Release Process

This document describes the automated release process for ProveAll.

## Overview

ProveAll uses an automated release process triggered by changes to the CHANGELOG.md file. When a pull request is merged to the main branch that contains a new version entry in the CHANGELOG, the GitHub Actions workflow automatically:

1. Runs all tests
2. Builds binaries for supported platforms
3. Creates a GitHub release with the binaries attached using the CHANGELOG description
4. Tags the release with the appropriate version

## How to Create a Release

### 1. Prepare Your Changes

Commit and push your changes to a feature branch.

### 2. Update the Changelog

Run the Helper Script to change versions automatically

```bash
# Interactive mode - will prompt for release type
./scripts/prepare-release.sh

# Or specify the release type directly
./scripts/prepare-release.sh patch  # for bug fixes (0.1.0 → 0.1.1)
./scripts/prepare-release.sh minor  # for new features (0.1.0 → 0.2.0)
./scripts/prepare-release.sh major  # for breaking changes (0.1.0 → 1.0.0)
```

This script will:
- Detect the current version from CHANGELOG.md
- Automatically bump the version based on semantic versioning

### 3. Create a Pull Request

1. Commit your changelog changes
2. Push your branch and create a pull request to `main`
3. The PR will trigger the test workflow to ensure everything passes

### 4. Merge and Release

Once the PR is approved and merged to main:

1. The release workflow automatically starts
2. It detects the new version in CHANGELOG.md
3. Runs all tests
4. Builds binaries for supported platforms
5. Creates a GitHub release with:
   - Version tag (e.g., `v1.2.0`)
   - Release title
   - Changelog excerpt for this version
   - All built binaries attached
   - Installation instructions
