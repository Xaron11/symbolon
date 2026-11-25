# Decision records

An architecture decision record (ADR) captures one significant decision: the context, what was decided, and the consequences. ADRs are how this project deviates from or refines the [specification](../start.md) without losing track of why.

| ADR                                                | Decision                                                                |
| -------------------------------------------------- | ----------------------------------------------------------------------- |
| [0001](0001-stack-and-layout.md)                   | Workspace layout, toolchain pin, frontend stack                         |
| [0002](0002-nix-and-cross-compilation.md)          | Nix flake, rust-overlay, crane, zig cross-compilation for Android       |
| [0003](0003-android-agent-musl-spike.md)           | Static musl agent on Android; spike results; `sha256sum` fallback rules |
| [0004](0004-raw-path-encoding.md)                  | Raw path bytes, WTF-8 on Windows, path keys                             |
| [0005](0005-export-format-and-import-conflicts.md) | `.symbolon` file identity and import conflict policy                    |
| [0006](0006-core-feature-gates.md)                 | Cargo features in `symbolon-core`                                       |
| [0007](0007-development-workflow.md)               | Branches, hooks, CI, releases, project management                       |
| [0008](0008-documentation-and-editor-tooling.md)   | rustdoc and TSDoc rules, mdBook site, link checks, language servers     |
| [0009](0009-core-scan-design.md)                   | Catalog access, schema v1 choices, hashing and the scan pipeline        |
| [0010](0010-comparison-engine.md)                  | Comparison categories, move matching, hashing on demand, paged results  |
| [0011](0011-cli-conventions.md)                    | Catalog location, output formats, exit codes, Ctrl-C                    |
| [0012](0012-scan-lock.md)                          | One scan per location, enforced with an OS file lock                    |
| [0013](0013-imported-locations.md)                 | Imported locations, export contents, replacing data on import           |
| [0014](0014-imports-and-local-locations.md)        | Imports never replace a scanned local location without a choice         |

## Writing a new ADR

1. Copy [`0000-template.md`](0000-template.md) to the next free number with a short slug.
2. Fill in context, decision and consequences. Be specific enough that someone can check the code against it.
3. Add it to the table above and to `docs/SUMMARY.md`, and open it in the same PR as the change it describes.
4. To reverse a decision, write a new ADR and mark the old one `Superseded by NNNN`; do not rewrite history.
