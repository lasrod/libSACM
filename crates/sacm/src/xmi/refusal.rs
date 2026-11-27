//! The one list of what a strict [`read`](super::read) refuses as
//! `ReadError::Unsupported`, and of what a tolerant read does with each
//! entry (LIB-API-011, ADR 0033, ADR 0035).
//!
//! Content that does not conform is preserved, not refused
//! (LIB-COMPAT-003). What is left is a limit of this version, input that
//! only tolerant mode reads (a tolerance, which strict mode never applies,
//! ADR 0022), and input that does not conform and that a strict read
//! refuses. Conforming input this version does not read yet
//! would be an `Interim` entry; since slice 0073 the list has none. Each
//! refusal names which by one [`Unread`] entry, and its reason ends with that
//! entry's label, so a caller can tell a gap of the library from a defect of
//! the file. A tolerance's input need not be defective: a document in
//! another SACM namespace is not (D-NS-PIN).
//!
//! A tolerant read keeps the defects as preserved content, with one
//! SACM-XMI-003 Warning ([`Unread::tolerant`], ADR 0035 D1, D2). It reads a
//! root in an older SACM revision's namespace or a SACM-like one, with
//! SACM-XMI-008 or SACM-XMI-002 (LIB-COMPAT-004, ADR 0041), and the XMI
//! names EMF writes in `http://www.omg.org/XMI`, with SACM-XMI-015
//! (LIB-COMPAT-002, ADR 0044 D1). It refuses only the limits, a SACM 2.4
//! root and an `XMI` element in any other namespace. XMI
//! metadata is read since slice 0073a, so no input gives SACM-XMI-014
//! (SACM23-XMI-019, ADR 0042), and every link form is read or held since
//! slice 0073b, so none gives SACM-XMI-007 (SACM23-XMI-029, ADR 0040
//! amendment 1).
//!
//! The xtask test `xtask/tests/interim_refusals.rs` keeps this list honest:
//! no other file of the crate builds the refusal, every `Interim` row is
//! open and gates SACM23-CP-001 and the compliance point of its own package
//! (every point, for a row of no package), and every `Tolerated` row exists
//! and is neither deferred nor a candidate.

use super::ReadError;
use super::input::locate;
use crate::diagnostic::{Argument, Code, Details, SourceLocation};

/// Why input is refused rather than read or preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// Conforming input this version does not read yet, and the row reads.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "no entry is Interim since slice 0073; the kind stays for later rows"
        )
    )]
    Interim(&'static str),
    /// Input strict mode never reads, which a tolerant read reads under the
    /// row (ADR 0041 D6), such as a root in an older SACM revision's
    /// namespace (D-NS-PIN) or a name in the XMI namespace EMF writes (ADR
    /// 0044 D1). It need not be defective.
    Tolerated(&'static str),
    /// A limit of this version.
    Limit,
    /// Input that does not conform and has nowhere to be kept.
    Defect,
}

impl Refusal {
    /// The label a reason ends with, by the kind of refusal: what
    /// [`label`] gives by code, which the tests check it against.
    #[cfg(test)]
    fn label(self) -> String {
        match self {
            Self::Interim(row) => format!("(a limit of this version; {row} reads it)"),
            Self::Tolerated(row) => {
                format!("(strict mode never reads it; {row} reads it in tolerant mode)")
            }
            Self::Limit => "(a limit of this version)".to_owned(),
            Self::Defect => "(a defect in the input)".to_owned(),
        }
    }

    /// The row that reads the input, if one does.
    pub(super) const fn row(self) -> Option<&'static str> {
        match self {
            Self::Interim(row) | Self::Tolerated(row) => Some(row),
            Self::Limit | Self::Defect => None,
        }
    }
}

/// What a tolerant read does with input a strict read refuses (ADR 0035 D1,
/// D2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tolerant {
    /// It is kept as preserved content, with one Warning of this code.
    Kept(Code),
    /// It is read, with one Warning of this code (ADR 0041 D6).
    Read(Code),
    /// It is refused, as in strict mode.
    Refused,
}

