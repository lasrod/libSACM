//! Helpers shared by the integration tests.
//!
//! Each test file that uses them declares `mod common;`.

/// The catalog code of a `ReadError::Unsupported` refusal, from the label its
/// reason and its `Display` end with (LIB-API-017; ADR 0030 amendment 3):
/// SACM-XMI-014 for `(a limit of this version; ROW reads it)`, except
/// SACM-XMI-007 when ROW is SACM23-XMI-027, which reads cross-document
/// references; SACM-XMI-002 and SACM-XMI-015 for the tolerances of
/// LIB-COMPAT-004 and LIB-COMPAT-002; SACM-XMI-003 for `(a defect in the
/// input)`; and, for `(a limit of this version)`, SACM-SEC-002 for a parser
/// capacity limit and SACM-XML-002 for an encoding.
///
/// # Panics
///
/// If `text` ends with no label: a test of an `Unsupported` refusal then fails.
pub(crate) fn unsupported_code(text: &str) -> &'static str {
    if text.ends_with("(a limit of this version; SACM23-XMI-027 reads it)") {
        "SACM-XMI-007"
    } else if text.ends_with(" reads it)") && text.contains("(a limit of this version; ") {
        "SACM-XMI-014"
    } else if text
        .ends_with("(strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)")
    {
        "SACM-XMI-002"
    } else if text
        .ends_with("(strict mode never reads it; LIB-COMPAT-002 is to read it in tolerant mode)")
    {
        "SACM-XMI-015"
    } else if text.ends_with("(a defect in the input)") {
        "SACM-XMI-003"
    } else if text.ends_with("than the parser can hold (a limit of this version)") {
        "SACM-SEC-002"
    } else if text.ends_with("(a limit of this version)") {
        "SACM-XML-002"
    } else {
        panic!("LIB-API-017: test oracle: {text:?} ends with no Unsupported label")
    }
}
