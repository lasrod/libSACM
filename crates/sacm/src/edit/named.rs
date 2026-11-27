//! The checks that no edit removes an `xmi:id` that is still named, and that
//! no copy brings in XMI metadata naming an id the target lacks
//! (LIB-EDIT-026; XML Schema Part 1 §3.3.4 clause 1).
//!
//! - **After the operation's own checks,** before LIB-EDIT-025's text check.
//! - **Typed references first** ([`EditError::PartReference`]): by holder in
//!   pre-order, then by end in [`ReferenceEnd::ALL`] order, then by position.
//! - **Then XMI metadata** ([`EditError::MetadataReference`]): the
//!   document's, then each remaining element's in pre-order, with the tokens
//!   a strict write checks (SACM23-XMI-019), through the same walk.
//! - **Only when an id is freed.** A delete always frees some; a text set
//!   only when it drops a value or the text.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use super::EditError;
use super::plan::{Copied, Plan, Removal, Value};
use crate::model::{
    CapturedElement, Document, ElementId, Preserved, ReferenceEnd, SacmElement, Text, TextRole,
};
use crate::validation::{Found, tokens, walk_document, walk_fragments};

/// Refuses `plan` if it removes an `xmi:id` that a remaining reference end
/// or XMI metadata names, or, for a copy, brings in metadata naming no id of
/// the target.
pub(super) fn check(document: &Document, plan: &Plan<'_>) -> Result<(), EditError> {
    match plan {
        Plan::Delete(removal) => check_delete(document, removal),
        Plan::Set {
            element,
            value: Value::Text(role, text),
        } => check_text(document, element, *role, text.as_ref()),
        Plan::Copy(copied) => check_copy(document, copied),
        _ => Ok(()),
    }
}

/// Delete checks 5 and 6: no remaining reference end names the `xmi:id` of a
/// deleted name or text, and no XMI metadata outside the deleted elements
/// names an id they held. A reference to a deleted element, or to the id of
/// its preserved content or metadata, is the delete policies' (check 3).
fn check_delete(document: &Document, removal: &Removal) -> Result<(), EditError> {
    let deleted: BTreeSet<&ElementId> = removal.deleted.iter().collect();
    let elements: Vec<SacmElement<'_>> = removal
        .deleted
        .iter()
        .filter_map(|id| document.get(id))
        .collect();
    let parts: BTreeSet<&str> = elements
        .iter()
        .flat_map(|element| part_ids(*element))
        .map(ElementId::as_str)
        .collect();
    first_referrer(document, &parts, |holder| !deleted.contains(holder))?;
    let freed: BTreeSet<&str> = elements
        .iter()
        .flat_map(|&element| {
            std::iter::once(element.id())
                .chain(part_ids(element))
                .chain(element.preserved().into_iter().flat_map(|held| &held.ids))
                .chain(element.metadata_parts().flat_map(|held| &held.ids))
        })
        .map(ElementId::as_str)
        .collect();
    first_metadata(document, &freed, |element| {
        (!deleted.contains(element.id())).then(|| element.metadata_parts().collect())
    })
}

/// The text-set check: `SetContent`, `SetKey`, `SetLocation` and
/// `SetTaggedValue` free the ids of the values beyond the new length, and of
/// their metadata, and a removal frees all of the text's.
fn check_text(
    document: &Document,
    element: &ElementId,
    role: TextRole,
    new: Option<&Text>,
) -> Result<(), EditError> {
    let Some(held) = document.get(element).and_then(|found| found.text(role)) else {
        return Ok(());
    };
    let kept: BTreeSet<&str> = new.into_iter().flat_map(text_ids).collect();
    let freed: BTreeSet<&str> = text_ids(held).filter(|id| !kept.contains(id)).collect();
    if freed.is_empty() {
        return Ok(());
    }
    first_referrer(document, &freed, |_| true)?;
    first_metadata(document, &freed, |found| {
        if found.id() != element {
            return Some(found.metadata_parts().collect());
        }
        // The edited element's metadata as the set leaves it: the metadata
        // of removed values goes with them.
        let texts = TextRole::ALL.into_iter().flat_map(|other| {
            let text = if other == role {
                new
            } else {
                found.text(other)
            };
            text.into_iter().flat_map(Text::metadata)
        });
        Some(
            found
                .metadata()
                .into_iter()
                .chain(found.name_metadata())
                .chain(texts)
                .collect(),
        )
    })
}

