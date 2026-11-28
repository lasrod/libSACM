# Releasing

How to cut a release of `sacm` and `sacm-cli`, the first (0.1.0) included (LIB-QA-005).
Both crates share one version, `X.Y.Z` below.

**The publish, the tag and the GitHub release wait for the maintainer's explicit go for
that version.** Steps 1 to 4 can be prepared without it.

## Steps

### 1. Bump the version

In the root `Cargo.toml`, set the same version in both places:

```toml
[workspace.package]
version = "X.Y.Z"

[workspace.dependencies]
sacm = { path = "crates/sacm", version = "X.Y.Z" }
```

Then run `cargo update -w` to update `Cargo.lock`.

**Check:** `cargo test -p xtask --test release` passes. It checks that the two versions agree.

### 2. Date the CHANGELOG

In `CHANGELOG.md`, turn `## [Unreleased]` into `## [X.Y.Z] - YYYY-MM-DD` with today's
date. Above it, open a new empty section:

```markdown
## [Unreleased]

## [X.Y.Z] - YYYY-MM-DD
```

**Check:** the `X.Y.Z` section lists every user-visible change since the last release, with
breaking changes under **Breaking**, and the `[Unreleased]` section reads as the release's
notes: no entry says something is refused or missing "until" a row that is verified, and
nothing lists a change between unreleased states.

### 3. Run the gate and the dry run

From the release after 0.1.0 on, first run the full mutation run on the release candidate
(`gh workflow run mutants.yml --ref main`), and kill or triage every survivor in
`docs/quality/mutants-triage.md` before going on (ADR 0006 amendment 6). 0.1.0 is
published without it, on CI's per-PR evidence (the maintainer, 2026-11-17).

Run the full gate in [CONTRIBUTING.md](../CONTRIBUTING.md#the-gate), then:

```sh
cargo publish --dry-run --workspace --locked
```

**Check:** every command passes. The dry run packages each crate and builds it from its
packaged sources alone. `cargo package --list -p sacm` and `cargo package --list -p sacm-cli`
show only `Cargo.toml`, `README.md`, `LICENSE`, the files under `src/` and the files cargo
adds itself. There must be nothing from `tests/` and no test data. The unit tests and
doctests in `src/` ship, but they run only from the repository: the published manifest
drops the `test-support` dev-dependency that three doctests use, and the unit tests read
test data that is not packaged. The dry run warns `ignoring test … as
… is not included in the published package` once for each integration test. That is
expected: the tests stay in the repository.

The public API check (LIB-QA-006, the required `semver checks` job):

- `cargo xtask semver` passes; on the release commit the version bump decides.

### 4. Merge the release commit

Commit the bump and the CHANGELOG as `chore: release X.Y.Z`. Merge it to `main` through a
pull request, as usual.

**Check:** CI is green on `main` at the merge commit.

### 5. Tag the release

On `main`, once `git log -1` shows the merge of the release pull request (if anything was
merged after it, stop and start again from step 1):

```sh
git switch main && git pull
git tag -a vX.Y.Z -m "sacm X.Y.Z"
git push origin vX.Y.Z
```

**Check:** `git show vX.Y.Z` shows an annotated tag on the release commit.

### 6. Publish

If the go comes on a later day than the step 4 merge, re-date the CHANGELOG section in a
new release PR first. Publish `sacm` first, because `sacm-cli` depends on it:

```sh
cargo publish -p sacm
```

Wait until `sacm X.Y.Z` is on <https://crates.io/crates/sacm>, then:

```sh
cargo publish -p sacm-cli
```

**Check:** both versions are on crates.io, and the `sacm` docs build on docs.rs.

### 7. Create the GitHub release

Save the `## [X.Y.Z]` section of `CHANGELOG.md`, without its heading, to a file such as
`notes.md`, then:

```sh
gh release create vX.Y.Z --verify-tag --title "sacm X.Y.Z" --notes-file notes.md
```

**Check:** the release appears at <https://github.com/lasrod/libSACM/releases>, on tag
`vX.Y.Z`.

## When something fails

- **Before the publish (steps 1 to 5):** fix the problem and start again from the failed
  step. To move a pushed tag that no one has published from, delete it with
  `git push --delete origin vX.Y.Z` and `git tag -d vX.Y.Z`.
- **A mistake in a published version:** never re-publish it. Fix it on `main` and
  release the next patch version. Yanking does not fix a version; yank one only if new
  builds must not pick it (for example, it loses data), and only after the fix is out.
- **`sacm` is published but `sacm-cli` fails:** fix the problem and publish `sacm-cli`
  alone at the same version, if its sources are unchanged. If they had to change,
  release the next patch version of both crates.