/// What a refusal does not read: one entry per kind of input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unread {
    /// A root in a SACM-like namespace: one whose URI contains `sacm` in any
    /// letter case, neither the pinned one nor a recognised revision's.
    OtherSacmNamespace,
    /// A root in the namespace of an older SACM revision the reader
    /// recognises.
    OtherRevision,
    /// A root in the SACM 2.4 namespace, which LIB-COMPAT-018 is to decide
    /// on.
    Sacm24,
    /// A name in the XMI namespace EMF writes, `http://www.omg.org/XMI`:
    /// the `XMI` element, or any other element or attribute name (ADR 0044
    /// D1).
    EmfXmiNamespace,
    /// An `XMI` element in neither XMI namespace, or in none.
    OtherXmiNamespace,
    /// An encoding other than UTF-8, ASCII, ISO-8859-1 and UTF-16 with a
    /// mark.
    Encoding,
    /// More nodes, attributes or namespaces than the parser can hold.
    Capacity,
    /// An element the document could not hold, which checked input never
    /// gives.
    Unheld,
    /// An attribute, text, or an XMI-namespace element XMI 2.5.1 does not
    /// define, on or under the `xmi:XMI` element, which has no element to
    /// keep it (ADR 0042 D4).
    WrapperContent,
    /// A preserved attribute whose prefix `sacm` or `xmi` is bound to
    /// another namespace, which the written start tag binds to SACM 2.3 or
    /// XMI 2.5.1.
    UnwritablePrefix,
}

impl Unread {
    /// Why it is refused, with the row that reads it.
    pub(super) const fn refusal(self) -> Refusal {
        match self {
            Self::OtherSacmNamespace | Self::OtherRevision => Refusal::Tolerated("LIB-COMPAT-004"),
            Self::EmfXmiNamespace => Refusal::Tolerated("LIB-COMPAT-002"),
            Self::Sacm24
            | Self::OtherXmiNamespace
            | Self::Encoding
            | Self::Capacity
            | Self::Unheld => Refusal::Limit,
            Self::WrapperContent | Self::UnwritablePrefix => Refusal::Defect,
        }
    }

    /// What a tolerant read does with it: the one table of ADR 0035 D1 and
    /// D2. A defect is kept with SACM-XMI-003; a root in an older SACM
    /// revision's or a SACM-like namespace is read, with SACM-XMI-008 or
    /// SACM-XMI-002, and EMF's XMI namespace with SACM-XMI-015; a SACM 2.4
    /// root, an `XMI` element in any other namespace, and the limits are
    /// refused.
    pub(super) const fn tolerant(self) -> Tolerant {
        match self {
            Self::WrapperContent | Self::UnwritablePrefix => {
                Tolerant::Kept(Code::XMI_UNKNOWN_CONTENT)
            }
            Self::OtherRevision => Tolerant::Read(Code::XMI_OTHER_REVISION),
            Self::OtherSacmNamespace => Tolerant::Read(Code::XMI_NAMESPACE_NOT_PINNED),
            Self::EmfXmiNamespace => Tolerant::Read(Code::XMI_WRAPPER_NAMESPACE),
            Self::Sacm24
            | Self::OtherXmiNamespace
            | Self::Encoding
            | Self::Capacity
            | Self::Unheld => Tolerant::Refused,
        }
    }

    /// The catalog code of its refusal: the code a tolerant read keeps or
    /// reads it with, and, for what a tolerant read refuses too, SACM-XMI-008
    /// for a SACM 2.4 root, SACM-XMI-015 for an `XMI`
    /// element in another namespace, SACM-XML-002 for an encoding and
    /// SACM-SEC-002 for a limit of the parser or the document (LIB-API-017,
    /// ADR 0030 amendment 3). The tests check each entry against it.
    #[cfg(test)]
    pub(super) const fn code(self) -> Code {
        match self {
            Self::WrapperContent | Self::UnwritablePrefix => Code::XMI_UNKNOWN_CONTENT,
            Self::OtherSacmNamespace => Code::XMI_NAMESPACE_NOT_PINNED,
            Self::OtherRevision | Self::Sacm24 => Code::XMI_OTHER_REVISION,
            Self::EmfXmiNamespace | Self::OtherXmiNamespace => Code::XMI_WRAPPER_NAMESPACE,
            Self::Encoding => Code::XML_ENCODING_NOT_READ,
            Self::Capacity | Self::Unheld => Code::PARSER_LIMIT,
        }
    }
}

