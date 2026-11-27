//! An element's ancestors and descendants (LIB-API-021; `lasrod/libSACM#113`
//! A3; slice 0082).
//!
//! - `SacmElement::ancestors` follows the parent links: nearest first, up to
//!   the root, never the element itself.
//! - `SacmElement::descendants` is every element the element contains, at
//!   any depth, in `Document::elements` pre-order, each with its depth below
//!   the element (a child is 1); never the element itself. `UtilityElement`s
//!   are included.
//! - The two agree with each other on every element of every valid fixture,
//!   walk a 10,000-deep chain without recursion, and outlive the
//!   `SacmElement` they came from: they borrow the document only.

use std::path::{Path, PathBuf};

use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
use sacm::edit::{CreateDescription, CreateNote, CreateTaggedValue, Operation};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadOptions};

const ROW: &str = "LIB-API-021";

/// The fixtures of SACM 2.3.
const SACM23: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23");

fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) {
    if let Err(error) = edit::apply(doc, operation) {
        panic!("{ROW}: setup: {what}: refused: {error}");
    }
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: setup: the document has no element {id:?}"))
}

fn id_of(found: SacmElement<'_>) -> &str {
    found.id().as_str()
}

/// The ids of `e.ancestors()`.
fn ancestor_ids(e: SacmElement<'_>) -> Vec<&str> {
    e.ancestors().map(|a| a.id().as_str()).collect()
}

/// The ids and depths of `e.descendants()`.
fn descendant_ids(e: SacmElement<'_>) -> Vec<(&str, usize)> {
    e.descendants()
        .map(|(d, depth)| (d.id().as_str(), depth))
        .collect()
}

/// The ancestors of the element `id`, returned past the `SacmElement`
/// they came from: they capture the document's lifetime only.
fn lineage<'d>(doc: &'d Document, id: &str) -> impl Iterator<Item = SacmElement<'d>> + use<'d> {
    element(doc, id).ancestors()
}

/// The descendants of the element `id`, as [`lineage`].
fn subtree<'d>(
    doc: &'d Document,
    id: &str,
) -> impl Iterator<Item = (SacmElement<'d>, usize)> + use<'d> {
    element(doc, id).descendants()
}

/// `case` > `p` > (`c1` > (`n1`, `t1`), `c2`), `p` > `d1`; and the second
/// root `q` > `cq`.
///
/// In pre-order: `case`, `p`, `d1`, `c1`, `n1`, `t1`, `c2`, `q`, `cq`.
fn tree() -> Document {
    let mut doc = Document::new();
    let name = LangString::new;
    let operations: Vec<Operation> = vec![
        CreateAssuranceCasePackage::root(name("Case"))
            .with_id("case")
            .into(),
        CreateArgumentPackage::new("case", name("P"))
            .with_id("p")
            .into(),
        CreateDescription::new("p", LangString::new("About P"))
            .with_id("d1")
            .into(),
        CreateClaim::new("p", name("C1")).with_id("c1").into(),
        CreateNote::new("c1", LangString::new("Reviewed"))
            .with_id("n1")
            .into(),
        CreateTaggedValue::new("c1", LangString::new("ASIL"), LangString::new("D"))
            .with_id("t1")
            .into(),
        CreateClaim::new("p", name("C2")).with_id("c2").into(),
        CreateArgumentPackage::root(name("Q")).with_id("q").into(),
        CreateClaim::new("q", name("CQ")).with_id("cq").into(),
    ];
    for operation in operations {
        apply("tree", &mut doc, operation);
    }
    doc
}

#[test]
fn lib_api_021_ancestors_are_nearest_first_up_to_the_root() {
    let doc = tree();
    assert_eq!(
        ancestor_ids(element(&doc, "t1")),
        ["c1", "p", "case"],
        "{ROW}: the tagged value's ancestors are not claim, package, case"
    );
    assert_eq!(
        ancestor_ids(element(&doc, "d1")),
        ["p", "case"],
        "{ROW}: the description's ancestors"
    );
    assert_eq!(
        ancestor_ids(element(&doc, "cq")),
        ["q"],
        "{ROW}: a child of the second root"
    );
    for root in ["case", "q"] {
        assert!(
            ancestor_ids(element(&doc, root)).is_empty(),
            "{ROW}: the root {root} has ancestors"
        );
    }
    for e in doc.elements() {
        let ancestors = ancestor_ids(e);
        assert!(
            !ancestors.contains(&id_of(e)),
            "{ROW}: {} is among its own ancestors: {ancestors:?}",
            id_of(e)
        );
        let parents: Vec<&str> = std::iter::successors(e.parent(), |p| p.parent())
            .map(id_of)
            .collect();
        assert_eq!(
            ancestors,
            parents,
            "{ROW}: the ancestors of {} are not its parent chain",
            id_of(e)
        );
    }
    // The nearest of a kind, as an application asks it.
    let package = element(&doc, "t1")
        .ancestors()
        .find(|a| a.kind() == ElementKind::ArgumentPackage);
    assert_eq!(
        package.map(id_of),
        Some("p"),
        "{ROW}: the nearest ArgumentPackage of t1"
    );
}

#[test]
fn lib_api_021_descendants_are_depth_first_in_document_order_with_their_depth() {
    let doc = tree();
    assert_eq!(
        descendant_ids(element(&doc, "case")),
        [
            ("p", 1),
            ("d1", 2),
            ("c1", 2),
            ("n1", 3),
            ("t1", 3),
            ("c2", 2)
        ],
        "{ROW}: the descendants of case"
    );
    // A root's subtree minus itself, in Document::elements() order.
    let pre_order: Vec<&str> = doc.elements().map(id_of).collect();
    assert_eq!(
        pre_order,
        ["case", "p", "d1", "c1", "n1", "t1", "c2", "q", "cq"],
        "{ROW}: setup: the pre-order"
    );
    let of_case: Vec<&str> = descendant_ids(element(&doc, "case"))
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        of_case,
        pre_order[1..7],
        "{ROW}: the descendants of case are not its subtree in pre-order"
    );
    assert_eq!(
        descendant_ids(element(&doc, "c1")),
        [("n1", 1), ("t1", 1)],
        "{ROW}: the UtilityElements of c1 are not its descendants at depth 1"
    );
    assert_eq!(
        descendant_ids(element(&doc, "q")),
        [("cq", 1)],
        "{ROW}: the descendants of the second root"
    );
    for leaf in ["d1", "n1", "t1", "c2", "cq"] {
        assert!(
            descendant_ids(element(&doc, leaf)).is_empty(),
            "{ROW}: the leaf {leaf} has descendants"
        );
    }
}

