# 0013. A missing name stays missing

- Status: accepted (the maintainer decided the model rule and approved the save rule on 2026-11-10; amendment 1 accepted the same day)
- Date: 2026-11-10
- Matrix rows: SACM23-BASE-003; inherited by SACM23-XMI-012, SACM23-VAL-018, LIB-COMPAT-007, LIB-EDIT-001,
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
  - XMI-012, writing: a name is written as exactly one `name` child (XMI-011 rules).
    Tolerant save writes a missing name as no `name` child. Strict save refuses the
    document, because it fails validation (amendment 1). Neither mode repairs it.
  - VAL-018: a missing name on a ModelElement is a SACM-MULT-001 error that names the
    element. The element is kept.
  - D-VAL-LAYER: a missing name read from a file is a content-level finding, reported by
    validation. The structural rule binds only edits: no edit may leave a ModelElement
    without a name.
  - LIB-EDIT-001 and LIB-EDIT-008: create operations take a name, as C++
    `commands/operations.h` does, and no operation leaves a ModelElement without one.
  - LIB-RT-001: a missing name differs from `<name/>`.
- **Deviation from C++:** this replaces the always-present `name_` (`element.h:177-179`,
  `:202`) and the always-written `<name/>` (`xmi_writer.cpp:255-256`). The create
  operations still follow C++.

## Consequences

- Callers handle `Option` for names, and use `kind().is_a(AbstractClass::ModelElement)` to
  tell "missing" from "has no name property".
- A document with a missing name can be saved only in tolerant mode, until the name is
  set.
- The `insert` seam takes the name because C++ create operations do. Later optional
  attributes get seam setters, mirroring the C++ `Set*` operations, rather than more
  `insert` parameters.

## Alternatives considered

- **Follow C++: always a LangString, empty by default.** Rejected: a missing name and
  `<name/>` become the same value, and saving invents `<name/>`.
- **A LangString plus a "present" flag.** Rejected: it does what `Option` does, with two
  fields that can disagree (as in ADR 0012).
- **Strict save writes a missing name as no `name` child**, as first accepted. Replaced by
  amendment 1: the output conforms to the schema but is not a valid model.
- **Refuse such a file on load.** Rejected: the crate holds files as read, and a refusal
  would cost the user the whole case.

## Amendment 1 (2026-11-10, accepted by the maintainer): strict save writes only valid models

- **Why:** a compliance audit against SACM 2.3 clause 2 found that the schema produced by
  the default XMI 2.5.1 rules enforces no multiplicities ("By default, XMI 2 produces
  schemas that ignore multiplicities also", XMI 2.5.1 §7.8.2). So an element with no `name` still
  conforms to the schema, but it is not a valid SACM model (§8.6 `name:LangString[1]`).
  `compliance-points.md` promises that strict output is conforming.
- **The rule:** strict save refuses any document for which `validate()` reports an Error.
  The refusal names the codes and the affected elements, and writes nothing. Tolerant save
  writes the document as it is held. This is one policy for every validation error, in
  row LIB-COMPAT-007. Strict output is therefore schema-conforming and a valid SACM 2.3
  model.
- **Deviation from C++:** C++ strict save does not validate. It refuses only preserved
  content (SACM-XMI-006, `xmi_writer.cpp:485-500`), and it writes `<name/>` for a missing
  name (`:255-256`).
- **Moving (2026-11-13).** This rule is about load and save modes, not names. It moves to
  the load and save modes ADR that comes with LIB-COMPAT-001 and LIB-COMPAT-007. That ADR
  also states the semver stance (a new validation rule can make the default save fail in a
  minor release) and settles one conflict: this amendment names every affected element,
  while ADR 0017 W4 names the first. Until then, this amendment stands, and the writer does
  not validate yet.
