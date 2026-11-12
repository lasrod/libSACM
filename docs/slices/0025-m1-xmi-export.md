---
slice: 0025
name: m1-xmi-export
state: done
step: verified
rows: [SACM23-XMI-003, SACM23-XMI-004, SACM23-XMI-006, SACM23-XMI-007, SACM23-XMI-013, LIB-API-010]
mode: pr
base: slice/0024-m1-utf16-input (PR #34)
---

# Slice 0025 — XMI export

## Scope

The first XMI writer, an `xmi::write` or similar. It writes a `Document`'s roots and their
contained elements deterministically:
- the pinned SACM and XMI namespaces, with `xmi:version="2.0"`;
- an `xmi:id` on every SACMElement;
- a type attribute where a role's declared type is abstract;
- one root bare, several in `xmi:XMI`.

Import halves that need package content to be read, and the LangString `xmi:id` rule, may
move to the rows that read that content. Research decides.

## Basis

XMI 2.5.1 production rules; formal/23-05-08 clause 2; D-NS-PIN; D-ID; ADR 0005 (a
hand-written std writer). The maintainer's decisions of 2026-11-12 are in the
SACM23-XMI-003 and SACM23-XMI-004 notes.

## Design

ADR 0017.

```rust
pub fn write(document: &Document, options: &WriteOptions) -> Result<String, WriteError>;
#[non_exhaustive] #[derive(Default, ..)] pub struct WriteOptions {}
#[non_exhaustive] pub enum WriteError {
    NoRoot,
    UnrepresentableCharacter { element: ElementId, character: char },
    Unsupported { element: ElementId, reason: String },
}
impl WriteError { pub fn element(&self) -> Option<&ElementId>; pub fn code(&self) -> Option<Code>; }
```

The code is private:
- `xmi/output.rs` (the writer) and `xmi/namespaces.rs` (shared constants);
- `model::containment::role(parent, child) -> Option<Role>`, from which `admits` is derived.

## Log

- 2026-11-12 choose: the writer rows. XMI-007 needs XMI-003, XMI-004 and XMI-006, and
  XMI-013 needs XMI-007; the rest of M1 builds on export. Base: `slice/0024-m1-utf16-input`
  (PR #34).
- 2026-11-12 research. The maintainer decided:
  - export writes `xmi:type`, per XMI 2.5.1, not C++'s `xsi:type`, and no `xsi` namespace
    is declared;
  - an empty document is refused.

  Lead decisions:
  - follow C++ on `xmi:version`, on writing every package kind as a root, and on ids that
    are not NCNames;
  - names and a non-default `assertionDeclaration` are refused for now (new LIB-API-010,
    in this slice);
  - import halves move to SACM23-XMI-012, BASE-004 and XMI-016, and to the new XMI-020 and
    LIB-COMPAT-010;
  - ADR 0016 D2 names the C++ case-insensitive class match.

  Five rows are `analyzed`, and LIB-API-010 is added.
- 2026-11-12 design (architect): accepted. It is recorded in ADR 0017. T1–T5 are accepted;
  W1–W5 are proposed and wait for the maintainer before `gh pr ready`.
- 2026-11-12 maintainer: confirmed `xmi:type` after a fuller analysis, now recorded in
  ADR 0017 T1.
- 2026-11-12 tests: 48 tests in `xmi_write.rs` and 3 golden files. The test engineer checked
  them against a throwaway writer, and 38 deliberate bugs were all caught. The red is a
  compile failure (no `xmi::write`), which the lead confirmed. ADR 0017 now pins ` />`,
  pre-order and checking the id first.
- 2026-11-12 implementation: `xmi::write` (`xmi/output.rs`), which plans and checks the document and then builds the output, with no panics. The namespaces are shared (`xmi/namespaces.rs`), and `model::containment::role` is the one role table, from which `admits` is derived. The local gate passes.
- 2026-11-12 verification round 1: FAIL on 4 rows; XMI-007 and XMI-013 wait on their dependencies.
  - F1, traceability: XMI 2.5.1 both requires and limits the type attribute. Recorded as specification defect D11, with the reading taken stated in ADR 0017 and the row.
  - F2: the C++ `namespace_uri` option is not named. Now ADR 0017 W6, deferred to LIB-COMPAT-001.
  - F3 and F4: two orders are not pinned (the first bad character, and the name before the declaration).
  - The docs review found nothing blocking; its should-fix items go to the implementer.
- 2026-11-12 verification round 2: PASS at a2be90c. All six rows are verified. `gh pr ready` waits for the maintainer to approve ADR 0017 W1–W6.
