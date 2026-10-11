## Agent skills

### Issue tracker

Issues live in GitHub Issues for `softmaxe/gbat`, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the five default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `CONTEXT.md` plus `docs/adr/`. See `docs/agents/domain.md`.

## Versioning

"Bump the version" (更新版本) means a patch bump: add 0.0.1 to the current version in `Cargo.toml` (e.g. `0.0.1` → `0.0.2`), unless a specific version is given.
