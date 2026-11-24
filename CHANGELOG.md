# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-11-24

### Added

- **core:** Add hash module with streaming and partial digests (#27)
- **core:** Add pathkey module for lossless paths and matching keys (#28)
- **catalog:** Add SQLite catalog with schema v1, migrations and locations (#29)
- **core:** Add Source trait and LocalSource walker (#30)
- **scan:** Add Scanner with events, cancel and pause, incremental and resumable scans (#31)
- **core:** Add Query for browsing, search, duplicates and scan history (#32)

### Changed

- **scan:** Skip index writes for unchanged entries; add 100k acceptance test and benchmarks (#33)

### Fixed

- **docs:** Keep the API reference when the book rebuilds (#26)

## [0.1.0] - 2026-11-24

### Added

- **agent:** Add PoC probe binary for the Android spike
- **app:** Add Tauri 2 and SvelteKit skeleton

### Fixed

- **app:** Record pnpm package manager in lockfile
