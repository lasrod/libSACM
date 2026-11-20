//! Tests for same-file proxies of a root package (SACM23-XMI-030; XMI 2.5.1
//! formal/15-06-07 §7.10.1 "Elements act as a union, where they are either a
//! definition or a proxy. Proxies use the `LinkAttribs` attribute group to
//! define the link, and contain no nested elements." "Proxies may be
//! chained." "When following the link from a proxy, the definition of the
//! proxy is replaced by the referenced element."; production 2c and rules
//! 2l, 2n; D-STRICT; ADR 0042 D8).
//!
//! A role element of a composite carrying `xmi:idref="id"` or `href="#id"`
//! (trimmed), at most one type attribute and no other attribute or content,
//! is a proxy of an element of the same document. After all roots are read,
//! a proxy whose id names a root of a kind its role and its type admit is
//! resolved in both modes with no diagnostic: the root moves into the
//! proxy's place and stops being a root, and chains are followed. Otherwise
//! nothing moves, and the proxy is kept whole as preserved content of its
//! container, with one SACM-XMI-003 at its `<` (an Error strict, a Warning
//! tolerant), in this order of checks: `proxy-of-no-root`,
//! `proxy-wrong-kind`, `proxy-shared` (each of two or more proxies naming
//! one root), `proxy-cycle` (each proxy of a cycle). A proxy with anything
//! else on or in it is `element-kept`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity};
use sacm::model::{Document, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-030";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a diagnostic is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The start tag of the wrapper, binding `xmi` and `sacm`.
const WRAPPER: &str = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                       xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">";

/// Golden P1 of the design brief: its input and its output in both modes.
const P1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/same-file-proxies-valid.sacm.xmi"
);
const P1_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/same-file-proxies.golden.sacm.xmi"
);

// --- Inputs ----------------------------------------------------------------------

fn file(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// `roots`, in a wrapper.
fn wrapped(roots: &[String]) -> String {
    format!("{WRAPPER}{}</xmi:XMI>", roots.concat())
}

/// A root of `class` with the id `id`, named after it, holding `body`.
fn root(class: &str, id: &str, body: &str) -> String {
    format!("<sacm:{class} xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</sacm:{class}>")
}

/// The root AssuranceCasePackage `id`, holding `body`.
fn case(id: &str, body: &str) -> String {
    root("AssuranceCasePackage", id, body)
}

/// The root ArgumentPackage `args`, holding the Claim `c1`, whose name has
/// the id `c1_name`.
fn args() -> String {
    root(
        "ArgumentPackage",
        "args",
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
         <name xmi:id=\"c1_name\" content=\"C1\"/></argumentElement>",
    )
}

/// The root ArtifactPackage `arts`.
fn arts() -> String {
    root("ArtifactPackage", "arts", "")
}

/// A proxy in the role `role` naming `id` by `xmi:idref`, marked at its `<`.
fn proxy(role: &str, id: &str) -> String {
    format!("{MARK}<{role} xmi:idref=\"{id}\"/>")
}

/// `template` without its marks, and the line and column of each mark in
/// order: lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<(u32, u32)>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push((line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

// --- Reading and writing ---------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which must return a document.
fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
    }
}

/// Reads `input` in `mode`, which must return a document with no
/// diagnostic.
fn read_clean(what: &str, mode: Mode, input: &str) -> Document {
    let output = read_ok(what, mode, input);
    assert_eq!(
        output.diagnostics(),
        [],
        "{ROW}: {what}: a {mode:?} read gives diagnostics"
    );
    output.into_document()
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error:?} ({error})"),
    }
}

