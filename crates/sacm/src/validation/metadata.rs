//! The schema check of XMI metadata a strict save runs (SACM23-XMI-019;
//! XMI.xsd; ADR 0042): the `xmi:id`s it holds must be `NCName`s, and its
//! `IDREF` and `IDREFS` attributes must name ids the output writes.

use std::collections::BTreeSet;

use crate::diagnostic::template::{ID_002_METADATA_ID_NOT_AN_NCNAME, REF_001_METADATA_DANGLING};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Attr, Document, ElementId, Event, Item, Preserved, QName, XMI_NS, is_ncname};

/// The row the findings enforce.
const ROW: &str = "SACM23-XMI-019";

/// The XMI.xsd type of an element of XMI metadata, which says which of its
/// attributes are `IDREF` or `IDREFS` and which of its children are walked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    /// `documentation` or `Documentation`.
    Documentation,
    /// `extension` or `Extension`.
    Extension,
    /// `difference`, `Difference` or `Delete`.
    Difference,
    /// `Add`.
    Add,
    /// `Replace`.
    Replace,
}

impl Type {
    /// The type of the XMI element `name`, if it is one of XMI.xsd's.
    fn of_xmi(name: &QName) -> Option<Self> {
        if name.namespace.as_deref() != Some(XMI_NS) {
            return None;
        }
        match name.local.as_str() {
            "documentation" | "Documentation" => Some(Self::Documentation),
            "extension" | "Extension" => Some(Self::Extension),
            "difference" | "Difference" | "Delete" => Some(Self::Difference),
            "Add" => Some(Self::Add),
            "Replace" => Some(Self::Replace),
            _ => None,
        }
    }

    /// The type of the child `name` of an element of this type, if it is
    /// walked: an `xmi:Extension` in a Documentation or a Difference, an XMI
    /// element in an Extension, and an unqualified `difference` or
    /// `container` in a Difference. A Difference's `target` never is.
    fn child(self, name: &QName) -> Option<Self> {
        let is_extension = name.namespace.as_deref() == Some(XMI_NS) && name.local == "Extension";
        match self {
            Self::Documentation => is_extension.then_some(Self::Extension),
            Self::Extension => Self::of_xmi(name),
            Self::Difference | Self::Add | Self::Replace => {
                if is_extension {
                    Some(Self::Extension)
                } else if name.namespace.is_none()
                    && matches!(name.local.as_str(), "difference" | "container")
                {
                    Some(Self::Difference)
                } else {
                    None
                }
            }
        }
    }

    /// Whether `attr` of an element of this type is an `IDREF` or `IDREFS`
    /// attribute of XMI.xsd: `xmi:idref` on each, and the unqualified
    /// `target` and `container` of a Difference, with `addition` on an Add
    /// and `replacement` on a Replace.
    fn is_reference(self, attr: &Attr) -> bool {
        let name = &attr.name;
        if name.namespace.as_deref() == Some(XMI_NS) {
            return name.local == "idref";
        }
        if name.namespace.is_some() {
            return false;
        }
        matches!(
            (self, name.local.as_str()),
            (
                Self::Difference | Self::Add | Self::Replace,
                "target" | "container"
            ) | (Self::Add, "addition")
                | (Self::Replace, "replacement")
        )
    }
}

/// SACM23-XMI-019: the XMI metadata of `document` as XMI.xsd types it, for a
/// strict save (ADR 0042). Each `xmi:id` in metadata that is not an `NCName`
/// is one SACM-ID-002 Error, with clause 7.6.1; each distinct token of an
/// `IDREF` or `IDREFS` attribute of an element of metadata that names no
/// `xmi:id` of the document is one SACM-REF-001 Error, per attribute: the
/// `xmi:idref` of `xmi:XMI`, and those of the elements XMI.xsd defines, by
/// their type. Both affect the element that holds the metadata, if one does;
/// a dangling one affects the token too. What an extension holds is not
/// checked otherwise, which is a documented limit.
pub(super) fn unwritten(document: &Document, found: &mut Vec<Diagnostic>) {
    if let Some(metadata) = document.document_metadata() {
        let wrapper = QName {
            namespace: Some(XMI_NS.to_owned()),
            prefix: Some("xmi".to_owned()),
            local: "XMI".to_owned(),
        };
        let written = wrapper.written();
        for attr in &metadata.attributes {
            let name = &attr.name;
            if name.namespace.as_deref() != Some(XMI_NS) {
                continue;
            }
            match name.local.as_str() {
                "id" => check_id(&written, &attr.value, None, found),
                "idref" => check_references(document, attr, &written, None, found),
                _ => {}
            }
        }
        check_fragments(document, metadata, None, found);
    }
    for element in document.elements() {
        for metadata in element.metadata_parts() {
            check_fragments(document, metadata, Some(element.id()), found);
        }
    }
}

