## Agent skills

### Issue tracker

Issues live in GitHub Issues for `softmaxe/gbat`, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the five default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `CONTEXT.md` plus `docs/adr/`. See `docs/agents/domain.md`.

## Versioning

"Bump the version" means a patch bump unless a specific version is given: increment the last component of `version` in `Cargo.toml` (`X.Y.Z` → `X.Y.Z+1`, e.g. `0.0.1` → `0.0.2`, `0.0.9` → `0.0.10`). Then run `cargo update --workspace` so `Cargo.lock` records the new version, and commit both files; CI builds with `--locked` and fails on a stale lockfile.

A release is cut by pushing the tag `v<version>` on the bump commit. `release.yml` rejects any tag that does not match the version in `Cargo.toml`.
