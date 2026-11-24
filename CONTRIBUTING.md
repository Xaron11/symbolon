# Contributing

Read [`AGENTS.md`](AGENTS.md): it is the working agreement for humans and AI agents alike (commands, code rules, workflow). In short:

- Enter the dev shell (`direnv allow` or `nix develop`); it installs the git hooks.
- Work on a `<type>/<short-name>` branch and open a PR. `main` is protected; PRs are squash-merged and the PR title, a [Conventional Commit](https://www.conventionalcommits.org), becomes the commit message.
- `just fmt lint test` must pass (the pre-push hook runs it). CI must be green.
- Decisions that change the architecture, the catalog schema or the spec in `docs/start.md` get an ADR in `docs/adr/`.
- Catalog schema changes need a forward-only migration and a fixture test (see `docs/schema.md`).
- Never commit secrets, personal paths, or real device serials; use fixtures.

## License of contributions

By submitting a contribution you agree that it is licensed under [GPL-3.0-or-later](LICENSE), the license of this project. There is no CLA and no sign-off requirement.
