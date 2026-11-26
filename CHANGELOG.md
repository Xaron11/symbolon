# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.0] - 2026-11-26

### Added

- **catalog:** Store symlink targets (#137)
- **android:** Symlink targets from the agent and the fallback (#138)
- **compare:** Directories, symlinks and kind clashes (#139)
- **Breaking:** **cli:** Show directories and symlinks in diff (#140). Diff's JSON and CSV output names the count and the row list "entries" instead of "files"

## [0.6.0] - 2026-11-25

### Added

- **core:** AndroidTransport, SystemAdb and device discovery (#120)
- **core:** Hash where the files are (#121)
- **agent:** The on-device scanner built on core (#122)
- **core:** Scan a phone through the agent, with the toybox fallback (#124)
- **catalog:** Phone locations and recognizing the phone (#125)
- **cli:** Devices, phone locations and finding the agent (#126)

### Changed

- **android:** Hash with four threads on phones (#129)

## [0.5.0] - 2026-11-25

### Added

- **core:** Filesystem fingerprints on Linux (#104)
- **catalog:** Record fingerprints and locate moved locations (#105)
- **cli:** Change a location's label, root or hash policy (#106)
- **cli:** Follow moved locations and confirm other devices (#107)

## [0.4.0] - 2026-11-25

### Added

- **scan:** Allow only one scan per location at a time (#71)
- **scan:** Never let a slow event consumer stall a scan (#72)
- **cli:** Command skeleton, output formats and location management (#73)
- **cli:** Scan locations with progress and Ctrl-C (#74)
- **cli:** Diff locations with filters and formats (#75)
- **catalog:** Export a location and import it with the ADR-0005 conflict policy (#76)
- **cli:** Export and import commands (#77)
- **cli:** List duplicates within locations (#78)
- **cli:** Shell completions and man pages (#79)

### Fixed

- **catalog:** Never replace a scanned local location's data without a choice (#88)
- **catalog:** Validate imported entries and Windows separators in paths (#90)
- **core:** Private catalog files and temporary copies, algorithm-guarded digest stores (#93)
- **cli:** Escape control characters in tables and warnings (#89)
- **scan:** Open files beneath the root without following symlinks (#91)

## [0.3.0] - 2026-11-24

### Added

- **compare:** Classify paths across sides with algorithm checks and key options (#51)
- **compare:** Match moved and renamed files across sides (#52)
- **compare:** Hash on demand with size pruning and partial digests (#53)
- **compare:** Filter rows and list duplicates within sides (#54)

### Changed

- **compare:** Compare two 200k-entry locations in 1.6 s (#56)

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