/// The severity of SACM-XMI-003 in `mode`.
fn kept_severity(mode: Mode) -> Severity {
    if mode == Mode::Strict {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// The ids of the roots of `doc`, in order.
fn root_ids(doc: &Document) -> Vec<String> {
    doc.roots()
        .map(|found| found.id().as_str().to_owned())
        .collect()
}

/// The id of the parent of `id` in `doc`, or `None` for a root.
fn parent_of(what: &str, doc: &Document, id: &str) -> Option<String> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    found.parent().map(|parent| parent.id().as_str().to_owned())
}

/// The ids of the children of `id` in `doc`, in order.
fn children_of(what: &str, doc: &Document, id: &str) -> Vec<String> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    found
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

/// A diagnostic as its code, severity, template, location, affected ids and
/// arguments, each argument as its text or count.
type Found = (
    &'static str,
    Severity,
    &'static str,
    Option<(u32, u32)>,
    Vec<String>,
    Vec<(&'static str, String)>,
);

fn found(diagnostic: &Diagnostic) -> Found {
    (
        diagnostic.code().as_str(),
        diagnostic.severity(),
        diagnostic.template(),
        diagnostic.location().map(|at| (at.line(), at.column())),
        diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        diagnostic
            .arguments()
            .iter()
            .map(|argument| {
                let value = match argument.value() {
                    ArgumentValue::Text(text) => text.clone(),
                    ArgumentValue::Count(count) => count.to_string(),
                    other => format!("{other:?}"),
                };
                (argument.name(), value)
            })
            .collect(),
    )
}

/// The diagnostics of `output`, each as [`Found`], with the row and clause
/// of each checked to be SACM23-XMI-030's.
fn diagnostics(what: &str, output: &ReadOutput) -> Vec<Found> {
    for diagnostic in output.diagnostics() {
        if diagnostic.template().starts_with("SACM-XMI-003/proxy-") {
            assert_eq!(
                (diagnostic.requirement_id(), diagnostic.clause()),
                (ROW, Some("7.10.1")),
                "{ROW}: {what}: the row and clause of {}",
                diagnostic.template()
            );
        }
    }
    output.diagnostics().iter().map(found).collect()
}

/// A kept proxy: SACM-XMI-003 with `template` at `at`, affecting its holder,
/// with `arguments`.
fn kept(
    mode: Mode,
    template: &'static str,
    at: (u32, u32),
    holder: &str,
    arguments: &[(&'static str, &str)],
) -> Found {
    (
        "SACM-XMI-003",
        kept_severity(mode),
        template,
        Some(at),
        vec![holder.to_owned()],
        arguments
            .iter()
            .map(|(name, value)| (*name, (*value).to_owned()))
            .collect(),
    )
}

/// The arguments of a proxy kept with one of the four proxy templates other
/// than `proxy-wrong-kind`.
fn proxy_arguments<'v>(
    element: &'v str,
    target: &'v str,
    holder_class: &'v str,
    holder_id: &'v str,
) -> [(&'static str, &'v str); 4] {
    [
        ("element", element),
        ("target", target),
        ("holder_class", holder_class),
        ("holder_id", holder_id),
    ]
}

// --- 1. Golden P1 ------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_a_proxy_of_a_root_moves_the_root_into_place() {
    let input = file(P1);
    let golden = file(P1_GOLDEN);
    for mode in MODES {
        let what = format!("Golden P1, {mode:?}");
        let doc = read_clean(&what, mode, &input);
        assert_eq!(root_ids(&doc), ["case"], "{ROW}: {what}: roots()");
        for moved in ["args", "arts"] {
            assert_eq!(
                parent_of(&what, &doc, moved).as_deref(),
                Some("case"),
                "{ROW}: {what}: the parent of {moved}"
            );
        }
        assert_eq!(
            children_of(&what, &doc, "case"),
            ["args", "arts"],
            "{ROW}: {what}: the children of case, in the proxies' places"
        );
        assert_eq!(
            parent_of(&what, &doc, "c1").as_deref(),
            Some("args"),
            "{ROW}: {what}: c1 moves with args"
        );
        assert!(
            !doc.has_preserved_content() && !doc.elements().any(SacmElement::has_preserved_content),
            "{ROW}: {what}: a resolved proxy is kept as preserved content"
        );
        for write_mode in MODES {
            assert_eq!(
                written(&what, &doc, write_mode),
                golden,
                "{ROW}: {what}: a {write_mode:?} write is not the P1 output"
            );
        }
        let back = read_clean("the P1 output", Mode::Strict, &golden);
        assert!(
            back == doc,
            "{ROW}: {what}: the output read back is not ==: {:?}",
            semantic_compare(&doc, &back)
        );
    }
}

// --- 2. href="#id" -------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_href_hash_id_is_a_same_file_proxy() {
    let input = file(P1).replace("href=\"#arts\"", "href=\" #arts \"");
    assert!(
        input.contains("href=\" #arts \""),
        "{ROW}: setup: the P1 input has no href=\"#arts\""
    );
    let expected = read_clean("Golden P1", Mode::Tolerant, &file(P1));
    for mode in MODES {
        let what = format!("href=\" #arts \", {mode:?}");
        let doc = read_clean(&what, mode, &input);
        assert_eq!(
            parent_of(&what, &doc, "arts").as_deref(),
            Some("case"),
            "{ROW}: {what}: arts is not moved into case"
        );
        assert!(
            doc == expected,
            "{ROW}: {what}: not the document of P1: {:?}",
            semantic_compare(&expected, &doc)
        );
    }
}

// --- 3. Chains -------------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_chains_are_followed() {
    let a = case("a", "<assuranceCasePackage xmi:idref=\"b\"/>");
    let b = case("b", "<assuranceCasePackage xmi:idref=\"c\"/>");
    let c = case("c", "");
    for (order, roots) in [
        ("a, b, c", [a.clone(), b.clone(), c.clone()]),
        ("c, b, a", [c.clone(), b.clone(), a.clone()]),
    ] {
        let input = wrapped(&roots);
        for mode in MODES {
            let what = format!("the chain a -> b -> c in the root order {order}, {mode:?}");
            let doc = read_clean(&what, mode, &input);
            assert_eq!(root_ids(&doc), ["a"], "{ROW}: {what}: roots()");
            assert_eq!(
                (
                    parent_of(&what, &doc, "b").as_deref(),
                    parent_of(&what, &doc, "c").as_deref()
                ),
                (Some("a"), Some("b")),
                "{ROW}: {what}: not c under b under a"
            );
        }
    }
}

