//! Tests for the error convention and for panic freedom on hostile input
//! (LIB-API-004, ADR 0011).
//!
//! Today the only public error type is `model::InsertError`, public under the
//! `test-support` feature. The crate dev-depends on itself with that feature,
//! so these tests always see it. Each later error enum (`xmi::ReadError`,
//! `xmi::WriteError`, `edit::EditError`) joins the checks here with its row.
//!
//! Not checked here:
//! - Item 1's `#[non_exhaustive]`: a test cannot show that an exhaustive match
//!   fails to compile. The verifier checks the attribute in the source and the
//!   rustdoc.
//! - Item 5 (`Document::insert` documents `# Errors` with each variant it
//!   returns; no public item has a `# Panics` section): a rustdoc review item.
//! - Item 7 (ADR 0011 is accepted and names per-operation error enums; no
//!   crate-wide `Error` exists): a document and API review item.

use std::error::Error;
use std::fmt::{Debug, Display};

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ElementKind, InsertError, LangString, SacmElement};

/// One mebibyte.
const MIB: usize = 1 << 20;

fn assert_error_type<T: Error + Debug + Display + Send + Sync + 'static>() {}

/// Attempts an insert that the test expects to be refused, and returns why.
fn refusal(doc: &mut Document, parent: Option<&str>, id: &str) -> InsertError {
    let parent = parent.map(ElementId::from);
    let Err(error) = doc.insert(
        parent.as_ref(),
        ElementKind::AssuranceCasePackage,
        ElementId::from(id),
        None,
    ) else {
        panic!("LIB-API-004: insert accepted {id:?} under {parent:?}");
    };
    error
}

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), None);
    assert_eq!(
        result,
        Ok(()),
        "LIB-API-004: setup could not insert an id of {} bytes",
        id.len()
    );
}

/// Every `InsertError` variant, each produced by `Document::insert`.
fn every_insert_error() -> Vec<(&'static str, InsertError)> {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    vec![
        ("EmptyId", refusal(&mut doc, None, "")),
        ("DuplicateId", refusal(&mut doc, None, "case")),
        (
            "UnknownParent",
            refusal(&mut doc, Some("nowhere"), "orphan"),
        ),
        ("NameNotAllowed", named_note_refusal(&mut doc)),
        (
            "ContainmentNotAllowed",
            containment_refusal(&mut doc, "case", "c1"),
        ),
    ]
}

/// Inserts a Claim directly under the AssuranceCasePackage `parent`, which the
/// seam refuses: no role of an AssuranceCasePackage admits a Claim
/// (LIB-API-008).
fn containment_refusal(doc: &mut Document, parent: &str, id: &str) -> InsertError {
    let Err(error) = doc.insert(
        Some(&ElementId::from(parent)),
        ElementKind::Claim,
        ElementId::from(id),
        None,
    ) else {
        panic!(
            "LIB-API-004: insert accepted a Claim {id:?} under the AssuranceCasePackage {parent:?}"
        );
    };
    error
}

/// Inserts a Note with a name, which the seam refuses: a `UtilityElement` has
/// no name.
fn named_note_refusal(doc: &mut Document) -> InsertError {
    let Err(error) = doc.insert(
        None,
        ElementKind::Note,
        ElementId::from("note_1"),
        Some(LangString::default()),
    ) else {
        panic!("LIB-API-004: insert accepted a Note with a name");
    };
    error
}

/// Checks the ADR 0011 message rules: non-empty, one line, starting lowercase
/// and not ending in `.`, `!` or `:`.
fn assert_message_rules(what: &str, message: &str) {
    assert!(
        !message.is_empty(),
        "LIB-API-004: the {what} message is empty"
    );
    assert!(
        !message.contains(['\n', '\r']),
        "LIB-API-004: the {what} message is not one line: {message:?}"
    );
    assert!(
        message.chars().next().is_some_and(char::is_lowercase),
        "LIB-API-004: the {what} message does not start lowercase: {message:?}"
    );
    assert!(
        !message.ends_with(['.', '!', ':']),
        "LIB-API-004: the {what} message ends in punctuation: {message:?}"
    );
}

