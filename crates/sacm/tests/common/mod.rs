//! Helpers shared by the integration tests.
//!
//! Each test file that uses them declares `mod common;`.

/// The catalog code of a `ReadError::Unsupported` refusal, from the label its
/// reason and its `Display` end with (LIB-API-017; ADR 0030 amendment 3):
/// SACM-XMI-007 for `(a limit of this version; SACM23-XMI-029 reads it)`,
/// the cross-document links SACM23-XMI-027 does not read (slice 0068, ADR
/// 0040), the only `Interim` label left since slice 0073a reads XMI
/// metadata and retires SACM-XMI-014 (SACM23-XMI-019, ADR 0042); for
/// `(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)`
/// (ADR 0041 D6), SACM-XMI-008 for a root in an older SACM revision's
/// namespace, whose reason names the revision (`… of SACM 2.2, not …`), and
/// SACM-XMI-002 for a SACM-like one; SACM-XMI-015 for the tolerance of
/// LIB-COMPAT-002; SACM-XMI-003 for `(a defect in the input)`; and, for
/// `(a limit of this version)`, SACM-XMI-008 for a SACM 2.4 root, whose
/// reason names LIB-COMPAT-018, SACM-SEC-002 for a parser capacity limit and
/// SACM-XML-002 for an encoding.
///
/// # Panics
///
/// If `text` ends with no label: a test of an `Unsupported` refusal then fails.
pub(crate) fn unsupported_code(text: &str) -> &'static str {
    if text.ends_with("(a limit of this version; SACM23-XMI-029 reads it)") {
        "SACM-XMI-007"
    } else if text
        .ends_with("(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)")
    {
        if text.contains("\" of SACM 2.") {
            "SACM-XMI-008"
        } else {
            "SACM-XMI-002"
        }
    } else if text
        .ends_with("(strict mode never reads it; LIB-COMPAT-002 is to read it in tolerant mode)")
    {
        "SACM-XMI-015"
    } else if text.ends_with("(a defect in the input)") {
        "SACM-XMI-003"
    } else if text.ends_with("until LIB-COMPAT-018 decides how (a limit of this version)") {
        "SACM-XMI-008"
    } else if text.ends_with("than the parser can hold (a limit of this version)") {
        "SACM-SEC-002"
    } else if text.ends_with("(a limit of this version)") {
        "SACM-XML-002"
    } else {
        panic!("LIB-API-017: test oracle: {text:?} ends with no Unsupported label")
    }
}
