//! More tests for the cross-document links the reader holds (SACM23-XMI-029;
//! ADR 0040 amendment 1), written with the implementation in step 2 of slice
//! 0073b: the output bound of `write` with proxies and hrefs (LIB-SEC-003,
//! ADR 0041), and the decisions of the reader and the edits that hand
//! mutation showed the tests of `cross_document_links.rs` and
//! `edit_hrefs.rs` did not pin.

use std::fmt::Write as _;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{ArgumentValue, Diagnostic, Severity};
use sacm::edit::{
    self, AddReference, CreateArgumentPackageBinding, CreateArgumentPackageInterface, CreateClaim,
    EditError, RemoveReference, SetName, SetReferences,
};
use sacm::model::{Document, ElementId, Href, LangString, ReferenceEnd};
use sacm::validation;
use sacm::xmi::{self, Limits, Mode, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-029";
const EDIT_ROW: &str = "LIB-EDIT-020";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// Declares `xsi`.
const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

/// Declares `p`, bound to the SACM 2.3 namespace: the shortest prefix a
/// type can be written with.
const P: &str = " xmlns:p=\"http://www.omg.org/spec/SACM/20220301\"";

fn read(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error}\n{input}"),
    }
}

/// Reads `input` in both modes, which must each give no diagnostic and the
/// same document; returns it.
fn read_clean(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read(what, mode, input);
        assert_eq!(
            output.diagnostics(),
            [],
            "{ROW}: {what}: a {mode:?} read gives diagnostics"
        );
        documents.push(output.into_document());
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents: {:?}",
        semantic_compare(&documents[0], &documents[1])
    );
    documents.swap_remove(0)
}

fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error}"),
    }
}

fn href(value: &str) -> Href {
    match Href::new(value) {
        Ok(href) => href,
        Err(error) => panic!("{ROW}: setup: Href::new({value:?}) refused it: {error}"),
    }
}

/// A root of `class`, `id`, declaring `sacm`, `xmi` and `declarations`, named
/// after its id, holding `body`.
fn root(class: &str, id: &str, declarations: &str, body: &str) -> String {
    format!(
        "<sacm:{class}{NS}{declarations} xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</sacm:{class}>"
    )
}

/// The proxies of `id`, each as its role and its href.
fn proxies(doc: &Document, id: &str) -> Vec<(&'static str, String)> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: the document has no element {id:?}"))
        .proxies()
        .iter()
        .map(|proxy| (proxy.role(), proxy.href().as_str().to_owned()))
        .collect()
}

/// A diagnostic as its code, severity, template, affected ids and its
/// text arguments by name.
type Found = (
    &'static str,
    Severity,
    &'static str,
    Vec<String>,
    Vec<(&'static str, String)>,
);

fn found(diagnostic: &Diagnostic) -> Found {
    (
        diagnostic.code().as_str(),
        diagnostic.severity(),
        diagnostic.template(),
        diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        diagnostic
            .arguments()
            .iter()
            .filter_map(|argument| match argument.value() {
                ArgumentValue::Text(text) => Some((argument.name(), text.clone())),
                _ => None,
            })
            .collect(),
    )
}

// =================================================================================
// The output bound of write (LIB-SEC-003), with proxies and hrefs
// =================================================================================

/// The bound of `write`'s docs, "# Output size": `(2·D + 48)·B + 6·M + 8192`.
fn bound(depth: usize, input: usize, kept: usize) -> usize {
    (2 * depth + 48) * input + 6 * kept + 8192
}

/// A chain of `AssuranceCasePackage`s `depth - 1` deep, each holding a
/// minimal proxy, so that every proxy line is written as deep as it was read.
fn proxies_down_a_chain(depth: usize) -> String {
    let nested = depth - 2;
    let proxy = "<argumentPackage href=\"a#b\"/>";
    let mut input = format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"a0\">{proxy}");
    for n in 1..=nested {
        let _ = write!(
            input,
            "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackage\" xmi:id=\"a{n}\">{proxy}"
        );
    }
    input.push_str(&"</assuranceCasePackage>".repeat(nested));
    input.push_str("</sacm:AssuranceCasePackage>");
    input
}

