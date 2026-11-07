## What and why

<!-- One paragraph. Link the issue if there is one. -->

## Matrix rows

<!-- Rows this change serves and their status change, e.g. SACM23-ARG-007: tests-failing → implemented -->

## Checklist

- [ ] Tests first, named after their matrix rows; negative cases included
- [ ] `cargo xtask matrix render` run; `cargo xtask matrix check` passes
- [ ] Public items documented; examples are doctests
- [ ] No conforming SACM data can be dropped silently (preserved with a diagnostic, or refused)
- [ ] CHANGELOG `[Unreleased]` updated for user-visible changes
- [ ] Breaking change? Listed under **Breaking** in the CHANGELOG