// --- 1, 2. Traits ---------------------------------------------------------

#[test]
fn lib_api_004_insert_error_is_a_send_sync_static_error() {
    // Checked statically: this does not compile unless the bounds hold.
    assert_error_type::<InsertError>();
}

#[test]
fn lib_api_004_insert_error_boxes_and_downcasts_as_a_send_sync_error() {
    for (variant, error) in every_insert_error() {
        let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(error.clone());
        assert_eq!(
            boxed.to_string(),
            error.to_string(),
            "LIB-API-004: boxing {variant} changed its message"
        );
        assert_eq!(
            boxed.downcast_ref::<InsertError>(),
            Some(&error),
            "LIB-API-004: a boxed {variant} does not downcast back to itself"
        );
    }
}

#[test]
fn lib_api_004_insert_produces_each_variant_with_its_offending_id() {
    let errors: Vec<InsertError> = every_insert_error()
        .into_iter()
        .map(|(_, error)| error)
        .collect();
    assert_eq!(
        errors,
        [
            InsertError::EmptyId,
            InsertError::DuplicateId(ElementId::from("case")),
            InsertError::UnknownParent(ElementId::from("nowhere")),
            InsertError::NameNotAllowed(ElementId::from("note_1"), ElementKind::Note),
            InsertError::ContainmentNotAllowed {
                id: ElementId::from("c1"),
                kind: ElementKind::Claim,
                parent: ElementId::from("case"),
                parent_kind: ElementKind::AssuranceCasePackage,
            },
        ],
        "LIB-API-004: insert did not report the expected variants"
    );
}

#[test]
fn lib_api_004_insert_error_debug_names_the_variant() {
    for (variant, error) in every_insert_error() {
        let debug = format!("{error:?}");
        assert!(
            debug.starts_with(variant),
            "LIB-API-004: Debug of {variant} does not name it: {debug}"
        );
    }
}

// --- 3. Messages ----------------------------------------------------------

#[test]
fn lib_api_004_insert_error_messages_follow_the_message_rules() {
    for (variant, error) in every_insert_error() {
        assert_message_rules(variant, &error.to_string());
    }
}

/// Ids that would break the message rules if a message ended with the id
/// unquoted, or started with it.
const PUNCTUATED_IDS: [&str; 7] = [
    "Claim.",
    "claim!",
    "claim:",
    "UPPER",
    "ünïcödé-標識",
    "a\0b",
    " padded ",
];

#[test]
fn lib_api_004_insert_error_messages_follow_the_rules_for_awkward_ids() {
    for id in PUNCTUATED_IDS {
        let mut doc = Document::new();
        add(&mut doc, None, ElementKind::AssuranceCasePackage, id);
        let duplicate = refusal(&mut doc, None, id);
        assert_eq!(
            duplicate,
            InsertError::DuplicateId(ElementId::from(id)),
            "LIB-API-004: re-inserting {id:?} is not DuplicateId"
        );
        assert_message_rules(&format!("DuplicateId({id:?})"), &duplicate.to_string());

        let unknown = refusal(&mut Document::new(), Some(id), "child");
        assert_eq!(
            unknown,
            InsertError::UnknownParent(ElementId::from(id)),
            "LIB-API-004: inserting under a missing {id:?} is not UnknownParent"
        );
        assert_message_rules(&format!("UnknownParent({id:?})"), &unknown.to_string());

        // The id as the parent that refuses a child, and as the refused child.
        for (parent, child) in [(id, "c1"), ("case", id)] {
            let mut doc = Document::new();
            add(&mut doc, None, ElementKind::AssuranceCasePackage, parent);
            let refused = containment_refusal(&mut doc, parent, child);
            assert_eq!(
                refused,
                InsertError::ContainmentNotAllowed {
                    id: ElementId::from(child),
                    kind: ElementKind::Claim,
                    parent: ElementId::from(parent),
                    parent_kind: ElementKind::AssuranceCasePackage,
                },
                "LIB-API-004: a Claim {child:?} under {parent:?} is not ContainmentNotAllowed"
            );
            assert_message_rules(
                &format!("ContainmentNotAllowed({child:?} under {parent:?})"),
                &refused.to_string(),
            );
        }
    }
}

