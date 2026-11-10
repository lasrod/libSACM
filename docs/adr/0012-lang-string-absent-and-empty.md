# 0012. LangString keeps absent and empty apart

- Status: accepted (the maintainer decided it on 2026-11-10)
- Date: 2026-11-10
- Matrix rows: SACM23-BASE-001

## Context

SACM 2.3 §8.3 gives LangString two attributes, `lang : String [0..1]` and
`content : String [0..1]`. For each one, "no value" and "the empty string" are different
states. A conforming file can hold either: `lang=""`, or no `lang` attribute at all.

The assurance-forge C++ library (`libs/sacm`) treats the two as the same:
- `model/lang_string.h:18-24` stores `std::string lang` and `std::string content`, so
  there is no absent state;
- `io/xmi_reader.cpp:804-809` reads a missing `lang` or `content` as `""`;
- `io/xmi_writer.cpp:59-64` and `:260-265` leave out an attribute whose value is empty.

So `lang=""` that is read and written again comes back as no `lang`. That is a silent
change to conforming data, which this crate's first rule forbids.

## Decision

- **Storage:** `model::LangString` stores each field as `Option<String>`.
  - Absent (`None`), empty (`Some("")`) and non-empty are three distinct values.
  - `lang()` and `content()` return `Option<&str>`.
  - `Eq`, `Ord` and `Hash` tell the three apart.
- **No changes to values:** values are kept byte for byte. There is no trimming, case
  folding, Unicode normalisation, and no format check on `lang`.
- **The later XMI rows inherit this:**
  - the reader keeps an absent attribute absent and an empty one empty;
  - the writer leaves out an absent field and writes an empty one as `=""`.
- **Deviation from C++:** this replaces the `std::string` fields, which have no absent
  state (`lang_string.h:19-20`), and the reader and writer handling above. Everything
  else about LangString follows C++: it is a value type, with no id and no kind.

## Consequences

- Round trips keep the distinction, and semantic comparison can see it.
- Rust callers handle `Option` where C++ callers compare against `""`.
- The XMI rows must test both states for both attributes.
- How ExpressionLangString (SACM23-BASE-009) relates to LangString is still open. That row
  must say whether "content should be empty" means absent or `""`.

## Alternatives considered

- **Follow C++, with `String` and empty meaning absent.** Rejected: it silently changes
  `lang=""` and `content=""` on a round trip.
- **`String` plus a "present" flag.** Rejected: it does the same as `Option` with two
  fields that can disagree.