/// The label a reason of the refusal of `code` ends with, given the row the
/// refusal names: `(a limit of this version; ROW reads it)` for
/// SACM-XMI-007, `(strict mode never reads it; ROW reads it in tolerant
/// mode)` for SACM-XMI-002, SACM-XMI-008 and SACM-XMI-015 (ADR 0041 D6, ADR
/// 0044 D1), `(a defect in the input)` for SACM-XMI-003, and `(a limit of
/// this version)` for every other, SACM-XMI-008 without a row (a SACM 2.4
/// root) and SACM-XMI-015 without one (an `XMI` element in another
/// namespace) included (ADR 0030 amendment 3).
pub(super) fn label(code: Code, row: Option<&str>) -> String {
    let tolerated = [
        Code::XMI_NAMESPACE_NOT_PINNED,
        Code::XMI_OTHER_REVISION,
        Code::XMI_WRAPPER_NAMESPACE,
    ];
    match row {
        Some(row) if code == Code::XMI_EXTERNAL_HREF => {
            format!("(a limit of this version; {row} reads it)")
        }
        Some(row) if tolerated.contains(&code) => {
            format!("(strict mode never reads it; {row} reads it in tolerant mode)")
        }
        _ if code == Code::XMI_UNKNOWN_CONTENT => "(a defect in the input)".to_owned(),
        _ => "(a limit of this version)".to_owned(),
    }
}

/// The reason of the refusal `details` give: what is not read, and why,
/// followed by the label of its code and its `row` argument.
pub(super) fn reason(details: &Details) -> String {
    format!(
        "{} {}",
        details.render(),
        label(details.code(), details.text("row"))
    )
}

/// The refusal of `unread` at `location`, whose message `details` give: what
/// is not read, without the row, which this adds as the last argument when
/// the entry names one. The one place the crate builds
/// `ReadError::Unsupported`.
pub(super) fn unsupported_at(
    location: Option<SourceLocation>,
    unread: Unread,
    details: Details,
) -> ReadError {
    let details = match unread.refusal().row() {
        Some(row) => details.with(Argument::text("row", row)),
        None => details,
    };
    from_details(location, details)
}

/// The refusal at `location` whose message, the row included, `details`
/// give.
fn from_details(location: Option<SourceLocation>, details: Details) -> ReadError {
    ReadError::Unsupported {
        location,
        reason: reason(&details),
        details,
    }
}

/// The refusal of `unread` at byte `offset` of `text`, as
/// [`unsupported_at`] builds it.
pub(super) fn unsupported(
    text: &str,
    offset: usize,
    unread: Unread,
    details: Details,
) -> ReadError {
    unsupported_at(locate(text, offset), unread, details)
}

/// The serialized form of [`ReadError`] and [`WriteError`](super::WriteError)
/// (LIB-API-017; ADR 0030 amendment 3, R8, R10 and R11). It lives in this
/// file because it names the variant this list builds.
///
/// A refusal serializes as `{"type", "code", "template", "arguments", …its
/// fields}`, every key required, and reads back only when its code is one of
/// its variant's, its template one of its variant's with that code, and its
/// arguments exactly the template's, by name and kind, in any key order (ADR
/// 0030 amendment 5).
#[cfg(feature = "serde")]
mod record {
    use serde::{Deserialize, Serialize};

    use super::super::{Limit, ReadError, WriteAttribute, WriteError};
    use crate::diagnostic::template::Carrier;
    use crate::diagnostic::{
        ArgumentsRecord, Code, Details, Diagnostic, SourceLocation, details_from_record,
    };
    use crate::model::{ElementId, ElementKind};

    /// An optional location, which must be given, as `null` for none.
    type Location = Option<SourceLocation>;