#[test]
fn sacm23_xmi_029_writing_proxies_and_hrefs_stays_within_the_output_bound() {
    const D: usize = 64;
    // What grows on the way out: a type written with a one-letter prefix is
    // held, and written, as `sacm:<Class>`; a proxy and an href element gain
    // a space before `/>`, an indentation and a line end; and an expression
    // href puts its name's end tag on a line of its own.
    let typed_proxies = root(
        "TerminologyPackage",
        "t",
        P,
        &"<terminologyElement href=\"p:Term a#b\"/>".repeat(2000),
    );
    let typed_hrefs = root(
        "ArgumentPackage",
        "p",
        P,
        &format!(
            "<argumentElement xmi:type=\"p:Claim\" xmi:id=\"c\">{}</argumentElement>",
            "<metaClaim href=\"p:Claim a#b\"/>".repeat(2000)
        ),
    );
    let mut terms = String::new();
    for n in 0..500 {
        let _ = write!(
            terms,
            "<terminologyElement xmi:type=\"p:Term\" xmi:id=\"t{n}\" value=\"v\">\
             <name xmi:type=\"p:ExpressionLangString\"><expression href=\"p:Term a#b\"/></name>\
             </terminologyElement>"
        );
    }
    let expression_hrefs = root("TerminologyPackage", "t", P, &terms);
    let escaped = root(
        "AssuranceCasePackage",
        "case",
        "",
        &"<argumentPackage href='a#\"\"\"\"&amp;&amp;&lt;&lt;'/>".repeat(1000),
    );
    for (what, input) in [
        (
            "2,000 proxies typed with a one-letter prefix",
            typed_proxies,
        ),
        ("2,000 hrefs typed with a one-letter prefix", typed_hrefs),
        ("500 expression hrefs", expression_hrefs),
        ("1,000 proxies whose hrefs need escaping", escaped),
        (
            "a proxy in each of 62 nested packages",
            proxies_down_a_chain(D),
        ),
    ] {
        let b = input.len();
        let limits = Limits::default()
            .with_max_depth(u64::try_from(D).unwrap_or(u64::MAX))
            .with_max_input_bytes(u64::try_from(b).unwrap_or(u64::MAX));
        for mode in MODES {
            let options = ReadOptions::default().with_mode(mode).with_limits(limits);
            let output = match xmi::read(input.as_bytes(), &options) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error}"),
            };
            assert_eq!(
                output.diagnostics(),
                [],
                "{ROW}: {what}: a {mode:?} read gives diagnostics"
            );
            let document = output.into_document();
            let text = written(what, &document, mode);
            let limit = bound(D, b, b);
            assert!(
                text.len() <= limit,
                "{ROW}: {what}: a {mode:?} write gives {} bytes from {b}, over the bound {limit} \
                 of the write docs (LIB-SEC-003)",
                text.len()
            );
            // The output reads back under the same limits, and to the same
            // document: nothing a read keeps here is amplified.
            let back_limits = limits.with_max_input_bytes(u64::try_from(limit).unwrap_or(u64::MAX));
            let back = match xmi::read(
                text.as_bytes(),
                &ReadOptions::default()
                    .with_mode(mode)
                    .with_limits(back_limits),
            ) {
                Ok(output) => output.into_document(),
                Err(error) => panic!("{ROW}: {what}: the {mode:?} output does not read: {error}"),
            };
            assert!(
                back == document,
                "{ROW}: {what}: the {mode:?} output read back is not =="
            );
        }
    }
}

// =================================================================================
// The type of a proxy, however it is given
// =================================================================================

/// The AssuranceCasePackage `case` holding `proxy`, and the roots `args` (an
/// ArgumentPackage) and `arts` (an ArtifactPackage), under `xmi:XMI`.
fn case_and_roots(proxy: &str) -> String {
    format!(
        "<xmi:XMI{NS}{XSI}><sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"case\"/>\
         {proxy}</sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:id=\"args\"><name content=\"args\"/></sacm:ArgumentPackage>\
         <sacm:ArtifactPackage xmi:id=\"arts\"><name content=\"arts\"/></sacm:ArtifactPackage>\
         </xmi:XMI>"
    )
}

/// The ids of the roots of `doc`.
fn root_ids(doc: &Document) -> Vec<&str> {
    doc.roots().map(|root| root.id().as_str()).collect()
}

/// Reads `input` in both modes, each of which must give exactly one
/// diagnostic, of `template`, affecting `case`; returns the strict document.
fn kept_in_case(what: &str, input: &str, template: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read(what, mode, input);
        let severity = if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        };
        assert_eq!(
            output
                .diagnostics()
                .iter()
                .map(|one| (one.severity(), one.template(), found(one).3))
                .collect::<Vec<_>>(),
            [(severity, template, vec!["case".to_owned()])],
            "{ROW}: {what}: a {mode:?} read does not give one {template} affecting case"
        );
        documents.push(output.into_document());
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    let doc = documents.swap_remove(0);
    assert!(
        doc.get("case")
            .is_some_and(|case| case.has_preserved_content() && case.proxies().is_empty()),
        "{ROW}: {what}: case does not keep the element, or holds a proxy"
    );
    doc
}

