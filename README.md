# ProveAll

**Code Coverage for Human Understanding**

[![License](https://img.shields.io/github/license/ethanbond64/proveall)](LICENSE)
[![Release](https://img.shields.io/github/v/release/ethanbond64/proveall)](https://github.com/ethanbond64/proveall/releases)

> ## ⚠️ This project is sunset and no longer maintained
>
> **Final release: [v0.6.2](https://github.com/ethanbond64/proveall/releases/tag/v0.6.2) — sunset November 15, 2026.**
>
> Development has ended. There will be no further releases, bug fixes, or support, and
> issues and pull requests are closed. This repository remains public, archived, and
> read-only as a record of the work.
>
> The code is MIT licensed and free to read, fork, or build on. The last release is still
> downloadable and still runs, but it is unsupported — use it at your own risk.
>
> ProveAll stores all data locally on your machine. There was never a server, account, or
> hosted component, so nothing is being shut off and no data needs to be migrated or
> exported. Uninstalling removes the app; its local review database lives in the standard
> macOS application support directory for `com.proveall`.

**Available on:** macOS (Apple Silicon)

ProveAll is a desktop app that helps developers maintain understanding and control of AI-generated code. It's built on top of Git, storing a review state for every line of code at each commit. ProveAll gives you a structured way to prove you understand every line before it ships.

## Installation

Download the final `.dmg` from the [GitHub Releases page](https://github.com/ethanbond64/proveall/releases), open it, and drag ProveAll to your Applications folder.

Note that the in-app updater will never find a newer version; v0.6.2 is the last build.

## Example usage

### Project Page
View commit history, review history, and open issues in your branch at a glance.

![Project Page](docs/imgs/ProjectPageScreenshot.png)

### Branch Comparison View
See the full diff of your branch against its base, with line-level approval status and inline issues.

![Branch Comparison View](docs/imgs/ReviewPageScreenshot.png)

## Suggested Workflow

1. **Commit your work.** Commit any work done by an AI coding agent immediately. This gives you a reviewable unit of work.
2. **Review the commit in ProveAll.** Open the commit in ProveAll's editor. Go through the diff file by file, or line by line. Approve what you understand, raise issues on anything that looks wrong or unclear.
   - **File-level reviews:** Click the first circle next to the filename to review every line in the file at once.
   - **Line-level reviews:** Highlight specific lines in the editor, then click the blue circles in the gutter to review just those lines.
   - **Chunk-level reviews:** Click a diff chunk to select all lines in it and review them in a single action.
3. **Iterate in parallel.** While reviewing one commit, kick off your next AI prompt in the background. Commit or discard the result based on how the previous review went.
4. **Check the branch view.** Once all commits are reviewed, open the branch comparison to see the composite state: which files are fully approved, which still have open issues, and which lines need to be addressed before merging.

## Building from source

The release pipeline has been retired, but the project still builds locally:

```bash
npm install
npm run tauri:build   # unsigned local build
```

Signed and notarized builds required Apple credentials that have been revoked as part of
the sunset, so only unsigned local builds are reproducible from this repository now.
See [RELEASE.md](RELEASE.md) for a record of how releases were previously produced.
