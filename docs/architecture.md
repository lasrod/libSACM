# Architecture

This is the shape of libSACM. The modules below exist; the
[functionality matrix](matrix/functionality-matrix.md) says what each one does yet. Lasting
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

## Library modules

Built at M2 (the Base and Argumentation packages); later milestones add the other
packages' element kinds to the same modules, as M3 does for the Artifact package
(ADR 0034).

```text
sacm
├── model          Document, ElementId, SacmElement views, element kinds, values (ADR 0010)
│   ├── document     Document (roots, look-up, traversal) and the SacmElement view
│   ├── id           ElementId
│   ├── kind         ElementKind, AbstractClass, MetamodelPackage
│   ├── containment  the composite-role table: which role holds which child (crate-private)
│   ├── reference    Flag and ReferenceEnd: the flags and non-containment ends (ADR 0026-0028, 0034)
│   ├── lang_string  LangString, MultiLangString (ADR 0012)
│   ├── argumentation AssertionDeclaration
│   ├── revision     Revision, one state of a document (ADR 0031)
│   └── preserved    preserved content read from XMI (crate-private; ADR 0033)
├── xmi            read and write, each strict or tolerant (Mode); XML stays private (ADR 0005)
│   ├── input        bytes to a well-formed XML tree, or a refusal (ADR 0015, 0016)
│   ├── roots        the interchange roots and the package content under them (ADR 0016, 0021, 0024)
│   ├── preserve     non-SACM content kept as written (ADR 0033)
│   ├── refusal      the one list of what read refuses as Unsupported (ADR 0033)
│   ├── namespaces   the SACM and XMI namespace pins (ADR 0017)
│   └── output       the writer: plan, then one deterministic form (ADR 0017, 0022)
├── validation     validate: clause-level rules reported as diagnostics (ADR 0018)
│                  ids, names, gid, descriptions, multi_lang, constraints, homogeneity,
│                  references, relationships, repeats: one file per group of rules;
│                  the NCName-id rule is ADR 0015 amendment 2 and ADR 0019 E6
├── diagnostic     Diagnostic, Severity, Code, SourceLocation, has_errors (ADR 0011)
├── edit           operations as values; check, preview, apply, apply_preview, batch (ADR 0019)
│   ├── create, utility, argumentation, artifact   the create operations
│   ├── set, references, reorder         set a feature, edit reference ends, reorder (ADR 0029)
│   ├── delete, preview                  DeleteElement and its policies, Preview (ADR 0031)
│   ├── batch                            Batch, BatchMutation, BatchError (ADR 0032)
│   ├── plan                             what the checks computed, applied once (crate-private)
│   └── record                           the serde form of EditError (`serde` feature; ADR 0030)
└── compare        semantic_compare, Difference and Document: Eq (ADR 0020)
```

```text
bytes    ──► xmi::read            ──► Document + Vec<Diagnostic>
Document ──► validation::validate ──► Vec<Diagnostic>
Document ──► edit::apply(op)      ──► Result<Mutation, EditError>         (atomic)
Document ──► edit::preview(op)    ──► Result<Preview, EditError>          (edit::apply_preview applies it)
Document ──► edit::apply_all(ops) ──► Result<BatchMutation, BatchError>   (all or nothing, ADR 0032)
Document ──► xmi::write           ──► Result<String, WriteError>          (deterministic; strict by default: refuses a validation Error, ADR 0022)
```

- **Model** ([ADR 0010](adr/0010-document-model.md)): a `Document` owns its root
  packages, in document order, and every element under them. Elements live in a
  private arena with an `ElementId` index; containment is an ordered list of children,
  each admitted by a SACM composite role of its parent (LIB-API-008; the role table is
  crate-private), and references are ids. Users work through read-only views that
  borrow the document (`SacmElement<'a>`, later typed views such as `Claim<'a>`),
  never raw storage.
  Content that is not SACM 2.3 where it stands is kept, as written, as preserved content
  of its element ([ADR 0033](adr/0033-preserved-content.md)), so a strict save can find
  and refuse it.
- **Validation:** structural invariants hold after every edit; content constraints are
  reported by `validation::validate` (D-VAL-LAYER). Severity follows the clause's modal verb
  (D-SEV).
- **Editing** ([ADR 0019](adr/0019-edit-operations.md)): operations are values
  (`CreateClaim::new(parent, name)`) applied with `edit::apply`. Each is checked fully
  before anything changes (D-ATOMIC). Destructive operations have a
  `preview` (D-PREVIEW).

## Quality gates

Every change passes rustfmt, clippy (pedantic, `-D warnings`), tests on Linux, Windows
and macOS, the MSRV build, rustdoc with `-D warnings`, `cargo deny` and
`cargo xtask matrix check`. Mutation testing runs on every pull request's diff.
