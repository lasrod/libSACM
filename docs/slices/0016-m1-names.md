---
slice: 0016
name: m1-names
state: in-progress
step: verification
rows: [SACM23-BASE-002, SACM23-BASE-003]
mode: pr
---

# Slice 0016 — MultiLangString and element names

## Scope

- **SACM23-BASE-002:** `MultiLangString`, an ordered list of LangString values.
- **SACM23-BASE-003:** the required `name` of every ModelElement.

Out of scope: XMI reading and writing, and editing. Research decides what each row keeps
and gives every obligation it drops a home row.

## Basis

formal/23-05-08 §8.5 and §8.6; ptc/22-03-13 `Base::MultiLangString` and
`Base::ModelElement.name`; the assurance-forge C++ `libs/sacm` model.

## Design

ADR 0013 (accepted). In `sacm::model`:
- **`MultiLangString`** (`model/lang_string.rs`): private `Vec<LangString>`; `Default`, `FromIterator<LangString>`, `values() -> &[LangString]`, `len`, `is_empty`, `push`. `Clone + Debug + Eq + Hash`, no `Ord`. The C++ `find`, `set` and `primary` are deferred to the editing rows.
- **`SacmElement::name() -> Option<&'a LangString>`**; the private `Node` holds `name: Option<LangString>`.
- **`test-support` `Document::insert(parent, kind, id, name: Option<LangString>)`**, with the new `InsertError::NameNotAllowed(ElementId, ElementKind)` for a name on a UtilityElement kind. The existing callers pass `None`.

## Log

- 2026-11-10 brief written. The researcher is next.
- 2026-11-10 research: 34 of the 38 kinds are ModelElements and carry a name; the four UtilityElement kinds do not. C++ never diagnoses a missing name: a missing `<name>` becomes `<name/>` on save. Neither source declares the MultiLangString list ordered, so the order is kept by D-RT. Homes for what this slice leaves out: new rows SACM23-XMI-012 (the `name` containment in XMI) and SACM23-VAL-018 (a missing name is diagnosed); wording in XMI-004, BASE-004, VAL-010, LIB-EDIT-001, LIB-EDIT-008 and LIB-RT-001. New spec defect D8 (the Base properties have no names). BASE-003 depends on LIB-API-003. Both rows are `analyzed`.
- 2026-11-10 the maintainer decided: a missing name is held as missing, not invented (ADR 0013, departing from C++ `element.h`); only no name at all is a missing name; MultiLangString equality depends on order. The architect is next.
- Logged for later rows: the strict-save treatment of a missing name (XMI-012, ADR 0013); whether absent and `""` languages are the same, and zero values (VAL-010); C++'s tolerant name paths (the legacy multi-language `<name>` and the `name="…"` attribute) for a LIB-COMPAT row.
- 2026-11-10 design: the architect follows C++ `lang_string.h`, `element.h` and the create operations, which take a name. The maintainer approved ADR 0013's save rule: a missing name is written as no `name` child in strict and tolerant save alike. The acceptance names the API. LIB-API-004 is re-verified with this slice, because `InsertError` gains a variant. The test engineer is next.
- 2026-11-10 tests: 34 tests (15 in `crates/sacm/tests/multi_lang_string.rs`, 19 in `crates/sacm/tests/element_name.rs`); the LIB-API-002 and LIB-API-004 helpers pass `None`, and LIB-API-004's variant list gains `NameNotAllowed`. They are red for the stated reasons: the `insert` arity, `name()`, `NameNotAllowed` and `MultiLangString` (confirmed by the lead). The lead changed the message to `element "note_1" of kind Note cannot have a name`, to avoid "a ImplementationConstraint". Both rows are `tests-failing`. The implementer is next.
- 2026-11-10 implementation: `MultiLangString` in `model/lang_string.rs`; `SacmElement::name`, the `insert` name parameter and `InsertError::NameNotAllowed` in `model/document.rs`; doctests and Status docs updated. The full gate passes (594 tests). Both rows are `implemented`.
- 2026-11-10 the maintainer asked that every departure from C++ stay fully compliant with SACM 2.3. An independent compliance audit of ADRs 0010–0013 and this slice's choices is running before verification.
