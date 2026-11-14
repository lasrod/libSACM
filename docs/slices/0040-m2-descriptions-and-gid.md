---
slice: 0040
name: m2-descriptions-and-gid
state: in-progress
step: verification
rows: [SACM23-BASE-004, SACM23-BASE-005, SACM23-BASE-006, SACM23-VAL-010]
mode: pr
base: main
---

# Slice 0040 — Descriptions, notes, tagged values and gid

## Scope

The rest of the SACM base model on every element. Each is represented, read, written,
compared and editable:
- **SACM23-BASE-004:** a ModelElement's `description`, `note` and `implementationConstraint`
  (UtilityElements whose `content` is a MultiLangString).
- **SACM23-BASE-005:** `taggedValue` (TaggedValue, with `key` and `content`).
- **SACM23-BASE-006:** a SACMElement's `gid`.
- **SACM23-VAL-010:** a MultiLangString with no values, or with two values in one language,
  is a SACM-MULT-001 validation error.

These end the interim refusals of utility children (ADR 0021) and of `gid`.

## Basis

SACM 2.3 §8.2-§8.6 (SACMElement, ModelElement, UtilityElement, LangString,
MultiLangString, TaggedValue); ptc/22-03-13; XMI 2.5.1 §7.8; ADR 0012, ADR 0013, ADR 0021,
ADR 0024; D-DESC-MULT.

## Design

[ADR 0025](../adr/0025-utility-elements-and-gid.md), accepted by the maintainer on
2026-11-14. An absent `content` gives no finding. The API is additive:

```rust
impl SacmElement<'a> {
    fn gid(self) -> Option<&'a str>;
    fn descriptions(self) / implementation_constraints(self) / notes(self) / tagged_values(self)
        -> impl Iterator<Item = SacmElement<'a>>;
    fn content(self) -> Option<&'a MultiLangString>;          // UtilityElement
    fn content_id(self) -> Option<&'a ElementId>;
    fn content_value_ids(self) -> &'a [Option<ElementId>];
    fn key(self) / key_id(self) / key_value_ids(self);          // TaggedValue
}
impl From<LangString> for MultiLangString;
edit::{CreateDescription, CreateImplementationConstraint, CreateNote}::new(parent, content)
edit::CreateTaggedValue::new(parent, key, content)            // + with_id, accessors
Difference::{Gid, Content, ContentId, ContentValueIds, Key, KeyId, KeyValueIds}
WriteAttribute::{Gid, ContentValueLang, ContentValueContent, KeyValueLang, KeyValueContent}
```

- **Validation (VAL-010, SACM-MULT-001):** an empty MultiLangString, one finding per
  repeated language, and a TaggedValue with no `key`.
- **Re-checked rows:** SACM23-XMI-004, -XMI-014, -VAL-020 and LIB-RT-001 are reworded.
- **New row:** SACM23-VAL-021 (the §8.6 `isAbstract` rule).

## Log

- 2026-11-14 choose: the base-model rows that end the utility-child and `gid` refusals.