/// Every `*-valid*.xmi` file under `dir`, recursively, sorted by path.
fn valid_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = std::fs::read_dir(&next)
            .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()))
                .path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            if path.is_dir() {
                pending.push(path);
            } else if name.contains("-valid")
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xmi"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn lib_api_021_descendants_and_ancestors_agree_with_the_document() {
    let fixtures = valid_fixtures(Path::new(SACM23));
    assert!(
        fixtures.len() > 10,
        "{ROW}: setup: only {} valid fixtures under {SACM23}",
        fixtures.len()
    );
    let mut checked = 0_usize;
    for path in &fixtures {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
        let doc = match xmi::read(&bytes, &ReadOptions::default().with_mode(Mode::Tolerant)) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ROW}: setup: {} does not read: {error}", path.display()),
        };
        let all: Vec<SacmElement<'_>> = doc.elements().collect();
        for &e in &all {
            let expected: Vec<(&ElementId, usize)> = all
                .iter()
                .filter_map(|&d| {
                    d.ancestors()
                        .position(|a| a.id() == e.id())
                        .map(|at| (d.id(), at + 1))
                })
                .collect();
            let found: Vec<(&ElementId, usize)> =
                e.descendants().map(|(d, depth)| (d.id(), depth)).collect();
            assert_eq!(
                found,
                expected,
                "{ROW}: {}: the descendants of {} are not the elements it is an ancestor of, \
                 in pre-order, at the depth of its place among their ancestors",
                path.display(),
                e.id()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 100,
        "{ROW}: setup: only {checked} elements checked"
    );
}

#[test]
fn lib_api_021_a_ten_thousand_deep_chain_is_walked_without_recursion() {
    const DEPTH: usize = 10_000;
    let mut doc = Document::new();
    apply(
        "the root package",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P0")).with_id("p0"),
    );
    for level in 1..DEPTH {
        apply(
            "a nested package",
            &mut doc,
            CreateArgumentPackage::new(format!("p{}", level - 1), LangString::new("P"))
                .with_id(format!("p{level}")),
        );
    }
    let deepest = element(&doc, &format!("p{}", DEPTH - 1));
    assert_eq!(
        deepest.ancestors().count(),
        DEPTH - 1,
        "{ROW}: the deepest package's ancestors"
    );
    assert_eq!(
        deepest.ancestors().last().map(id_of),
        Some("p0"),
        "{ROW}: the last ancestor is not the root"
    );
    let root = element(&doc, "p0");
    let mut count = 0_usize;
    let mut last = None;
    for (index, (d, depth)) in root.descendants().enumerate() {
        assert_eq!(
            depth,
            index + 1,
            "{ROW}: descendant {index} of the chain is at depth {depth}"
        );
        count = index + 1;
        last = Some((id_of(d).to_owned(), depth));
    }
    assert_eq!(count, DEPTH - 1, "{ROW}: the root's descendants");
    assert_eq!(
        last,
        Some((format!("p{}", DEPTH - 1), DEPTH - 1)),
        "{ROW}: the last descendant is not the deepest package at depth 9,999"
    );
}

#[test]
fn lib_api_021_the_views_outlive_the_element_they_came_from() {
    let doc = tree();
    // Each iterator is made in a block that drops the element it came from;
    // it borrows only the document.
    let ancestors = {
        let e = element(&doc, "t1");
        e.ancestors()
    };
    let descendants = {
        let e = element(&doc, "p");
        e.descendants()
    };
    let ancestors: Vec<&str> = ancestors.map(id_of).collect();
    let descendants: Vec<&str> = descendants.map(|(d, _)| id_of(d)).collect();
    assert_eq!(
        ancestors,
        ["c1", "p", "case"],
        "{ROW}: the ancestors collected later"
    );
    assert_eq!(
        descendants,
        ["d1", "c1", "n1", "t1", "c2"],
        "{ROW}: the descendants collected later"
    );
    // They outlive a helper that returns them, capturing only the document.
    assert_eq!(
        lineage(&doc, "n1").count(),
        3,
        "{ROW}: the ancestors returned from a function"
    );
    let id = String::from("c1");
    let below = subtree(&doc, &id);
    drop(id);
    assert_eq!(
        below.count(),
        2,
        "{ROW}: the descendants returned from a function"
    );
}
