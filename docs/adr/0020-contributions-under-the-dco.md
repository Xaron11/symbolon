# 0020. Contributions under the DCO

- **Status:** Accepted
- **Date:** 2026-11-26

## Context

The spec (§3) licenses Symbolon under `GPL-3.0-or-later` and accepts contributions under the same license, with no DCO and no CLA "for now", to be revisited before the first outside contribution (#19); §13 adds that no sign-off check or CLA is added unless the maintainer asks. The maintainer chose the DCO in #19. The repository is public, so that contribution can arrive at any time.

Inbound = outbound licensing already happens by default: GitHub's terms say contributions to a repository are made under its license. What it lacks is a record, per change, that the author had the right to contribute it, for example that it is not copied from code under an incompatible license or owned by an employer.

A CLA would also let the project relicense (dual licensing, a permissive `symbolon-core`). Symbolon has no plan to relicense; the GPL is meant to stay. A CLA costs every contributor a signing step and needs a bot or a signing service.

## Decision

1. Contributions are accepted under `GPL-3.0-or-later` with a Developer Certificate of Origin 1.1 sign-off (the `DCO` file at the repository root), as in the Linux kernel. There is no CLA and no copyright assignment: contributors keep their copyright.
2. Every commit of a pull request carries a `Signed-off-by: Name <email>` trailer whose email matches the commit's author (`git commit -s`). Merge commits are exempt, and so are bots (`…[bot]@users.noreply.github.com`), which cannot certify anything.
3. CI checks this on every pull request (`.github/scripts/check-dco.sh`, job `dco`, part of `ci-ok`); `just dco` runs the same check locally. A pull request with an unsigned commit cannot merge.
4. The maintainer's commits follow the same rule, including the release commit `just release` makes.
5. `CONTRIBUTING.md` explains how to sign off and how to fix a branch that is missing sign-offs.

## Consequences

- Each change on record names who certified its origin; the squash commit on `main` links to the pull request whose commits carry the sign-offs.
- Relicensing stays impossible without the consent of every contributor, as before. That is accepted: the copyleft is intended to be permanent.
- Contributors who forget `-s` must rewrite their branch (`git rebase --signoff`), which needs a force-push of their own branch.
- Revisit if the project ever wants to relicense, or if a contributor's employer requires a CLA instead.