    /// A [`ReadError`] as serialized.
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", deny_unknown_fields)]
    enum ReadErrorRecord {
        Doctype {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        Malformed {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
            reason: String,
        },
        InvalidRoot {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
            element_name: String,
        },
        NoPackage {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        DuplicateId {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            id: ElementId,
            locations: Vec<SourceLocation>,
        },
        InvalidId {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            id: String,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        Unsupported {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
            reason: String,
        },
        RepeatedValue {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            element: ElementId,
            kind: ElementKind,
            property: String,
            locations: Vec<SourceLocation>,
        },
        InvalidLiteral {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            element: ElementId,
            property: String,
            value: String,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        LegacyShape {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            element: ElementId,
            shape: String,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        MissingType {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            role: String,
            declared_type: String,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
        LimitExceeded {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            limit: Limit,
            max: u64,
            #[serde(deserialize_with = "serde::Deserialize::deserialize")]
            location: Location,
        },
    }

    /// The code, template and arguments of a record of `error`.
    fn head(
        code: Code,
        template: &str,
        details: Option<&Details>,
    ) -> (Code, String, ArgumentsRecord) {
        (
            code,
            template.to_owned(),
            ArgumentsRecord::of(details.map_or(&[], Details::arguments)),
        )
    }

    impl From<&ReadError> for ReadErrorRecord {
        #[expect(
            clippy::too_many_lines,
            reason = "one arm per variant, each a field-for-field copy"
        )]
        fn from(error: &ReadError) -> Self {
            let (code, template, arguments) = head(error.code(), error.template(), error.details());
            match error.clone() {
                ReadError::Doctype { location } => Self::Doctype {
                    code,
                    template,
                    arguments,
                    location,
                },
                ReadError::Malformed {
                    location, reason, ..
                } => Self::Malformed {
                    code,
                    template,
                    arguments,
                    location,
                    reason,
                },
                ReadError::InvalidRoot {
                    location,
                    element_name,
                    ..
                } => Self::InvalidRoot {
                    code,
                    template,
                    arguments,
                    location,
                    element_name,
                },
                ReadError::NoPackage { location } => Self::NoPackage {
                    code,
                    template,
                    arguments,
                    location,
                },
                ReadError::DuplicateId { id, locations, .. } => Self::DuplicateId {
                    code,
                    template,
                    arguments,
                    id,
                    locations,
                },
                ReadError::InvalidId { id, location, .. } => Self::InvalidId {
                    code,
                    template,
                    arguments,
                    id,
                    location,
                },
                ReadError::Unsupported {
                    location, reason, ..
                } => Self::Unsupported {
                    code,
                    template,
                    arguments,
                    location,
                    reason,
                },
                ReadError::RepeatedValue {
                    element,
                    kind,
                    property,
                    locations,
                    ..
                } => Self::RepeatedValue {
                    code,
                    template,
                    arguments,
                    element,
                    kind,
                    property,
                    locations,
                },
                ReadError::InvalidLiteral {
                    element,
                    property,
                    value,
                    location,
                    ..
                } => Self::InvalidLiteral {
                    code,
                    template,
                    arguments,
                    element,
                    property,
                    value,
                    location,
                },
                ReadError::LegacyShape {
                    element,
                    shape,
                    location,
                    ..
                } => Self::LegacyShape {
                    code,
                    template,
                    arguments,
                    element,
                    shape,
                    location,
                },
                ReadError::MissingType {
                    role,
                    declared_type,
                    location,
                    ..
                } => Self::MissingType {
                    code,
                    template,
                    arguments,
                    role,
                    declared_type,
                    location,
                },
                ReadError::LimitExceeded {
                    limit,
                    max,
                    location,
                    ..
                } => Self::LimitExceeded {
                    code,
                    template,
                    arguments,
                    limit,
                    max,
                    location,
                },
            }
        }
    }

    impl TryFrom<ReadErrorRecord> for ReadError {
        type Error = String;

        #[expect(
            clippy::too_many_lines,
            reason = "one arm per variant, each a field-for-field copy"
        )]
        fn try_from(record: ReadErrorRecord) -> Result<Self, String> {
            let read = |variant, code, template: &str, arguments| {
                details_from_record(Carrier::Read(variant), code, template, arguments)
            };
            Ok(match record {
                ReadErrorRecord::Doctype {
                    code,
                    template,
                    arguments,
                    location,
                } => {
                    read("Doctype", code, &template, arguments)?;
                    Self::Doctype { location }
                }
                ReadErrorRecord::Malformed {
                    code,
                    template,
                    arguments,
                    location,
                    reason,
                } => Self::Malformed {
                    location,
                    reason,
                    details: read("Malformed", code, &template, arguments)?,
                },
                ReadErrorRecord::InvalidRoot {
                    code,
                    template,
                    arguments,
                    location,
                    element_name,
                } => Self::InvalidRoot {
                    location,
                    element_name,
                    details: read("InvalidRoot", code, &template, arguments)?,
                },
                ReadErrorRecord::NoPackage {
                    code,
                    template,
                    arguments,
                    location,
                } => {
                    read("NoPackage", code, &template, arguments)?;
                    Self::NoPackage { location }
                }
                ReadErrorRecord::DuplicateId {
                    code,
                    template,
                    arguments,
                    id,
                    locations,
                } => Self::DuplicateId {
                    id,
                    locations,
                    details: read("DuplicateId", code, &template, arguments)?,
                },
                ReadErrorRecord::InvalidId {
                    code,
                    template,
                    arguments,
                    id,
                    location,
                } => Self::InvalidId {
                    id,
                    location,
                    details: read("InvalidId", code, &template, arguments)?,
                },
                ReadErrorRecord::Unsupported {
                    code,
                    template,
                    arguments,
                    location,
                    reason,
                } => {
                    // A record keeps its reason as given (ADR 0030
                    // amendment 3, "Records").
                    ReadError::Unsupported {
                        location,
                        reason,
                        details: details_from_record(Carrier::Unread, code, &template, arguments)?,
                    }
                }
                ReadErrorRecord::RepeatedValue {
                    code,
                    template,
                    arguments,
                    element,
                    kind,
                    property,
                    locations,
                } => Self::RepeatedValue {
                    element,
                    kind,
                    property,
                    locations,
                    details: read("RepeatedValue", code, &template, arguments)?,
                },
                ReadErrorRecord::InvalidLiteral {
                    code,
                    template,
                    arguments,
                    element,
                    property,
                    value,
                    location,
                } => Self::InvalidLiteral {
                    element,
                    property,
                    value,
                    location,
                    details: read("InvalidLiteral", code, &template, arguments)?,
                },
                ReadErrorRecord::LegacyShape {
                    code,
                    template,
                    arguments,
                    element,
                    shape,
                    location,
                } => Self::LegacyShape {
                    element,
                    shape,
                    location,
                    details: read("LegacyShape", code, &template, arguments)?,
                },
                ReadErrorRecord::MissingType {
                    code,
                    template,
                    arguments,
                    role,
                    declared_type,
                    location,
                } => Self::MissingType {
                    role,
                    declared_type,
                    location,
                    details: read("MissingType", code, &template, arguments)?,
                },
                ReadErrorRecord::LimitExceeded {
                    code,
                    template,
                    arguments,
                    limit,
                    max,
                    location,
                } => Self::LimitExceeded {
                    limit,
                    max,
                    location,
                    details: read("LimitExceeded", code, &template, arguments)?,
                },
            })
        }
    }

    /// A [`ReadError`] serializes through its record (ADR 0030 amendment 3,
    /// R10).
    impl Serialize for ReadError {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            ReadErrorRecord::from(self).serialize(serializer)
        }
    }