// --- 4. Cycles -------------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_a_cycle_keeps_each_proxy() {
    let role = "assuranceCasePackage";
    let cycle = |holder: &str, target: &str, at: (u32, u32), mode: Mode| {
        kept(
            mode,
            "SACM-XMI-003/proxy-cycle",
            at,
            holder,
            &proxy_arguments(role, target, "AssuranceCasePackage", holder),
        )
    };
    // a names b, and b names a.
    let (input, marks) = marked(&wrapped(&[
        case("a", &proxy(role, "b")),
        case("b", &proxy(role, "a")),
    ]));
    for mode in MODES {
        let what = format!("a -> b -> a, {mode:?}");
        let output = read_ok(&what, mode, &input);
        assert_eq!(
            diagnostics(&what, &output),
            [
                cycle("a", "b", marks[0], mode),
                cycle("b", "a", marks[1], mode),
            ],
            "{ROW}: {what}: not both proxies kept with proxy-cycle"
        );
        assert_eq!(
            root_ids(output.document()),
            ["a", "b"],
            "{ROW}: {what}: both roots do not stay"
        );
    }
    // a names a.
    let (input, marks) = marked(&wrapped(&[case("a", &proxy(role, "a"))]));
    for mode in MODES {
        let what = format!("a -> a, {mode:?}");
        let output = read_ok(&what, mode, &input);
        assert_eq!(
            diagnostics(&what, &output),
            [cycle("a", "a", marks[0], mode)],
            "{ROW}: {what}: the self-link is not proxy-cycle"
        );
        assert_eq!(root_ids(output.document()), ["a"], "{ROW}: {what}: roots()");
    }
    // a names b, b names c, and c names b: b is named by two proxies that
    // pass checks 1 and 2, so a's and c's are `proxy-shared` (check 3, before
    // check 4); b's proxy then resolves, so c moves under b.
    let shared = |holder: &str, target: &str, at: (u32, u32), mode: Mode| {
        kept(
            mode,
            "SACM-XMI-003/proxy-shared",
            at,
            holder,
            &proxy_arguments(role, target, "AssuranceCasePackage", holder),
        )
    };
    let (input, marks) = marked(&wrapped(&[
        case("a", &proxy(role, "b")),
        case("b", &format!("<{role} xmi:idref=\"c\"/>")),
        case("c", &proxy(role, "b")),
    ]));
    for mode in MODES {
        let what = format!("a -> b -> c -> b, {mode:?}");
        let output = read_ok(&what, mode, &input);
        assert_eq!(
            diagnostics(&what, &output),
            [
                shared("a", "b", marks[0], mode),
                shared("c", "b", marks[1], mode),
            ],
            "{ROW}: {what}: not a's and c's proxies kept with proxy-shared"
        );
        let doc = output.document();
        assert_eq!(root_ids(doc), ["a", "b"], "{ROW}: {what}: roots()");
        assert_eq!(
            parent_of(&what, doc, "c").as_deref(),
            Some("b"),
            "{ROW}: {what}: c is not moved under b"
        );
        assert!(
            doc.get("a").is_some_and(SacmElement::has_preserved_content)
                && doc.get("c").is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: a and c do not keep their proxies"
        );
    }
}