#[test]
fn lib_api_004_insert_error_messages_stay_one_line_for_ids_with_line_breaks() {
    // ElementId keeps any string, so an id read from a file may hold a line
    // break. The message must still be one line.
    for id in ["claim\ntop", "claim\r\ntop", "trailing\n", "\rleading"] {
        let mut doc = Document::new();
        add(&mut doc, None, ElementKind::AssuranceCasePackage, id);
        assert_message_rules(
            &format!("DuplicateId({id:?})"),
            &refusal(&mut doc, None, id).to_string(),
        );
        assert_message_rules(
            &format!("UnknownParent({id:?})"),
            &refusal(&mut Document::new(), Some(id), "child").to_string(),
        );
        let child = format!("{id}_child");
        assert_message_rules(
            &format!("ContainmentNotAllowed({child:?} under {id:?})"),
            &containment_refusal(&mut doc, id, &child).to_string(),
        );
    }
}

// --- 4. Sources -----------------------------------------------------------

#[test]
fn lib_api_004_insert_error_has_no_source() {
    for (variant, error) in every_insert_error() {
        assert!(
            error.source().is_none(),
            "LIB-API-004: {variant} wraps no cause but has a source"
        );
    }
}

// --- 6. No panics on hostile input: the document --------------------------

#[test]
fn lib_api_004_insert_refuses_an_empty_id_as_root_or_child() {
    let mut doc = Document::new();
    assert_eq!(
        refusal(&mut doc, None, ""),
        InsertError::EmptyId,
        "LIB-API-004: an empty root id is not EmptyId"
    );
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    assert_eq!(
        refusal(&mut doc, Some("case"), ""),
        InsertError::EmptyId,
        "LIB-API-004: an empty child id is not EmptyId"
    );
    assert_eq!(
        doc.element_count(),
        1,
        "LIB-API-004: a refused empty id changed the document"
    );
    assert!(
        doc.get("").is_none(),
        "LIB-API-004: the document has an element with the empty id"
    );
}

/// Ids with NUL and non-ASCII content. All are distinct and valid for `insert`.
const ODD_IDS: [&str; 9] = [
    "\0",
    "a\0b",
    "Ünïcödé-ид-標識",
    "\u{FEFF}bom",
    "e\u{301}",
    "\u{E9}",
    "🛡️",
    "\u{10FFFF}",
    "tab\tand\nnewline",
];

#[test]
fn lib_api_004_insert_and_lookup_keep_nul_and_non_ascii_ids() {
    let mut doc = Document::new();
    // An ArgumentPackage, as a Claim can be contained in one (LIB-API-008).
    add(&mut doc, None, ElementKind::ArgumentPackage, "case");
    for id in ODD_IDS {
        add(&mut doc, Some("case"), ElementKind::Claim, id);
    }
    assert_eq!(
        doc.element_count(),
        ODD_IDS.len() + 1,
        "LIB-API-004: odd ids collapsed into each other"
    );
    for id in ODD_IDS {
        let Some(element) = doc.get(id) else {
            panic!("LIB-API-004: {id:?} cannot be looked up");
        };
        assert_eq!(
            element.id().as_str(),
            id,
            "LIB-API-004: {id:?} came back changed"
        );
        assert_eq!(
            element.parent().map(|parent| parent.id().as_str()),
            Some("case"),
            "LIB-API-004: {id:?} lost its parent"
        );
    }
    let debug = format!("{doc:?}");
    for id in ODD_IDS {
        assert!(
            debug.contains(&format!("{id:?}")),
            "LIB-API-004: Debug of the document does not show {id:?}"
        );
    }
}