#[test]
fn sacm23_xmi_029_a_proxy_s_type_given_by_its_href_must_be_admitted_by_its_role() {
    // A cross-file proxy whose href's type its role does not admit is no
    // proxy of the role, as with a type attribute: kept whole.
    for proxy in [
        "<argumentPackage href=\"sacm:ArtifactPackage o.xmi#a\"/>",
        "<argumentPackage href=\"sacm:Claim o.xmi#c\"/>",
        "<argumentPackage href=\"sacm:ArtifactPackage #xpointer(id('a'))\"/>",
    ] {
        kept_in_case(proxy, &case_and_roots(proxy), "SACM-XMI-003/element-kept");
    }
    // So is a same-file proxy: `href="sacm:<Class> #id"` names the id, and
    // its type must be admitted too, although it is not held.
    for proxy in [
        "<argumentPackage href=\"sacm:ArtifactPackage #arts\"/>",
        "<argumentPackage href=\"sacm:ArtifactPackage #args\"/>",
        // A type attribute and the href's type that disagree.
        "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" \
         href=\"sacm:ArgumentPackageInterface #args\"/>",
        "<argumentPackage href=\"sacm:ArgumentPackageInterface #args\" \
         xmi:type=\"sacm:ArgumentPackage\"/>",
    ] {
        let doc = kept_in_case(proxy, &case_and_roots(proxy), "SACM-XMI-003/element-kept");
        assert_eq!(
            root_ids(&doc),
            ["case", "args", "arts"],
            "{ROW}: {proxy}: a root moved"
        );
    }
    // The type the href gives a same-file proxy is the proxy's type: the
    // root it names must be of it (SACM23-XMI-030, check 2).
    let proxy = "<argumentPackage href=\"sacm:ArgumentPackageInterface #args\"/>";
    for mode in MODES {
        let output = read(proxy, mode, &case_and_roots(proxy));
        let severity = if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        };
        assert_eq!(
            output.diagnostics().iter().map(found).collect::<Vec<_>>(),
            [(
                "SACM-XMI-003",
                severity,
                "SACM-XMI-003/proxy-wrong-kind",
                vec!["case".to_owned()],
                vec![
                    ("element", "argumentPackage".to_owned()),
                    ("target", "args".to_owned()),
                    ("target_class", "ArgumentPackage".to_owned()),
                    ("class", "ArgumentPackageInterface".to_owned()),
                    ("holder_class", "AssuranceCasePackage".to_owned()),
                    ("holder_id", "case".to_owned()),
                ]
            )],
            "{ROW}: {proxy}, {mode:?}: not one proxy-wrong-kind naming the href's type"
        );
        assert_eq!(
            root_ids(output.document()),
            ["case", "args", "arts"],
            "{ROW}: {proxy}, {mode:?}: a root moved"
        );
    }
    // A type that agrees, given both ways, resolves the proxy.
    let proxy = "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" \
                 href=\"sacm:ArgumentPackage #args\"/>";
    let doc = read_clean(proxy, &case_and_roots(proxy));
    assert_eq!(
        (
            root_ids(&doc),
            doc.get("args")
                .and_then(sacm::model::SacmElement::parent)
                .map(|parent| parent.id().as_str().to_owned())
        ),
        (vec!["case", "arts"], Some("case".to_owned())),
        "{ROW}: {proxy}: args is not moved into case"
    );
}

#[test]
fn sacm23_xmi_029_a_proxy_carries_at_most_one_type_attribute() {
    // Two type attributes, even in agreement, are more than §7.10.1's one:
    // the role element is kept whole.
    for proxy in [
        "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" xsi:type=\"sacm:ArgumentPackage\" \
         href=\"o.xmi#a\"/>",
        "<argumentPackage href=\"o.xmi#a\" xsi:type=\"sacm:ArgumentPackage\" \
         xmi:type=\"sacm:ArgumentPackage\"/>",
    ] {
        kept_in_case(proxy, &case_and_roots(proxy), "SACM-XMI-003/element-kept");
    }
}