/// Checks the elements of `metadata`, which `holder` holds, or the document
/// for `None`: each `xmi:id` in them, and the references of each element
/// XMI.xsd types, walked from the top of each fragment by type.
fn check_fragments(
    document: &Document,
    metadata: &Preserved,
    holder: Option<&ElementId>,
    found: &mut Vec<Diagnostic>,
) {
    for placed in &metadata.children {
        let Item::Element(fragment) = &placed.item else {
            continue;
        };
        // The type of each open element, innermost last; `None` for one that
        // is not walked, whose children are not either.
        let mut open: Vec<Option<Type>> = Vec::new();
        for event in &fragment.events {
            match event {
                Event::Start {
                    name, attributes, ..
                } => {
                    let kind = match open.last() {
                        None => Type::of_xmi(name),
                        Some(parent) => parent.and_then(|parent| parent.child(name)),
                    };
                    let written = name.written();
                    for attr in attributes {
                        let attribute = &attr.name;
                        if attribute.namespace.as_deref() == Some(XMI_NS) && attribute.local == "id"
                        {
                            check_id(&written, &attr.value, holder, found);
                        } else if kind.is_some_and(|kind| kind.is_reference(attr)) {
                            check_references(document, attr, &written, holder, found);
                        }
                    }
                    open.push(kind);
                }
                Event::End => {
                    open.pop();
                }
                Event::Text(_) => {}
            }
        }
    }
}

/// The SACM-ID-002 Error of the `xmi:id` `value` of the element `element`, if
/// its value, without the XML white space around it, is no `NCName`.
fn check_id(element: &str, value: &str, holder: Option<&ElementId>, found: &mut Vec<Diagnostic>) {
    if is_ncname(value.trim_matches(is_xml_space)) {
        return;
    }
    let arguments = vec![
        Argument::text("element", element),
        Argument::text("xmi_id", value),
    ];
    found.push(
        Diagnostic::emit(
            &ID_002_METADATA_ID_NOT_AN_NCNAME,
            Severity::Error,
            ROW,
            arguments,
        )
        .set_clause(Specification::Xmi251, "7.6.1")
        .set_affected(holder.cloned()),
    );
}

/// The SACM-REF-001 Errors of the `IDREF` or `IDREFS` attribute `attr` of
/// the element `element`: one per distinct token, split on XML white space,
/// that is no `xmi:id` of `document`.
fn check_references(
    document: &Document,
    attr: &Attr,
    element: &str,
    holder: Option<&ElementId>,
    found: &mut Vec<Diagnostic>,
) {
    let attribute = attr.name.written();
    let mut reported = BTreeSet::new();
    for token in attr
        .value
        .split(is_xml_space)
        .filter(|token| !token.is_empty())
    {
        if document.is_id_in_use(token) || !reported.insert(token) {
            continue;
        }
        let arguments = vec![
            Argument::text("attribute", attribute.as_str()),
            Argument::text("element", element),
            Argument::text("target", token),
        ];
        found.push(
            Diagnostic::emit(&REF_001_METADATA_DANGLING, Severity::Error, ROW, arguments)
                .set_affected(holder.cloned().into_iter().chain([ElementId::new(token)])),
        );
    }
}