#[test]
fn lib_api_004_insert_and_lookup_handle_a_one_mebibyte_id() {
    let big = "x".repeat(MIB);
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, &big);
    add(
        &mut doc,
        Some(&big),
        ElementKind::ArgumentPackage,
        "arguments",
    );

    let Some(element) = doc.get(&big) else {
        panic!("LIB-API-004: a 1 MiB id cannot be looked up");
    };
    assert_eq!(
        element.id().as_str().len(),
        MIB,
        "LIB-API-004: a 1 MiB id came back with another length"
    );
    assert!(
        doc.get(&big[..MIB - 1]).is_none(),
        "LIB-API-004: a 1 MiB id matched its own prefix"
    );
    assert_eq!(
        doc.get("arguments")
            .and_then(SacmElement::parent)
            .map(|parent| parent.id().as_str().len()),
        Some(MIB),
        "LIB-API-004: the child of a 1 MiB id lost its parent"
    );

    let duplicate = refusal(&mut doc, None, &big);
    assert_eq!(
        duplicate,
        InsertError::DuplicateId(ElementId::from(big.as_str())),
        "LIB-API-004: re-inserting a 1 MiB id is not DuplicateId"
    );
    let message = duplicate.to_string();
    assert!(
        message.contains(&big),
        "LIB-API-004: the DuplicateId message does not name the 1 MiB id"
    );
    assert_message_rules("DuplicateId(1 MiB)", &message);

    let other = "y".repeat(MIB);
    assert_eq!(
        refusal(&mut doc, Some(&other), "orphan"),
        InsertError::UnknownParent(ElementId::from(other.as_str())),
        "LIB-API-004: a missing 1 MiB parent is not UnknownParent"
    );
    assert_eq!(
        doc.element_count(),
        2,
        "LIB-API-004: refused 1 MiB inserts changed the document"
    );
    assert!(
        format!("{doc:?}").len() > MIB,
        "LIB-API-004: Debug of a document with a 1 MiB id does not show it"
    );
}

#[test]
fn lib_api_004_insert_refuses_every_kind_of_duplicate_and_keeps_the_first() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    add(
        &mut doc,
        Some("case"),
        ElementKind::ArgumentPackage,
        "arguments",
    );
    add(&mut doc, Some("arguments"), ElementKind::Claim, "claim");
    // A root again, a child of another parent, and a child of itself.
    for (parent, id) in [
        (None, "claim"),
        (Some("case"), "claim"),
        (Some("claim"), "claim"),
        (Some("arguments"), "case"),
        (None, "case"),
    ] {
        assert_eq!(
            refusal(&mut doc, parent, id),
            InsertError::DuplicateId(ElementId::from(id)),
            "LIB-API-004: inserting {id:?} again under {parent:?} is not DuplicateId"
        );
    }
    assert_eq!(
        doc.element_count(),
        3,
        "LIB-API-004: refused duplicates changed the element count"
    );
    assert_eq!(
        doc.get("claim").map(SacmElement::kind),
        Some(ElementKind::Claim),
        "LIB-API-004: a refused duplicate replaced the first element"
    );
    assert_eq!(
        doc.get("claim")
            .and_then(SacmElement::parent)
            .map(|parent| parent.id().as_str()),
        Some("arguments"),
        "LIB-API-004: a refused duplicate moved the first element"
    );
}

#[test]
fn lib_api_004_insert_refuses_every_kind_of_unknown_parent() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::AssuranceCasePackage, "case");
    // "refused" names an element whose own insert was refused.
    let _ = refusal(&mut doc, Some("nowhere"), "refused");
    for parent in ["nowhere", "", "Case", "case ", "refused", "\0", "ünï"] {
        assert_eq!(
            refusal(&mut doc, Some(parent), "orphan"),
            InsertError::UnknownParent(ElementId::from(parent)),
            "LIB-API-004: inserting under {parent:?} is not UnknownParent"
        );
    }
    assert_eq!(
        doc.element_count(),
        1,
        "LIB-API-004: inserts under unknown parents changed the document"
    );
    assert!(
        !doc.contains("orphan") && !doc.contains("refused"),
        "LIB-API-004: an element under an unknown parent was added"
    );
}