// --- 5. No root ------------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_a_proxy_of_no_root_is_kept() {
    for (what, target) in [
        ("a proxy of the nested Claim c1", "c1"),
        ("a proxy of nope", "nope"),
        ("a proxy of the id of a name", "c1_name"),
    ] {
        let (input, marks) = marked(&wrapped(&[
            case("case", &proxy("argumentPackage", target)),
            args(),
        ]));
        for mode in MODES {
            let what = format!("{what}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            assert_eq!(
                diagnostics(&what, &output),
                [kept(
                    mode,
                    "SACM-XMI-003/proxy-of-no-root",
                    marks[0],
                    "case",
                    &proxy_arguments("argumentPackage", target, "AssuranceCasePackage", "case"),
                )],
                "{ROW}: {what}: not one proxy-of-no-root"
            );
            let doc = output.document();
            assert_eq!(root_ids(doc), ["case", "args"], "{ROW}: {what}: roots()");
            assert!(
                doc.get("case")
                    .is_some_and(SacmElement::has_preserved_content),
                "{ROW}: {what}: case does not keep the proxy"
            );
            // Nothing moved: c1 is still in args.
            assert_eq!(
                parent_of(&what, doc, "c1").as_deref(),
                Some("args"),
                "{ROW}: {what}: c1 moved"
            );
        }
    }
}

// --- 6. Shared -------------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_a_root_two_proxies_name_is_moved_by_neither() {
    let (input, marks) = marked(&wrapped(&[
        case(
            "case",
            &format!(
                "{}{}",
                proxy("argumentPackage", "args"),
                proxy("argumentPackage", "args")
            ),
        ),
        args(),
    ]));
    for mode in MODES {
        let what = format!("two proxies of args, {mode:?}");
        let output = read_ok(&what, mode, &input);
        let shared = |at: (u32, u32)| {
            kept(
                mode,
                "SACM-XMI-003/proxy-shared",
                at,
                "case",
                &proxy_arguments("argumentPackage", "args", "AssuranceCasePackage", "case"),
            )
        };
        assert_eq!(
            diagnostics(&what, &output),
            [shared(marks[0]), shared(marks[1])],
            "{ROW}: {what}: not both proxies proxy-shared"
        );
        assert_eq!(
            root_ids(output.document()),
            ["case", "args"],
            "{ROW}: {what}: args does not stay a root"
        );
    }
    // Two proxies in two containers are shared too.
    let (input, marks) = marked(&wrapped(&[
        case("one", &proxy("argumentPackage", "args")),
        case("two", &proxy("argumentPackage", "args")),
        args(),
    ]));
    for mode in MODES {
        let what = format!("proxies of args in two cases, {mode:?}");
        let output = read_ok(&what, mode, &input);
        assert_eq!(
            diagnostics(&what, &output),
            [
                kept(
                    mode,
                    "SACM-XMI-003/proxy-shared",
                    marks[0],
                    "one",
                    &proxy_arguments("argumentPackage", "args", "AssuranceCasePackage", "one"),
                ),
                kept(
                    mode,
                    "SACM-XMI-003/proxy-shared",
                    marks[1],
                    "two",
                    &proxy_arguments("argumentPackage", "args", "AssuranceCasePackage", "two"),
                ),
            ],
            "{ROW}: {what}: not both proxies proxy-shared"
        );
        assert_eq!(
            root_ids(output.document()),
            ["one", "two", "args"],
            "{ROW}: {what}: roots()"
        );
    }
}

// --- 7. Wrong kind ---------------------------------------------------------------------

