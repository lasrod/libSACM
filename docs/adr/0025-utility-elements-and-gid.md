# 0025. UtilityElements, MultiLangString roles and gid

- Status: accepted (the maintainer, 2026-11-14; an absent `content` gives no finding)
- Date: 2026-11-14
- Matrix rows: SACM23-BASE-004, SACM23-BASE-005, SACM23-BASE-006, SACM23-VAL-010; it widens
  SACM23-VAL-020 and narrows ADR 0021's interim list and ADR 0024's value-element bullet.

## Context

SACM 2.3 §8.6 gives every ModelElement `description`, `implementationConstraint`, `note`
and `taggedValue` children. These are UtilityElements (§8.7: "Superclass SACMElement"),
and each holds a `content:MultiLangString`. A TaggedValue also holds a
`key:MultiLangString[1]` (§8.12). Every SACMElement has `gid:String[0..1]` (§8.2).

A MultiLangString holds `value:LangString[1..*]`, and "their +lang must be unique" (§8.5).
In XMI, `content`, `key` and `value` are always role elements (XMI 2.5.1 §7.8.6), and each
is a class that may carry an `xmi:id` (§7.8.3).

## Decision

- **UtilityElements are elements.**
  - Description, ImplementationConstraint, Note and TaggedValue are SACMElements. They keep
    an id, a kind and a `gid`, and they can be cited.
  - They are nodes in the one ordered child list (D-ORDER), which the model already admits.
  - `SacmElement::descriptions()`, `implementation_constraints()`, `notes()` and
    `tagged_values()` filter that list. Several descriptions are kept (D-DESC-MULT).
- **Roles hold a MultiLangString or nothing.**
  - `content()` (on a UtilityElement) and `key()` (on a TaggedValue) return
    `Option<&MultiLangString>`.
  - No role element is `None`. `<content/>` is an empty MultiLangString. Both round-trip
    (ADR 0012, ADR 0013).
  - Values are kept in order, with duplicates. Each `value` is read like `name` (attribute
    and element forms, absent ≠ empty).
- **Ids are kept per role and position** (ADR 0024's rule for later LangString slots).
  - The `xmi:id` of the `content` or `key` element is `content_id()` or `key_id()`. The
    `xmi:id` of each `value` is held by position in `content_value_ids()` or
    `key_value_ids()`.
  - They share the file's id space, identify no element, are written as the first attribute
    of their element, and are compared.
  - Under `name` or `value`, `<content>` is the String value element (SACM23-XMI-014), and
    an `xmi:id` on it stays refused.
- **`gid`.**
  - It is held exactly on every element: `None`, `""` and any other string all differ.
  - It is read from the unqualified attribute or from a `gid` value element. Given twice, it
    is `RepeatedValue`.
  - It is written as an attribute after `xmi:id`. It is not an id: it is never trimmed,
    generated or checked for syntax.
  - Its uniqueness is SACM23-VAL-002.
- **Write order.**
  - Attributes: `xmi:type`, `xmi:id`, `gid`, then the class's own attributes.
  - Inside a UtilityElement, `content` comes before `key`, superclass first (XMI 2.5.1
    Annex B.5.2).
  - An empty MultiLangString is `<content/>`, and an absent one writes nothing.
- **Read refusals.**
  - A second `content` or `key` is `RepeatedValue`.
  - A `name` on a UtilityElement is refused.
  - The C++ single-language shorthand (`lang`/`content` attributes or text on the role
    element), `content="…"` on a UtilityElement, and `key=`/`value=` on `taggedValue` stay
    `Unsupported` until LIB-COMPAT-008.
- **Editing.**
  - `CreateDescription`, `CreateImplementationConstraint`, `CreateNote` and
    `CreateTaggedValue` follow the LIB-EDIT-001 contract. Each takes a full MultiLangString,
    and a parent that is a UtilityElement is `InvalidParent`.
  - Setting `content`, `key` and `gid` is LIB-EDIT-008. Removal is LIB-EDIT-004/005.
- **Validation (SACM-MULT-001, Error, so a strict save refuses).**
  - For each `content` and `key` that is present: an empty MultiLangString (§8.5 `[1..*]`),
    and one finding per language that two or more values share. Absent equals absent, and
    `""` differs from absent (ADR 0012).
  - A TaggedValue with no `key` (§8.12 `[1]`, ptc `[1..1]`; D-SEV).
  - SACM23-VAL-020 also reports `content`, `key` and `value` ids that are not NCNames.
- **Comparison.** `Difference` gains `Gid`, `Content`, `ContentId`, `ContentValueIds`, `Key`,
  `KeyId` and `KeyValueIds`. They are reported after `Parent` (`Gid`) and after `NameId`
  (the rest), before `AssertionDeclaration`.
  Value ids are compared by position, and a position one side lacks counts as "no id", so
  a different number of values is only a `Content` or `Key` difference. `==` still means
  "writes the same bytes".

**Differences from C++, one line each:**
- C++ keeps a vector per role (`model/element.h:183-206`); here UtilityElements are nodes
  in the one ordered child list.
- C++ has no absent MultiLangString and writes nothing for an empty one
  (`xmi_writer.cpp:49-51`); here absent and empty stay apart.
- C++ drops a LangString id and ignores a wrapper's id (`xmi_reader.cpp:785-803`); here both
  are kept, written and compared.
- C++ writes `key` before `content` (`xmi_writer.cpp:245-251`); here `content` comes first
  (Annex B.5.2).
- C++ reads the single-language shorthand, text content and `key=`/`value=` silently; here
  they are refused until LIB-COMPAT-008.
- C++ merges a repeated `content` into one MultiLangString (`xmi_reader.cpp:1160-1169`);
  here it is `RepeatedValue`.
- C++ reports each repeated language occurrence under SACM23-BASE-001, keys `lang` as a
  string (absent equals `""`) and cannot see an empty MultiLangString; here there is one
  finding per repeated language, absent ≠ `""`, and empty is reported.
- C++ edits through `SetDescription`, `AddTaggedValue` and similar operations, one
  language at a time (`operations.h:247-360`); here there are four `Create*` operations
  taking a full MultiLangString.
- C++ reads `gid` only as an attribute (`xmi_reader.cpp:894`), and `SetGid` clears `""`;
  here the element form is read, and `""` is a value.

## Consequences

- Real files with descriptions, notes and tagged values now load, and are compared and
  saved without loss.
- An edit-built ImplementationConstraint will fail a strict save once the §8.6 rule
  "implementationConstraint implies isAbstract" has a row (with SACM23-BASE-008).