    /// A [`ReadError`] reads back from its record, which must be consistent
    /// (ADR 0030 amendment 3, R11).
    impl<'de> Deserialize<'de> for ReadError {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let record = ReadErrorRecord::deserialize(deserializer)?;
            Self::try_from(record).map_err(serde::de::Error::custom)
        }
    }

    /// A [`WriteError`] as serialized.
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", deny_unknown_fields)]
    enum WriteErrorRecord {
        NoRoot {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
        },
        UnrepresentableCharacter {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            element: ElementId,
            character: char,
            attribute: WriteAttribute,
            // Left out, as 0.4.0 wrote it: `None` (LIB-API-013).
            #[serde(default)]
            position: Option<usize>,
        },
        Invalid {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            diagnostics: Vec<Diagnostic>,
        },
        PreservedContent {
            code: Code,
            template: String,
            arguments: ArgumentsRecord,
            elements: Vec<ElementId>,
        },
    }

    impl From<&WriteError> for WriteErrorRecord {
        fn from(error: &WriteError) -> Self {
            let (code, template, arguments) = head(error.code(), error.template(), error.details());
            match error.clone() {
                WriteError::NoRoot => Self::NoRoot {
                    code,
                    template,
                    arguments,
                },
                WriteError::UnrepresentableCharacter {
                    element,
                    character,
                    attribute,
                    position,
                    ..
                } => Self::UnrepresentableCharacter {
                    code,
                    template,
                    arguments,
                    element,
                    character,
                    attribute,
                    position,
                },
                WriteError::Invalid { diagnostics, .. } => Self::Invalid {
                    code,
                    template,
                    arguments,
                    diagnostics,
                },
                WriteError::PreservedContent { elements, .. } => Self::PreservedContent {
                    code,
                    template,
                    arguments,
                    elements,
                },
            }
        }
    }