#[test]
fn sacm23_xmi_029_an_xsi_type_on_a_cross_file_proxy_is_held_with_its_sacm_xmi_010() {
    // The proxy is held, its type as the href's; its SACM-XMI-010 affects the
    // container and names the proxy, its container and the class (ADR 0040
    // amendment 1, A9).
    let proxy = "<argumentPackage xsi:type=\"sacm:ArgumentPackageInterface\" href=\"o.xmi#api\"/>";
    let input = root("AssuranceCasePackage", "case", XSI, proxy);
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read(proxy, mode, &input);
        let severity = if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        };
        assert_eq!(
            output.diagnostics().iter().map(found).collect::<Vec<_>>(),
            [(
                "SACM-XMI-010",
                severity,
                "SACM-XMI-010/xsi-type-on-proxy",
                vec!["case".to_owned()],
                vec![
                    ("element", "argumentPackage".to_owned()),
                    ("holder_class", "AssuranceCasePackage".to_owned()),
                    ("holder_id", "case".to_owned()),
                    ("class", "ArgumentPackageInterface".to_owned()),
                ]
            )],
            "{ROW}: {proxy}, {mode:?}: not one SACM-XMI-010 affecting the container"
        );
        let at = output.diagnostics()[0]
            .location()
            .map(|at| (at.line(), at.column()));
        let column = input.find("xsi:type").map(|offset| offset + 1);
        assert_eq!(
            at.map(|(line, column)| (line, usize::try_from(column).ok())),
            Some((1, Some(column.unwrap_or(0)))),
            "{ROW}: {proxy}, {mode:?}: the SACM-XMI-010 is not at the attribute"
        );
        documents.push(output.into_document());
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {proxy}: the modes read different documents"
    );
    let doc = &documents[0];
    assert_eq!(
        proxies(doc, "case"),
        [(
            "argumentPackage",
            "sacm:ArgumentPackageInterface o.xmi#api".to_owned()
        )],
        "{ROW}: {proxy}: the proxy is not held with its type"
    );
    assert!(
        !doc.has_preserved_content(),
        "{ROW}: {proxy}: the proxy is kept as preserved content"
    );
    // Both saves write it, with no xsi:type.
    for mode in MODES {
        let text = written(proxy, doc, mode);
        assert!(
            text.contains("<argumentPackage href=\"sacm:ArgumentPackageInterface o.xmi#api\" />")
                && !text.contains("xsi:"),
            "{ROW}: {proxy}: a {mode:?} write does not write the proxy with its type\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_029_a_spelled_type_on_a_cross_file_proxy_is_read_tolerantly_and_kept_strictly() {
    // The model's spelling of ArtifactAssetRelationship as the proxy's type
    // (SACM23-XMI-009): a tolerant read holds the proxy under the resolved
    // class, with one SACM-XMI-013 Warning affecting the container; a strict
    // read keeps the role element whole with the Error.
    let proxy =
        "<artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" href=\"o.xmi#r\"/>";
    let input = root("ArtifactPackage", "holder", "", proxy);
    let tolerant = read(proxy, Mode::Tolerant, &input);
    assert_eq!(
        tolerant.diagnostics().iter().map(found).collect::<Vec<_>>(),
        [(
            "SACM-XMI-013",
            Severity::Warning,
            "SACM-XMI-013/model-type-in-root-read",
            vec!["holder".to_owned()],
            vec![
                ("element", "artifactElement".to_owned()),
                ("container", "sacm:ArtifactPackage".to_owned()),
            ]
        )],
        "{ROW}: {proxy}, Tolerant: not one SACM-XMI-013 affecting the container"
    );
    assert_eq!(
        (
            proxies(tolerant.document(), "holder"),
            tolerant.document().has_preserved_content()
        ),
        (
            vec![(
                "artifactElement",
                "sacm:ArtifactAssetRelationship o.xmi#r".to_owned()
            )],
            false
        ),
        "{ROW}: {proxy}, Tolerant: the proxy is not held under the resolved class"
    );
    let strict = read(proxy, Mode::Strict, &input);
    assert_eq!(
        strict
            .diagnostics()
            .iter()
            .map(|one| (one.severity(), one.template(), found(one).3))
            .collect::<Vec<_>>(),
        [(
            Severity::Error,
            "SACM-XMI-013/model-type-in-root-kept",
            vec!["holder".to_owned()]
        )],
        "{ROW}: {proxy}, Strict: not one SACM-XMI-013 Error"
    );
    assert!(
        strict
            .document()
            .get("holder")
            .is_some_and(|holder| holder.has_preserved_content() && holder.proxies().is_empty()),
        "{ROW}: {proxy}, Strict: the role element is not kept whole"
    );
    // The same spelling as the href's type names no class: it is no QName of
    // a concrete class (rule 2n), in either mode.
    let by_href = "<artifactElement href=\"sacm:ArtifactAssertedRelationship o.xmi#r\"/>";
    let input = root("ArtifactPackage", "holder", "", by_href);
    for mode in MODES {
        let output = read(by_href, mode, &input);
        assert_eq!(
            output
                .diagnostics()
                .iter()
                .map(Diagnostic::template)
                .collect::<Vec<_>>(),
            ["SACM-XMI-003/element-kept"],
            "{ROW}: {by_href}, {mode:?}"
        );
    }
}

// =================================================================================
// Where a proxy stands, and what is written around it
// =================================================================================

#[test]
fn sacm23_xmi_029_a_cross_file_proxy_takes_no_place_among_the_children() {
    // Preserved content keeps its place among the typed children, which a
    // proxy before it does not shift: `<v:x/>` stays between `a` and `b`.
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmlns:v=\"urn:v\" xmi:id=\"case\">\
         <argumentPackage xmi:id=\"a\"/><argumentPackage href=\"o.xmi#p\"/><v:x/>\
         <argumentPackage xmi:id=\"b\"/></sacm:AssuranceCasePackage>"
    );
    let what = "a proxy before preserved content";
    let output = read(what, Mode::Tolerant, &input);
    let doc = output.document();
    assert_eq!(
        (
            proxies(doc, "case"),
            doc.get("case").map(|case| case
                .children()
                .map(|child| child.id().as_str().to_owned())
                .collect::<Vec<_>>())
        ),
        (
            vec![("argumentPackage", "o.xmi#p".to_owned())],
            Some(vec!["a".to_owned(), "b".to_owned()])
        ),
        "{ROW}: {what}: the proxy and the children"
    );
    let text = written(what, doc, Mode::Tolerant);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert_eq!(
        &lines[2..],
        [
            "<argumentPackage xmi:id=\"a\" />",
            "<v:x xmlns:v=\"urn:v\" />",
            "<argumentPackage xmi:id=\"b\" />",
            "<argumentPackage href=\"o.xmi#p\" />",
            "</sacm:AssuranceCasePackage>",
        ],
        "{ROW}: {what}: the preserved element does not keep its place between a and b\n{text}"
    );
    let back = read(what, Mode::Tolerant, &text).into_document();
    assert!(
        back == *doc,
        "{ROW}: {what}: the output read back is not =="
    );
}

#[test]
fn sacm23_xmi_029_an_element_holding_only_a_proxy_is_written_with_an_end_tag() {
    // No name, no child, no text: the proxy alone makes the element a start
    // and an end tag.
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\
         <argumentPackage xmi:id=\"args\"><argumentElement href=\"o.xmi#c\"/></argumentPackage>\
         </sacm:AssuranceCasePackage>"
    );
    let what = "an unnamed package holding only a proxy";
    let doc = read_clean(what, &input);
    assert_eq!(
        proxies(&doc, "args"),
        [("argumentElement", "o.xmi#c".to_owned())],
        "{ROW}: {what}: the proxy of args"
    );
    for mode in MODES {
        let text = written(what, &doc, mode);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        assert_eq!(
            &lines[2..],
            [
                "<argumentPackage xmi:id=\"args\">",
                "<argumentElement href=\"o.xmi#c\" />",
                "</argumentPackage>",
                "</sacm:AssuranceCasePackage>",
            ],
            "{ROW}: {what}: a {mode:?} write\n{text}"
        );
        let back = read(what, mode, &text).into_document();
        assert!(
            back == doc,
            "{ROW}: {what}: the {mode:?} output read back is not =="
        );
    }
}

