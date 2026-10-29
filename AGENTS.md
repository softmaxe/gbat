## Versioning

"Bump the version" means a patch bump unless a specific version is given: increment the last component of `version` in `Cargo.toml` (`X.Y.Z` → `X.Y.Z+1`, e.g. `0.0.1` → `0.0.2`, `0.0.9` → `0.0.10`). Then run `cargo update --workspace` so `Cargo.lock` records the new version, and commit both files; CI builds with `--locked` and fails on a stale lockfile.

A release is cut by pushing the tag `v<version>` on the bump commit. `release.yml` rejects any tag that does not match the version in `Cargo.toml`.