#[test]
fn lib_api_004_ten_thousand_levels_of_nesting_are_walked_without_panicking() {
    const DEPTH: usize = 10_000;
    let id_at = |level: usize| format!("acp_{level}");
    let mut doc = Document::new();
    for level in 0..DEPTH {
        let parent = level.checked_sub(1).map(id_at);
        add(
            &mut doc,
            parent.as_deref(),
            ElementKind::AssuranceCasePackage,
            &id_at(level),
        );
    }

    assert!(
        doc.elements()
            .map(|element| element.id().to_string())
            .eq((0..DEPTH).map(id_at)),
        "LIB-API-004: elements() of a 10,000-deep document is not in pre-order"
    );

    // Up from the deepest element through every parent.
    let Some(deepest) = doc.get(&id_at(DEPTH - 1)) else {
        panic!("LIB-API-004: the deepest element is missing");
    };
    let mut chain = vec![deepest.id().to_string()];
    let mut current = deepest;
    while let Some(parent) = current.parent() {
        chain.push(parent.id().to_string());
        current = parent;
    }
    assert!(
        chain.iter().cloned().eq((0..DEPTH).rev().map(id_at)),
        "LIB-API-004: the parent() chain of a 10,000-deep document is wrong"
    );

    // Down from the root through every child.
    let mut depth = 1;
    let mut current = doc.roots().next();
    while let Some(element) = current {
        current = element.children().next();
        if current.is_some() {
            depth += 1;
        }
    }
    assert_eq!(
        depth, DEPTH,
        "LIB-API-004: children() of a 10,000-deep document stops early"
    );

    let debug = format!("{doc:?}");
    assert!(
        debug.contains("\"acp_0\"") && debug.contains(&format!("\"acp_{}\"", DEPTH - 1)),
        "LIB-API-004: Debug of a 10,000-deep document lost elements"
    );
    assert!(
        format!("{deepest:?}").contains(&format!("\"acp_{}\"", DEPTH - 1)),
        "LIB-API-004: Debug of the deepest view does not show its id"
    );

    let copy = doc.clone();
    drop(doc);
    assert_eq!(
        copy.element_count(),
        DEPTH,
        "LIB-API-004: a clone of a 10,000-deep document lost elements"
    );
}

// --- 6. No panics on hostile input: diagnostics -------------------------

#[test]
fn lib_api_004_source_location_new_refuses_zero_and_accepts_the_maximum() {
    for (line, column) in [(0, 0), (0, 5), (5, 0), (0, u32::MAX), (u32::MAX, 0)] {
        assert_eq!(
            SourceLocation::new(line, column),
            None,
            "LIB-API-004: SourceLocation::new({line}, {column}) is not None"
        );
    }
    let Some(max) = SourceLocation::new(u32::MAX, u32::MAX) else {
        panic!("LIB-API-004: SourceLocation::new(u32::MAX, u32::MAX) refused a valid position");
    };
    assert_eq!(
        (max.line(), max.column()),
        (u32::MAX, u32::MAX),
        "LIB-API-004: the maximum location does not round-trip"
    );
    assert_eq!(
        max.to_string(),
        "line 4294967295, column 4294967295",
        "LIB-API-004: the maximum location displays wrongly"
    );
    assert!(
        !format!("{max:?}").is_empty(),
        "LIB-API-004: Debug of the maximum location is empty"
    );
}

/// Single-line strings that are awkward to format: empty, NUL, non-ASCII,
/// combining marks, bidirectional controls and format-like braces.
const ODD_TEXTS: [&str; 8] = [
    "",
    "\0",
    "a\0b",
    "Ünïcödé-ид-標識 🛡️",
    "e\u{301}",
    "\u{202E}reversed\u{202C}",
    "{} {0} {:?} %s",
    "\u{10FFFF}",
];

