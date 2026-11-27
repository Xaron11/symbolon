# Testing

The bar comes from spec §11. This document says where each kind of test lives and how it runs.

## Running

| Command                  | What                                                                     |
| ------------------------ | ------------------------------------------------------------------------ |
| `just test`              | All Rust tests with nextest, then doctests.                              |
| `just test-ci`           | What CI runs: on Linux instrumented, with the ignored acceptance tests.  |
| `just test-ui`           | Frontend unit tests (vitest).                                            |
| `just coverage [--html]` | Instrumented nextest run with a line-coverage summary (cargo-llvm-cov).  |
| `just bench`             | criterion benchmarks; results and method in [Benchmarks](benchmarks.md). |
| `just test-acceptance`   | Slow acceptance tests (100k-file scan); CI runs them on Linux.           |

nextest profiles live in `.config/nextest.toml`; `just test-ci` uses `--profile ci` (no fail-fast, JUnit output, published as an artifact). On Linux CI runs the suite once, instrumented, and writes the coverage summary to the job page.

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
- **Frontend tests share state.** For speed, vitest runs the test files of a worker in one jsdom instead of a fresh one per file. The setup file reloads the app's own modules for each file, so `vi.mock` works as usual; a test that changes globals, the DOM or a package's state restores it (`afterEach`, `vi.restoreAllMocks()`), or the leak fails another file.
- **Edge cases are first-class.** Integration trees cover moves, renames, duplicates, Unicode NFC/NFD, case collisions, non-UTF-8 names (Unix), symlinks and cycles, permission errors, files changing during hashing, and empty files.
- **Failures and partial states.** Cancellation, crashes mid-scan (resume) and vanished devices are tested, not just happy paths. Write operations (M7) get fault-injection tests.
- **Bug fixes start with a failing test.**
- **Property tests** run a fixed number of cases in CI (200 for the comparison model). Run more locally after touching the engine: `PROPTEST_CASES=3000 cargo nextest run --release -p symbolon-core --test compare_model`. Failing inputs are shrunk and saved under `proptest-regressions/`; commit those files.
- **Snapshots** (`insta`) are for stable, human-readable output (CLI tables, JSON reports). Review every snapshot change.
- **Mutation testing** (`just mutants`, uses `cargo-mutants`) changes the code, for example flipping a comparison, and checks that a test fails. Run it after substantial changes to core logic, then add a test for every surviving mutant that changes behavior. Some survivors are equivalent (no observable difference, such as a buffer size) or platform code that only Windows CI compiles; note those in the PR. The first run (comparison, hashing, paths; 319 mutants) caught 220 and led to tests for the behaviors behind the rest.
- **Coverage is a signal, not a target.** Look at what is uncovered in the diff; do not write tests just to move a number.
