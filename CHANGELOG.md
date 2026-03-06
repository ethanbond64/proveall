# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-03-06

### Added
- In-app updater functionality and settings to automatically install updates.

## [0.4.1] - 2026-03-05

### Changed
- Improved UI usability for line reviews: single click on a diff chunk will select the full chunk for review.
- Minor UI improvements in the project page and review page.

### Fixed
- Bug where bulk commit reviews did not save properly if it was the first review on the branch.

## [0.4.0] - 2026-03-04

### Added
- Ability to bulk review multiple commits by selecting a more recent commit on the project page.
- Ability to line review lines that were not touched in the diff in a commit review.

### Changed
- Line level review now will apply the review to all lines selected by the cursor (or just the single line if none).
- Double click commit chunks to select the full commit chunk for review.
- Blue outline to signify which lines are selected for review. 

### Fixed
- Issue with resolving shell's full PATH for built app.
- LLM was attempting to commit even if no changes were made.

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
