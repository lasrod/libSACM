//! Tests for reading the containment children of packages (SACM23-XMI-016;
//! formal/23-05-08 §9.2, §11.3, §11.4; ptc/22-03-13; XMI 2.5.1 §7.6.3,
//! §7.8.6; ADR 0010 D2, ADR 0015, ADR 0016 D5 and D6, ADR 0017).
//!
//! In this release the readable children are an AssuranceCasePackage in
//! `assuranceCasePackage`, an ArgumentPackage in `argumentPackage` and in
//! `argumentElement`, and a Claim in `argumentElement`, and on each of them
//! the four `UtilityElement`s in their roles (SACM23-BASE-004 and
//! SACM23-BASE-005 test those). Everything else in a
//! package is refused as `ReadError::Unsupported` at an exact location, and no
//! document is returned; an empty or a duplicate `xmi:id` is refused with its
//! own coded variant, `InvalidId` or `DuplicateId` (ADR 0015 amendment 2).
//!
//! Inputs are inline strings, LF-only. The expected location of a refusal is
//! marked in the input with [`MARK`] (U+2038), which the test removes before
//! reading: an element is refused at the `<` of its start tag, an attribute at
//! the first character of its qualified name, text at its first character
//! that is not white space.
//!
//! The containment roles and their declared types in [`roles_of`] are written
//! here from the "Concrete class -> containment roles" table of
//! `docs/sacm/metamodel-inventory.md`, not taken from the crate. Whether a
//! kind conforms to a declared type is asked of the public `is_kind_of` (a
//! concrete type) and `is_a` (an abstract one), as LIB-API-008's drift test
//! does.
//!
//! "The same document" is compared on a projection ([`shape`]): every element
//! in `Document::elements` order with its id, kind, parent, children and name,
//! and the roots in order, because `Document` has no `PartialEq`.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-016";

/// Marks, in a test input, where the refusal is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

// --- Reading --------------------------------------------------------------