/// Whether `c` is XML white space (XML 1.0 §2.3 `S`).
fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::Diagnostic;
    use crate::xmi::{Mode, ReadOptions, read};

    /// The schema errors of `metadata` under `xmi:XMI`, beside the root
    /// package `p`, read strictly: each as its template and its arguments'
    /// texts.
    fn errors(metadata: &str) -> Vec<(&'static str, Vec<String>)> {
        let input = format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">{metadata}\
             <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
        );
        let options = ReadOptions::default().with_mode(Mode::Strict);
        let output = read(input.as_bytes(), &options).unwrap_or_else(|error| panic!("{error:?}"));
        assert!(output.diagnostics().is_empty(), "{metadata}");
        crate::validation::schema_errors(output.document())
            .iter()
            .map(|found: &Diagnostic| {
                (
                    found.template(),
                    found
                        .arguments()
                        .iter()
                        .filter_map(|argument| argument.value().as_text().map(str::to_owned))
                        .collect(),
                )
            })
            .collect()
    }

    fn dangling(attribute: &str, element: &str, target: &str) -> (&'static str, Vec<String>) {
        (
            "SACM-REF-001/metadata-dangling",
            vec![attribute.to_owned(), element.to_owned(), target.to_owned()],
        )
    }

    /// Each IDREF and IDREFS attribute XMI.xsd gives an element, by its type,
    /// is checked once per distinct token; no other attribute is.
    #[test]
    fn the_idrefs_of_each_type_are_checked_and_nothing_else() {
        for (element, attribute) in [
            ("Difference", "target"),
            ("difference", "container"),
            ("Delete", "target"),
            ("Add", "addition"),
            ("Replace", "replacement"),
            ("Documentation", "xmi:idref"),
            ("extension", "xmi:idref"),
        ] {
            let found = errors(&format!("<xmi:{element} {attribute}=\"nope nope p\"/>"));
            assert_eq!(
                found,
                [dangling(attribute, &format!("xmi:{element}"), "nope")],
                "{element} {attribute}"
            );
        }
        // Not IDREFS on these types.
        for metadata in [
            "<xmi:Difference addition=\"nope\"/>",
            "<xmi:Add replacement=\"nope\"/>",
            "<xmi:Documentation target=\"nope\"/>",
            "<xmi:Extension target=\"nope\"/>",
        ] {
            assert_eq!(errors(metadata), [], "{metadata}");
        }
    }

    /// The children walked are those XMI.xsd types: an `xmi:Extension` in a
    /// Documentation or a Difference, an XMI element in an Extension, and a
    /// Difference's `difference` and `container`, never its `target`.
    #[test]
    fn only_the_children_xmi_xsd_types_are_walked() {
        for (metadata, found) in [
            (
                "<xmi:Documentation><xmi:Extension xmi:idref=\"nope\"/></xmi:Documentation>",
                vec![dangling("xmi:idref", "xmi:Extension", "nope")],
            ),
            (
                "<xmi:Documentation><v xmi:idref=\"nope\"/></xmi:Documentation>",
                vec![],
            ),
            (
                "<xmi:Extension><xmi:Difference target=\"nope\"/></xmi:Extension>",
                vec![dangling("target", "xmi:Difference", "nope")],
            ),
            (
                "<xmi:Extension><v target=\"nope\"/></xmi:Extension>",
                vec![],
            ),
            (
                "<xmi:Difference><difference target=\"nope\"/><container container=\"nope\"/>\
                 <target target=\"nope\"/></xmi:Difference>",
                vec![
                    dangling("container", "container", "nope"),
                    dangling("target", "difference", "nope"),
                ],
            ),
            (
                "<xmi:Difference><xmi:Extension xmi:idref=\"nope\"/></xmi:Difference>",
                vec![dangling("xmi:idref", "xmi:Extension", "nope")],
            ),
        ] {
            let mut found = found;
            found.sort();
            let mut got = errors(metadata);
            got.sort();
            assert_eq!(got, found, "{metadata}");
        }
    }

    /// Every `xmi:id` in metadata, at any depth, must be an `NCName`.
    #[test]
    fn every_metadata_id_must_be_an_ncname() {
        assert_eq!(
            errors("<xmi:Extension><v xmi:id=\"1y\"/></xmi:Extension>"),
            [(
                "SACM-ID-002/metadata-id-not-an-ncname",
                vec!["v".to_owned(), "1y".to_owned()]
            )]
        );
        assert_eq!(errors("<xmi:Extension xmi:id=\" ok \"/>"), []);
    }
}
