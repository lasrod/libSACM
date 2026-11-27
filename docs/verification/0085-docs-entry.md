# Verification 0085 — The style guide and the entry documents

- Date: 2026-11-27
- Row verdict: LIB-DOC-009: PASS
- Row verdict: LIB-DOC-006: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 1bf2889 (first round 4329246), plus the verifier's diagnostic.rs wording; evidence: hand mutations and gate logs in `target/scratch/v0085/`
- Slice brief: [docs/slices/0085-docs-entry.md](../slices/0085-docs-entry.md)

## Result

The verifier read the crate root, the three READMEs, docs/architecture.md and docs/style.md
against the guide, checked every present-tense claim against the matrix and the CLI's facts
against the binary, traced every removed fact and doctest to where it still lives, and
mutated the style check. In the first round two claims were false; the fix round applied the
verifier's texts and added five controls to the check.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-DOC-006, LIB-DOC-009 | A tolerant read was said to keep "what strict refuses"; it still refuses some input. | "keeps most of", with what it still refuses. |
| Blocking | LIB-DOC-006, LIB-DOC-009 | "Nothing dropped", though comments and processing instructions are not kept. | Said so in the crate root and both READMEs. |
| Non-blocking | LIB-DOC-006, LIB-DOC-009 | Wording: the Atomic bullet, the feature bullets, the style guide's README cell, architecture.md, table paths, the diagnostic module's strict-write sentence. | Fixed with the verifier's text. |
| Non-blocking | LIB-DOC-009 | Five checker mutants survived (fence length and info string, two backticks, empty revision, a one-letter Mermaid run). | Five controls. |
| Non-blocking | LIB-DOC-009 | The check can be passed with a long block quote, a lazy continuation or a `<br>`. | docs/follow-ups.md; the verifier reads for them. |
