# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-03-03

### Modified
- Fixed issue with resolving shell's full PATH for built app.
- Fixed issue where LLM was trying to commit even if no changes were made.

## [0.3.0] - 2026-03-02

### Added
- Automatic issue resolution. Users can now send issues to an LLM to generate a fix.
  - Claude by default, configurable via settings                                                                                      
- Issue resolution events to the event log on the project page.

## [0.2.0] - 2026-03-01

### Added
- Automatic release support through Github actions.

### Modified
- Combine "branch review" and "issue review" into the same review mode for more intuitive UX.
- Filter and separate files related to specific issues and open issues in the filetree.


## [0.1.0] - 2026-02-24

### Added
- Initial commit and public release.
- Functionality to track issues related to specific lines from commit diffs.
