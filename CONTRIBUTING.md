# Contributing

Read [`AGENTS.md`](AGENTS.md): it is the working agreement for humans and AI agents alike (commands, code rules, workflow). In short:

- Enter the dev shell (`direnv allow` or `nix develop`); it installs the git hooks.
- Work on a `<type>/<short-name>` branch, sign off your commits (`git commit -s`, see [below](#license-of-contributions)) and open a PR. `main` is protected; PRs are squash-merged and the PR title, a [Conventional Commit](https://www.conventionalcommits.org), becomes the commit message.
- `just fmt lint test` must pass (the pre-push hook runs it). CI must be green.
- Decisions that change the architecture, the catalog schema or the spec in `docs/start.md` get an ADR in `docs/adr/`.
- Catalog schema changes need a forward-only migration and a fixture test (see `docs/schema.md`).
- Never commit secrets, personal paths, or real device serials; use fixtures.

## License of contributions

Contributions are accepted under [GPL-3.0-or-later](LICENSE), the license of this project, with a [Developer Certificate of Origin](DCO) sign-off ([ADR-0020](docs/adr/0020-contributions-under-the-dco.md)). There is no CLA, and you keep your copyright.

Sign off every commit with `git commit -s`. It adds a line like

```text
Signed-off-by: Ann Example <ann@example.org>
```

with your git name and email, certifying that you wrote the change or otherwise have the right to submit it under the project's license (read [`DCO`](DCO) for the exact terms). The email must match the commit's author. CI checks every commit of a pull request; `just dco` runs the same check locally. If you forgot, sign off the commits of your branch with `git rebase --signoff origin/main` and force-push your branch.