#[test]
fn sacm23_xmi_030_a_proxy_of_the_wrong_kind_is_kept() {
    for (what, proxy_element, target, target_class, class) in [
        (
            "an argumentPackage proxy of the ArtifactPackage arts",
            format!("{MARK}<argumentPackage xmi:idref=\"arts\"/>"),
            "arts",
            "ArtifactPackage",
            "ArgumentPackage",
        ),
        (
            "an ArgumentPackageInterface proxy of the ArgumentPackage args",
            format!(
                "{MARK}<argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:idref=\"args\"/>"
            ),
            "args",
            "ArgumentPackage",
            "ArgumentPackageInterface",
        ),
    ] {
        let (input, marks) = marked(&wrapped(&[case("case", &proxy_element), args(), arts()]));
        for mode in MODES {
            let what = format!("{what}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            assert_eq!(
                diagnostics(&what, &output),
                [kept(
                    mode,
                    "SACM-XMI-003/proxy-wrong-kind",
                    marks[0],
                    "case",
                    &[
                        ("element", "argumentPackage"),
                        ("target", target),
                        ("target_class", target_class),
                        ("class", class),
                        ("holder_class", "AssuranceCasePackage"),
                        ("holder_id", "case"),
                    ],
                )],
                "{ROW}: {what}: not one proxy-wrong-kind"
            );
            assert_eq!(
                root_ids(output.document()),
                ["case", "args", "arts"],
                "{ROW}: {what}: a root moved"
            );
        }
    }
    // An untyped proxy under an abstract role resolves to a root whose kind
    // the role admits, with no SACM-XMI-005 (LIB-COMPAT-010).
    let input = wrapped(&[
        root(
            "ArgumentPackage",
            "ap1",
            "<argumentElement xmi:idref=\"ap2\"/>",
        ),
        root("ArgumentPackage", "ap2", ""),
    ]);
    for mode in MODES {
        let what = format!("an argumentElement proxy of the ArgumentPackage ap2, {mode:?}");
        let doc = read_clean(&what, mode, &input);
        assert_eq!(root_ids(&doc), ["ap1"], "{ROW}: {what}: roots()");
        assert_eq!(
            parent_of(&what, &doc, "ap2").as_deref(),
            Some("ap1"),
            "{ROW}: {what}: ap2 is not moved into ap1"
        );
    }
}

