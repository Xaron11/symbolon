# Writing documentation

Documentation is part of a change, not a follow-up: a PR that changes behavior, an interface, the schema or the workflow updates the docs in the same PR.

## Where things go

| Kind                                   | Place                                             | Audience                     |
| -------------------------------------- | ------------------------------------------------- | ---------------------------- |
| What the code does and how to call it  | rustdoc comments (`///`, `//!`)                   | developers using the API     |
| Concepts, architecture, how-tos        | this book (`docs/`)                               | users and contributors       |
| Why a decision was made                | an ADR (`docs/adr/`)                              | future maintainers           |
| Rules for contributors and AI agents   | `AGENTS.md`                                       | anyone changing the code     |
| What changed in a release              | `CHANGELOG.md`, generated from commit messages    | users                        |
| Why a particular line is the way it is | a code comment (only when the code cannot say it) | the next reader of that line |

Do not duplicate: link to the place that owns the information.

## Rust API documentation

rustdoc lints are part of the workspace lints and CI builds docs with warnings denied.

- **Every public item is documented** (`missing_docs`). Each crate starts with a `//!` overview that says what the crate is for and where to start.
- **First line:** one sentence saying what the item is or does, in the present tense. Details go in later paragraphs.
- **Sections**, in this order when they apply: `# Errors` (every function returning `Result`, listing the conditions), `# Panics` (should be rare), `# Examples`.
- **Examples are doctests.** They compile and run in CI, so they cannot go stale. Prefer a short realistic example over prose. Hide setup lines with `#`.
- **Link with intra-doc links**: ``[`Scanner`]``, ``[`Query::compare`]``. Broken links fail the build.
- **Say what matters to callers:** units, invariants, cost (does it read files? hit the database?), thread-safety and what happens on error. Do not restate the signature.

````rust
/// Computes the BLAKE3 hash of everything `reader` yields.
///
/// Reads until EOF and returns the digest with the number of bytes hashed.
///
/// # Errors
///
/// Returns the I/O error if reading fails; a partial digest is never returned.
///
/// # Examples
///
/// ```
/// # use symbolon_core::hash::blake3_reader;
/// let (digest, len) = blake3_reader(&b"abc"[..])?;
/// assert_eq!(len, 3);
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn blake3_reader(reader: impl Read) -> io::Result<(Hash, u64)>
````

## Frontend documentation

Exported functions, types and components in `ui/src/lib` have TSDoc comments (`/** … */`), checked by ESLint. Describe behavior and edge cases; the types already say the shapes.

## The book

- Source is Markdown in `docs/`; the table of contents is `docs/SUMMARY.md`. Add every new page there.
- Diagrams are [Mermaid](https://mermaid.js.org) code blocks (` ```mermaid `), rendered in the book and readable as text on GitHub.
- Write for someone new to the project: start with what and why, then how. Prefer tables and examples over long prose.
- Mark things that do not exist yet as _(planned)_ with their milestone, and remove the mark when they land.

`just docs-serve` serves the book with live reload. `just docs-check` builds the book and the API reference and checks every internal link and anchor; CI runs the same check, and rumdl lints Markdown structure in the pre-commit hook.