/// Copy check 7: each copied element, in pre-order, each `IDREF` token of its
/// XMI metadata names an `xmi:id` of the target after the copy.
fn check_copy(document: &Document, copied: &Copied<'_>) -> Result<(), EditError> {
    let elements = || copied.snapshot.elements();
    let brought: BTreeSet<&str> = copied
        .ids
        .iter()
        .chain(&copied.kept)
        .chain(elements().flat_map(CapturedElement::preserved_ids))
        .chain(elements().flat_map(CapturedElement::metadata_ids))
        .map(ElementId::as_str)
        .collect();
    let names = |token: &str| document.is_id_in_use(token) || brought.contains(token);
    for (element, id) in elements().zip(&copied.ids) {
        for metadata in element.metadata() {
            let found = walk_fragments(metadata, &mut |item| match item {
                Found::Reference { carrier, attribute } => {
                    match tokens(&attribute.value).find(|token| !names(token)) {
                        Some(token) => ControlFlow::Break(EditError::MetadataReference {
                            holder: Some(id.clone()),
                            element: carrier.written(),
                            attribute: attribute.name.written(),
                            id: ElementId::new(token),
                        }),
                        None => ControlFlow::Continue(()),
                    }
                }
                Found::Id { .. } => ControlFlow::Continue(()),
            });
            if let ControlFlow::Break(error) = found {
                return Err(error);
            }
        }
    }
    Ok(())
}

/// The ids of the name, `content`, `key` and `location` of `element`, and of
/// their values.
fn part_ids(element: SacmElement<'_>) -> impl Iterator<Item = &ElementId> {
    element.name_id().into_iter().chain(
        TextRole::ALL
            .into_iter()
            .filter_map(move |role| element.text(role))
            .flat_map(Text::ids),
    )
}

/// The ids of `text`, its values and its metadata.
fn text_ids(text: &Text) -> impl Iterator<Item = &str> {
    text.ids()
        .chain(text.metadata().flat_map(|metadata| &metadata.ids))
        .map(ElementId::as_str)
}

/// Refuses as [`EditError::PartReference`] the first reference end, of a
/// holder `keeps`, that names an id of `freed`: by holder in pre-order, then
/// by end in [`ReferenceEnd::ALL`] order, then by position.
fn first_referrer(
    document: &Document,
    freed: &BTreeSet<&str>,
    keeps: impl Fn(&ElementId) -> bool,
) -> Result<(), EditError> {
    let referrers: Vec<_> = freed
        .iter()
        .flat_map(|id| {
            document
                .referrers(id)
                .into_iter()
                .map(move |referrer| (referrer, *id))
        })
        .filter(|(referrer, _)| keeps(referrer.holder().id()))
        .collect();
    if referrers.is_empty() {
        return Ok(());
    }
    let order: BTreeMap<&ElementId, usize> = document
        .elements()
        .enumerate()
        .map(|(index, element)| (element.id(), index))
        .collect();
    let rank = |end: ReferenceEnd| ReferenceEnd::ALL.iter().position(|other| *other == end);
    let first = referrers.into_iter().min_by_key(|(referrer, _)| {
        (
            order.get(referrer.holder().id()).copied(),
            rank(referrer.end()),
            referrer.position(),
        )
    });
    match first {
        Some((referrer, id)) => Err(EditError::PartReference {
            holder: referrer.holder().id().clone(),
            end: referrer.end().name(),
            id: ElementId::new(id),
        }),
        None => Ok(()),
    }
}

/// Refuses as [`EditError::MetadataReference`] the first token of XMI
/// metadata that names an id of `freed`: the document's own metadata first,
/// then each element's in pre-order, as `metadata_of` gives it (`None`
/// skips the element), in event, attribute and token order.
fn first_metadata<'d>(
    document: &'d Document,
    freed: &BTreeSet<&str>,
    metadata_of: impl Fn(SacmElement<'d>) -> Option<Vec<&'d Preserved>>,
) -> Result<(), EditError> {
    let named = |holder: Option<&ElementId>, item: Found<'_>| match item {
        Found::Reference { carrier, attribute } => {
            match tokens(&attribute.value).find(|token| freed.contains(token)) {
                Some(token) => ControlFlow::Break(EditError::MetadataReference {
                    holder: holder.cloned(),
                    element: carrier.written(),
                    attribute: attribute.name.written(),
                    id: ElementId::new(token),
                }),
                None => ControlFlow::Continue(()),
            }
        }
        Found::Id { .. } => ControlFlow::Continue(()),
    };
    if let Some(metadata) = document.document_metadata() {
        if let ControlFlow::Break(error) = walk_document(metadata, &mut |item| named(None, item)) {
            return Err(error);
        }
    }
    for element in document.elements() {
        let Some(parts) = metadata_of(element) else {
            continue;
        };
        for metadata in parts {
            let walked = walk_fragments(metadata, &mut |item| named(Some(element.id()), item));
            if let ControlFlow::Break(error) = walked {
                return Err(error);
            }
        }
    }
    Ok(())
}
