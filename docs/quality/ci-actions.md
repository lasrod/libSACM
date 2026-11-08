# Third-party CI actions

This is how the workflows in `.github/workflows/` use actions that GitHub does not own (LIB-QA-009). The `lib_qa_009_*` tests in `xtask/tests/repo_checks.rs` check it.

## The rule

- **Third-party actions are pinned by commit.** Every `uses` whose owner is not `actions` (compared without regard to case) has the form `<owner>/<repo>@<40 lowercase hex digits>`, followed by a `# <ref>` comment naming the tag or branch the SHA was resolved from:

  ```yaml
  - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
    with:
      toolchain: stable
  ```

- **GitHub's own `actions/*` stay on major tags,** such as `actions/checkout@v7`. Dependabot updates them.
- **Why:** a tag such as `@v2` moves, so the code that runs can change without a change in this repository. That matters because `EmbarkStudios/cargo-deny-action` splices its inputs unquoted into the cargo-deny command line, so an input can change what cargo-deny checks (Verification 0002, rounds 3 and 4). A pin fixes the code, and the reviewed table fixes which inputs the repository may pass to it.

## The current pins

The source of truth is the `REVIEWED_ACTIONS` table in `xtask/tests/repo_checks.rs`. Each entry there also says how each input reaches the action's command line. This copy is for reading only.

| Action | Ref | Commit | Inputs the repository may pass |
|---|---|---|---|
| `EmbarkStudios/cargo-deny-action` | `v2.1.1` (= `v2`) | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` | `command`, `arguments`, `command-arguments`, `manifest-path`, `log-level` |
| `Swatinem/rust-cache` | `v2.9.3` (= `v2`) | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` | none |
| `dtolnay/rust-toolchain` | `master` | `683b588c487a69d01f6c6ddf40f036ff682cc2c8` | `toolchain`, `components` |

- **cargo-deny-action:** the values of its five inputs are LIB-QA-003's to judge. `rust-version`, `credentials`, `ssh-key`, `ssh-known-hosts` and `use-git-cli` are not reviewed inputs, because the action splices them into its arguments too.
- **rust-toolchain:** at `master` the action cannot take the toolchain from the ref name, as it does at `@stable`, so every use passes `toolchain` explicitly: `stable`, or `"1.85"` in the MSRV job.

## Reviewing a bump

A bump changes the SHA, so it fails `lib_qa_009_*` with `[action-unreviewed]` until someone reviews it. A Dependabot PR for a third-party action therefore always fails at first. To review it:

1. **Resolve the new ref to its commit SHA.** A lightweight tag points at the commit. An annotated tag points at a tag object, which must be dereferenced to its commit:

   ```sh
   gh api repos/OWNER/REPO/git/ref/tags/TAG    # .object.type is "commit" or "tag"
   gh api repos/OWNER/REPO/git/tags/SHA        # for "tag": .object.sha is the commit
   ```

   For a branch, use `gh api repos/OWNER/REPO/git/ref/heads/BRANCH`. The pin must be the commit SHA, not the tag object's.
2. **Read `action.yml` at that SHA:** `gh api repos/OWNER/REPO/contents/action.yml?ref=SHA`. For a docker or composite action, read the entrypoint or script it runs as well.
3. **Check each input the repository passes.** Follow it to the action's command line: is it quoted, passed through `env`, or spliced into a command unquoted?
4. **Check that nothing else changes what runs:** no new input, new default or new `args` entry that alters the command.
5. **Update `REVIEWED_ACTIONS` and the workflow pins in one commit.** The entry gets the new SHA, the ref it was resolved from, and the inputs with their comments. The pin gets the matching `# <ref>` comment.

The table is in the test engineer's scope and the workflows are the lead's, so the change is routed to both. A Dependabot PR already changes the pins, so the reviewed entry is added to that PR, and the check passes once it lands there.

## What is not checked here

- **The `# <ref>` comment:** the YAML parser drops comments, so the test cannot see it. A reviewer checks it against `resolved_from`.
- **The inputs against `action.yml`:** the test has no network. A reviewer checks them at the pinned SHA with `gh api`.
- **Third-party reusable workflows:** they are refused outright. A job-level `uses` names a workflow by path, which never has the pinned form, so it is `[action-unpinned]`, even at a full SHA. The same goes for a local `./` workflow. GitHub's own `actions/*` ones are not judged.
