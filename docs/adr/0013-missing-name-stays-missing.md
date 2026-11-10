# 0013. A missing name stays missing

- Status: accepted (the maintainer decided the model rule and approved the save rule on 2026-11-10)
- Date: 2026-11-10
- Matrix rows: SACM23-BASE-003; inherited by SACM23-XMI-012, SACM23-VAL-018, LIB-EDIT-001,
  LIB-EDIT-008, LIB-RT-001

## Context

SACM 2.3 §8.6 gives every ModelElement `name : LangString [1]` ("the name of the
ModelElement"). The four UtilityElement kinds (Description, ImplementationConstraint, Note
and TaggedValue; §8.7, superclass SACMElement) have no name. A file can still leave out a
ModelElement's `name` child, and this crate must hold the file as read.

The assurance-forge C++ library (`libs/sacm`) always has a name:
- `model/element.h:177-179` returns `const LangString&`, and `:202` stores
  `LangString name_`, which starts with empty fields;
- `io/xmi_reader.cpp:1186-1201` sets the name only from a `name` child, so an element
  without one keeps the empty default;
- `io/xmi_writer.cpp:255-256` always writes a `name` child ("always emitted for
  determinism and conformance").

So a file with no `name` comes back with `<name/>`, which invents content. Under ADR 0012
the empty default is also a real value: `<name/>` is a name with both fields absent. The
default cannot also mean "no name".

## Decision

- **Storage:** the Document holds a ModelElement's name as `Option<LangString>`, and
  `SacmElement::name()` returns `Option<&LangString>`. `None` is a missing name.
- **Only "no name at all" is missing.** A name whose `lang` or `content` is absent or
  empty, including `LangString::default()`, is a name.
- **UtilityElements have no name.** `name()` is always `None` for them, and the
  `test-support` `Document::insert` refuses a name for them
  (`InsertError::NameNotAllowed`). The kinds with a name are exactly those where
  `is_a(AbstractClass::ModelElement)` holds.
- **Nothing invents a name:** not the reader, not the writer, and not a default.
- **The later rows inherit this:**
  - XMI-012, reading: no `name` child gives `None`; `<name/>` gives
    `Some(LangString::default())`.
  - XMI-012, writing: a name is written as exactly one `name` child (XMI-011 rules). A
    missing name is written as no `name` child, in strict and tolerant save alike. Neither
    mode refuses or repairs it.
  - VAL-018: a missing name on a ModelElement is a SACM-MULT-001 error that names the
    element. The element is kept.
  - LIB-EDIT-001 and LIB-EDIT-008: create operations take a name, as C++
    `commands/operations.h` does, and no operation leaves a ModelElement without one.
  - LIB-RT-001: a missing name differs from `<name/>`.
- **Deviation from C++:** this replaces the always-present `name_` (`element.h:177-179`,
  `:202`) and the always-written `<name/>` (`xmi_writer.cpp:255-256`). The create
  operations still follow C++.

## Why strict save writes a missing name as missing

- C++ strict save refuses only content it cannot write as SACM: preserved fragments, with
  SACM-XMI-006 (`xmi_writer.cpp:485-500`). It does not validate, and it writes other
  multiplicity violations as they are.
- Refusing a missing name would make the writer a partial validator, and inventing one is
  ruled out above.
- Writing the element without a `name` child keeps D-RT: load, save and load again give the
  same model. `validate()` reports the gap.

## Consequences

- Callers handle `Option` for names, and use `kind().is_a(AbstractClass::ModelElement)` to
  tell "missing" from "has no name property".
- Strict output of a document with validation errors does not conform, as in C++. Callers
  that need conformance run `validate()` first.
- The `insert` seam takes the name because C++ create operations do. Later optional
  attributes get seam setters, mirroring the C++ `Set*` operations, rather than more
  `insert` parameters.

## Alternatives considered

- **Follow C++: always a LangString, empty by default.** Rejected: a missing name and
  `<name/>` become the same value, and saving invents `<name/>`.
- **A LangString plus a "present" flag.** Rejected: it does what `Option` does, with two
  fields that can disagree (as in ADR 0012).
- **Strict save refuses a ModelElement with no name.** Not chosen: C++ strict save does
  not validate. If strict save should refuse invalid documents, that is one policy for
  every validation error, decided in its own row.
- **Refuse such a file on load.** Rejected: the crate holds files as read, and a refusal
  would cost the user the whole case.