// --- 8. Only the link and a type -------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_030_a_proxy_carries_only_its_link_and_type() {
    for (what, element) in [
        (
            "xmi:idref with href",
            "<argumentPackage xmi:idref=\"args\" href=\"#args\"/>",
        ),
        (
            "xmi:idref with xmi:uuid",
            "<argumentPackage xmi:idref=\"args\" xmi:uuid=\"u\"/>",
        ),
        (
            "xmi:idref with xmi:id",
            "<argumentPackage xmi:idref=\"args\" xmi:id=\"x\"/>",
        ),
        (
            "xmi:idref with foo",
            "<argumentPackage xmi:idref=\"args\" foo=\"1\"/>",
        ),
        (
            "xmi:idref with a name child",
            "<argumentPackage xmi:idref=\"args\"><name/></argumentPackage>",
        ),
    ] {
        let (input, marks) = marked(&wrapped(&[
            case("case", &format!("{MARK}{element}")),
            args(),
        ]));
        for mode in MODES {
            let what = format!("{what}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            assert_eq!(
                diagnostics(&what, &output),
                [kept(
                    mode,
                    "SACM-XMI-003/element-kept",
                    marks[0],
                    "case",
                    &[
                        ("element", "argumentPackage"),
                        ("holder_class", "AssuranceCasePackage"),
                        ("holder_id", "case"),
                    ],
                )],
                "{ROW}: {what}: not one element-kept at the proxy"
            );
            assert_eq!(
                root_ids(output.document()),
                ["case", "args"],
                "{ROW}: {what}: args moved"
            );
        }
    }
    // A type attribute is allowed: xsi:type with its SACM-XMI-010, and
    // xmi:type with no diagnostic.
    let typed = wrapped(&[
        case(
            "case",
            "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:idref=\"args\"/>",
        ),
        args(),
    ]);
    for mode in MODES {
        let what = format!("a proxy typed sacm:ArgumentPackage, {mode:?}");
        let doc = read_clean(&what, mode, &typed);
        assert_eq!(
            parent_of(&what, &doc, "args").as_deref(),
            Some("case"),
            "{ROW}: {what}: args is not moved into case"
        );
    }
    // `xsi` is declared on the wrapper, so the proxy carries only its type
    // and its link.
    let xsi_typed = wrapped(&[
        case(
            "case",
            "<argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:idref=\"args\"/>",
        ),
        args(),
    ])
    .replacen(
        "<xmi:XMI ",
        "<xmi:XMI xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        1,
    );
    for mode in MODES {
        let what = format!("a proxy with xsi:type, {mode:?}");
        let output = read_ok(&what, mode, &xsi_typed);
        let codes: Vec<&str> = output
            .diagnostics()
            .iter()
            .map(|found| found.code().as_str())
            .collect();
        assert_eq!(
            codes,
            ["SACM-XMI-010"],
            "{ROW}: {what}: not one SACM-XMI-010"
        );
        assert_eq!(
            parent_of(&what, output.document(), "args").as_deref(),
            Some("case"),
            "{ROW}: {what}: args is not moved into case"
        );
    }
    // The xmi:id of a kept proxy is still checked for duplicates.
    let input = wrapped(&[
        case(
            "case",
            "<argumentPackage xmi:idref=\"args\" xmi:id=\"c1\"/>",
        ),
        args(),
    ]);
    for mode in MODES {
        let what = format!("a proxy whose xmi:id repeats c1, {mode:?}");
        match read(mode, &input) {
            Err(error @ ReadError::DuplicateId { .. }) => {
                assert_eq!(error.code(), Code::ID_DUPLICATE, "{ROW}: {what}: code()");
            }
            Err(error) => panic!("{ROW}: {what}: not DuplicateId: {error:?}"),
            Ok(output) => panic!(
                "{ROW}: {what}: the read returned a document, with {:?}",
                output.diagnostics()
            ),
        }
    }
}

// --- 9. The document's own content -----------------------------------------------------

#[test]
fn sacm23_xmi_030_document_content_keeps_its_place() {
    // `xmi:XMI` holds [case, <xmi:foo/>, args, <xmi:bar/>]: two XMI-namespace
    // elements XMI does not define, the document's own content in a
    // tolerant read (SACM23-XMI-019).
    let input = format!(
        "{WRAPPER}{}<xmi:foo/>{}<xmi:bar/></xmi:XMI>",
        case("case", "<argumentPackage xmi:idref=\"args\"/>"),
        args()
    );
    let output = read_ok("document content", Mode::Tolerant, &input);
    let templates: Vec<&str> = output
        .diagnostics()
        .iter()
        .map(Diagnostic::template)
        .collect();
    assert_eq!(
        templates,
        [
            "SACM-XMI-003/wrapper-element-kept",
            "SACM-XMI-003/wrapper-element-kept"
        ],
        "{ROW}: document content: not two wrapper-element-kept"
    );
    let doc = output.document();
    assert_eq!(root_ids(doc), ["case"], "{ROW}: document content: roots()");
    let text = written("document content", doc, Mode::Tolerant);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let index = |about: &str, matches: &dyn Fn(&str) -> bool| {
        lines
            .iter()
            .position(|line| matches(line))
            .unwrap_or_else(|| panic!("{ROW}: document content: no line {about}:\n{text}"))
    };
    let wrapper = index("<xmi:XMI", &|line| line.starts_with("<xmi:XMI "));
    let case_start = index("of case", &|line| line.contains("xmi:id=\"case\""));
    let args_start = index("of args", &|line| line.contains("xmi:id=\"args\""));
    let case_end = index("</sacm:AssuranceCasePackage>", &|line| {
        line == "</sacm:AssuranceCasePackage>"
    });
    let foo = index("<xmi:foo />", &|line| line == "<xmi:foo />");
    let bar = index("<xmi:bar />", &|line| line == "<xmi:bar />");
    assert!(
        wrapper < case_start
            && case_start < args_start
            && args_start < case_end
            && case_end < foo
            && foo < bar,
        "{ROW}: document content: not wrapped, with args in case, then <xmi:foo />, then \
         <xmi:bar />:\n{text}"
    );
}