#[test]
fn sacm23_xmi_029_a_proxy_is_written_after_the_placed_and_preserved_children() {
    // A child, then XMI metadata, then the proxy, and in tolerant mode
    // preserved content after it: the proxy is written last, before the
    // container's end tag, wherever the input had it.
    for mode in MODES {
        let trailing = if mode == Mode::Tolerant {
            "<v:x xmlns:v=\"urn:v\"/>"
        } else {
            ""
        };
        let input = format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name/>\
             <argumentPackage xmi:id=\"args\"><name content=\"args\"/></argumentPackage>\
             <xmi:Extension extender=\"t\"/>\
             <argumentPackage href=\"o.xmi#a\"/>{trailing}\
             </sacm:AssuranceCasePackage>"
        );
        let what = format!("a proxy between the metadata and preserved content, {mode:?}");
        let output = read(&what, mode, &input);
        if mode == Mode::Strict {
            assert_eq!(
                output.diagnostics(),
                [],
                "{ROW}: {what}: a strict read gives diagnostics"
            );
        }
        let doc = output.into_document();
        assert_eq!(
            proxies(&doc, "case"),
            [("argumentPackage", "o.xmi#a".to_owned())],
            "{ROW}: {what}: the proxy of case"
        );
        let text = written(&what, &doc, mode);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        let extension = lines
            .iter()
            .position(|line| line.starts_with("<xmi:Extension "))
            .unwrap_or_else(|| panic!("{ROW}: {what}: no extension is written\n{text}"));
        let rest = &lines[extension + 1..];
        let preserved = usize::from(mode == Mode::Tolerant);
        assert_eq!(
            rest.len(),
            preserved + 2,
            "{ROW}: {what}: not the proxy and the end tag after the extension\n{text}"
        );
        if mode == Mode::Tolerant {
            assert!(
                rest[0].starts_with("<v:x") && rest[0].ends_with("/>"),
                "{ROW}: {what}: the preserved element is not written right after the \
                 extension\n{text}"
            );
        }
        assert_eq!(
            &rest[preserved..],
            [
                "<argumentPackage href=\"o.xmi#a\" />",
                "</sacm:AssuranceCasePackage>"
            ],
            "{ROW}: {what}: the proxy is not written after the placed and preserved \
             children\n{text}"
        );
        assert!(
            lines[..extension].contains(&"</argumentPackage>"),
            "{ROW}: {what}: the child package is not written before the extension\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_029_an_expression_href_is_written_before_the_metadata_of_its_name() {
    // The `expression` reference element comes first in its name or value,
    // then the XMI metadata children, wherever the input had it.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
         <name xmi:type=\"sacm:ExpressionLangString\"><xmi:Extension extender=\"tool\"/>\
         <expression href=\"g.xmi#t\"/></name>\
         <note xmi:id=\"n\"><content><value xmi:type=\"sacm:ExpressionLangString\">\
         <xmi:Extension extender=\"tool\"/><expression href=\"g.xmi#u\"/></value></content></note>\
         </argumentElement></sacm:ArgumentPackage>"
    );
    let what = "an expression href beside an xmi:Extension";
    let doc = read_clean(what, &input);
    for mode in MODES {
        let text = written(what, &doc, mode);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (open, href, close) in [
            (
                "<name xmi:type=\"sacm:ExpressionLangString\">",
                "g.xmi#t",
                "</name>",
            ),
            (
                "<value xmi:type=\"sacm:ExpressionLangString\">",
                "g.xmi#u",
                "</value>",
            ),
        ] {
            let Some(at) = lines.iter().position(|line| *line == open) else {
                panic!("{ROW}: {what}: a {mode:?} write has no line {open}\n{text}");
            };
            assert_eq!(
                lines.get(at + 1..at + 4),
                Some(
                    [
                        format!("<expression href=\"{href}\" />").as_str(),
                        "<xmi:Extension extender=\"tool\" />",
                        close,
                    ]
                    .as_slice()
                ),
                "{ROW}: {what}: a {mode:?} write does not write the expression first\n{text}"
            );
        }
        let back = read(what, mode, &text).into_document();
        assert!(
            back == doc,
            "{ROW}: {what}: the {mode:?} output read back is not =="
        );
    }
}

// =================================================================================
// What names an expression held in another document
// =================================================================================