    impl TryFrom<WriteErrorRecord> for WriteError {
        type Error = String;

        fn try_from(record: WriteErrorRecord) -> Result<Self, String> {
            let write = |variant, code, template: &str, arguments| {
                details_from_record(Carrier::Write(variant), code, template, arguments)
            };
            Ok(match record {
                WriteErrorRecord::NoRoot {
                    code,
                    template,
                    arguments,
                } => {
                    write("NoRoot", code, &template, arguments)?;
                    Self::NoRoot
                }
                WriteErrorRecord::UnrepresentableCharacter {
                    code,
                    template,
                    arguments,
                    element,
                    character,
                    attribute,
                    position,
                } => Self::UnrepresentableCharacter {
                    element,
                    character,
                    attribute,
                    position,
                    details: write("UnrepresentableCharacter", code, &template, arguments)?,
                },
                WriteErrorRecord::Invalid {
                    code,
                    template,
                    arguments,
                    diagnostics,
                } => Self::Invalid {
                    diagnostics,
                    details: write("Invalid", code, &template, arguments)?,
                },
                WriteErrorRecord::PreservedContent {
                    code,
                    template,
                    arguments,
                    elements,
                } => Self::PreservedContent {
                    elements,
                    details: write("PreservedContent", code, &template, arguments)?,
                },
            })
        }
    }

