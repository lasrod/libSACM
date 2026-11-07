# Architecture

This page describes the intended shape of libSACM. Parts that do not exist yet are
marked *planned*; the [functionality matrix](matrix/functionality-matrix.md) says what
is actually built. Decisions with lasting consequences are recorded as
[ADRs](adr/); interpretation of the standard is in
[sacm/decisions.md](sacm/decisions.md).

## Goals

1. **Faithful.** A typed model of the whole SACM 2.3 metamodel. Nothing conforming is
   lost on the way through.
2. **Honest.** Strict SACM 2.3 behaviour is distinct from tolerant interoperability
   behaviour, and every claim is backed by a verified matrix row.
3. **Easy to use.** Load, inspect, validate, edit and save in a few lines, with
   documentation that teaches SACM as it goes.
4. **Safe.** No `unsafe`, no panics on input, no XML external entities, bounded
   resource use.
5. **Neutral.** No tool-, notation- or layout-specific concepts. Any assurance-case
   tool can build on it.

## Workspace

```text
crates/sacm        library crate `sacm`        (published)
crates/sacm-cli    binary `sacm`               (published as `sacm-cli`)
xtask              repository automation        (not published)
```

The CLI depends on the library's public API only; anything the CLI needs that the API
cannot do is an API gap, not a reason to reach into internals.

## Library modules (planned)

```text
sacm
├── model        typed SACM elements, Document, ElementId, LangString/MultiLangString
├── xmi          read (tolerant | strict) and write (strict) — XML details private
├── validate     structural invariants + clause-level content constraints
├── diagnostic   Diagnostic, Severity, Code, Location
├── edit         operations, previews, delete policies, mutation results
└── compare      semantic equality and differences between documents
```

### Data flow

```text
            ┌──────────── xmi::read ───────────┐
 bytes ───► │ XML parse (no DTD) → typed model │ ──► Document + Vec<Diagnostic>
            └──────────────────────────────────┘
 Document ──► validate::validate ──► Vec<Diagnostic>
 Document ──► edit::apply(op)     ──► Result<MutationResult, EditError>   (atomic)
 Document ──► xmi::write(strict)  ──► Result<bytes, WriteError>           (deterministic)
```

### Model (proposed — confirmed by the architect in the first model slice)

- A `Document` owns one root interchange package and every element under it.
- Elements are stored by `ElementId` in an insertion-ordered store; containment is an
  ordered list of child ids per parent; references are ids. Users navigate with typed
  views (`doc.claim(id)`, `package.argument_elements()`), never raw storage.
- Typed views expose SACM attributes as methods; the shared `ModelElement` and
  `SACMElement` features are reachable uniformly on every element.
- Preserved-but-untyped content (tolerant mode) is held separately, attached to its
  parent position, so strict save can find and refuse it.

### Validation layers

Structural invariants hold after every mutation; content constraints are reported by
`validate()` (decision D-VAL-LAYER). Severity follows the clause's modal verb (D-SEV).

### Editing

Operations are values (`CreateClaim { .. }`, `DeleteElement { .. }`) applied to a
document. Each is checked fully before anything changes (D-ATOMIC). Destructive
operations have a `preview` that lists consequences and carries the document revision
it was computed against (D-PREVIEW).

## Quality gates

Every change passes: rustfmt, clippy (pedantic, `-D warnings`), tests on Linux,
Windows and macOS, the MSRV build, rustdoc with `-D warnings`, `cargo deny`, and
`cargo xtask matrix check`. See `.github/workflows/ci.yml`.

## How the code gets written

Development runs as a loop of small slices over the functionality matrix, each driven
through research → design → failing tests → implementation → independent verification
by dedicated agent roles. See [loop.md](loop.md).