#[test]
fn sacm23_xmi_029_an_expression_href_is_named_in_messages_and_in_debug() {
    let name = "<name xmi:type=\"sacm:ExpressionLangString\" content=\"x\">\
                <expression href=\"glossary.sacm.xmi#blade\"/></name>";
    let input = format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"terms\"><name content=\"terms\"/>\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"blade\">{name}\
         </terminologyElement></sacm:TerminologyPackage>"
    );
    let what = "a content beside an expression href";
    let doc = read_clean(what, &input);
    // SACM-EXPR-001 names the href as its expression.
    let findings = validation::validate(&doc);
    assert_eq!(
        findings.iter().map(found).collect::<Vec<_>>(),
        [(
            "SACM-EXPR-001",
            Severity::Warning,
            "SACM-EXPR-001/name-with-content",
            vec!["t1".to_owned()],
            vec![
                ("class", "Term".to_owned()),
                ("id", "t1".to_owned()),
                ("expression", "glossary.sacm.xmi#blade".to_owned()),
            ]
        )],
        "{ROW}: {what}: validate"
    );
    // Debug shows the href as the expression; a compared name shows it too.
    let held = doc
        .get("t1")
        .and_then(sacm::model::SacmElement::name)
        .unwrap_or_else(|| panic!("{ROW}: {what}: t1 has no name"));
    assert_eq!(
        format!("{held:?}"),
        "ExpressionLangString { lang: None, content: Some(\"x\"), \
         expression: Some(Href(\"glossary.sacm.xmi#blade\")) }",
        "{ROW}: {what}: Debug"
    );
    let by_id = LangString::new("x").with_expression("blade");
    let none = match xmi::read(
        input
            .replace("<expression href=\"glossary.sacm.xmi#blade\"/>", "")
            .as_bytes(),
        &ReadOptions::default(),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: the name with no expression: {error}"),
    };
    let without = none
        .get("t1")
        .and_then(sacm::model::SacmElement::name)
        .unwrap_or_else(|| panic!("{ROW}: setup: t1 has no name"));
    assert_eq!(
        (format!("{by_id:?}"), format!("{without:?}")),
        (
            "ExpressionLangString { lang: None, content: Some(\"x\"), \
             expression: Some(ElementId(\"blade\")) }"
                .to_owned(),
            "ExpressionLangString { lang: None, content: Some(\"x\"), expression: None }"
                .to_owned()
        ),
        "{ROW}: Debug of an expression by id and of none"
    );
    // An id sorts before an href, and no expression before both.
    let by_href = held.clone();
    assert!(
        *without < by_id && by_id < by_href,
        "{ROW}: the order of no expression, an id and an href"
    );
    let differences = semantic_compare(&doc, &none);
    assert_eq!(
        differences
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "element \"t1\" differs in name: [no lang, content \"x\", expression href \
          \"glossary.sacm.xmi#blade\"] on the left, [no lang, content \"x\", no expression] on \
          the right"
        ],
        "{ROW}: {what}: the Name difference"
    );
}

#[test]
fn sacm23_xmi_029_a_proxies_difference_names_each_role_and_href() {
    let with = |proxies: &str| root("AssuranceCasePackage", "case", "", proxies);
    let left = read_clean(
        "left",
        &with(
            "<argumentPackage href=\"a.xmi#a\"/><artifactPackage href=\"sacm:ArtifactPackage b.xmi#b\"/>",
        ),
    );
    let right = read_clean("right", &with(""));
    let differences = semantic_compare(&left, &right);
    assert!(
        matches!(differences.as_slice(), [Difference::Proxies { .. }]),
        "{ROW}: not one Proxies difference: {differences:?}"
    );
    assert_eq!(
        differences[0].to_string(),
        "element \"case\" differs in its proxies: [argumentPackage \"a.xmi#a\", artifactPackage \
         \"sacm:ArtifactPackage b.xmi#b\"] on the left, [] on the right",
        "{ROW}: the Display of a Proxies difference"
    );
}

// =================================================================================
// A resolved same-file proxy, against a kept one (A9)
// =================================================================================

#[test]
fn sacm23_xmi_029_a_resolved_proxy_s_diagnostics_affect_the_root_or_the_container() {
    // A kept proxy stands for no element, so what its start tag says affects
    // its container (`cross_document_links.rs`). A resolved one stands for
    // the root that takes its place: its `xsi:type` is that root's, and names
    // it; the spelling of its role is its container's, as a child's is.
    let input = format!(
        "<xmi:XMI{NS}{XSI}><sacm:ArtifactPackage xmi:id=\"holder\"><name content=\"holder\"/>\
         <artifactElement xsi:type=\"sacm:ArtifactPackage\" xmi:idref=\"arts\"/>\
         </sacm:ArtifactPackage>\
         <sacm:ArtifactPackage xmi:id=\"arts\"><name content=\"arts\"/></sacm:ArtifactPackage>\
         </xmi:XMI>"
    );
    for mode in MODES {
        let what = format!("a resolved xsi:type proxy, {mode:?}");
        let output = read(&what, mode, &input);
        let severity = if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        };
        assert_eq!(
            output.diagnostics().iter().map(found).collect::<Vec<_>>(),
            [(
                "SACM-XMI-010",
                severity,
                "SACM-XMI-010/xsi-type",
                vec!["arts".to_owned()],
                vec![
                    ("class", "ArtifactPackage".to_owned()),
                    ("id", "arts".to_owned()),
                ]
            )],
            "{ROW}: {what}: its SACM-XMI-010 does not affect and name the root it resolves to"
        );
        assert_eq!(
            root_ids(output.document()),
            ["holder"],
            "{ROW}: {what}: arts is not moved"
        );
    }
    let input = format!(
        "<xmi:XMI{NS}><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/>\
         <argumentationElement xmi:idref=\"q\"/></sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"q\"><name content=\"q\"/></sacm:ArgumentPackage>\
         </xmi:XMI>"
    );
    let what = "a resolved proxy in the role argumentationElement, Tolerant";
    let output = read(what, Mode::Tolerant, &input);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|one| (one.code().as_str(), one.severity(), found(one).3))
            .collect::<Vec<_>>(),
        [("SACM-XMI-013", Severity::Warning, vec!["p".to_owned()])],
        "{ROW}: {what}: the spelling of its role does not affect its container"
    );
    assert_eq!(
        root_ids(output.document()),
        ["p"],
        "{ROW}: {what}: q is not moved"
    );
}

