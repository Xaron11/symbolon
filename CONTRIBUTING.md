# Contributing

Read [`AGENTS.md`](AGENTS.md): it is the working agreement for humans and AI agents alike (commands, code rules, workflow). In short:

- Enter the dev shell (`direnv allow` or `nix develop`); it installs the git hooks.
- Work on a `<type>/<short-name>` branch and open a PR. `main` is protected; PRs are squash-merged and the PR title, a [Conventional Commit](https://www.conventionalcommits.org), becomes the commit message.
- `just fmt check` must pass (it runs what CI checks). CI must be green.
- Decisions that change the architecture, the catalog schema or the spec in `docs/start.md` get an ADR in `docs/adr/`.
- Catalog schema changes need a forward-only migration and a fixture test (see `docs/schema.md`).
- Never commit secrets, personal paths, or real device serials; use fixtures.

## License of contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Symbolon by you, as defined in the Apache-2.0 license, shall be dual licensed under the [Apache License, Version 2.0](LICENSE-APACHE) and the [MIT license](LICENSE-MIT), without any additional terms or conditions. There is no CLA and no sign-off requirement ([ADR-0020](docs/adr/0020-mit-or-apache-license.md)).
