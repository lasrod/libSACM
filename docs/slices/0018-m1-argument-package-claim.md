---
slice: 0018
name: m1-argument-package-claim
state: done
step: close
rows: [SACM23-ARG-001, SACM23-ARG-002]
mode: pr
---

# Slice 0018 — ArgumentPackage and Claim

## Scope

- **SACM23-ARG-001:** ArgumentPackage, as a root or nested, holding argumentation
  elements in document order.
- **SACM23-ARG-002:** Claim, with its `assertionDeclaration`.

ARG-002 depends on ARG-001, so the two are taken together. Out of scope: XMI reading and
writing (the XMI rows), the other argumentation elements, package homogeneity
(SACM23-VAL-012), interfaces and bindings, and editing. Research decides what each row
keeps, and gives every obligation it drops a home row.

## Basis

formal/23-05-08 §11.2, §11.4, §11.10 and §11.11; ptc/22-03-13
`Argumentation::ArgumentPackage` and `Argumentation::Claim`; the assurance-forge C++
`libs/sacm` model.

## Design

No ADR (ADR 0013: later attributes get seam setters). ARG-001 needs no code.
- **Stable:** `SacmElement::assertion_declaration(self) -> Option<AssertionDeclaration>`,
  which is `None` exactly when the kind is not an Assertion.
- **`test-support`:** `Document::set_assertion_declaration(&mut self, id: &ElementId,
  declaration: AssertionDeclaration) -> Result<(), SetAssertionDeclarationError>`, and
  `#[non_exhaustive] enum SetAssertionDeclarationError { UnknownElement(ElementId),
  NotAnAssertion(ElementId, ElementKind) }`. This mirrors C++ `SetAssertionDeclaration`.
- **Storage:** the private `Node` gains `assertion_declaration: Option<AssertionDeclaration>`,
  which `insert_node` sets to `Some(Asserted)` for Assertion kinds.
- **Docs:** `argumentation.rs`, `model.rs`, `lib.rs`, both READMEs and `architecture.md`.

## Log

- 2026-11-10 brief written. The researcher is next.
- 2026-11-10 research:
  - ARG-001 adds no model code: containment and kinds are verified, so its tests characterise the root, the 12 kinds mixed in order, nesting, and homogeneity not being refused.
  - ARG-002 holds `assertionDeclaration` on all six Assertion kinds. There is no absent state: the property is `[1] = asserted`, XMI 2.5.1 §9.4.1 writes no default, and C++ does the same, so no decision is needed.
  - New rows: SACM23-XMI-017 (assertionDeclaration in XMI) and SACM23-VAL-019 (asCited, a warning).
  - Reworded: ARG-007, ARG-014, ART-012 and TERM-007. The packages' `interface` reference is recorded as a model-followed divergence in the inventory.
  - Both rows are `analyzed`. The architect is next.
- 2026-11-10 design: the architect follows C++ `argumentation.h` Assertion and `check_set_assertion_declaration`. The error type is proven by ARG-002's own tests (acceptance item 7), as ARG-003 proved its parse error, so LIB-API-004 stays verified. The test engineer is next.
- 2026-11-10 tests: 9 `sacm23_arg_001_*` in `crates/sacm/tests/argument_package.rs`, which already pass, as expected: they characterise behaviour LIB-API-008 and LIB-API-002 already provide. 22 `sacm23_arg_002_*` in `crates/sacm/tests/claim.rs`. The red was confirmed by the lead: E0432 and E0599 for the three missing items. Both rows are `tests-failing`. The implementer is next.
- 2026-11-10 implementation: `SacmElement::assertion_declaration`, the `test-support` `Document::set_assertion_declaration` and `SetAssertionDeclarationError` in `model/document.rs`; docs in `argumentation.rs`, `model.rs`, `lib.rs`, both READMEs and `architecture.md`; a CHANGELOG entry. The full gate passes (656 tests). Both rows are `implemented`. Verification is next.
- 2026-11-10 verification round 1 at `61671a7` ([record](../verification/0018-m1-argument-package-claim.md)): both rows PASS, and both are now `verified`. Mutants: the PR diff caught 5 of 5, the full run caught 568 of 568; 23 of 24 hand mutations were killed, and the survivor is equivalent. Copilot left no comments. The docs review found nothing blocking; its wording fixes are applied (rustdoc, `Cargo.toml` comment, READMEs, CHANGELOG, two matrix typos). The slice is done.
