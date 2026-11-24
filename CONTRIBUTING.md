# Contributing

- Enter the dev shell (`direnv allow` or `nix develop`) and run `just test lint deny` before opening a PR.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org) (`feat(core): ...`, `fix(cli): ...`). The changelog is generated from them with `just changelog`.
- Decisions that change the architecture, the catalog schema, or the spec in `docs/start.md` get an ADR in `docs/adr/` (copy `0000-template.md`).
- Catalog schema changes need a forward-only migration and a test that opens databases from every previous schema version (see `docs/schema.md`).
- Never commit secrets, personal paths, or real device serials; use fixtures.

## License of contributions

By submitting a contribution you agree that it is licensed under [GPL-3.0-or-later](LICENSE), the license of this project. There is no CLA and no sign-off requirement.