// =================================================================================
// LIB-EDIT-020: what the tests of edit_hrefs.rs did not pin
// =================================================================================

/// Applies `operation`, which must be accepted.
fn applied(doc: &mut Document, operation: impl Into<edit::Operation>) {
    let operation = operation.into();
    if let Err(error) = edit::apply(doc, operation.clone()) {
        panic!("{EDIT_ROW}: {operation:?} was refused: {error}");
    }
}

/// The ids and the hrefs of the end `end` of `id`.
fn end_of(doc: &Document, id: &str, end: ReferenceEnd) -> (Vec<String>, Vec<String>) {
    let element = doc
        .get(id)
        .unwrap_or_else(|| panic!("{EDIT_ROW}: the document has no element {id:?}"));
    (
        element
            .references(end)
            .iter()
            .map(|one| one.as_str().to_owned())
            .collect(),
        element
            .hrefs(end)
            .iter()
            .map(|one| one.as_str().to_owned())
            .collect(),
    )
}

#[test]
fn lib_edit_020_href_new_refuses_white_space_that_follows_no_type() {
    // Either parenthesis alone makes a same-document fragment an XPointer.
    for value in ["#a(b", "#a)b", "sacm:Claim #(", "sacm:Claim #)"] {
        assert_eq!(
            Href::new(value).map(|href| (href.is_xpointer(), href.document().to_owned())),
            Ok((true, String::new())),
            "{EDIT_ROW}: Href::new({value:?})"
        );
    }
    // The one space allowed stands after a type: before it stands no `#`,
    // and after it something.
    for value in [
        "#a b",
        "o.xmi#a b",
        "o.xmi#a b#c",
        "sacm:Claim ",
        "foo ",
        "o.xmi# ",
    ] {
        assert_eq!(
            Href::new(value),
            Err(sacm::model::HrefError::WhiteSpace),
            "{EDIT_ROW}: Href::new({value:?})"
        );
    }
    // A type is what stands before the space, whatever else is wrong after.
    for value in ["foo o.xmi#a#b", "x:Claim #c", "sacm:Assertion o.xmi"] {
        assert_eq!(
            Href::new(value),
            Err(sacm::model::HrefError::Type),
            "{EDIT_ROW}: Href::new({value:?})"
        );
    }
}

#[test]
fn lib_edit_020_the_builders_replace_and_the_accessors_give_what_was_given() {
    let (h1, h2) = (href("a.xmi#a"), href("b.xmi#b"));
    let end = ReferenceEnd::ParticipantPackage;
    let set = SetReferences::new("b", end, ["p"])
        .with_hrefs([h1.clone(), h1.clone()])
        .with_hrefs([h2.clone(), h1.clone()]);
    assert_eq!(
        (set.ids(), set.hrefs()),
        (
            [ElementId::from("p")].as_slice(),
            [h2.clone(), h1.clone()].as_slice()
        ),
        "{EDIT_ROW}: with_hrefs does not replace the hrefs given before"
    );
    assert_eq!(
        (
            SetReferences::new("b", end, ["p"]).hrefs(),
            SetReferences::clear("b", end).hrefs()
        ),
        ([].as_slice(), [].as_slice()),
        "{EDIT_ROW}: new and clear give hrefs"
    );
    let add = AddReference::href("b", end, h1.clone());
    let remove = RemoveReference::href("b", end, h2.clone());
    assert_eq!(
        (
            add.element().as_str(),
            add.end(),
            add.id(),
            add.cross_document(),
            remove.element().as_str(),
            remove.end(),
            remove.id(),
            remove.cross_document()
        ),
        ("b", end, None, Some(&h1), "b", end, None, Some(&h2)),
        "{EDIT_ROW}: the accessors of the href forms"
    );
    let by_id = (
        AddReference::new("b", end, "p"),
        RemoveReference::new("b", end, "q"),
    );
    assert_eq!(
        (
            by_id.0.id().map(ElementId::as_str),
            by_id.0.cross_document(),
            by_id.1.id().map(ElementId::as_str),
            by_id.1.cross_document()
        ),
        (Some("p"), None, Some("q"), None),
        "{EDIT_ROW}: the accessors of the id forms"
    );
    let binding = CreateArgumentPackageBinding::root(LangString::new("B"), ["p"])
        .with_participant_package_hrefs([h1.clone(), h1.clone()])
        .with_participant_package_hrefs([h2.clone()]);
    assert_eq!(
        (
            binding.participant_packages(),
            binding.participant_package_hrefs()
        ),
        ([ElementId::from("p")].as_slice(), [h2.clone()].as_slice()),
        "{EDIT_ROW}: with_participant_package_hrefs does not replace the hrefs given before"
    );
    assert_eq!(
        CreateArgumentPackageBinding::new("case", LangString::new("B"), ["p", "q"])
            .participant_package_hrefs(),
        [],
        "{EDIT_ROW}: a binding created with ids gives hrefs"
    );
    let interface = CreateArgumentPackageInterface::root(LangString::new("I"), "p");
    assert_eq!(
        (
            interface.implements().map(ElementId::as_str),
            interface.implements_href()
        ),
        (Some("p"), None),
        "{EDIT_ROW}: an interface created with an id"
    );
}

