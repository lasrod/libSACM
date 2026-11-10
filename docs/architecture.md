# Architecture

This is the planned shape of libSACM. Almost none of it exists yet; the
[functionality matrix](matrix/functionality-matrix.md) says what is built. Lasting
decisions are in the [ADRs](adr/), and interpretations of the standard in
[sacm/decisions.md](sacm/decisions.md).

## Goals

1. **Faithful:** a typed model of the whole SACM 2.3 metamodel. Nothing conforming is
   lost on load and save.
2. **Honest:** strict SACM 2.3 behaviour is kept apart from tolerant interoperability,
   and every claim is backed by a verified matrix row.
3. **Easy to use:** load, inspect, validate, edit and save in a few lines.
4. **Safe:** no `unsafe`, no panics on input, no XML external entities, bounded
   resource use.
5. **Neutral:** no tool-, notation- or layout-specific concepts.

## Workspace

```text
crates/sacm        library crate `sacm`     (published)
crates/sacm-cli    binary `sacm`            (published as `sacm-cli`)
xtask              repository tooling       (not published)
```

The CLI uses only the library's public API. Anything it cannot do through that API is
an API gap.

## Library modules (planned)

```text
sacm
├── model        typed SACM elements, Document, ElementId, LangString/MultiLangString
│                (built: Document, ElementId, SacmElement, element kinds, element
│                names, LangString, MultiLangString, AssertionDeclaration)
├── xmi          read (tolerant | strict) and write (strict | tolerant); XML details stay private
├── validate     structural invariants and clause-level constraints
├── diagnostic   Diagnostic, Severity, Code, Location
├── edit         operations, previews, delete policies, mutation results
└── compare      semantic equality and differences
```

```text
bytes    ──► xmi::read            ──► Document + Vec<Diagnostic>
Document ──► validate::validate   ──► Vec<Diagnostic>
Document ──► edit::apply(op)      ──► Result<MutationResult, EditError>   (atomic)
Document ──► xmi::write(strict)   ──► Result<bytes, WriteError>           (deterministic; refuses invalid models)
```

- **Model** ([ADR 0010](adr/0010-document-model.md)): a `Document` owns its root
  packages, in document order, and every element under them. Elements live in a
  private arena with an `ElementId` index; containment is an ordered list of children,
  and references are ids. Users work through read-only views that borrow the document
  (`SacmElement<'a>`, later typed views such as `Claim<'a>`), never raw storage.
  Content tolerant mode cannot type is kept beside its parent, so strict save can find
  and refuse it.
- **Validation:** structural invariants hold after every edit; content constraints are
  reported by `validate()` (D-VAL-LAYER). Severity follows the clause's modal verb
  (D-SEV).
- **Editing:** operations are values (`CreateClaim { .. }`) applied to a document. Each
  is checked fully before anything changes (D-ATOMIC). Destructive operations have a
  `preview` (D-PREVIEW).

## Quality gates

Every change passes rustfmt, clippy (pedantic, `-D warnings`), tests on Linux, Windows
and macOS, the MSRV build, rustdoc with `-D warnings`, `cargo deny` and
`cargo xtask matrix check`. Mutation testing runs on every pull request's diff.
