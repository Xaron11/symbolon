# Releasing

## Versioning

- One [SemVer](https://semver.org) version for all crates and the frontend, set in `[workspace.package]` in `Cargo.toml`.
- `0.x` until the first stable release; breaking changes bump the minor version.
- Each milestone ends with a minor release: M0 is `v0.1.0`, M1 is `v0.2.0`, and so on. Fixes in between are patch releases.
- The catalog schema has its own version, independent of the app version ([Catalog schema](../schema.md)).

The next version is derived from the Conventional Commits since the last tag: `feat` bumps the minor version, `fix` and others the patch version.

## Cutting a release

1. On an up-to-date `main`, run `just release` (or `just release 0.3.0` to choose the version). It bumps the versions, regenerates `CHANGELOG.md`, pushes `release/vX.Y.Z` and opens the PR `chore(release): vX.Y.Z`.
2. Review the changelog in the PR and merge it once CI is green.
3. Run `just tag`. It checks that `main`'s last commit is that release commit, creates an SSH-signed annotated tag `vX.Y.Z`, verifies it against `.github/allowed_signers` and pushes it.
4. The release workflow then:
   - verifies the tag signature against the allowed signers on `main`;
   - checks that the tag matches the workspace version;
   - builds the CLI for Linux and Windows and the Android agent, after checking in every job that the tag still points at the verified commit;
   - attests the build provenance of every archive;
   - publishes a GitHub release with `SHA256SUMS` and the changelog section.

   `0.x` releases are marked pre-release. Releases are immutable: once published, assets and the tag cannot change.

If the release workflow fails after the tag exists (for example because of a bug in the workflow itself), fix it on `main` and re-run it for the existing tag: `gh workflow run release.yml -f tag=vX.Y.Z`. The run uses the workflow from `main`, builds the tagged source and verifies the tag exactly as a push would. Do not delete or move a published tag.

Only repository admins can create `v*` tags, and the signing key must be listed in `.github/allowed_signers`. Never create tags or releases another way.

## Verifying a download

Each archive has a build-provenance attestation signed by the release workflow:

```sh
gh attestation verify symbolon-v0.4.0-x86_64-unknown-linux-gnu.tar.gz --repo Xaron11/symbolon
```

The attestation confirms the archive was built by this repository's release workflow from the tagged commit. `sha256sum -c SHA256SUMS` only checks the files against a list published on the same page.

## After a release

- Close the milestone and move its open issues forward.
- Check the release page: binaries, checksums, notes and the source-tag link required by the GPL.
