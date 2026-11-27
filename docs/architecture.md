# Architecture

The shape of libSACM: its goals, modules and data flow.

- **What each module does today:** the [functionality matrix](matrix/functionality-matrix.md).
- **Lasting decisions:** the [ADRs](adr/).
- **Interpretations of the standard:** [sacm/decisions.md](sacm/decisions.md).

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

One set of modules serves every SACM package: a milestone adds its element kinds to the
same modules instead of adding a module (ADR 0034).

```text
sacm
├── model          Document, ElementId, SacmElement views, element kinds, values (ADR 0010)
│   ├── document     Document (roots, look-up, traversal) and the SacmElement view
│   ├── id           ElementId
│   ├── kind         ElementKind, AbstractClass, MetamodelPackage
│   ├── containment  the composite-role table: which role holds which child (crate-private)
│   ├── reference    Flag and ReferenceEnd: the flags and non-containment ends (ADR 0026-0028, 0034)
│   ├── href         Href and Proxy: links into other files, never resolved (ADR 0040)
│   ├── lang_string  LangString, MultiLangString (ADR 0012)
│   ├── argumentation AssertionDeclaration
│   ├── subclass     Subclass: an element's declared subclass (ADR 0044)
│   ├── revision     Revision, one state of a document (ADR 0031)
│   └── preserved    preserved content read from XMI (crate-private; ADR 0033)
├── xmi            read and write, each strict or tolerant (Mode); XML stays private (ADR 0005)
│   ├── input        bytes to a well-formed XML tree, or a refusal (ADR 0015, 0016)
│   ├── limits       Limits: the bounds of a read (ADR 0041)
│   ├── roots        the interchange roots and the package content under them (ADR 0016, 0021, 0024)
│   ├── preserve     non-SACM content kept as written (ADR 0033)
│   ├── refusal      the one list of what a strict read refuses as Unsupported, and what a
│   │                tolerant read does with each entry (ADR 0033, 0035)
│   ├── namespaces   the SACM and XMI namespace pins (ADR 0017)
│   └── output       the writer: plan, then one deterministic form (ADR 0017, 0022)
├── validation     validate: clause-level rules reported as diagnostics (ADR 0018)
│                  ids, names, gid, descriptions, multi_lang, constraints, homogeneity,
│                  references, relationships, repeats, expressions, terminology, modularity,
│                  metadata: one file per group of rules;
│                  the NCName-id rule is ADR 0015 amendment 2 and ADR 0019 E6
├── diagnostic     Diagnostic, Severity, Code, SourceLocation, has_errors (ADR 0011)
├── edit           operations as values; check, preview, apply, apply_preview, batch (ADR 0019)
│   ├── create, utility, argumentation, artifact, terminology   the create operations
│   ├── set, references, reorder         set a feature, edit reference ends, reorder (ADR 0029)
│   ├── move_element, copy               MoveElement, CopyElements (ADR 0036)
│   ├── delete, preview                  DeleteElement and its policies, Preview (ADR 0031)
│   ├── batch                            Batch, BatchMutation, BatchError (ADR 0032)
│   ├── plan                             what the checks computed, applied once (crate-private)
│   └── record                           the serde form of EditError (`serde` feature; ADR 0030)
└── compare        semantic_compare, Difference, changes and Document: Eq (ADR 0020)
```

## Data flow

A `Document` is the centre. Each call below takes one or returns one.

| Call | From | To | Note |
|---|---|---|---|
| `xmi::read` | bytes | `Document` + `Vec<Diagnostic>` | Strict by default. |
| `validation::validate` | `Document` | `Vec<Diagnostic>` | Never changes or refuses a document. |
| `edit::apply(op)` | `Document` | `Result<Mutation, EditError>` | Atomic. |
| `edit::preview(op)` | `Document` | `Result<Preview, EditError>` | `edit::apply_preview` applies it. |
| `edit::apply_all(ops)` | `Document` | `Result<BatchMutation, BatchError>` | All or nothing (ADR 0032). |
| `xmi::write` | `Document` | `Result<String, WriteError>` | Deterministic. Strict by default: schema-valid output only (ADR 0035). |

## Design rules

### Model ([ADR 0010](adr/0010-document-model.md))

- **Ownership.** A `Document` owns its root packages, in document order, and every
  element under them.
- **Storage.** Elements live in a private arena with an `ElementId` index.
- **Containment.** An ordered list of children, each admitted by a SACM composite role of
  its parent (LIB-API-008; the role table is crate-private).
- **References.** They are ids.
- **Views.** Users work through read-only views that borrow the document
  (`SacmElement<'a>`), never raw storage.

### Preserved content ([ADR 0033](adr/0033-preserved-content.md))

- **Kept as written.** Content that is not SACM 2.3 where it stands is kept as preserved
  content of its element, so a strict save can find and refuse it.
- **On or under `xmi:XMI`.** Such content is held by the document, outside its elements
  ([ADR 0035](adr/0035-tolerant-read-and-schema-only-save.md)).

### Modes ([ADR 0035](adr/0035-tolerant-read-and-schema-only-save.md))

- **Strict read, the default.** It refuses what it does not read.
- **Tolerant read.** It keeps most of what a strict read refuses, as preserved content
  with a Warning. So it opens every well-formed file whose roots are SACM packages, with
  the exceptions below.
- **What a tolerant read still refuses** (LIB-COMPAT-001): a DOCTYPE, a SACM 2.4 root, an
  `XMI` element in a namespace other than XMI 2.5.1's or EMF's, a repeated `xmi:id`, an
  encoding it cannot decode, and input over the parser's capacity or the read limits.
- **Strict write.** It checks schema validity only: no preserved content, no `xmi:id`
  that is no `NCName`, no reference to an `xmi:id` it does not write.
- **No validation on write.** A strict write never runs `validate`, so a draft saves
  strictly; an application calls `validate` for model checks.

### Validation

- **Two layers** (D-VAL-LAYER). Structural invariants hold after every edit, every
  string held being an `xsd:string` among them (LIB-EDIT-025); content constraints are
  reported by `validation::validate`.
- **Severity** follows the clause's modal verb (D-SEV).

### Editing ([ADR 0019](adr/0019-edit-operations.md))

- **Operations are values,** such as `CreateClaim::new(parent, name)`, applied with
  `edit::apply`.
- **Atomic** (D-ATOMIC). Each operation is checked fully before anything changes.
- **Previews** (D-PREVIEW). Destructive operations have a `preview`.

## Quality gates

Every change passes:

- rustfmt, and clippy (pedantic, `-D warnings`);
- the tests, on Linux, Windows and macOS;
- the MSRV build;
- rustdoc with `-D warnings`;
- `cargo deny`;
- `cargo xtask matrix check`.

Mutation testing runs on every pull request's diff.