/// How long one `read` may take; a hang fails the test instead of stalling.
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` with the default options, on a worker thread. Fails the test
/// when the read panics or does not return within [`DEADLINE`].
fn read(input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: read did not return within {DEADLINE:?} for {:?}",
            String::from_utf8_lossy(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!(
                "{ROW}: read panicked on {:?}",
                String::from_utf8_lossy(input)
            )
        }
    }
}

/// Reads `input`, which the test expects to be read.
fn read_ok(input: &str) -> Document {
    match read(input.as_bytes()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: read refused {input:?}: {error:?} ({error})"),
    }
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its one [`MARK`], and the location of the mark: lines
/// end at LF, CRLF or a lone CR, and columns count characters.
fn marked(template: &str) -> (String, SourceLocation) {
    let Some(offset) = template.find(MARK) else {
        panic!("{ROW}: the test input {template:?} has no mark");
    };
    let text = template.replacen(MARK, "", 1);
    assert!(
        !text.contains(MARK),
        "{ROW}: the test input {template:?} has more than one mark"
    );
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    for c in template[..offset].chars() {
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    (text, loc(line, column))
}

/// One input that must be refused as `Unsupported` at its mark.
struct Refused {
    what: String,
    template: String,
}

fn refused(what: impl Into<String>, template: impl Into<String>) -> Refused {
    Refused {
        what: what.into(),
        template: template.into(),
    }
}

/// Reads every case and fails, listing each case that is not refused as
/// `Unsupported` at its mark, with no document.
fn assert_all_unsupported_at_mark(what: &str, cases: &[Refused]) {
    assert_all_refused_at_mark(what, cases, "Unsupported", |error| {
        matches!(error, ReadError::Unsupported { .. }) && error.code().is_none()
    });
}

/// Reads every case and fails, listing each case that is not refused at its
/// mark with a refusal that `is_expected` accepts, named `expected`.
fn assert_all_refused_at_mark(
    what: &str,
    cases: &[Refused],
    expected: &str,
    is_expected: fn(&ReadError) -> bool,
) {
    assert!(!cases.is_empty(), "{ROW}: setup: no cases for {what}");
    let mut failures = Vec::new();
    for case in cases {
        let (input, at) = marked(&case.template);
        match read(input.as_bytes()) {
            Ok(output) => failures.push(format!(
                "{}: a document of {} elements was returned for {input:?}",
                case.what,
                output.document().element_count()
            )),
            Err(error) if is_expected(&error) && error.location() == Some(at) => {}
            Err(error) => failures.push(format!(
                "{}: expected {expected} at {at}, got {error:?} for {input:?}",
                case.what
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} of {} {what} cases are not {expected} at the mark:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// A `DuplicateId` refusal with SACM-ID-001 (ADR 0015 amendment 2).
fn is_duplicate_id(error: &ReadError) -> bool {
    matches!(error, ReadError::DuplicateId { .. }) && error.code() == Some(Code::ID_DUPLICATE)
}

/// An `InvalidId` refusal of the empty id, with SACM-ID-002.
fn is_empty_invalid_id(error: &ReadError) -> bool {
    matches!(error, ReadError::InvalidId { id, .. } if id.is_empty())
        && error.code() == Some(Code::ID_INVALID)
}

// --- Documents ------------------------------------------------------------

/// `body` on line 2 of an AssuranceCasePackage root `case`.
fn in_acp(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  {body}\n</sacm:AssuranceCasePackage>"
    )
}

/// `body` on line 2 of an ArgumentPackage root `arguments`.
fn in_ap(body: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  {body}\n</sacm:ArgumentPackage>")
}

/// `body` in an AssuranceCasePackage `sub`, in the role `assuranceCasePackage`
/// of the root `case`.
fn in_acp_child(body: &str) -> String {
    in_acp(&format!(
        "<assuranceCasePackage xmi:id=\"sub\">{body}</assuranceCasePackage>"
    ))
}

/// `body` in an ArgumentPackage `ap`, in the role `argumentPackage` of the
/// root `case`.
fn in_ap_in_acp(body: &str) -> String {
    in_acp(&format!(
        "<argumentPackage xmi:id=\"ap\">{body}</argumentPackage>"
    ))
}

/// `body` in an ArgumentPackage `ap`, in the role `argumentElement` of the
/// root `arguments`.
fn in_ap_in_ap(body: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\">{body}</argumentElement>"
    ))
}

/// `body` in a Claim `c`, at depth 3: in `argumentElement` of the
/// ArgumentPackage `ap`, in `argumentPackage` of the root `case`.
fn in_claim(body: &str) -> String {
    in_ap_in_acp(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">{body}</argumentElement>"
    ))
}

/// A function that puts a body inside a parent element.
type Wrap = fn(&str) -> String;

/// The readable parent contexts: a label, the parent's kind, and the
/// function that puts a body inside the parent.
fn parent_contexts() -> [(&'static str, ElementKind, Wrap); 6] {
    [
        (
            "an AssuranceCasePackage root",
            ElementKind::AssuranceCasePackage,
            in_acp,
        ),
        (
            "an ArgumentPackage root",
            ElementKind::ArgumentPackage,
            in_ap,
        ),
        (
            "an AssuranceCasePackage child",
            ElementKind::AssuranceCasePackage,
            in_acp_child,
        ),
        (
            "an ArgumentPackage in argumentPackage",
            ElementKind::ArgumentPackage,
            in_ap_in_acp,
        ),
        (
            "an ArgumentPackage in argumentElement",
            ElementKind::ArgumentPackage,
            in_ap_in_ap,
        ),
        ("a Claim at depth 3", ElementKind::Claim, in_claim),
    ]
}

// --- The containment roles, from docs/sacm/metamodel-inventory.md --------

/// The declared type of a role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Declared {
    Kind(ElementKind),
    Abstract(AbstractClass),
}

impl Declared {
    fn admits(self, kind: ElementKind) -> bool {
        match self {
            Self::Kind(declared) => kind.is_kind_of(declared),
            Self::Abstract(class) => kind.is_a(class),
        }
    }

    /// Whether a child of `kind` under a role of this type carries a type
    /// attribute when written: when the declared type is not its kind.
    fn types(self, kind: ElementKind) -> bool {
        self != Self::Kind(kind)
    }
}

/// The element-typed containment roles of the concrete class `parent`, as
/// the inventory's "Concrete class -> containment roles" table gives them.
/// The value-typed roles (`name`, `content`, `value`, `key`) hold values,
/// not elements, and are left out.
fn roles_of(parent: ElementKind) -> Vec<(&'static str, Declared)> {
    use Declared::{Abstract, Kind};
    use ElementKind as K;
    let utility = [
        ("description", Kind(K::Description)),
        (
            "implementationConstraint",
            Kind(K::ImplementationConstraint),
        ),
        ("note", Kind(K::Note)),
        ("taggedValue", Kind(K::TaggedValue)),
    ];
    let own: Vec<(&'static str, Declared)> = match parent {
        K::AssuranceCasePackage
        | K::AssuranceCasePackageBinding
        | K::AssuranceCasePackageInterface => vec![
            ("assuranceCasePackage", Kind(K::AssuranceCasePackage)),
            ("argumentPackage", Kind(K::ArgumentPackage)),
            ("artifactPackage", Kind(K::ArtifactPackage)),
            ("terminologyPackage", Kind(K::TerminologyPackage)),
        ],
        K::ArgumentPackage | K::ArgumentPackageBinding | K::ArgumentPackageInterface => {
            vec![(
                "argumentElement",
                Abstract(AbstractClass::ArgumentationElement),
            )]
        }
        K::ArtifactPackage | K::ArtifactPackageBinding | K::ArtifactPackageInterface => {
            vec![("artifactElement", Abstract(AbstractClass::ArtifactElement))]
        }
        K::TerminologyPackage | K::TerminologyPackageBinding | K::TerminologyPackageInterface => {
            vec![(
                "terminologyElement",
                Abstract(AbstractClass::TerminologyElement),
            )]
        }
        K::Activity
        | K::Artifact
        | K::ArtifactAssetRelationship
        | K::Event
        | K::Participant
        | K::Property
        | K::Resource
        | K::Technique => vec![("property", Kind(K::Property))],
        K::ArgumentGroup
        | K::ArgumentReasoning
        | K::ArtifactReference
        | K::AssertedArtifactContext
        | K::AssertedArtifactSupport
        | K::AssertedContext
        | K::AssertedEvidence
        | K::AssertedInference
        | K::Claim
        | K::ArtifactGroup
        | K::Category
        | K::Expression
        | K::Term
        | K::TerminologyGroup => Vec::new(),
        // The four UtilityElements hold only values.
        K::Description | K::ImplementationConstraint | K::Note | K::TaggedValue => {
            return Vec::new();
        }
        other => panic!("{ROW}: setup: the inventory table has no row for {other}"),
    };
    own.into_iter().chain(utility).collect()
}

/// The role of `parent`'s class that holds a `child`, by [`roles_of`].
fn expected_role(parent: ElementKind, child: ElementKind) -> Option<&'static str> {
    let admitting: Vec<_> = roles_of(parent)
        .into_iter()
        .filter(|(_, declared)| declared.admits(child))
        .collect();
    assert!(
        admitting.len() <= 1,
        "{ROW}: setup: {} roles of {parent} admit {child}: {admitting:?}",
        admitting.len()
    );
    admitting.first().map(|(role, _)| *role)
}

/// Whether this release reads a `child` under `role` of a `parent`: the
/// package children, and the four `UtilityElement`s in their own roles on
/// every `ModelElement` (SACM23-BASE-004, SACM23-BASE-005).
fn is_readable(parent: ElementKind, role: &str, child: ElementKind) -> bool {
    use ElementKind as K;
    let utility = matches!(
        (role, child),
        ("description", K::Description)
            | ("implementationConstraint", K::ImplementationConstraint)
            | ("note", K::Note)
            | ("taggedValue", K::TaggedValue)
    );
    (utility && parent.is_a(AbstractClass::ModelElement))
        || matches!(
            (parent, role, child),
            (
                K::AssuranceCasePackage,
                "assuranceCasePackage",
                K::AssuranceCasePackage
            ) | (
                K::AssuranceCasePackage,
                "argumentPackage",
                K::ArgumentPackage
            ) | (
                K::ArgumentPackage,
                "argumentElement",
                K::ArgumentPackage | K::Claim
            )
        )
}

// --- Shapes ---------------------------------------------------------------

/// One element of a [`Shape`].
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    id: String,
    kind: ElementKind,
    parent: Option<String>,
    children: Vec<String>,
    name: Option<LangString>,
}

/// What two documents must share to be the same document.
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    roots: Vec<String>,
    elements: Vec<Entry>,
    count: usize,
}

fn shape(document: &Document) -> Shape {
    let id = |element: SacmElement<'_>| element.id().as_str().to_owned();
    Shape {
        roots: document.roots().map(id).collect(),
        elements: document
            .elements()
            .map(|element| Entry {
                id: id(element),
                kind: element.kind(),
                parent: element.parent().map(id),
                children: element.children().map(id).collect(),
                name: element.name().cloned(),
            })
            .collect(),
        count: document.element_count(),
    }
}

fn entry(
    id: &str,
    kind: ElementKind,
    parent: Option<&str>,
    children: &[&str],
    name: Option<LangString>,
) -> Entry {
    Entry {
        id: id.to_owned(),
        kind,
        parent: parent.map(str::to_owned),
        children: children.iter().map(|&child| child.to_owned()).collect(),
        name,
    }
}

// --- Item 1: the four readable cases ---------------------------------------

#[test]
fn sacm23_xmi_016_each_readable_child_reads_to_its_kind_id_name_and_parent() {
    use ElementKind as K;
    // (role, type attribute, child kind): concrete roles untyped and with a
    // redundant type of either form; `argumentElement` with either form; and
    // an `xmi:type` and an `xsi:type` that agree, in either order.
    let cases = [
        (
            "argumentElement",
            " xmi:type=\"sacm:Claim\" xsi:type=\"sacm:Claim\"",
            K::Claim,
        ),
        (
            "argumentElement",
            " xsi:type=\"sacm:ArgumentPackage\" xmi:type=\"sacm:ArgumentPackage\"",
            K::ArgumentPackage,
        ),
        (
            "argumentPackage",
            " xmi:type=\"sacm:ArgumentPackage\" xsi:type=\" sacm:ArgumentPackage \"",
            K::ArgumentPackage,
        ),
        (
            "assuranceCasePackage",
            " xsi:type=\"sacm:AssuranceCasePackage\" xmi:type=\"sacm:AssuranceCasePackage\"",
            K::AssuranceCasePackage,
        ),
        ("assuranceCasePackage", "", K::AssuranceCasePackage),
        (
            "assuranceCasePackage",
            " xmi:type=\"sacm:AssuranceCasePackage\"",
            K::AssuranceCasePackage,
        ),
        (
            "assuranceCasePackage",
            " xsi:type=\"sacm:AssuranceCasePackage\"",
            K::AssuranceCasePackage,
        ),
        ("argumentPackage", "", K::ArgumentPackage),
        (
            "argumentPackage",
            " xmi:type=\"sacm:ArgumentPackage\"",
            K::ArgumentPackage,
        ),
        (
            "argumentPackage",
            " xsi:type=\"sacm:ArgumentPackage\"",
            K::ArgumentPackage,
        ),
        (
            "argumentElement",
            " xmi:type=\"sacm:ArgumentPackage\"",
            K::ArgumentPackage,
        ),
        (
            "argumentElement",
            " xsi:type=\"sacm:ArgumentPackage\"",
            K::ArgumentPackage,
        ),
        ("argumentElement", " xmi:type=\"sacm:Claim\"", K::Claim),
        ("argumentElement", " xsi:type=\"sacm:Claim\"", K::Claim),
    ];
    let name = || Some(LangString::new("Child").with_lang("en"));
    for (role, attribute, kind) in cases {
        let child = format!(
            "<{role}{attribute} xmi:id=\"child\"><name lang=\"en\" content=\"Child\"/></{role}>"
        );
        let (input, parent, parent_kind) = if role == "argumentElement" {
            (in_ap(&child), "arguments", K::ArgumentPackage)
        } else {
            (in_acp(&child), "case", K::AssuranceCasePackage)
        };
        let expected = Shape {
            roots: vec![parent.to_owned()],
            elements: vec![
                entry(parent, parent_kind, None, &["child"], None),
                entry("child", kind, Some(parent), &[], name()),
            ],
            count: 2,
        };
        assert_eq!(
            shape(&read_ok(&input)),
            expected,
            "{ROW}: a {kind} in {role}{attribute} does not read as expected"
        );
    }
    // A child without a name has none.
    let doc = read_ok(&in_ap(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"child\"/>",
    ));
    assert_eq!(
        doc.get("child")
            .map(|child| (child.kind(), child.name().cloned())),
        Some((K::Claim, None)),
        "{ROW}: a Claim without a name does not read as a nameless Claim"
    );
}

// --- Item 2: document order ------------------------------------------------

#[test]
fn sacm23_xmi_016_document_order_holds_across_roles_and_with_name_after_the_children_at_depth_four()
{
    use ElementKind as K;
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n\
         \x20 <argumentPackage xmi:id=\"ap1\">\n\
         \x20   <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n\
         \x20   <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap1_1\">\n\
         \x20     <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c2\"/>\n\
         \x20     <name content=\"AP 1.1\"/>\n\
         \x20   </argumentElement>\n\
         \x20   <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"><name content=\"C3\"/></argumentElement>\n\
         \x20   <name content=\"AP 1\"/>\n\
         \x20 </argumentPackage>\n\
         \x20 <assuranceCasePackage xmi:id=\"acp2\"/>\n\
         \x20 <argumentPackage xmi:id=\"ap2\"/>\n\
         \x20 <assuranceCasePackage xmi:id=\"acp3\">\n\
         \x20   <argumentPackage xmi:id=\"ap3\"><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c4\"/></argumentPackage>\n\
         \x20 </assuranceCasePackage>\n\
         \x20 <name lang=\"en\" content=\"Case\"/>\n\
         </sacm:AssuranceCasePackage>"
    );
    let content = |text: &str| Some(LangString::new(text));
    let expected = Shape {
        roots: vec!["case".to_owned()],
        elements: vec![
            entry(
                "case",
                K::AssuranceCasePackage,
                None,
                &["ap1", "acp2", "ap2", "acp3"],
                Some(LangString::new("Case").with_lang("en")),
            ),
            entry(
                "ap1",
                K::ArgumentPackage,
                Some("case"),
                &["c1", "ap1_1", "c3"],
                content("AP 1"),
            ),
            entry("c1", K::Claim, Some("ap1"), &[], None),
            entry(
                "ap1_1",
                K::ArgumentPackage,
                Some("ap1"),
                &["c2"],
                content("AP 1.1"),
            ),
            entry("c2", K::Claim, Some("ap1_1"), &[], None),
            entry("c3", K::Claim, Some("ap1"), &[], content("C3")),
            entry("acp2", K::AssuranceCasePackage, Some("case"), &[], None),
            entry("ap2", K::ArgumentPackage, Some("case"), &[], None),
            entry(
                "acp3",
                K::AssuranceCasePackage,
                Some("case"),
                &["ap3"],
                None,
            ),
            entry("ap3", K::ArgumentPackage, Some("acp3"), &["c4"], None),
            entry("c4", K::Claim, Some("ap3"), &[], None),
        ],
        count: 11,
    };
    assert_eq!(
        shape(&read_ok(&input)),
        expected,
        "{ROW}: the children are not read in document order across roles"
    );
}

// --- Item 3: type resolution -----------------------------------------------

#[test]
fn sacm23_xmi_016_a_child_type_resolves_as_a_qname_with_a_prefix_declared_on_the_child_and_trimmed()
{
    use ElementKind as K;
    let s = " xmlns:s=\"http://www.omg.org/spec/SACM/20220301\"";
    let cases = [
        (
            "a prefix declared on a Claim",
            in_ap(&format!(
                "<argumentElement{s} xmi:type=\"s:Claim\" xmi:id=\"x\"/>"
            )),
            K::Claim,
        ),
        (
            "a prefix declared on an AP",
            in_ap(&format!(
                "<argumentElement{s} xsi:type=\"s:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
            K::ArgumentPackage,
        ),
        (
            "a prefix declared on an AP in a concrete role",
            in_acp(&format!(
                "<argumentPackage{s} xmi:type=\"s:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
            K::ArgumentPackage,
        ),
        (
            "spaces around the type",
            in_ap("<argumentElement xmi:type=\"  sacm:Claim \" xmi:id=\"x\"/>"),
            K::Claim,
        ),
        (
            "TAB, LF and CR around the type",
            in_ap("<argumentElement xsi:type=\"&#9;&#10;sacm:Claim&#13;\" xmi:id=\"x\"/>"),
            K::Claim,
        ),
        (
            "spaces around a redundant type",
            in_acp("<assuranceCasePackage xmi:type=\" sacm:AssuranceCasePackage \" xmi:id=\"x\"/>"),
            K::AssuranceCasePackage,
        ),
    ];
    for (what, input, kind) in cases {
        let doc = read_ok(&input);
        assert_eq!(
            doc.get("x").map(SacmElement::kind),
            Some(kind),
            "{ROW}: {what}: the child is not read as {kind}"
        );
    }
}

#[test]
fn sacm23_xmi_016_a_rebound_sacm_prefix_or_a_miscased_class_in_a_child_type_is_unsupported() {
    let other = " xmlns:sacm=\"urn:not-sacm\"";
    let cases = [
        refused(
            "sacm rebound on a Claim",
            in_ap(&format!(
                "{MARK}<argumentElement{other} xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "sacm rebound on an AP in a concrete role",
            in_acp(&format!(
                "{MARK}<argumentPackage{other} xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "sacm:claim",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "sacm:claim as xsi:type",
            in_ap(&format!(
                "{MARK}<argumentElement xsi:type=\"sacm:claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "white space inside the type",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm: Claim\" xmi:id=\"x\"/>"
            )),
        ),
    ];
    assert_all_unsupported_at_mark("type resolution", &cases);
}

// --- Item 4: what is refused, and where -----------------------------------

/// A child `<role attribute="value" xmi:id="x"/>`, marked at its start tag,
/// inside `wrap`'s parent.
fn typed_child(wrap: Wrap, role: &str, attribute: &str, value: &str) -> String {
    wrap(&format!(
        "{MARK}<{role} {attribute}=\"{value}\" xmi:id=\"x\"/>"
    ))
}

#[test]
fn sacm23_xmi_016_a_type_that_does_not_conform_to_the_role_is_unsupported_at_the_child() {
    let mut cases: Vec<Refused> = [
        (in_acp as Wrap, "argumentPackage", "xmi:type", "sacm:Claim"),
        (
            in_acp,
            "argumentPackage",
            "xmi:type",
            "sacm:AssuranceCasePackage",
        ),
        (
            in_acp,
            "assuranceCasePackage",
            "xsi:type",
            "sacm:ArgumentPackage",
        ),
        (
            in_acp_child,
            "assuranceCasePackage",
            "xmi:type",
            "sacm:Claim",
        ),
        (
            in_ap,
            "argumentElement",
            "xmi:type",
            "sacm:AssuranceCasePackage",
        ),
        (in_ap, "argumentElement", "xsi:type", "sacm:Term"),
        (in_ap, "argumentElement", "xmi:type", "sacm:Note"),
        (in_ap_in_ap, "argumentElement", "xmi:type", "sacm:Artifact"),
    ]
    .into_iter()
    .map(|(wrap, role, attribute, value)| {
        refused(
            format!("{attribute}={value:?} in {role}"),
            typed_child(wrap, role, attribute, value),
        )
    })
    .collect();
    cases.extend([
        refused(
            "an untyped argumentElement",
            in_ap(&format!("{MARK}<argumentElement xmi:id=\"x\"/>")),
        ),
        refused(
            "an untyped argumentElement with a name",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:id=\"x\"><name content=\"X\"/></argumentElement>"
            )),
        ),
        refused(
            "disagreeing xmi:type and xsi:type",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "disagreeing types in a concrete role",
            in_acp(&format!(
                "{MARK}<argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "a bad type at depth 3",
            in_ap_in_acp(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Clam\" xmi:id=\"c\"/>"
            )),
        ),
    ]);
    assert_all_unsupported_at_mark("non-conforming type", &cases);
}

#[test]
fn sacm23_xmi_016_a_type_naming_an_abstract_unknown_or_foreign_class_is_unsupported_at_the_child() {
    let mut values: Vec<String> = [
        "Assertion",
        "ArgumentationElement",
        "ArgumentAsset",
        "AssertedRelationship",
        "ModelElement",
        "Element",
    ]
    .iter()
    .map(|class| format!("sacm:{class}"))
    .collect();
    // No SACM class, unresolvable, or not a QName.
    values.extend(
        [
            "sacm:Goal",
            "nope:Claim",
            "Claim",
            "sacm:sacm:Claim",
            "",
            "sacm:",
        ]
        .map(str::to_owned),
    );
    let mut cases: Vec<Refused> = values
        .iter()
        .map(|value| {
            refused(
                format!("xmi:type={value:?}"),
                typed_child(in_ap, "argumentElement", "xmi:type", value),
            )
        })
        .collect();
    cases.extend([
        refused(
            "a foreign namespace",
            in_ap(&format!(
                "{MARK}<argumentElement xmlns:o=\"urn:o\" xmi:type=\"o:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "a foreign namespace in a concrete role",
            in_acp(&format!(
                "{MARK}<argumentPackage xmlns:o=\"urn:o\" xsi:type=\"o:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "an abstract class in a concrete role",
            typed_child(in_acp, "argumentPackage", "xmi:type", "sacm:ArgumentationElement"),
        ),
    ]);
    assert_all_unsupported_at_mark("abstract or unknown type", &cases);
}

#[test]
fn sacm23_xmi_016_a_role_the_parent_class_lacks_or_a_qualified_role_element_is_unsupported_at_the_child()
 {
    let default_sacm = format!(
        "<ArgumentPackage xmlns=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arguments\">\n  \
         {MARK}<argumentElement xmi:type=\"Claim\" xmi:id=\"x\"/>\n</ArgumentPackage>"
    );
    let cases = [
        refused("<foo> in an ACP", in_acp(&format!("{MARK}<foo/>"))),
        refused(
            "<foo> in an AP",
            in_ap(&format!("{MARK}<foo xmi:id=\"x\"/>")),
        ),
        refused(
            "argumentElement in an ACP",
            in_acp(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "argumentElement in an ACP child",
            in_acp_child(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "argumentPackage in an AP",
            in_ap(&format!("{MARK}<argumentPackage xmi:id=\"x\"/>")),
        ),
        refused(
            "argumentationElement in an AP",
            in_ap(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "argumentationElement in a nested AP",
            in_ap_in_acp(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "sacm:argumentElement",
            in_ap(&format!(
                "{MARK}<sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "sacm:argumentPackage",
            in_acp(&format!("{MARK}<sacm:argumentPackage xmi:id=\"x\"/>")),
        ),
        refused(
            "a role element in a foreign namespace",
            in_acp(&format!(
                "{MARK}<o:assuranceCasePackage xmlns:o=\"urn:o\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "a role element under a default SACM namespace",
            default_sacm,
        ),
        refused(
            "a role element in an XMI-namespace",
            in_ap(&format!(
                "{MARK}<xmi:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "argumentElement in a Claim",
            in_claim(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "a Claim element as a child",
            in_ap(&format!("{MARK}<sacm:Claim xmi:id=\"x\"/>")),
        ),
    ];
    assert_all_unsupported_at_mark("role", &cases);
}

#[test]
fn sacm23_xmi_016_every_conforming_kind_outside_the_readable_set_is_unsupported_in_its_role() {
    let mut cases = Vec::new();
    for (context, parent, wrap) in parent_contexts() {
        for (role, declared) in roles_of(parent) {
            for &child in ElementKind::ALL {
                if !declared.admits(child) || is_readable(parent, role, child) {
                    continue;
                }
                let type_attribute = if declared.types(child) {
                    format!(" xmi:type=\"sacm:{child}\"")
                } else {
                    String::new()
                };
                cases.push(refused(
                    format!("{child} in {role} of {context}"),
                    wrap(&format!("{MARK}<{role}{type_attribute} xmi:id=\"x\"/>")),
                ));
            }
        }
    }
    // The 10 other ArgumentationElement kinds and 8 other package kinds in
    // their roles. The utility roles are read on every context
    // (SACM23-BASE-004, SACM23-BASE-005), so none of them is a case.
    assert!(
        cases.len() >= 2 * 10 + 3 * 10,
        "{ROW}: setup: only {} cases",
        cases.len()
    );
    assert_all_unsupported_at_mark("unreadable kind", &cases);
}

#[test]
fn sacm23_xmi_016_a_foreign_child_id_is_unsupported_at_its_name() {
    // A child with no `xmi:id` is read with a generated id, and a padded id
    // is read trimmed (SACM23-XMI-020, SACM23-XMI-025): an id attribute that
    // is not `xmi:id` is not an id, so it is an unread attribute.
    let cases = [
        refused(
            "an id in no namespace on a Claim",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" {MARK}id=\"x\"/>"
            )),
        ),
        refused(
            "an id in no namespace on an ACP",
            in_acp(&format!("<assuranceCasePackage {MARK}id=\"x\"/>")),
        ),
        refused(
            "an id in a foreign namespace on a Claim",
            in_ap(&format!(
                "<argumentElement xmlns:o=\"urn:o\" xmi:type=\"sacm:Claim\" {MARK}o:id=\"x\"/>"
            )),
        ),
    ];
    assert_all_unsupported_at_mark("id", &cases);
}

#[test]
fn sacm23_xmi_016_an_empty_child_id_is_invalid_id_at_the_child() {
    let cases = [
        refused(
            "an empty xmi:id on an AP",
            in_acp(&format!("{MARK}<argumentPackage xmi:id=\"\"/>")),
        ),
        refused(
            "an empty xmi:id on a Claim",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>"
            )),
        ),
    ];
    assert_all_refused_at_mark("empty id", &cases, "InvalidId", is_empty_invalid_id);
}

#[test]
fn sacm23_xmi_016_a_duplicate_child_id_is_duplicate_id_at_the_child() {
    let cases = [
        refused(
            "the root's id",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"arguments\"/>"
            )),
        ),
        refused(
            "a sibling's id",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>{MARK}<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>"
            )),
        ),
        refused(
            "a cousin's id at depth 3",
            in_acp(&format!(
                "<argumentPackage xmi:id=\"a\"><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/></argumentPackage>\
             <argumentPackage xmi:id=\"b\">{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/></argumentPackage>"
            )),
        ),
        refused(
            "an earlier root's id",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n\
             <sacm:ArgumentPackage xmi:id=\"first\"/>\n\
             <sacm:ArgumentPackage xmi:id=\"second\">{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"first\"/></sacm:ArgumentPackage>\n\
             </xmi:XMI>"
            ),
        ),
    ];
    assert_all_refused_at_mark("duplicate id", &cases, "DuplicateId", is_duplicate_id);
}

#[test]
fn sacm23_xmi_016_an_unread_attribute_of_a_child_is_unsupported_at_its_name() {
    let mut cases = Vec::new();
    let children = [
        (
            "a Claim",
            in_ap as Wrap,
            "argumentElement xmi:type=\"sacm:Claim\"",
        ),
        (
            "an AP in argumentElement",
            in_ap,
            "argumentElement xsi:type=\"sacm:ArgumentPackage\"",
        ),
        ("an AP in argumentPackage", in_acp, "argumentPackage"),
        ("an ACP", in_acp, "assuranceCasePackage"),
    ];
    // `gid` is read (SACM23-BASE-006), and `isCitation`, `isAbstract`,
    // `citedElement` and `abstractForm` in no namespace (SACM23-BASE-007/008),
    // so they are not among these; in the SACM namespace they are not read.
    let attributes = [
        "sacm:isCitation=\"false\"",
        "sacm:isCitation=\"true\"",
        "sacm:isAbstract=\"true\"",
        "xmi:version=\"2.0\"",
        "xmi:uuid=\"u\"",
        "xmi:label=\"l\"",
        "foo=\"1\"",
        "o:id=\"x\" xmlns:o=\"urn:o\"",
        "name=\"N\"",
    ];
    for (what, wrap, start) in children {
        for attribute in attributes {
            cases.push(refused(
                format!("{attribute} on {what}"),
                wrap(&format!("<{start} xmi:id=\"x\"\n      {MARK}{attribute}/>")),
            ));
        }
    }
    // `assertionDeclaration` is read on a Claim (SACM23-XMI-017), but a
    // package is not an Assertion: refused there, even "asserted", the
    // default.
    for (what, wrap, start) in &children[1..] {
        for declaration in ["asserted", "needsSupport", "assumed", "asCited"] {
            cases.push(refused(
                format!("assertionDeclaration {declaration:?} on {what}"),
                wrap(&format!(
                    "<{start} xmi:id=\"x\" {MARK}assertionDeclaration=\"{declaration}\"/>"
                )),
            ));
        }
    }
    // Before the type and the id, the attribute is still located at its name.
    cases.push(refused(
        "foo before the type and the id",
        in_ap(&format!(
            "<argumentElement {MARK}foo=\"true\" xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
        )),
    ));
    assert_all_unsupported_at_mark("attribute", &cases);
}

#[test]
fn sacm23_xmi_016_text_or_an_unreadable_name_in_a_child_is_unsupported_where_it_starts() {
    // A second name is `RepeatedValue` (SACM23-XMI-021), and an `xmi:id` on a
    // name is read (SACM23-XMI-022), so neither is among these.
    let claim = |content: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">{content}</argumentElement>"
        ))
    };
    let cases = [
        refused("text in a Claim", claim(&format!("{MARK}x"))),
        refused("text after white space", claim(&format!("\n     {MARK}x"))),
        refused(
            "text after a name",
            claim(&format!("<name content=\"X\"/>{MARK}x")),
        ),
        refused("CDATA in a Claim", claim(&format!("{MARK}<![CDATA[x]]>"))),
        refused(
            "text in an ACP child",
            in_acp(&format!(
                "<assuranceCasePackage xmi:id=\"x\">{MARK}t</assuranceCasePackage>"
            )),
        ),
        refused(
            "text in an AP child",
            in_acp(&format!(
                "<argumentPackage xmi:id=\"x\">  {MARK}t</argumentPackage>"
            )),
        ),
        refused(
            "an attribute of a name",
            claim(&format!("<name {MARK}xmi:uuid=\"n\" content=\"A\"/>")),
        ),
        refused(
            "a name in the SACM namespace",
            claim(&format!("{MARK}<sacm:name content=\"A\"/>")),
        ),
        // Not `lang` or `content`, which are value elements (SACM23-XMI-014).
        refused(
            "an element in a name",
            claim(&format!("<name content=\"A\">{MARK}<value/></name>")),
        ),
    ];
    assert_all_unsupported_at_mark("content", &cases);
}

#[test]
fn sacm23_xmi_016_a_duplicate_child_id_is_refused_at_the_child_before_later_content() {
    // Each duplicate is followed by content that is refused too, so the
    // duplicate must be found when its start tag is read, not later: the
    // refusal is the DuplicateId at the first repeat (ADR 0015 amendment 2).
    let wrapper = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                   xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n";
    let claim = |id: &str| format!("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"/>");
    let cases = [
        refused(
            "an earlier root's id, then <foo/>",
            format!(
                "{wrapper}<sacm:ArgumentPackage xmi:id=\"a\"/>\n<sacm:ArgumentPackage xmi:id=\"b\">\
                 {MARK}{}<foo/></sacm:ArgumentPackage>\n</xmi:XMI>",
                claim("a")
            ),
        ),
        refused(
            "an earlier root's child's id, then <foo/>",
            format!(
                "{wrapper}<sacm:ArgumentPackage xmi:id=\"a\">{}</sacm:ArgumentPackage>\n\
                 <sacm:ArgumentPackage xmi:id=\"b\">{MARK}{}<foo/></sacm:ArgumentPackage>\n</xmi:XMI>",
                claim("c"),
                claim("c")
            ),
        ),
        refused(
            "a sibling's id, then <foo/>",
            in_ap(&format!("{}{MARK}{}<foo/>", claim("x"), claim("x"))),
        ),
        refused(
            "the root's own id, then <foo/>",
            in_ap(&format!("{MARK}{}<foo/>", claim("arguments"))),
        ),
        refused(
            "a sibling's id, with <foo/> inside",
            in_ap(&format!(
                "{}{MARK}<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"><foo/></argumentElement>",
                claim("x")
            )),
        ),
        refused(
            "a cousin's id, then <foo/>",
            in_acp(&format!(
                "<argumentPackage xmi:id=\"a\">{}</argumentPackage>\
                 <argumentPackage xmi:id=\"b\">{MARK}{}<foo/></argumentPackage>",
                claim("x"),
                claim("x")
            )),
        ),
    ];
    assert_all_refused_at_mark(
        "duplicate id before later content",
        &cases,
        "DuplicateId",
        is_duplicate_id,
    );
}

// --- Item 5 (new numbering): white space written as CDATA or references ----

/// The ways of writing text whose value is all XML white space.
const WHITE_SPACE: [&str; 11] = [
    " \t\n\r ",
    "<![CDATA[ ]]>",
    "<![CDATA[]]>",
    "&#x20;",
    "&#32;",
    "&#9;",
    "&#xA;",
    "&#xD;",
    "\n  &#x20;<![CDATA[\t]]>&#xD;&#xA;\n",
    "<!--c-->&#9;<?pi x?><![CDATA[\n]]>",
    "&#x9;&#x0A;&#13;&#10;",
];

/// A document with eight slots for text, each replaced by `slots[i]`: in
/// the `xmi:XMI` wrapper (0 and 7), in a root (1), in an AP child (2), in a
/// Claim (3 and 5), and in a `name` (4 and 6).
fn with_slots(slots: &[&str; 8]) -> String {
    let [w0, w1, w2, w3, w4, w5, w6, w7] = slots;
    format!(
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
         xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">{w0}\
         <sacm:AssuranceCasePackage xmi:id=\"case\">{w1}\
         <argumentPackage xmi:id=\"ap\">{w2}\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">{w3}\
         <name lang=\"en\" content=\"C\">{w4}</name>{w5}\
         </argumentElement></argumentPackage>\
         <name content=\"Case\">{w6}</name></sacm:AssuranceCasePackage>{w7}\
         <sacm:ArgumentPackage xmi:id=\"second\"/></xmi:XMI>"
    )
}

/// What each slot of [`with_slots`] is in.
const SLOTS: [&str; 8] = [
    "the wrapper, before the roots",
    "a root",
    "an ArgumentPackage child",
    "a Claim, before its name",
    "a Claim's name",
    "a Claim, after its name",
    "a root's name",
    "the wrapper, between the roots",
];

#[test]
fn sacm23_xmi_016_white_space_written_literally_as_cdata_or_as_references_is_accepted() {
    let expected = shape(&read_ok(&with_slots(&[""; 8])));
    assert_eq!(expected.count, 4, "{ROW}: setup: the bare document");
    let mut failures = Vec::new();
    for space in WHITE_SPACE {
        // In each slot alone, then in all of them.
        let mut inputs: Vec<(String, String)> = (0..8)
            .map(|slot| {
                let mut slots = [""; 8];
                slots[slot] = space;
                (SLOTS[slot].to_owned(), with_slots(&slots))
            })
            .collect();
        inputs.push(("every slot".to_owned(), with_slots(&[space; 8])));
        for (place, input) in inputs {
            match read(input.as_bytes()) {
                Ok(output) if shape(output.document()) == expected => {}
                Ok(output) => failures.push(format!(
                    "{space:?} in {place}: read as {:?}",
                    shape(output.document())
                )),
                Err(error) => failures.push(format!("{space:?} in {place}: refused: {error}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} white-space inputs are not read as the document without them:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sacm23_xmi_016_a_no_break_space_reference_or_cdata_holding_text_is_unsupported_where_it_starts()
{
    let mut cases = Vec::new();
    for text in [
        "&#xA0;",
        "<![CDATA[x]]>",
        "<![CDATA[ x]]>",
        "&#x20;x",
        "&#160;",
    ] {
        for (slot, place) in SLOTS.iter().enumerate() {
            let marked = format!("{MARK}{text}");
            let mut slots = [""; 8];
            slots[slot] = &marked;
            cases.push(refused(format!("{text:?} in {place}"), with_slots(&slots)));
            // After literal white space, still at the construct.
            let spaced = format!(" \n {MARK}{text}");
            slots[slot] = &spaced;
            cases.push(refused(
                format!("{text:?} after white space in {place}"),
                with_slots(&slots),
            ));
        }
    }
    assert_all_unsupported_at_mark("non-white-space text", &cases);
}

// --- Item 5: a readable child under any other role ------------------------

#[test]
fn sacm23_xmi_016_each_readable_child_is_refused_under_every_other_role_name_of_its_parent() {
    use ElementKind as K;
    let mut cases = Vec::new();
    let readable = [
        (
            K::AssuranceCasePackage,
            "assuranceCasePackage",
            K::AssuranceCasePackage,
        ),
        (
            K::AssuranceCasePackage,
            "argumentPackage",
            K::ArgumentPackage,
        ),
        (K::ArgumentPackage, "argumentElement", K::ArgumentPackage),
        (K::ArgumentPackage, "argumentElement", K::Claim),
    ];
    for (context, parent, wrap) in parent_contexts() {
        for &(class, own_role, child) in &readable {
            if class != parent {
                continue;
            }
            for (role, _) in roles_of(parent) {
                if role == own_role {
                    continue;
                }
                for attribute in ["xmi:type", "xsi:type"] {
                    cases.push(refused(
                        format!("{child} under {role} of {context}, by {attribute}"),
                        wrap(&format!(
                            "{MARK}<{role} {attribute}=\"sacm:{child}\" xmi:id=\"x\"/>"
                        )),
                    ));
                }
            }
        }
    }
    // Two ACP contexts: 2 children x 7 other roles x 2 type attributes;
    // three AP contexts: 2 children x 4 other roles x 2; a Claim holds none.
    assert_eq!(cases.len(), 2 * 28 + 3 * 16, "{ROW}: setup: the case count");
    assert_all_unsupported_at_mark("other role", &cases);
}

// --- Item 6: the role names written for the 339 legal pairs ---------------

/// A document that holds an element `p` of kind `parent`: a root if it is a
/// package, otherwise in the `artifactElement` role of an ArtifactPackage
/// root, which admits every kind (§12.2; ADR 0014).
fn holding(parent: ElementKind) -> Document {
    let mut doc = Document::new();
    let p = ElementId::from("p");
    let result = if parent.is_package() {
        doc.insert(None, parent, p, None)
    } else {
        let root = ElementId::from("root");
        doc.insert(None, ElementKind::ArtifactPackage, root.clone(), None)
            .and_then(|()| doc.insert(Some(&root), parent, p, None))
    };
    if let Err(error) = result {
        panic!("{ROW}: setup could not insert the {parent} \"p\": {error}");
    }
    doc
}

/// The element name of the start tag on the line that carries `xmi:id="id"`.
fn written_tag<'a>(text: &'a str, id: &str) -> Option<&'a str> {
    let attribute = format!(" xmi:id=\"{id}\"");
    let line = text.lines().find(|line| line.contains(&attribute))?;
    let tag = line.trim_start().strip_prefix('<')?;
    tag.split([' ', '>', '/']).next()
}

#[test]
fn sacm23_xmi_016_write_puts_the_child_of_each_of_the_339_legal_pairs_under_the_inventory_role() {
    let mut pairs = 0;
    let mut failures = Vec::new();
    for &parent in ElementKind::ALL {
        let mut doc = holding(parent);
        let mut expected = Vec::new();
        for &child in ElementKind::ALL {
            let Some(role) = expected_role(parent, child) else {
                continue;
            };
            let id = format!("c_{child}");
            if let Err(error) = doc.insert(
                Some(&ElementId::from("p")),
                child,
                ElementId::from(id.as_str()),
                None,
            ) {
                panic!("{ROW}: setup could not insert a {child} under a {parent}: {error}");
            }
            expected.push((child, id, role));
        }
        pairs += expected.len();
        // Tolerant: the children are nameless, and a strict write refuses a
        // validation Error (LIB-COMPAT-007).
        let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
        let text = match xmi::write(&doc, &tolerant) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: write refused the {parent} document: {error}"),
        };
        for (child, id, role) in expected {
            let written = written_tag(&text, &id);
            if written != Some(role) {
                failures.push(format!(
                    "{child} under {parent}: written as {written:?}, not {role:?}"
                ));
            }
        }
    }
    assert_eq!(
        pairs, 339,
        "{ROW}: the inventory table gives {pairs} legal pairs, not 339"
    );
    assert!(
        failures.is_empty(),
        "{ROW}: {} children are not written under their inventory role:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// --- Item 7: documents built by edit::apply read back ----------------------

/// The name variants: absent, empty and non-ASCII fields, markup and line
/// breaks.
fn name_variant(index: u64) -> LangString {
    match index % 8 {
        0 => LangString::default(),
        1 => LangString::new(""),
        2 => LangString::default().with_lang("en"),
        3 => LangString::new("").with_lang(""),
        4 => {
            LangString::new("S\u{e4}kerhet \u{2713} \u{65e5}\u{672c} \u{1F600}").with_lang("sv-SE")
        }
        5 => LangString::new("a < b & \"c\" 'd' >\te\nf\rg"),
        6 => LangString::new("Plain").with_lang("en"),
        _ => LangString::new(" leading and trailing "),
    }
}

/// Applies `operation`, or fails the test as a setup error.
fn create(doc: &mut Document, operation: impl Into<edit::Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => mutation.created_ids()[0].clone(),
        Err(error) => panic!("{ROW}: setup: edit::apply refused an operation: {error}"),
    }
}

/// A document that holds all four readable pairs, at depth 4, in three
/// roots, with siblings interleaving roles and both caller and generated ids.
/// Each `ArgumentPackage` holds only Claims or only packages: a Claim
/// beside a nested ArgumentPackage is a SACM-PKG-001 Error (SACM23-VAL-012),
/// and a strict write refuses it.
fn fixed_document() -> Document {
    let mut doc = Document::new();
    let case = create(
        &mut doc,
        CreateAssuranceCasePackage::root(name_variant(4)).with_id("case"),
    );
    let ap1 = create(&mut doc, CreateArgumentPackage::new(&case, name_variant(0)));
    let side = create(
        &mut doc,
        CreateArgumentPackage::new(&ap1, name_variant(1)).with_id("side"),
    );
    create(&mut doc, CreateClaim::new(&side, name_variant(1)));
    let inner = create(
        &mut doc,
        CreateArgumentPackage::new(&ap1, name_variant(2)).with_id("inner"),
    );
    let deep = create(
        &mut doc,
        CreateArgumentPackage::new(&inner, name_variant(5)),
    );
    create(&mut doc, CreateClaim::new(&deep, name_variant(4)));
    create(
        &mut doc,
        CreateClaim::new(&deep, name_variant(6)).with_id("c_deep"),
    );
    create(
        &mut doc,
        CreateClaim::new(&side, name_variant(3)).with_id("c_after"),
    );
    let sub = create(
        &mut doc,
        CreateAssuranceCasePackage::new(&case, name_variant(6)).with_id("sub"),
    );
    create(&mut doc, CreateArgumentPackage::new(&sub, name_variant(7)));
    create(
        &mut doc,
        CreateArgumentPackage::new(&case, name_variant(1)).with_id("ap_third"),
    );
    create(
        &mut doc,
        CreateAssuranceCasePackage::new(&case, name_variant(2)),
    );
    let second = create(&mut doc, CreateArgumentPackage::root(name_variant(3)));
    create(&mut doc, CreateClaim::new(&second, name_variant(5)));
    create(&mut doc, CreateAssuranceCasePackage::root(name_variant(0)));
    doc
}

/// One operation of [`generated_document`]: the new element's kind, its
/// parent (`None` for a root), its name, and its id if the caller gives one.
fn operation(
    kind: ElementKind,
    parent: Option<&ElementId>,
    name: LangString,
    id: Option<String>,
) -> edit::Operation {
    use ElementKind as K;
    match (kind, parent, id) {
        (K::AssuranceCasePackage, None, None) => CreateAssuranceCasePackage::root(name).into(),
        (K::AssuranceCasePackage, None, Some(id)) => {
            CreateAssuranceCasePackage::root(name).with_id(id).into()
        }
        (K::AssuranceCasePackage, Some(p), None) => CreateAssuranceCasePackage::new(p, name).into(),
        (K::AssuranceCasePackage, Some(p), Some(id)) => {
            CreateAssuranceCasePackage::new(p, name).with_id(id).into()
        }
        (K::ArgumentPackage, None, None) => CreateArgumentPackage::root(name).into(),
        (K::ArgumentPackage, None, Some(id)) => {
            CreateArgumentPackage::root(name).with_id(id).into()
        }
        (K::ArgumentPackage, Some(p), None) => CreateArgumentPackage::new(p, name).into(),
        (K::ArgumentPackage, Some(p), Some(id)) => {
            CreateArgumentPackage::new(p, name).with_id(id).into()
        }
        (K::Claim, Some(p), None) => CreateClaim::new(p, name).into(),
        (K::Claim, Some(p), Some(id)) => CreateClaim::new(p, name).with_id(id).into(),
        (kind, parent, _) => panic!("{ROW}: setup: no operation creates a {kind} under {parent:?}"),
    }
}

/// A document built by `steps` pseudo-random operations from `seed`: a root
/// one time in five (and first), otherwise a legal child of an existing
/// package, with a caller id one time in three. Each ArgumentPackage is made
/// to hold either only Claims or only packages, chosen when it is
/// created, so that no package is a SACM-PKG-001 Error (SACM23-VAL-012).
fn generated_document(seed: u64, steps: u64) -> Document {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    let mut doc = Document::new();
    // Each package, its kind, and for an ArgumentPackage whether it holds
    // Claims (otherwise ArgumentPackages).
    let mut packages: Vec<(ElementId, ElementKind, bool)> = Vec::new();
    for step in 0..steps {
        let (choice, pick, name) = (next(), next(), name_variant(next()));
        let id = (choice % 3 == 0).then(|| format!("e{seed}_{step}"));
        let first_kind = choice % 2 == 0;
        let (kind, parent) = if packages.is_empty() || choice % 5 == 0 {
            let kind = if first_kind {
                ElementKind::ArgumentPackage
            } else {
                ElementKind::AssuranceCasePackage
            };
            (kind, None)
        } else {
            let index = usize::try_from(pick).unwrap_or_default() % packages.len();
            let (parent, parent_kind, holds_claims) = packages[index].clone();
            let kind = match (parent_kind, first_kind) {
                (ElementKind::AssuranceCasePackage, true) => ElementKind::AssuranceCasePackage,
                (ElementKind::ArgumentPackage, _) if holds_claims => ElementKind::Claim,
                _ => ElementKind::ArgumentPackage,
            };
            (kind, Some(parent))
        };
        let created = create(&mut doc, operation(kind, parent.as_ref(), name, id));
        if kind != ElementKind::Claim {
            packages.push((created, kind, (pick >> 7) % 2 == 0));
        }
    }
    doc
}

/// The deepest element's depth, a root being at depth 1.
fn depth(doc: &Document) -> usize {
    doc.elements()
        .map(|element| std::iter::successors(Some(element), |e| e.parent()).count())
        .max()
        .unwrap_or_default()
}

/// The (parent kind, child kind) pairs in `doc`.
fn pairs(doc: &Document) -> Vec<(ElementKind, ElementKind)> {
    let mut pairs: Vec<_> = doc
        .elements()
        .filter_map(|element| Some((element.parent()?.kind(), element.kind())))
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs
}

#[test]
fn sacm23_xmi_016_documents_built_by_edit_read_back_the_same_and_write_again_to_the_same_bytes() {
    use ElementKind as K;
    let fixed = fixed_document();
    assert!(
        depth(&fixed) >= 4,
        "{ROW}: setup: the fixed document is too shallow"
    );
    assert!(
        fixed.roots().count() >= 2,
        "{ROW}: setup: the fixed document has one root"
    );
    assert_eq!(
        pairs(&fixed),
        [
            (K::AssuranceCasePackage, K::AssuranceCasePackage),
            (K::AssuranceCasePackage, K::ArgumentPackage),
            (K::ArgumentPackage, K::ArgumentPackage),
            (K::ArgumentPackage, K::Claim),
        ],
        "{ROW}: setup: the fixed document lacks a readable pair"
    );
    let mut documents = vec![("the fixed document".to_owned(), fixed)];
    for seed in 1..=32 {
        documents.push((
            format!("generated document {seed}"),
            generated_document(seed, 24),
        ));
    }
    let deep = documents.iter().filter(|(_, doc)| depth(doc) >= 3).count();
    assert!(
        deep >= 8,
        "{ROW}: setup: only {deep} documents reach depth 3"
    );
    for (what, doc) in &documents {
        let text = match xmi::write(doc, &WriteOptions::default()) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: setup: write refused {what}: {error}"),
        };
        let back = match read(text.as_bytes()) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ROW}: {what} does not read back: {error}\n{text}"),
        };
        assert_eq!(
            shape(&back),
            shape(doc),
            "{ROW}: {what} does not read back the same\n{text}"
        );
        let again = match xmi::write(&back, &WriteOptions::default()) {
            Ok(again) => again,
            Err(error) => panic!("{ROW}: {what} read back does not write: {error}"),
        };
        assert_eq!(again, text, "{ROW}: {what} read back writes other bytes");
    }
}
