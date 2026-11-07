## CI and merging

For an authorized PR merge, wait for the checks required by the current GitHub branch rules and for GitHub's merge requirements to be satisfied. Use `gh pr checks <pr> --required` to inspect the required checks; an empty result is not proof that checks passed. Keep the branch up to date when required and recheck after pushing changes.

Once those requirements are satisfied, proceed with the merge while optional checks such as CodeQL continue asynchronously. Report optional checks that are still pending as pending, and investigate any known failures or security findings rather than describing the entire CI run as successful. Wait for all checks when the user explicitly requests that scope.

Validate PRs natively on macOS and Linux so platform-specific code and HID backends are checked. Keep Clippy and tests together in each platform job, run formatting once, and preserve `Check` as the required aggregate gate that passes only when both platforms succeed. Keep CodeQL enabled as an asynchronous check; add caching or further job splitting only when measured runtime justifies their overhead.

## Versioning

"Bump the version" means a patch bump unless a specific version is given: increment the last component of `version` in `Cargo.toml` (`X.Y.Z` → `X.Y.Z+1`, e.g. `0.0.1` → `0.0.2`, `0.0.9` → `0.0.10`). Then run `cargo update --workspace` so `Cargo.lock` records the new version, and commit both files; CI builds with `--locked` and fails on a stale lockfile.

A release is cut by pushing the tag `v<version>` on the bump commit. `release.yml` rejects any tag that does not match the version in `Cargo.toml`.

Release when requested, batching changes between releases. Routine PR merges do not need a version bump or release tag. Keep binary packaging and Homebrew installation checks in the release workflow.