/// The case of the edit tests: the ArgumentPackage `p`, its interface `i`
/// (implements `p`) and the Claim `c1` in `p`.
fn interface_case() -> Document {
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"p\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
         </argumentElement></argumentPackage>\
         <argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" implements=\"p\">\
         <name content=\"i\"/></argumentPackage></sacm:AssuranceCasePackage>"
    );
    read_clean("the interface case", &input)
}

#[test]
fn lib_edit_020_a_set_swaps_the_one_value_of_a_single_valued_end() {
    // `implements` holds exactly one value. A set that replaces its id by an
    // href, and one that replaces its href by an id, never holds two on the
    // way, and leaves exactly the value given.
    let end = ReferenceEnd::Implements;
    let remote = href("o.xmi#p");
    let mut doc = interface_case();
    applied(
        &mut doc,
        SetReferences::new("i", end, Vec::<ElementId>::new()).with_hrefs([remote.clone()]),
    );
    assert_eq!(
        end_of(&doc, "i", end),
        (Vec::new(), vec!["o.xmi#p".to_owned()]),
        "{EDIT_ROW}: the id of implements is not replaced by the href"
    );
    applied(&mut doc, SetReferences::new("i", end, ["p"]));
    assert_eq!(
        end_of(&doc, "i", end),
        (vec!["p".to_owned()], Vec::new()),
        "{EDIT_ROW}: the href of implements is not replaced by the id"
    );
    // Both at once are one too many, and change nothing.
    let before = doc.clone();
    let error = edit::apply(
        &mut doc,
        SetReferences::new("i", end, ["p"]).with_hrefs([remote.clone()]),
    );
    assert!(
        matches!(
            error,
            Err(EditError::TooManyReferences {
                end: "implements",
                given: 2,
                allowed: 1,
                ..
            })
        ) && doc == before,
        "{EDIT_ROW}: an id and an href on implements: {error:?}"
    );
    // The written form of the href end, and its read back.
    applied(
        &mut doc,
        SetReferences::new("i", end, Vec::<ElementId>::new()).with_hrefs([remote]),
    );
    let text = written("implements by href", &doc, Mode::Strict);
    assert!(
        text.contains("<implements href=\"o.xmi#p\" />") && !text.contains(" implements=\""),
        "{EDIT_ROW}: implements is not written in element form\n{text}"
    );
    assert!(
        read("implements by href", Mode::Strict, &text).into_document() == doc,
        "{EDIT_ROW}: the output read back is not =="
    );
}

#[cfg(feature = "serde")]
#[test]
fn lib_edit_020_json_refuses_null_for_a_link_key() {
    // Exactly one of the two keys, and the one given holds a value: `null`
    // is no absent key.
    for json in [
        r#"{"type":"AddReference","element":"b","end":"participantPackage","id":"p","href":null}"#,
        r#"{"type":"AddReference","element":"b","end":"participantPackage","id":null,"href":"o.xmi#p"}"#,
        r#"{"type":"RemoveReference","element":"b","end":"participantPackage","id":"p","href":null}"#,
        r#"{"type":"RemoveReference","element":"b","end":"participantPackage","id":null,"href":"o.xmi#p"}"#,
        r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"},"implements":"p","implements_href":null}"#,
        r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"},"implements":null,"implements_href":"o.xmi#p"}"#,
        r#"{"type":"CreateArgumentPackageInterface","parent":null,"id":null,"name":{"lang":"en","content":"I"},"implements":null}"#,
    ] {
        if let Ok(read) = serde_json::from_str::<edit::Operation>(json) {
            panic!("{EDIT_ROW}: {json} was read as {read:?}");
        }
    }
    // The controls: the same objects without the null key read.
    for json in [
        r#"{"type":"AddReference","element":"b","end":"participantPackage","id":"p"}"#,
        r#"{"type":"RemoveReference","element":"b","end":"participantPackage","href":"o.xmi#p"}"#,
        r#"{"type":"CreateArgumentPackageInterface","parent":null,"name":{"lang":"en","content":"I"},"implements_href":"o.xmi#p"}"#,
    ] {
        if let Err(error) = serde_json::from_str::<edit::Operation>(json) {
            panic!("{EDIT_ROW}: setup: {json} does not read: {error}");
        }
    }
}

#[test]
fn lib_edit_020_an_expression_href_stays_out_of_the_expression_end() {
    // A name whose expression is an href adds no id to the derived end, and
    // setting it over an id removes that id from it.
    let mut doc = interface_case();
    let remote = href("terms.sacm.xmi#blade");
    applied(
        &mut doc,
        CreateClaim::new(
            "p",
            LangString::new("x").with_expression_href(remote.clone()),
        )
        .with_id("c2"),
    );
    applied(
        &mut doc,
        SetName::new(
            "c1",
            LangString::new("y").with_expression_href(remote.clone()),
        ),
    );
    for id in ["c1", "c2"] {
        let element = doc
            .get(id)
            .unwrap_or_else(|| panic!("{EDIT_ROW}: no element {id}"));
        assert_eq!(
            (
                element.expressions(),
                element.name().and_then(LangString::expression_href)
            ),
            ([].as_slice(), Some(&remote)),
            "{EDIT_ROW}: the name of {id}"
        );
    }
    assert!(
        validation::validate(&doc)
            .iter()
            .all(|one| one.code().as_str() != "SACM-MULT-001"
                && !one.code().as_str().starts_with("SACM-REF-")),
        "{EDIT_ROW}: validate counts the expression href as missing or dangling"
    );
}