#[test]
fn lib_api_004_diagnostic_accessors_and_display_keep_odd_strings() {
    for text in ODD_TEXTS {
        let location = SourceLocation::new(u32::MAX, u32::MAX);
        let Some(location) = location else {
            panic!("LIB-API-004: SourceLocation::new(u32::MAX, u32::MAX) refused a valid position");
        };
        let diagnostic = Diagnostic::new(Code::new(text), Severity::Error, text, text)
            .with_clause(text)
            .with_affected([ElementId::from(text), ElementId::from("")])
            .with_location(location);

        assert_eq!(
            (
                diagnostic.code().as_str(),
                diagnostic.severity(),
                diagnostic.requirement_id(),
                diagnostic.clause(),
                diagnostic.location(),
                diagnostic.message(),
            ),
            (
                text,
                Severity::Error,
                text,
                Some(text),
                Some(location),
                text
            ),
            "LIB-API-004: a diagnostic of {text:?} changed a field"
        );
        assert_eq!(
            diagnostic.affected(),
            [ElementId::from(text), ElementId::from("")].as_slice(),
            "LIB-API-004: a diagnostic of {text:?} changed its affected ids"
        );
        assert_eq!(
            diagnostic.to_string(),
            format!(
                "error {text} [{text}, clause {text}] at line 4294967295, column 4294967295: {text}"
            ),
            "LIB-API-004: Display of a diagnostic of {text:?} is not the ADR 0011 format"
        );
        assert!(
            !format!("{diagnostic:?}").is_empty(),
            "LIB-API-004: Debug of a diagnostic of {text:?} is empty"
        );
        assert_eq!(
            diagnostic.cmp(&diagnostic.clone()),
            std::cmp::Ordering::Equal,
            "LIB-API-004: a diagnostic of {text:?} does not equal its clone"
        );
    }
}

#[test]
fn lib_api_004_diagnostic_display_handles_a_message_with_line_breaks() {
    // The exact escaped Display is pinned by LIB-DIAG-001's
    // `lib_diag_001_display_escapes_line_breaks_in_the_message`. Here, only
    // the message and the prefix.
    let message = "first line\nsecond line\r\nthird";
    let diagnostic = Diagnostic::new(Code::new("C"), Severity::Warning, "ROW-1", message);
    assert_eq!(
        diagnostic.message(),
        message,
        "LIB-API-004: a multi-line message came back changed"
    );
    assert!(
        diagnostic.to_string().starts_with("warning C [ROW-1]: "),
        "LIB-API-004: Display of a multi-line message lost its prefix: {diagnostic}"
    );
}

#[test]
fn lib_api_004_diagnostic_handles_a_one_mebibyte_message_and_many_ids() {
    let message = "m".repeat(MIB);
    let affected: Vec<ElementId> = (0..10_000)
        .map(|index| ElementId::from(format!("id_{index}")))
        .collect();
    let diagnostic = Diagnostic::new(
        Code::new("SACM-ID-001"),
        Severity::Info,
        "ROW-1",
        message.clone(),
    )
    .with_affected(affected.clone());
    assert_eq!(
        diagnostic.message().len(),
        MIB,
        "LIB-API-004: a 1 MiB message came back with another length"
    );
    assert_eq!(
        diagnostic.affected(),
        affected.as_slice(),
        "LIB-API-004: 10,000 affected ids came back changed"
    );
    assert_eq!(
        diagnostic.to_string(),
        format!("info SACM-ID-001 [ROW-1]: {message}"),
        "LIB-API-004: Display of a 1 MiB message is not the ADR 0011 format"
    );
    assert!(
        format!("{diagnostic:?}").len() > MIB,
        "LIB-API-004: Debug of a 1 MiB message does not show it"
    );
}