    /// A [`WriteError`] serializes through its record (ADR 0030 amendment 3,
    /// R10).
    impl Serialize for WriteError {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            WriteErrorRecord::from(self).serialize(serializer)
        }
    }

    /// A [`WriteError`] reads back from its record, which must be consistent
    /// (ADR 0030 amendment 3, R11).
    impl<'de> Deserialize<'de> for WriteError {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let record = WriteErrorRecord::deserialize(deserializer)?;
            Self::try_from(record).map_err(serde::de::Error::custom)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{Refusal, Tolerant, Unread, label, unsupported_at};
    use crate::diagnostic::{Argument, Code, Details, template};
    use crate::xmi::ReadError;

    /// Every entry, in declaration order.
    const ALL: [Unread; 10] = [
        Unread::OtherSacmNamespace,
        Unread::OtherRevision,
        Unread::Sacm24,
        Unread::EmfXmiNamespace,
        Unread::OtherXmiNamespace,
        Unread::Encoding,
        Unread::Capacity,
        Unread::Unheld,
        Unread::WrapperContent,
        Unread::UnwritablePrefix,
    ];

    /// The position of `entry` in [`ALL`]. The match has no wildcard, so an
    /// entry added to [`Unread`] does not compile here until it is listed.
    const fn position(entry: Unread) -> usize {
        match entry {
            Unread::OtherSacmNamespace => 0,
            Unread::OtherRevision => 1,
            Unread::Sacm24 => 2,
            Unread::EmfXmiNamespace => 3,
            Unread::OtherXmiNamespace => 4,
            Unread::Encoding => 5,
            Unread::Capacity => 6,
            Unread::Unheld => 7,
            Unread::WrapperContent => 8,
            Unread::UnwritablePrefix => 9,
        }
    }

    /// The tolerant table covers every entry, refuses exactly the limits, a
    /// SACM 2.4 root and an `XMI` element in another namespace, reads a root
    /// in another SACM namespace and the names in EMF's XMI namespace, and
    /// keeps the rest with SACM-XMI-003
    /// (LIB-API-011; ADR 0035 D1, D2); an `Interim` entry, of which there is
    /// none since slice 0073, would be kept with SACM-XMI-007. Its name
    /// carries no row id: traced tests live in test files.
    #[test]
    fn the_tolerant_table_is_total_and_refuses_exactly_the_refused_set() {
        for (index, &entry) in ALL.iter().enumerate() {
            assert_eq!(
                position(entry),
                index,
                "LIB-API-011: {entry:?} is listed out of order"
            );
        }
        let refused: Vec<Unread> = ALL
            .into_iter()
            .filter(|entry| entry.tolerant() == Tolerant::Refused)
            .collect();
        assert_eq!(
            refused,
            [
                Unread::Sacm24,
                Unread::OtherXmiNamespace,
                Unread::Encoding,
                Unread::Capacity,
                Unread::Unheld,
            ],
            "LIB-API-011: the refused set"
        );
        for entry in ALL {
            let expected = match entry.refusal() {
                Refusal::Interim(_) => Tolerant::Kept(Code::XMI_EXTERNAL_HREF),
                Refusal::Defect => Tolerant::Kept(Code::XMI_UNKNOWN_CONTENT),
                Refusal::Tolerated(_) => Tolerant::Read(entry.code()),
                Refusal::Limit => Tolerant::Refused,
            };
            assert_eq!(entry.tolerant(), expected, "LIB-API-011: {entry:?}");
            // A kept entry that conforms names the row that reads it.
            if matches!(entry.tolerant(), Tolerant::Kept(code) if code != Code::XMI_UNKNOWN_CONTENT)
            {
                assert!(entry.refusal().row().is_some(), "LIB-API-011: {entry:?}");
            }
        }
    }

    #[test]
    fn every_reason_ends_with_its_label() {
        let reason = |unread, template, arguments| match unsupported_at(
            None,
            unread,
            Details::new(template, arguments),
        ) {
            ReadError::Unsupported { reason, .. } => reason,
            other => panic!("{other:?}"),
        };
        // No entry is Interim since slice 0073 (SACM23-XMI-029); the kind
        // keeps its label, by code and by kind, for a later row.
        let row = "SACM23-XMI-029";
        let interim = Refusal::Interim(row);
        assert_eq!(
            (
                label(Code::XMI_EXTERNAL_HREF, interim.row()),
                interim.label()
            ),
            (
                "(a limit of this version; SACM23-XMI-029 reads it)".to_owned(),
                "(a limit of this version; SACM23-XMI-029 reads it)".to_owned()
            )
        );
        assert!(
            ALL.iter()
                .all(|entry| !matches!(entry.refusal(), Refusal::Interim(_))),
            "SACM23-XMI-029: an entry is Interim"
        );
        assert_eq!(
            reason(
                Unread::OtherSacmNamespace,
                &template::XMI_002_OTHER_NAMESPACE,
                vec![
                    Argument::text("element", "x"),
                    Argument::text("namespace", "urn:sacm")
                ]
            ),
            "the root element \"x\" is in the namespace \"urn:sacm\", not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)"
        );
        assert_eq!(
            reason(
                Unread::Sacm24,
                &template::XMI_008_SACM_2_4,
                vec![
                    Argument::text("element", "x"),
                    Argument::text("namespace", "u")
                ]
            ),
            "the root element \"x\" is in the SACM 2.4 namespace \"u\", and SACM 2.4 restructures the metamodel, so it is not read as SACM 2.3 until LIB-COMPAT-018 decides how (a limit of this version)"
        );
        assert_eq!(
            reason(Unread::Capacity, &template::SEC_002_NODES, Vec::new()),
            "the document has more nodes or nesting levels than the parser can hold (a limit of this version)"
        );
        assert_eq!(
            reason(
                Unread::WrapperContent,
                &template::XMI_003_WRAPPER_TEXT,
                vec![Argument::text("element", "x")]
            ),
            "the element \"x\" holds text, which has no element to be kept in (a defect in the input)"
        );
        // The label by code is the label by the kind of refusal, for every
        // entry.
        for entry in ALL {
            assert_eq!(
                label(entry.code(), entry.refusal().row()),
                entry.refusal().label(),
                "LIB-API-017: {entry:?}"
            );
        }
    }

    /// `Unread::code()` is the code of the tolerant Warning for every entry
    /// a tolerant read keeps, and the stated code for the rest
    /// (LIB-API-017, ADR 0030 amendment 3).
    #[test]
    fn each_entry_has_the_code_a_tolerant_read_gives_it() {
        for entry in ALL {
            match entry.tolerant() {
                Tolerant::Kept(code) | Tolerant::Read(code) => {
                    assert_eq!(entry.code(), code, "LIB-API-017: {entry:?}");
                }
                Tolerant::Refused => {
                    let expected = match entry {
                        Unread::Sacm24 => Code::XMI_OTHER_REVISION,
                        Unread::OtherXmiNamespace => Code::XMI_WRAPPER_NAMESPACE,
                        Unread::Encoding => Code::XML_ENCODING_NOT_READ,
                        Unread::Capacity | Unread::Unheld => Code::PARSER_LIMIT,
                        other => panic!("LIB-API-017: {other:?} is refused tolerantly"),
                    };
                    assert_eq!(entry.code(), expected, "LIB-API-017: {entry:?}");
                }
            }
        }
        // An Interim entry would be SACM-XMI-007 (none is, since slice
        // 0073); each Defect is SACM-XMI-003.
        for entry in ALL {
            let expected = match entry.refusal() {
                Refusal::Interim(_) => Some(Code::XMI_EXTERNAL_HREF),
                Refusal::Defect => Some(Code::XMI_UNKNOWN_CONTENT),
                Refusal::Tolerated(_) | Refusal::Limit => None,
            };
            if let Some(expected) = expected {
                assert_eq!(entry.code(), expected, "LIB-API-017: {entry:?}");
            }
        }
    }

    /// The SACM-SEC-002 refusals, which no input of a test's size reaches,
    /// built from each template, as `unsupported_at` builds them (LIB-API-017,
    /// acceptance 4).
    #[test]
    fn the_limit_refusals_have_their_code_template_and_arguments() {
        let cases = [
            (
                Unread::Capacity,
                &template::SEC_002_NODES,
                vec![],
                "the document has more nodes or nesting levels than the parser can hold",
            ),
            (
                Unread::Capacity,
                &template::SEC_002_ATTRIBUTES,
                vec![],
                "the document has more attributes than the parser can hold",
            ),
            (
                Unread::Capacity,
                &template::SEC_002_NAMESPACES,
                vec![],
                "the document declares more namespaces than the parser can hold",
            ),
            (
                Unread::Unheld,
                &template::SEC_002_UNHELD_OUTSIDE,
                vec![Argument::prose("error", "e")],
                "the content outside every element cannot be held in the document: e",
            ),
            (
                Unread::Unheld,
                &template::SEC_002_UNHELD_ROOT,
                vec![
                    Argument::text("element", "r"),
                    Argument::prose("error", "e"),
                ],
                "the root element \"r\" cannot be held in the document: e",
            ),
            (
                Unread::Unheld,
                &template::SEC_002_UNHELD_ELEMENT,
                vec![
                    Argument::text("element", "c"),
                    Argument::prose("error", "e"),
                ],
                "the element \"c\" cannot be held in the document: e",
            ),
        ];
        for (unread, template, arguments, english) in cases {
            let error = unsupported_at(None, unread, Details::new(template, arguments.clone()));
            assert_eq!(
                error.code(),
                Code::PARSER_LIMIT,
                "LIB-API-017: {}",
                template.id
            );
            assert_eq!(error.template(), template.id, "LIB-API-017");
            assert_eq!(
                error.arguments(),
                arguments.as_slice(),
                "LIB-API-017: {}",
                template.id
            );
            assert_eq!(
                error.to_string(),
                format!(
                    "the reader cannot represent the input: {english} (a limit of this version)"
                ),
                "LIB-API-017: {}",
                template.id
            );
        }
    }

    #[test]
    fn no_entry_names_the_spelling_row() {
        // The spellings are read, not refused (SACM23-XMI-009): no entry
        // names that row.
        for entry in ALL {
            assert_ne!(entry.refusal().row(), Some("SACM23-XMI-009"), "{entry:?}");
        }
    }
}
