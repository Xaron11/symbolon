# Testing

The bar comes from spec §11. This document says where each kind of test lives and how it runs.

## Running

| Command                  | What                                                                    |
| ------------------------ | ----------------------------------------------------------------------- |
| `just test`              | All Rust tests with nextest, then doctests.                             |
| `just test-ui`           | Frontend unit tests (vitest).                                           |
| `just coverage [--html]` | Instrumented nextest run with a line-coverage summary (cargo-llvm-cov). |
| `just bench`             | criterion benchmarks.                                                   |

nextest profiles live in `.config/nextest.toml`; CI uses `--profile ci` (no fail-fast, JUnit output, published as an artifact). On Linux CI runs the suite once, instrumented, and writes the coverage summary to the job page.

## Kinds of tests

| Kind                 | Where                                                         | Tools                                   |
| -------------------- | ------------------------------------------------------------- | --------------------------------------- |
| Unit                 | `#[cfg(test)] mod tests` next to the code                     | std, `pretty_assertions` where useful   |
| Property             | Same module or `tests/`; invariants of compare, pathkey, hash | `proptest`                              |
| Integration (core)   | `crates/symbolon-core/tests/`, generated trees in tempdirs    | `tempfile`                              |
| CLI end-to-end       | `crates/symbolon-cli/tests/`                                  | `assert_cmd`, `insta` snapshots         |
| Schema migrations    | `crates/symbolon-core/tests/migrations.rs` + fixtures         | one SQLite fixture per released version |
| Android logic        | Against a mock `AndroidTransport`; no device needed           | —                                       |
| Frontend             | `*.test.ts` next to the code                                  | vitest                                  |
| Benchmarks           | `crates/*/benches/`                                           | criterion                               |
| Real device (manual) | `just android-probe`; record results in the PR or an ADR      | —                                       |

These crates are added as dev-dependencies when the first test needs them (M1).

## Rules

- **Deterministic.** No sleeps, no wall-clock or network dependence, no reliance on directory iteration order. Seed randomness; proptest regressions files are committed.
- **Hermetic.** Tests create their own trees in tempdirs and never touch the user's home or real devices.
- **Edge cases are first-class.** Integration trees cover moves, renames, duplicates, Unicode NFC/NFD, case collisions, non-UTF-8 names (Unix), symlinks and cycles, permission errors, files changing during hashing, and empty files.
- **Failures and partial states.** Cancellation, crashes mid-scan (resume) and vanished devices are tested, not just happy paths. Write operations (M7) get fault-injection tests.
- **Bug fixes start with a failing test.**
- **Snapshots** (`insta`) are for stable, human-readable output (CLI tables, JSON reports). Review every snapshot change.
- **Coverage is a signal, not a target.** Look at what is uncovered in the diff; do not write tests just to move a number.
