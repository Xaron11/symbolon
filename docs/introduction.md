# Introduction

> _symbolon_ (Greek σύμβολον): a token broken in two, each party keeping a half, later matched to prove identity.

Symbolon answers one question: **do these locations match, and if not, how exactly do they differ?**

It scans two or more locations (directories, mounted disks, pendrives, Android phones over ADB), hashes file contents into a local catalog, and reports for any set of them which files are identical, missing, changed, moved or renamed, and duplicated. The catalog persists, so a device can be compared while it is unplugged, and rescans only read what changed.

**Status:** pre-alpha. The scaffolding (milestone M0) is done; scanning arrives with M1. The [specification](start.md) describes the whole plan and its milestones.

## How this book is organized

| Part               | Read it to                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------ |
| **Using Symbolon** | understand the vocabulary (locations, catalog, hash tiers, comparison categories) and use the tools.   |
| **Design**         | see what is being built and why: the specification, architecture, catalog schema and decision records. |
| **Development**    | set up the environment and editor, test, document and release.                                         |
| **API reference**  | browse the Rust API generated from the code.                                                           |

The working agreement for contributors (and AI agents) is `AGENTS.md` at the repository root; this book explains the background behind it.
