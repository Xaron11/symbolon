# Introduction

> _symbolon_ (Greek σύμβολον): a token broken in two, each party keeping a half, later matched to prove identity.

Symbolon answers one question: **do these locations match, and if not, how exactly do they differ?**

It scans two or more locations (directories, mounted disks, pendrives, Android phones over ADB), hashes file contents into a local catalog, and reports for any set of them which files are identical, missing, changed, moved or renamed, and duplicated. The catalog persists, so a device can be compared while it is unplugged, and rescans only read what changed.

**Status:** pre-alpha. The `symbolon` command line scans local locations and Android phones over ADB into a catalog, compares them, finds duplicates and exports locations to compare elsewhere ([Command-line usage](cli.md)).

The desktop app (Linux first) does the same read-only work without the command line, on the same catalog: **Locations** to add folders and phones and scan them, browsing a location while it is scanned; **Scans** to follow running scans and see past ones; **Compare** to pick locations, see the result at once while unverified files are hashed in the background, and drill down by folder or list ([Design system](design/index.md)). The [specification](start.md) describes the whole plan and its milestones.

## How this book is organized

| Part               | Read it to                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------ |
| **Using Symbolon** | understand the vocabulary (locations, catalog, hash tiers, comparison categories) and use the tools.   |
| **Design**         | see what is being built and why: the specification, architecture, catalog schema and decision records. |
| **Development**    | set up the environment and editor, test, document and release.                                         |
| **API reference**  | browse the Rust API generated from the code.                                                           |

The working agreement for contributors (and AI agents) is `AGENTS.md` at the repository root; this book explains the background behind it.
