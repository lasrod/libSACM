---
slice: 0015
name: m1-value-types
state: in-progress
step: verification
rows: [SACM23-BASE-001, SACM23-ARG-003]
mode: pr
---

# Slice 0015 — LangString and AssertionDeclaration

## Scope

The first two SACM value types:
- **SACM23-BASE-001:** `LangString`, an optional language tag and optional content, preserved exactly.
- **SACM23-ARG-003:** the `AssertionDeclaration` enumeration, with its exact literals and its default.

Out of scope: XMI reading and writing, and editing. Those rows build on these types.

## Basis

formal/23-05-08 clauses 8 and 11; ptc/22-03-13 `Base::LangString` and
`Argumentation::AssertionDeclaration`; the assurance-forge C++ `model/lang_string.h`
and `model/argumentation.h`.

## Design

ADR 0012 (accepted) and ADR 0011 amendment 2 (accepted). In `sacm::model`, re-exported flat:
- **`LangString`** (`model/lang_string.rs`): private `Option<String>` fields; `Default`, `new(content)`, `with_lang`, `lang()`/`content() -> Option<&str>`, `set_lang`/`set_content(Option<String>)`. Users can build it.
- **`AssertionDeclaration`** (`model/argumentation.rs`): `#[non_exhaustive]`, `#[default] Asserted`, `ALL`, `as_str`, `Display`, `FromStr`.
- **`ParseAssertionDeclarationError`**: a `#[non_exhaustive]` struct with `input()`.

## Log

- 2026-11-10 brief written. The researcher is next.
- 2026-11-10 research: the text and the model agree on both types. BASE-001 is narrowed to the value type; XMI, comparison and editing go to BASE-002/003, LIB-RT-001 and LIB-EDIT-001/008. ARG-003 keeps `as_str`/`FromStr`; the XML attribute goes to ARG-002, and unknown literals to VAL-006. New row SACM23-XMI-011 (string values survive XML exactly). XMI-004 now says "every SACMElement". ARG-003 depends on LIB-API-004, and BASE-001 has no dependency. Both rows are `analyzed`.
- 2026-11-10 the maintainer chose to keep absent, empty and non-empty distinct in LangString, departing from C++ (ADR 0012). The architect is next.
- Logged for later rows: whether VAL-006 keeps the raw invalid literal; whether a `name` with absent or empty content is a missing name (BASE-003); asCited derivation as a later warning-level VAL row; the §11.11 "assertionDeclared" typo for specification-defects.md.
- 2026-11-10 design: the architect follows C++ `lang_string.h` and `argumentation.h/.cpp`. ADR 0012 records the maintainer's decision. The maintainer approved ADR 0011 amendment 2 (`Parse<Type>Error` structs). The acceptance names the API. The test engineer is next.
- 2026-11-10 tests: 46 tests (27 in `crates/sacm/tests/lang_string.rs`, 19 in `crates/sacm/tests/assertion_declaration.rs`). They are red: the items are unresolved. Both rows are `tests-failing`. The parse error quotes its input with `{:?}`, as `InsertError` does (confirmed by the lead). Review items: `#[non_exhaustive]`, private fields, and no `id()`/`kind()` on LangString. The implementer is next.
- 2026-11-10 implementation: `LangString`, `AssertionDeclaration` and `ParseAssertionDeclarationError` in `sacm::model`. All 46 tests pass. The local mutation run caught 17 of 17. Both rows are `implemented`, and the full gate passes.
