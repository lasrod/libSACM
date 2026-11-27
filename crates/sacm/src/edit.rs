//! Change a document: an operation is a plain value, applied with [`apply`].
//!
//! # Entry points
//!
//! | Function | What it does |
//! |---|---|
//! | [`apply`] | Applies one operation and returns its [`Mutation`], or refuses with an [`EditError`]. |
//! | [`check`] | Says whether `apply` would accept an operation, without changing the document. |
//! | [`preview`] | Also gives the `Mutation` that `apply` would return, as a [`Preview`]. |
//! | [`apply_preview`] | Applies a `Preview`, unless the document has changed since (SACM-CMD-003). |
//! | [`batch`], [`apply_all`] | Apply several operations as one change: a closure's, or a list. |
//! | [`preview_batch`], [`preview_all`] | Give the [`BatchMutation`] a batch would return. |
//!
//! The operations are listed below: those that [create](#operations-that-create-an-element)
//! an element, those that [change](#operations-that-change-an-element) one,
//! [`DeleteElement`], [`MoveElement`] and [`CopyElements`].
//!
//! # Rules
//!
//! - **Atomic.** `apply`, `apply_preview`, `batch` and `apply_all` are each all or
//!   nothing (D-ATOMIC). A refused operation changes nothing: not the elements, the
//!   written XMI, the ids generated next or the document's [`revision`](Document::revision).
//! - **Batches.** A later operation can use an id an earlier one generated. On a
//!   refusal passed on, an error or a panic, the document is as it was.
//! - **What changed.** A [`Mutation`] holds the operation and the ids it created,
//!   changed and deleted.
//! - **No dangling reference.** Every id an edit newly references must name an element
//!   of the document, of a kind its end admits.
//! - **Content is not refused.** An empty name or a duplicate gid is accepted;
//!   [`validation::validate`](crate::validation::validate) reports it (D-VAL-LAYER).
//! - **Refusals are ordered.** The first check that fails is the result.
//!   [`apply`](apply#errors) lists the checks of each operation, in order.
//!
//! # Operations that create an element
//!
//! | Operation | Creates |
//! |---|---|
//! | [`CreateAssuranceCasePackage`] | an `AssuranceCasePackage` (§9.2) |
//! | [`CreateAssuranceCasePackageInterface`] | an `AssuranceCasePackageInterface` (§9.3), with its `implements` |
//! | [`CreateAssuranceCasePackageBinding`] | an `AssuranceCasePackageBinding` (§9.4), with its participants (`participantPackage`) |
//! | [`CreateArgumentPackage`] | an `ArgumentPackage` (§11.4) |
//! | [`CreateArgumentPackageInterface`] | an `ArgumentPackageInterface` (§11.6), with its `implements` |
//! | [`CreateArgumentPackageBinding`] | an `ArgumentPackageBinding` (§11.5), with its participants (`participantPackage`) |
//! | [`CreateClaim`] | a `Claim` (§11.11) |
//! | [`CreateArgumentReasoning`] | an `ArgumentReasoning` (§11.12), with an optional `structure` |
//! | [`CreateAssertedInference`] | an `AssertedInference` (§11.14), with its sources and target, and an optional `reasoning` and `isCounter` |
//! | [`CreateAssertedContext`] | an `AssertedContext` (§11.16), with its sources and target, and an optional `reasoning` and `isCounter` |
//! | [`CreateArtifactReference`] | an `ArtifactReference` (§11.9), with its optional `referencedArtifactElement` ids |
//! | [`CreateArgumentGroup`] | an `ArgumentGroup` (§11.2), with its optional members (`argumentElement`) |
//! | [`CreateAssertedEvidence`] | an `AssertedEvidence` (§11.15), with its sources and target, and an optional `reasoning` and `isCounter` |
//! | [`CreateAssertedArtifactSupport`] | an `AssertedArtifactSupport` (§11.17), with its sources and target, and an optional `reasoning` and `isCounter` |
//! | [`CreateAssertedArtifactContext`] | an `AssertedArtifactContext` (§11.18), with its sources and target, and an optional `reasoning` and `isCounter` |
//! | [`CreateDescription`] | a `Description` (§8.9) |
//! | [`CreateImplementationConstraint`] | an `ImplementationConstraint` (§8.8) |
//! | [`CreateNote`] | a `Note` (§8.11) |
//! | [`CreateTaggedValue`] | a `TaggedValue` (§8.12) |
//! | [`CreateArtifactPackage`] | an `ArtifactPackage` (§12.2) |
//! | [`CreateArtifactPackageInterface`] | an `ArtifactPackageInterface` (§12.5), with its `implements` |
//! | [`CreateArtifactPackageBinding`] | an `ArtifactPackageBinding` (§12.4), with its participants (`participantPackage`) |
//! | [`CreateProperty`] | a `Property` (§12.8) |
//! | [`CreateTechnique`] | a `Technique` (§12.12) |
//! | [`CreateParticipant`] | a `Participant` (§12.13) |
//! | [`CreateArtifactGroup`] | an `ArtifactGroup` (§12.3), with its optional members (`artifactElement`) |
//! | [`CreateArtifact`] | an `Artifact` (§12.7), with an optional `version` and `date` |
//! | [`CreateEvent`] | an `Event` (§12.9), with an optional `date` |
//! | [`CreateResource`] | a `Resource` (§12.10), with an optional `location` |
//! | [`CreateActivity`] | an `Activity` (§12.11), with an optional `startTime` and `endTime` |
//! | [`CreateArtifactAssetRelationship`] | an `ArtifactAssetRelationship` (§12.14), with its sources and targets |
//! | [`CreateTerminologyPackage`] | a `TerminologyPackage` (§10.4) |
//! | [`CreateTerminologyPackageInterface`] | a `TerminologyPackageInterface` (§10.5), with its `implements` |
//! | [`CreateTerminologyPackageBinding`] | a `TerminologyPackageBinding` (§10.6), with its participants (`participantPackage`) |
//! | [`CreateCategory`] | a `Category` (§10.8), with its optional `category` ids |
//! | [`CreateTerm`] | a `Term` (§10.11), with its `value` and its optional categories, `origin` and `externalReference` |
//! | [`CreateExpression`] | an `Expression` (§10.10), with its `value` and its optional categories and `element` ids |
//! | [`CreateTerminologyGroup`] | a `TerminologyGroup` (§10.3), with its optional members (`terminologyElement`) |
//!
//! Where the new element goes:
//!
//! - **Root or child.** `new` puts the element under a parent. Only a package, an
//!   interface or a binding can be a root, so only their creates also have `root`.
//! - **Position.** A root is added after the document's last root; a child after its
//!   parent's last child.
//! - **Legal parents.** A parent is legal exactly when SACM lets its class contain the
//!   new kind, the rules [`xmi::read`](crate::xmi::read) uses. A Claim goes in an
//!   ArgumentPackage, not directly in an AssuranceCasePackage.
//! - **Interfaces and bindings.** A package's interface or binding is a legal parent
//!   wherever its package is: it holds its package's roles.
//!
//! What it takes:
//!
//! - **The name is required.** SACM gives every `ModelElement` exactly one name (§8.6
//!   `name:LangString[1]`). It is a [`LangString`](crate::model::LangString), stored as
//!   given, or plain text: `"C1"` and `LangString::new("C1")` give the same name.
//! - **Any plain name is accepted,** `LangString::default()` included: an empty name is
//!   still a name. An `ExpressionLangString` name must have an `expression` that names
//!   a `Term` or an `Expression` (checks 5a to 8 of [Creates](apply#creates)).
//! - **Every text must be savable.** A name, value or other text holding a character
//!   XML 1.0 cannot represent is refused, last (check 9 of [Creates](apply#creates)).
//! - **`UtilityElement`s have no name.** A description, implementation constraint, note
//!   or tagged value goes in any `ModelElement`, in the role of its kind. It contains no
//!   elements, so it is never a parent.
//! - **`content` and `key`.** Each is a [`MultiLangString`](crate::model::MultiLangString),
//!   stored exactly as given; a `LangString` converts into one with one value.
//!
//! What it references (ADR 0027, ADR 0028):
//!
//! - **An asserted relationship** takes its sources (at least one, in order, repeats
//!   kept) and its target when it is created (§11.13 `source:ArgumentAsset[1..*]`,
//!   `target:ArgumentAsset[1]`).
//! - **An `ArtifactAssetRelationship`** takes its sources and targets, at least one each
//!   (§12.14 `source:ArtifactAsset[1..*]`, `target:ArtifactAsset[1..*]`).
//! - **Every referenced id names an element** of the document. The id of a name,
//!   `content`, `key`, `location` or `value` names no element.
//! - **Every referenced element has the type of its end** (D-VAL-LAYER); the table
//!   below gives it.
//! - **Narrower rules are content.** §11.15, §11.17 and §11.18 ask that the ends of an
//!   `AssertedEvidence`, `AssertedArtifactSupport` or `AssertedArtifactContext` be
//!   `ArtifactReference`s. A create does not refuse them; `validate` reports them
//!   (SACM23-VAL-016).
//! - **Set later.** The `metaClaim`, `citedElement` and `abstractForm` of an element are
//!   set after it is created, with [`SetReferences`].
//!
//! | End | Type of the referenced element |
//! |---|---|
//! | `source` or `target` of an asserted relationship | `ArgumentAsset` |
//! | `reasoning` | `ArgumentReasoning` |
//! | `structure` | `ArgumentPackage` |
//! | `referencedArtifactElement` | `ArtifactElement` (any element but a `UtilityElement`) |
//! | member of an `ArgumentGroup` | `ArgumentationElement` |
//! | member of an `ArtifactGroup` | `ArtifactElement` |
//! | `source` or `target` of an `ArtifactAssetRelationship` | `ArtifactAsset` |
//!
//! # Operations that change an element
//!
//! | Operation | Changes | Forms |
//! |---|---|---|
//! | [`SetName`] | the `name` of a `ModelElement` (§8.6) | [`new`](SetName::new) |
//! | [`SetGid`] | the `gid` of any element (§8.2) | [`new`](SetGid::new), [`remove`](SetGid::remove) |
//! | [`SetFlag`] | one [`Flag`](crate::model::Flag): `isCitation` or `isAbstract` of any element (§8.2), `isCounter` of an asserted relationship (§11.13) | [`new`](SetFlag::new) |
//! | [`SetAssertionDeclaration`] | the `assertionDeclaration` of an Assertion (§11.10) | [`new`](SetAssertionDeclaration::new) |
//! | [`SetContent`] | the `content` of a `UtilityElement` (§8.7) | [`new`](SetContent::new), [`remove`](SetContent::remove) |
//! | [`SetKey`] | the `key` of a `TaggedValue` (§8.12) | [`new`](SetKey::new) |
//! | [`SetTaggedValue`] | the `content` of the element's `TaggedValue` with a plain key (§8.12), which it creates if there is none | [`new`](SetTaggedValue::new) |
//! | [`SetVersion`] | the `version` of an `Artifact` (§12.7) | [`new`](SetVersion::new), [`remove`](SetVersion::remove) |
//! | [`SetDate`] | the `date` of an `Artifact` or an `Event` (§12.7, §12.9) | [`new`](SetDate::new), [`remove`](SetDate::remove) |
//! | [`SetLocation`] | the `location` of a `Resource` (§12.10) | [`new`](SetLocation::new), [`remove`](SetLocation::remove) |
//! | [`SetStartTime`] | the `startTime` of an `Activity` (§12.11) | [`new`](SetStartTime::new), [`remove`](SetStartTime::remove) |
//! | [`SetEndTime`] | the `endTime` of an `Activity` (§12.11) | [`new`](SetEndTime::new), [`remove`](SetEndTime::remove) |
//! | [`SetValue`] | the `value` of a `Term` or an `Expression` (§10.9) | [`new`](SetValue::new) |
//! | [`SetExternalReference`] | the `externalReference` of a `Term` (§10.11) | [`new`](SetExternalReference::new), [`remove`](SetExternalReference::remove) |
//! | [`SetReferences`] | the ids one [`ReferenceEnd`](crate::model::ReferenceEnd) holds, such as `source` or `metaClaim`, and its cross-document references ([`Href`]) | [`new`](SetReferences::new), [`clear`](SetReferences::clear), [`with_hrefs`](SetReferences::with_hrefs) |
//! | [`AddReference`] | one [`ReferenceEnd`](crate::model::ReferenceEnd), such as `source` or `metaClaim`, with one id or one href appended | [`new`](AddReference::new), [`new_href`](AddReference::new_href) |
//! | [`RemoveReference`] | one [`ReferenceEnd`](crate::model::ReferenceEnd), such as `source` or `metaClaim`, with every occurrence of one id or of one href removed | [`new`](RemoveReference::new), [`new_href`](RemoveReference::new_href) |
//! | [`ReorderElements`] | the order of an element's children, or of the roots | [`new`](ReorderElements::new), [`roots`](ReorderElements::roots) |
//!
//! - **One feature, by id.** Each names the element it changes by id, and replaces one
//!   feature exactly as given (ADR 0029).
//! - **A value is never refused for its content** (D-DATE, D-VAL-LAYER): a name, gid,
//!   flag, declaration, `content`, key, `version`, `date`, `location`, `startTime` or
//!   `endTime`. Only a character XML 1.0 cannot represent is refused, last (LIB-EDIT-025).
//! - **Content is for `validate`,** which reports an empty name or `content`, a duplicate
//!   gid, or an `asCited` declaration on an element that cites nothing.
//! - **A flag and its reference are independent,** such as `isCitation` and `citedElement`.
//! - **A reference end stays within its bounds:** `source` `[1..*]`, an asserted
//!   relationship's `target` `[1]`, an `ArtifactAssetRelationship`'s `target` `[1..*]`,
//!   the other single-valued ends `[0..1]`.
//! - **Only new ids are checked.** Every id an end newly holds names an element of a
//!   kind it admits. An id it already holds is kept, even a dangling or wrongly typed
//!   one, so an end read from a file can be repaired one id at a time.
//! - **Ids are kept by position.** A replaced name keeps its `xmi:id`. A replaced
//!   `content`, `key` or `location` keeps the id of its element and, for each value, the
//!   id of the value at its position.
//! - **Freed ids.** The ids beyond the new length, every id of a removed `content`, and
//!   an id no reference holds any more are freed, so a later create may generate them.
//! - **Setting the value held changes nothing.** The document is not written, and
//!   [`Mutation::is_empty`] is true ([`BatchMutation::is_empty`] for a batch).
//! - **Otherwise [`Mutation::changed_ids`] is the element,** or for
//!   [`ReorderElements::new`] the parent.
//! - **A root reorder lists no id.** The order of the roots belongs to no element, so
//!   its mutation is empty although the document changed; [`Mutation::is_empty`] says
//!   how to tell.
//!
//! # Deleting an element
//!
//! [`DeleteElement`] deletes an element of any kind, everything it contains, and what
//! its three policies allow. It never leaves a reference dangling (LIB-EDIT-005,
//! LIB-EDIT-006, ADR 0031). Each policy refuses by default, so nothing cascades unless
//! asked (D-POLICY):
//!
//! | Policy | Refuses (default) | Allows | What it does |
//! |---|---|---|---|
//! | [`ReferenceDeletePolicy`]: a reference to a deleted element, held outside its subtree | [`RejectIfReferenced`](ReferenceDeletePolicy::RejectIfReferenced) | [`DeleteReferencingRelationships`](ReferenceDeletePolicy::DeleteReferencingRelationships) | deletes each asserted relationship whose `source`, `target` or `reasoning`, and each `ArtifactAssetRelationship` whose `source` or `target`, names a deleted element |
//! | | | [`ScrubReferences`](ReferenceDeletePolicy::ScrubReferences) | removes the id; a relationship of either class left with no `source` or no `target` is deleted |
//! | [`PackageDeletePolicy`]: an element that contains elements other than `UtilityElement`s | [`RejectIfNonEmpty`](PackageDeletePolicy::RejectIfNonEmpty) | [`DeleteRecursively`](PackageDeletePolicy::DeleteRecursively) | deletes the element and everything it contains |
//! | [`CrossPackageReferencePolicy`]: a reference like that, whose holder's nearest package is not that of the element it names | [`RejectIfExternalReferencesExist`](CrossPackageReferencePolicy::RejectIfExternalReferencesExist) | [`ApplyReferencePolicy`](CrossPackageReferencePolicy::ApplyReferencePolicy) | leaves the reference to the reference policy |
//!
//! - **Ask first.** [`preview`] lists the references to deleted elements, which the
//!   delete removes, as [`DanglingReference`]s.
//! - **The counter is not rewound.** The next generated id is still the smallest unused
//!   number above the per-kind counter, so generation stays deterministic.
//! - **A freed id may be used again.** A caller may give it with `with_id`, and a freed
//!   id the caller gave may be generated later. An application that needs ids unique
//!   across its history gives its own.
//!
//! # Moving and copying elements
//!
//! | Operation | What it does |
//! |---|---|
//! | [`MoveElement`] | Moves an element, with everything it contains and its preserved content, under another parent or to the roots. |
//! | [`CopyElements`] | Copies elements of a source document, each with everything it contains, under a parent or as roots. |
//!
//! The containment table decides the legal parents, as for a create, and a root must be
//! a package (LIB-EDIT-017, LIB-EDIT-018, ADR 0036).
//!
//! A move:
//!
//! - **Keeps** every id, reference and feature, and the ids generated next.
//! - **Changes** the element's parent, its role and the order. A preserved child of
//!   either parent keeps its place after the typed child it followed.
//! - **Reports what `semantic_compare` reports** in [`Mutation::changed_ids`]: the
//!   element when its parent changes, and a parent whose child order or preserved
//!   placement changes, in pre-order after.
//! - **Changes nothing** when it moves the element to the place it holds.
//!
//! A copy:
//!
//! - **Captures its source elements when it is built.** An id the source lacks is
//!   [`EditError::ElementNotFound`] from [`CopyElements::new`] or [`CopyElements::root`].
//! - **Keeps an id** that is an `NCName` free in the target. Otherwise the copy gets a
//!   generated id that avoids every id the copy keeps or names.
//! - **Rewrites references to copied elements,** which name the copy. Every other
//!   reference is kept as written, so it must resolve in the target.
//! - **Reports** [`Mutation::created_ids`], which pair with
//!   [`CopyElements::source_ids`]. It changes and deletes nothing.
//! - **Copies nothing** for an empty list, once the parent is found.
//!
//! [`apply`](apply#moves) lists the checks of a move, and [`apply`](apply#copies)
//! those of a copy, in order.
//!
//! # Undo and redo
//!
//! Undo and redo are the application's, on whole-document copies (ADR 0037,
//! LIB-EDIT-011). The crate has no history type and no inverse operations.
//!
//! - **The rule.** Clone the document, make exactly one atomic call ([`apply`],
//!   [`apply_preview`], [`batch`] or [`apply_all`]), and push the clone only on `Ok`.
//! - **One step is one call.** Several `apply` calls are not atomic; use [`batch`]. One
//!   batch, or an accepted draft ([`CopyElements`] in a batch), is one step.
//! - **A restored copy is the document before the change.** It is `==` to it and writes
//!   the same XMI, preserved content included.
//! - **Ids may repeat.** The copy has its revision and its generated-id counters, so the
//!   next create may generate the id an undone create had. An application that needs
//!   ids unique across its history gives its own.
//! - **Previews.** A preview taken before an undone change applies again after the undo.
//! - **Cost.** A step costs one clone, in time and memory proportional to the document:
//!   about 1.4 to 2.3 ms and 3.3 MiB at 5,500 elements in a release build. The
//!   application chooses the limit.
//!
//! ```
//! use std::collections::VecDeque;
//!
//! use sacm::edit::{self, BatchError, CreateArgumentPackage, CreateClaim, SetName};
//! use sacm::model::{Document, LangString};
//!
//! /// Undo and redo as an application keeps them: whole documents.
//! struct History {
//!     undo: VecDeque<Document>,
//!     redo: Vec<Document>,
//!     limit: usize,
//! }
//!
//! impl History {
//!     /// Records `before`, the document as it was before a change that
//!     /// succeeded. A limit of 0 records nothing.
//!     fn push(&mut self, before: Document) {
//!         if self.limit == 0 {
//!             return;
//!         }
//!         if self.undo.len() == self.limit {
//!             self.undo.pop_front();
//!         }
//!         self.undo.push_back(before);
//!         self.redo.clear();
//!     }
//!     fn undo(&mut self, doc: &mut Document) -> bool {
//!         let Some(before) = self.undo.pop_back() else {
//!             return false;
//!         };
//!         self.redo.push(std::mem::replace(doc, before));
//!         true
//!     }
//!     fn redo(&mut self, doc: &mut Document) -> bool {
//!         let Some(after) = self.redo.pop() else {
//!             return false;
//!         };
//!         self.undo.push_back(std::mem::replace(doc, after));
//!         true
//!     }
//! }
//!
//! let mut doc = Document::new();
//! edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
//! let mut history = History { undo: VecDeque::new(), redo: Vec::new(), limit: 100 };
//!
//! // Clone first, then one atomic call, then push the clone only on Ok.
//! let before = doc.clone();
//! edit::batch(&mut doc, |tx| {
//!     tx.apply(CreateClaim::new("p", LangString::new("The brake is safe")))?;
//!     tx.apply(SetName::new("p", LangString::new("Brake")))?;
//!     Ok::<(), BatchError>(())
//! })?;
//! let unchanged = before.clone(); // only to check the undo below
//! history.push(before);
//! let changed = doc.clone();
//!
//! assert!(history.undo(&mut doc));
//! assert!(doc == unchanged);
//! assert!(history.redo(&mut doc));
//! assert!(doc == changed);
//!
//! // A new change after an undo clears the redo stack.
//! assert!(history.undo(&mut doc));
//! let before = doc.clone();
//! edit::apply(&mut doc, SetName::new("p", LangString::new("Brakes")))?;
//! history.push(before);
//! assert!(!history.redo(&mut doc));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Ids
//!
//! - **Given by the caller** with `with_id`: it must be an XML `NCName` (an
//!   `xmi:id` is an `xsd:ID`, XMI 2.5.1 §7.6.1), such as `claim_top` or
//!   `_c.1`, and not already in the document.
//! - **Generated** otherwise: `<snake_case kind>_<n>`, such as
//!   `assurance_case_package_1`, `argument_package_1` or `claim_1`.
//! - **The number `n`** is the smallest above the document's counter for the kind whose
//!   id is not in use, given by a caller or read from a file, and that no reference
//!   names. So a reference that names no element never comes to name the new one by
//!   accident (ADR 0026).
//! - **The counter** advances only when an operation succeeds with a generated id; a
//!   caller id does not move it.
//! - **Deterministic:** the same operations, applied to documents built or read
//!   the same way, give the same ids; a clone generates the same ids as its
//!   original.
//!
//! Ids read from a file are kept exactly as read; only new ids are checked.
//!
//! # Refusals
//!
//! - **Checks and their order.** [`apply`](apply#errors) lists the checks of each
//!   operation in the order they run, each with its refusal and code.
//! - **Each refusal.** [`EditError`] says what a refusal means and how to fix it.
//! - **Previews.** [`apply_preview`] also refuses a preview taken at another revision
//!   of the document ([`EditError::PreviewExpired`], SACM-CMD-003).
//!
//! # Examples
//!
//! ```
//! use sacm::diagnostic::Code;
//! use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
//! use sacm::model::{Document, LangString};
//! use sacm::validation;
//! use sacm::xmi::{self, WriteOptions};
//!
//! let name = |text: &str| LangString::new(text).with_lang("en");
//! let mut doc = Document::new();
//!
//! edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Safety case")).with_id("case"))?;
//! // No with_id: the id is generated. Use it as the next parent.
//! let created = edit::apply(&mut doc, CreateArgumentPackage::new("case", name("Arguments")))?;
//! let arguments = created.created_ids()[0].clone();
//! assert_eq!(arguments.as_str(), "argument_package_1");
//! edit::apply(&mut doc, CreateClaim::new(&arguments, name("The system is safe")).with_id("c_top"))?;
//!
//! // A Claim cannot sit directly in an AssuranceCasePackage.
//! let refused = edit::apply(&mut doc, CreateClaim::new("case", name("Misplaced"))).unwrap_err();
//! assert_eq!(refused.code(), Code::CMD_INVALID_PARENT);
//! assert_eq!(doc.element_count(), 3);
//!
//! // Every element has a name, so validation finds nothing.
//! assert!(validation::validate(&doc).is_empty());
//! let text = xmi::write(&doc, &WriteOptions::default())?;
//! assert!(text.contains(r#"xmi:id="c_top""#));
//!
//! // And it reads back.
//! let back = xmi::read(text.as_bytes(), &xmi::ReadOptions::default())?.into_document();
//! let parent = back.get("c_top").and_then(|claim| claim.parent());
//! assert_eq!(parent.map(|parent| parent.id().as_str()), Some("argument_package_1"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod argumentation;
mod artifact;
mod batch;
mod copy;
mod create;
mod delete;
mod move_element;
mod plan;
mod preview;
#[cfg(feature = "serde")]
mod record;
mod references;
mod reorder;
mod set;
mod terminology;
mod utility;

use std::fmt;

pub use argumentation::{
    CreateArgumentGroup, CreateArgumentPackageBinding, CreateArgumentPackageInterface,
    CreateArgumentReasoning, CreateArtifactReference, CreateAssertedArtifactContext,
    CreateAssertedArtifactSupport, CreateAssertedContext, CreateAssertedEvidence,
    CreateAssertedInference,
};
pub use artifact::{
    CreateActivity, CreateArtifact, CreateArtifactAssetRelationship, CreateArtifactGroup,
    CreateArtifactPackage, CreateArtifactPackageBinding, CreateArtifactPackageInterface,
    CreateEvent, CreateParticipant, CreateProperty, CreateResource, CreateTechnique,
};
pub use batch::{Batch, BatchError, BatchMutation};
pub use copy::CopyElements;
pub use create::{
    CreateArgumentPackage, CreateAssuranceCasePackage, CreateAssuranceCasePackageBinding,
    CreateAssuranceCasePackageInterface, CreateClaim,
};
pub use delete::{
    CrossPackageReferencePolicy, DeleteElement, PackageDeletePolicy, ReferenceDeletePolicy,
};
pub use move_element::MoveElement;
pub use preview::{DanglingReference, Preview};
pub use references::{AddReference, RemoveReference, SetReferences};
pub use reorder::ReorderElements;
pub use set::{
    SetAssertionDeclaration, SetContent, SetDate, SetEndTime, SetExternalReference, SetFlag,
    SetGid, SetKey, SetLocation, SetName, SetStartTime, SetTaggedValue, SetValue, SetVersion,
};
pub use terminology::{
    CreateCategory, CreateExpression, CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage,
    CreateTerminologyPackageBinding, CreateTerminologyPackageInterface,
};
pub use utility::{
    CreateDescription, CreateImplementationConstraint, CreateNote, CreateTaggedValue,
};

use crate::diagnostic::Code;
use crate::model::{Document, ElementId, ElementKind, Href, Revision, StringAttribute, TextRole};
use plan::{Features, Outcome, Plan};

/// Applies `operation` to `document`, or refuses it and leaves `document` unchanged.
///
/// - **Takes** any operation struct, such as [`CreateClaim`], or an [`Operation`]. The
///   [module docs](self) list the operations and say how ids are generated.
/// - **On success** the [`Mutation`] holds the operation and the ids it created, changed
///   and deleted, and the document has a new [`revision`](Document::revision).
/// - **On refusal** the document is `==` to what it was (see [`compare`](crate::compare)),
///   generates the same next ids and keeps its revision.
/// - **Structurally valid.** The document stays so, as every `Document` is: ids are
///   unique, parent and child links agree, every root is a package, and every child is
///   in a composite role of its parent's class.
/// - **Assertions.** Every Assertion keeps an
///   [`AssertionDeclaration`](crate::model::AssertionDeclaration) (SACM 2.3 §11.10).
/// - **Content is not checked.** No content finding refuses an operation; call
///   [`validate`](crate::validation::validate) again after editing.
/// - **Ask first.** [`check`] gives the same answer without changing the document.
/// - **The only ways to change a [`Document`]** are `apply`, [`apply_preview`], [`batch`]
///   and [`apply_all`], outside the unstable `test-support` feature. Each is all or
///   nothing.
///
/// # Errors
///
/// An [`EditError`]. Every check runs before the one change, in the order below, and
/// the first that fails is the result.
///
/// ## Creates
///
/// A root operation has no parent, so it skips checks 1 and 2.
///
/// | Check | Refusal | Code | Fix |
/// |---|---|---|---|
/// | 1. the parent is in the document | [`EditError::ParentNotFound`] | SACM-CMD-001 | Create the parent first, or correct its id. Ids are compared exactly, case included. |
/// | 2. the parent can contain the new kind | [`EditError::InvalidParent`] | SACM-CMD-005 | Choose a parent whose class can hold the new kind. For example, a Claim goes in an ArgumentPackage, and an ArgumentPackage in an AssuranceCasePackage or another ArgumentPackage. |
/// | 3. a caller id is an `NCName` | [`EditError::InvalidId`] | SACM-ID-002 | An `NCName` starts with a letter or `_` and continues with letters, digits, `-`, `.` or `_`; it has no spaces and no `:`. Or leave out `with_id` to have an id generated. |
/// | 4. a caller id is not in use | [`EditError::DuplicateId`] | SACM-CMD-004 | Ids are unique across the whole document, not per parent. Choose another id, or leave out `with_id` to have one generated. |
/// | 5. a relationship has a source, and an `ArtifactAssetRelationship` a target: the sources first, then the targets; a binding has two distinct participants | [`EditError::TooFewReferences`] | SACM-MULT-001 | Give at least one source, and at least one target of an `ArtifactAssetRelationship`: SACM requires them (`source:ArgumentAsset[1..*]`, §12.14 `source:ArtifactAsset[1..*]` and `target:ArtifactAsset[1..*]`). Give a binding two different packages (§9.4, §10.6, §11.5, §12.4 `participantPackage[2..*]`; a repeated id counts once, ADR 0039 D3). |
/// | 5a. every `ExpressionLangString` of the name and texts has an `expression` | [`EditError::TooFewReferences`] of the end `expression` | SACM-MULT-001 | Give it an expression with [`LangString::with_expression`](crate::model::LangString::with_expression), or make it plain with [`without_expression`](crate::model::LangString::without_expression) (§8.4 `expression[1]`; SACM23-BASE-009). |
/// | 6. no referenced id holds XML white space (space, TAB, LF, CR): each source in order, then the target or targets, the `reasoning`, the `structure`, the `referencedArtifactElement` ids, the group members (`argumentElement` or `artifactElement`), an interface's `implements` and a binding's participants (`participantPackage`) | [`EditError::InvalidReference`] | SACM-ID-002 | An id such as `a b`, read from a file as written, is not an `NCName` and would be written as two ids. Give the referenced element an `NCName` id in the file. |
/// | 7. every referenced id names an element, in the same order | [`EditError::ReferenceNotFound`] | SACM-CMD-001 | Create the referenced element first, or correct its id. Ids are compared exactly, case included. |
/// | 8. every referenced id names an element of a kind its end admits, in the same order | [`EditError::WrongReferenceType`] | SACM-REF-002 | Reference an element of the end's type: for example, an `ArgumentPackage` as a `structure`, and an `ArgumentAsset`, not a package, as a source. |
/// | 9. every text the new element would hold has only characters XML 1.0 can represent | [`EditError::UnrepresentableCharacter`] | SACM-XML-003 | Remove the character. |
///
/// In checks 6 to 8, the `expression`s of the name's and texts' `ExpressionLangStrings`
/// come last, as the end `expression`, which must name a `Term` or an `Expression`
/// (SACM23-BASE-009).
///
/// ## Operations that change an element
///
/// Each operation first checks:
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 1. the element (for a reorder, the parent) is in the document | [`EditError::ElementNotFound`] | SACM-CMD-001 |
/// | 2. its kind has the feature: a `ModelElement` for a name, an `AssertedRelationship` for `isCounter`, an Assertion for a declaration, a `UtilityElement` for a `content`, a `TaggedValue` for a key, a kind that holds tagged values for `taggedValue`, an `Artifact` for a `version`, an `Artifact` or an `Event` for a `date`, a `Resource` for a `location`, an `Activity` for a `startTime` or `endTime`, the end's class for a reference end | [`EditError::FeatureNotApplicable`] | SACM-CMD-008 |
///
/// What each checks after that:
///
/// - **Nothing more:** [`SetFlag`] and [`SetAssertionDeclaration`]. Every element has a
///   gid, `isCitation` and `isAbstract`, so [`SetGid`] and those flags skip check 2 too.
/// - **The `ExpressionLangString`s they set** (SACM23-BASE-009): [`SetName`],
///   [`SetContent`], [`SetKey`], [`SetLocation`] and [`SetTaggedValue`], for its content.
///   - One with no `expression` is [`EditError::TooFewReferences`] of the end `expression`.
///   - Each `expression` the element does not hold already is checked as checks 6 to 8
///     below check a new id, for the end `expression`.
/// - **Last, the text they store** (LIB-EDIT-025). A character XML 1.0 cannot represent
///   is [`EditError::UnrepresentableCharacter`] (SACM-XML-003). A set of the value held
///   stores nothing, so it is not checked. The operations:
///   - [`SetName`], [`SetGid::new`], [`SetContent::new`], [`SetKey`] and [`SetTaggedValue`];
///   - [`SetVersion::new`], [`SetDate::new`], [`SetLocation::new`], [`SetStartTime::new`],
///     [`SetEndTime::new`], [`SetValue`] and [`SetExternalReference::new`].
///
/// The reference operations ([`SetReferences`], [`AddReference`], [`RemoveReference`])
/// then check, in this order:
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 2a. the end is not the derived `expression` ([`ReferenceEnd::Expression`](crate::model::ReferenceEnd::Expression)), which the name and texts hold | [`EditError::DerivedEnd`] | SACM-CMD-008 |
/// | 3. for [`RemoveReference`] only: the end holds the id, or, for [`RemoveReference::new_href`], the href | [`EditError::ReferenceNotHeld`], [`EditError::HrefNotHeld`] | SACM-CMD-001 |
/// | 4. the end would hold at least the distinct values it needs, ids plus hrefs: one `source` and one `target`, of either relationship class, an interface's `implements`, and two distinct participants of a binding (`participantPackage`; ADR 0039 D3) | [`EditError::TooFewReferences`] | SACM-MULT-001 |
/// | 5. the end would hold at most the values it allows, ids plus hrefs: one for a single-valued end | [`EditError::TooManyReferences`] | SACM-MULT-001 |
/// | 6. no new id holds XML white space | [`EditError::InvalidReference`] | SACM-ID-002 |
/// | 7. every new id names an element | [`EditError::ReferenceNotFound`] | SACM-CMD-001 |
/// | 8. every new id names an element of a kind the end admits | [`EditError::WrongReferenceType`] | SACM-REF-002 |
///
/// - **A new id** is one the end does not hold already. Checks 6 to 8 each run over the
///   new ids in list order, as the creates' checks 6 to 8 do.
/// - **An href is never checked.** It names an element of another document, which no
///   edit opens (LIB-EDIT-020; ADR 0040 D6, amendment 1).
///
/// [`ReorderElements`] checks, after the parent (check 1; [`ReorderElements::roots`] has
/// no parent, so it skips check 1):
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 2. the list names at least one element | [`EditError::InvalidOrder`] | SACM-CMD-009 |
/// | 3. id by id, in list order: it is a direct child of the parent (or a root) | [`EditError::ChildNotFound`] | SACM-CMD-001 |
/// | 4. for the same id, right after check 3: it was not named before | [`EditError::InvalidOrder`] | SACM-CMD-009 |
///
/// ## Deletes
///
/// [`DeleteElement`] checks, in this order:
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 1. the element is in the document | [`EditError::ElementNotFound`] | SACM-CMD-001 |
/// | 2. under `RejectIfNonEmpty`: it contains no element but `UtilityElement`s, and no preserved child element (ADR 0033) | [`EditError::NotEmpty`] | SACM-CMD-006 |
/// | 3. each dangling reference, in the order of [`Preview::dangling_references`]: one that crosses a package boundary, under `RejectIfExternalReferencesExist` | [`EditError::CrossPackageReference`] | SACM-CMD-007 |
/// | 3. the same reference, otherwise, under `RejectIfReferenced` | [`EditError::Referenced`] | SACM-CMD-002 |
/// | 4. under `RejectIfNonEmpty`: the first interface or binding in pre-order that the delete removes for its bound (left with no `implements`, or fewer than two distinct participants) and that holds an element other than a `UtilityElement` or a preserved child element (LIB-EDIT-019) | [`EditError::NotEmpty`] naming it | SACM-CMD-006 |
///
/// ## Moves
///
/// [`MoveElement`] checks, in this order:
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 1. the element is in the document | [`EditError::ElementNotFound`] | SACM-CMD-001 |
/// | 2. under a parent: the parent is in the document | [`EditError::ParentNotFound`] | SACM-CMD-001 |
/// | 3. the parent's class can hold the element's kind; for the roots, the element is a package | [`EditError::InvalidParent`], [`EditError::InvalidRoot`] | SACM-CMD-005 |
/// | 4. the parent is not the element and not inside it | [`EditError::ParentInSubtree`] | SACM-CMD-005 |
/// | 5. under a parent: the element keeps no `xmi:type` or `xsi:type` as preserved content, which only a root can carry (LIB-COMPAT-019) | [`EditError::KeptRootType`] | SACM-CMD-005 |
/// | 6. the position is at most the number of the others | [`EditError::PositionOutOfRange`] | SACM-CMD-009 |
///
/// ## Copies
///
/// [`CopyElements`] checks, in this order:
///
/// | Check | Refusal | Code |
/// |---|---|---|
/// | 1. under a parent: the parent is in the target | [`EditError::ParentNotFound`] | SACM-CMD-001 |
/// | 2. each top, in list order: the parent's class can hold it, and it keeps no `xmi:type` or `xsi:type` as preserved content, which only a root can carry (LIB-COMPAT-019); for the roots, it is a package | [`EditError::InvalidParent`], [`EditError::KeptRootType`] (the source id), [`EditError::InvalidRoot`] (the source id) | SACM-CMD-005 |
/// | 3. each copied element, in pre-order: its preserved content's ids are free, and that content names no id the copy changes or drops | [`EditError::DuplicateId`] | SACM-CMD-004 |
/// | 4. each copied element, in pre-order, each end in [`ReferenceEnd::ALL`](crate::model::ReferenceEnd::ALL) order: it holds the distinct ids it needs, such as an interface's `implements` and a binding's two participants (ADR 0039 D3) | [`EditError::TooFewReferences`] | SACM-MULT-001 |
/// | 5. every reference, in the same order: it names an element of the target or a copy | [`EditError::ReferenceNotFound`] | SACM-CMD-001 |
/// | 6. every reference, in the same order: it names an element of a kind its end admits | [`EditError::WrongReferenceType`] | SACM-REF-002 |
///
/// An id the source lacks is refused earlier, when the copy is built
/// ([`CopyElements::new`], [`CopyElements::root`]).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, EditError};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let mutation = edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")))?;
/// assert_eq!(mutation.created_ids(), [ElementId::from("argument_package_1")]);
///
/// let again = CreateArgumentPackage::root(LangString::new("Again")).with_id("argument_package_1");
/// let before = doc.clone();
/// let error = edit::apply(&mut doc, again).unwrap_err();
/// assert!(matches!(error, EditError::DuplicateId { .. }));
/// assert_eq!(doc, before); // unchanged
/// # Ok::<(), EditError>(())
/// ```
pub fn apply(
    document: &mut Document,
    operation: impl Into<Operation>,
) -> Result<Mutation, EditError> {
    apply_operation(document, operation.into()).map_err(|(_, error)| error)
}

/// [`apply`], giving the operation back with a refusal: the one code path of
/// `apply`, [`apply_preview`] and [`Batch::apply`].
fn apply_operation(
    document: &mut Document,
    operation: Operation,
) -> Result<Mutation, (Box<Operation>, EditError)> {
    apply_planned(document, operation, plan)
}

/// [`apply`] without the LIB-EDIT-025 text check, for tests only.
///
/// - **For.** A test can build a document that [`xmi::write`](crate::xmi::write) refuses.
/// - **Test support.** Only with the `test-support` feature; not part of the stable API.
///
/// # Errors
///
/// Every refusal of [`apply`] except [`EditError::UnrepresentableCharacter`].
#[cfg(feature = "test-support")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
pub fn apply_unchecked_text(
    document: &mut Document,
    operation: impl Into<Operation>,
) -> Result<Mutation, EditError> {
    apply_planned(document, operation.into(), plan_unchecked_text).map_err(|(_, error)| error)
}

/// The body of [`apply_operation`], planned with `plan`.
fn apply_planned(
    document: &mut Document,
    operation: Operation,
    plan: for<'o> fn(&Document, &'o Operation) -> Result<Plan<'o>, EditError>,
) -> Result<Mutation, (Box<Operation>, EditError)> {
    // Every check runs in `plan`, before the one change, so a refusal leaves
    // the document untouched, its revision included. The plan also knows
    // what the change creates, changes and deletes (ADR 0031).
    let planned = plan(document, &operation).map(|plan| {
        let outcome = plan.outcome();
        plan.commit(document);
        outcome
    });
    match planned {
        Ok(outcome) => {
            document.renew_revision();
            Ok(Mutation::new(operation, outcome))
        }
        Err(error) => Err((Box::new(operation), error)),
    }
}

/// Checks `operation` against `document` without applying it: `Ok` exactly
/// when [`apply`] would accept it, and otherwise the same [`EditError`].
///
/// - **Same answer as `apply`.** It runs the same checks, in the same order, through the
///   same code, so the two cannot disagree (ADR 0019 amendment 3).
/// - **Changes nothing.** The elements, the written XMI and the ids generated next stay
///   as they are. A create without `with_id` gets the same id after any number of checks.
/// - **The caller keeps the operation,** to apply later or to show in a menu.
/// - **No ids reported.** What the operation would create or change is in the
///   [`Mutation`] that `apply` returns, or in a [`preview`].
/// - **No validation.** `Ok` does not mean the edited document will have no findings;
///   see [`validation::validate`](crate::validation::validate).
///
/// # Errors
///
/// Returns the [`EditError`] that `apply` would refuse `operation` with, in the order of
/// the checks that [`apply`](apply#errors) lists.
///
/// # Examples
///
/// An operation struct is passed as `&op.into()`. A refused operation is
/// reported, and the document is unchanged:
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError};
/// use sacm::model::Document;
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root("Arguments").with_id("p"))?;
/// let before = doc.clone();
///
/// // A Claim needs a parent the document holds.
/// let error = edit::check(&doc, &CreateClaim::new("nowhere", "C1").into()).unwrap_err();
/// assert!(matches!(error, EditError::ParentNotFound { .. }));
/// assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND);
/// assert_eq!(doc, before); // unchanged
///
/// // An accepted operation is still the caller's, to apply.
/// let claim = CreateClaim::new("p", "C1");
/// assert_eq!(edit::check(&doc, &claim.clone().into()), Ok(()));
/// let mutation = edit::apply(&mut doc, claim)?;
/// assert_eq!(mutation.created_ids()[0].as_str(), "claim_1");
/// # Ok::<(), EditError>(())
/// ```
pub fn check(document: &Document, operation: &Operation) -> Result<(), EditError> {
    plan(document, operation).map(drop)
}

/// Shows what [`apply`] would do with `operation`, without changing `document`
/// (LIB-EDIT-004, ADR 0031).
///
/// - **Gives** the [`Mutation`] `apply` would return and, for a delete, the references
///   to deleted elements, which the delete removes ([`Preview::dangling_references`]).
/// - **Same checks, same ids.** It refuses exactly as `apply` and [`check`] do, and its
///   mutation is computed by the code `apply` reports from.
/// - **Changes nothing.** The elements, the ids generated next and the
///   [`revision`](Document::revision) stay as they are.
/// - **Apply it** with [`apply_preview`], while the document is at the preview's revision.
/// - **Delete policies.** To learn whether a delete applies under some policy, preview it
///   with that policy.
///
/// # Errors
///
/// Returns the [`EditError`] that `apply` would refuse `operation` with.
///
/// # Examples
///
/// An operation struct is passed as `&op.into()`. Preview a create, then a
/// delete that scrubs the references to the deleted element, and apply it:
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement};
/// use sacm::edit::{ReferenceDeletePolicy, SetReferences};
/// use sacm::model::{Document, ElementId, ReferenceEnd};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root("Arguments").with_id("p"))?;
/// let create = edit::preview(&doc, &CreateClaim::new("p", "C1").into())?;
/// assert_eq!(create.mutation().created_id().map(ElementId::as_str), Some("claim_1"));
///
/// edit::apply(&mut doc, CreateClaim::new("p", "C1").with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", "C2").with_id("c2"))?;
/// edit::apply(&mut doc, SetReferences::new("c2", ReferenceEnd::MetaClaim, ["c1"]))?;
///
/// let scrub = DeleteElement::new("c1").with_reference_policy(ReferenceDeletePolicy::ScrubReferences);
/// let preview = edit::preview(&doc, &scrub.into())?;
/// let dangling = &preview.dangling_references()[0];
/// assert_eq!(dangling.holder().as_str(), "c2");
/// assert_eq!(dangling.end(), ReferenceEnd::MetaClaim);
/// assert_eq!(dangling.id().as_str(), "c1");
/// assert_eq!(preview.mutation().deleted_ids(), [ElementId::from("c1")]);
/// assert_eq!(preview.mutation().changed_ids(), [ElementId::from("c2")]);
/// assert!(doc.contains("c1")); // nothing has changed yet
///
/// let mutation = edit::apply_preview(&mut doc, &preview)?;
/// assert_eq!(&mutation, preview.mutation());
/// assert!(doc.get("c2").is_some_and(|c2| c2.meta_claims().is_empty()));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
pub fn preview(document: &Document, operation: &Operation) -> Result<Preview, EditError> {
    let plan = plan(document, operation)?;
    let dangling = plan.dangling_references().to_vec();
    let mutation = Mutation::new(operation.clone(), plan.outcome());
    Ok(Preview::new(document.revision(), mutation, dangling))
}

/// Applies a [`Preview`] taken by [`preview`], if `document` has not changed since
/// (LIB-EDIT-007, ADR 0031).
///
/// - **Unchanged** means its [`revision`](Document::revision) is still the preview's.
/// - **Result.** It applies the preview's operation, as [`apply`] does, and returns a
///   [`Mutation`] equal to [`Preview::mutation`].
/// - **At most once.** Every successful edit gives the document a new revision.
/// - **Clones.** A clone shares its original's revision until either changes, so a
///   preview taken on one applies to the other while both are unchanged.
///
/// # Errors
///
/// Refuses with [`EditError::PreviewExpired`] (SACM-CMD-003), changing nothing, when
/// the document's revision is not the preview's: preview the operation again.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Operation, SetName};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// let claim = Operation::from(CreateClaim::new("p", LangString::new("C1")));
/// let preview = edit::preview(&doc, &claim)?;
///
/// // The document changes after the preview, so the preview is stale.
/// edit::apply(&mut doc, SetName::new("p", LangString::new("Renamed")))?;
/// let error = edit::apply_preview(&mut doc, &preview).unwrap_err();
/// assert_eq!(error.code(), Code::CMD_PREVIEW_EXPIRED);
///
/// // Preview it again, and apply that.
/// let fresh = edit::preview(&doc, &claim)?;
/// assert_eq!(edit::apply_preview(&mut doc, &fresh)?.created_ids()[0].as_str(), "claim_1");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
pub fn apply_preview(document: &mut Document, preview: &Preview) -> Result<Mutation, EditError> {
    let (previewed, current) = (preview.revision(), document.revision());
    if previewed != current {
        return Err(EditError::PreviewExpired { previewed, current });
    }
    apply(document, preview.operation().clone())
}

/// Applies several operations to `document` as one change: all of them, or,
/// if the closure returns `Err`, none (LIB-EDIT-014, ADR 0032).
///
/// - **The closure gets a [`Batch`].** Each [`Batch::apply`] answers as [`apply`] would on
///   the state so far, so a later operation can use an id an earlier one generated.
///   [`Batch::document`] shows that state.
/// - **`Ok` commits.** Every accepted operation stays, and the document gets a new
///   [`revision`](Document::revision), even when nothing changed.
/// - **The result** is a [`BatchMutation`]: each accepted [`Mutation`] and the net ids.
/// - **An ignored refusal** changed nothing, so it does not stop the commit.
/// - **`Err(e)` commits nothing.** `e` is a refusal passed on with `?` or the closure's
///   own error. It is returned unchanged, and the document is `==` to before, with the
///   same next generated ids and the same revision.
/// - **Error type.** It needs `From<BatchError>` only if the closure uses `?` on
///   `tx.apply`.
/// - **A list of operations:** use [`apply_all`].
///
/// # Errors
///
/// Returns the closure's error, unchanged, and leaves the document as it was.
///
/// # Panics
///
/// Propagates a panic in the closure. The document is then unchanged, as for `Err`.
///
/// # Examples
///
/// Add an `ArgumentReasoning`, a claim and an `AssertedInference` from that
/// claim to the top claim, naming the reasoning, as one change:
///
/// ```
/// use sacm::edit::{self, BatchError, CreateArgumentPackage, CreateArgumentReasoning};
/// use sacm::edit::{CreateAssertedInference, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("The system is safe")).with_id("top"))?;
///
/// let batch = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
///     let reasoning = tx.apply(CreateArgumentReasoning::new("p", name("Argue over each hazard")))?;
///     let reasoning = reasoning.created_ids()[0].clone();
///     let sub = tx.apply(CreateClaim::new("p", name("Hazard H1 is mitigated")))?;
///     let sub = sub.created_ids()[0].clone();
///     let inference = CreateAssertedInference::new("p", name("Inference"), [sub], "top");
///     tx.apply(inference.with_reasoning(reasoning))?;
///     Ok(())
/// })?;
///
/// let created: Vec<&str> = batch.created_ids().iter().map(|id| id.as_str()).collect();
/// assert_eq!(created, ["argument_reasoning_1", "claim_1", "asserted_inference_1"]);
/// let inference = doc.get("asserted_inference_1").expect("the batch committed");
/// assert_eq!(inference.reasoning().map(|id| id.as_str()), Some("argument_reasoning_1"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn batch<E>(
    document: &mut Document,
    operations: impl FnOnce(&mut Batch<'_>) -> Result<(), E>,
) -> Result<BatchMutation, E> {
    batch::run(document, operations)
}

/// Applies `operations`, in order, as one change: [`batch`] over the list,
/// stopping at the first refusal (ADR 0032).
///
/// - **For** operations sent as data, such as a JSON array of [`Operation`]s.
/// - **All or none.** After a refusal the document is `==` to before, with the same next
///   generated ids and the same revision.
/// - **An empty list** commits, and gives a new revision.
///
/// # Errors
///
/// A [`BatchError`] naming the first refused operation, by its position in the list,
/// with its [`EditError`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Operation, SetName};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let operations = [
///     Operation::from(CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p")),
///     Operation::from(CreateClaim::new("p", LangString::new("C1")).with_id("c1")),
///     Operation::from(SetName::new("c1", LangString::new("The system is safe"))),
/// ];
/// let batch = edit::apply_all(&mut doc, operations)?;
/// assert_eq!(batch.mutations().len(), 3);
/// // Net: c1 is created, not changed.
/// assert_eq!(batch.created_ids().len(), 2);
/// assert!(batch.changed_ids().is_empty());
/// # Ok::<(), sacm::edit::BatchError>(())
/// ```
pub fn apply_all(
    document: &mut Document,
    operations: impl IntoIterator<Item = impl Into<Operation>>,
) -> Result<BatchMutation, BatchError> {
    batch(document, |tx| apply_each(tx, operations))
}

/// Shows what [`batch`] would return for `operations`, without changing
/// `document`: the closure runs on a copy (LIB-EDIT-024, ADR 0032).
///
/// - **Gives** the [`BatchMutation`] `batch` would return: each accepted [`Mutation`] with
///   its generated ids, and the net ids, a whole delete cascade included.
/// - **Holds while** `document` stays at the same [`revision`](Document::revision). After
///   another edit, the batch may generate other ids.
/// - **Changes nothing.** `document` is `==` to before, with the same revision and the
///   same next generated ids, so a [`Preview`] taken before still applies.
///
/// # Errors
///
/// Returns the closure's error, unchanged.
///
/// # Panics
///
/// Propagates a panic in the closure. The document is unchanged.
///
/// # Examples
///
/// Preview a delete with its cascade before asking the user:
///
/// ```
/// use sacm::edit::{self, BatchError, CreateArgumentPackage, CreateClaim, DeleteElement};
/// use sacm::edit::{PackageDeletePolicy, SetName};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::new("p", name("Brakes")).with_id("q"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::new("p", name("Steering")).with_id("s"))?;
/// edit::apply(&mut doc, CreateClaim::new("q", name("The brakes are safe")).with_id("c1"))?;
///
/// let plan = edit::preview_batch(&doc, |tx| -> Result<(), BatchError> {
///     tx.apply(DeleteElement::new("q").with_package_policy(PackageDeletePolicy::DeleteRecursively))?;
///     tx.apply(SetName::new("p", name("Arguments, steering only")))?;
///     Ok(())
/// })?;
/// assert_eq!(plan.deleted_ids(), [ElementId::from("q"), ElementId::from("c1")]);
/// assert_eq!(plan.changed_ids(), [ElementId::from("p")]);
/// assert!(doc.contains("q")); // nothing has changed yet
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn preview_batch<E>(
    document: &Document,
    operations: impl FnOnce(&mut Batch<'_>) -> Result<(), E>,
) -> Result<BatchMutation, E> {
    batch::preview(document, operations)
}

/// Shows what [`apply_all`] would return for `operations`, without changing `document`.
///
/// It is [`preview_batch`] over the list, stopping at the first refusal.
///
/// # Errors
///
/// A [`BatchError`] naming the first refused operation, by its position in the list,
/// with its [`EditError`], as `apply_all` gives it.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Operation, SetName};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// let operations = [
///     Operation::from(CreateClaim::new("p", LangString::new("C1"))),
///     Operation::from(SetName::new("nope", LangString::new("C2"))),
/// ];
/// let error = edit::preview_all(&doc, operations).unwrap_err();
/// assert_eq!(error.index(), 1);
/// assert_eq!(doc.element_count(), 1); // unchanged
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
pub fn preview_all(
    document: &Document,
    operations: impl IntoIterator<Item = impl Into<Operation>>,
) -> Result<BatchMutation, BatchError> {
    preview_batch(document, |tx| apply_each(tx, operations))
}

/// Applies `operations` in `tx`, in order, passing the first refusal on:
/// the body of [`apply_all`] and [`preview_all`].
fn apply_each(
    tx: &mut Batch<'_>,
    operations: impl IntoIterator<Item = impl Into<Operation>>,
) -> Result<(), BatchError> {
    for operation in operations {
        tx.apply(operation)?;
    }
    Ok(())
}

/// Runs every check of `operation` against `document`, and returns what the
/// change needs: the one code path of [`check`] and [`apply`]. Every refusal
/// is built here; [`Plan::commit`] builds none.
fn plan<'o>(document: &Document, operation: &'o Operation) -> Result<Plan<'o>, EditError> {
    let plan = plan_unchecked_text(document, operation)?;
    check_text(&plan)?;
    Ok(plan)
}

/// [`plan`] without its last check, [`check_text`]: the plan of
/// `apply_unchecked_text`.
fn plan_unchecked_text<'o>(
    document: &Document,
    operation: &'o Operation,
) -> Result<Plan<'o>, EditError> {
    let plan = plan_operation(document, operation)?;
    if let Plan::Create(new) = &plan {
        check_new_expressions(document, &new.features)?;
    }
    Ok(plan)
}

/// The last check of every operation that stores text: no text it stores
/// holds a character XML 1.0 cannot represent (LIB-EDIT-025). The texts are
/// checked in the order `xmi::write` checks them. A plan that stores no new
/// text, such as [`Plan::Unchanged`] or a copy, passes.
fn check_text(plan: &Plan<'_>) -> Result<(), EditError> {
    use crate::xmi::{first_unrepresentable, name_texts, string_attribute, value_texts};
    use plan::Value;

    let (element, found) = match plan {
        Plan::Create(new) => {
            let found = match &new.features {
                Features::Name(name) | Features::Referencing { name, .. } => {
                    first_unrepresentable(name_texts(Some(name)))
                }
                Features::Texts { content, key } => first_unrepresentable(
                    value_texts(TextRole::Content, content).chain(
                        key.iter()
                            .flat_map(|key| value_texts(TextRole::Key, key.as_ref())),
                    ),
                ),
                Features::Asset {
                    name,
                    strings,
                    location,
                } => first_unrepresentable(
                    written_strings(strings)
                        .chain(name_texts(Some(name)))
                        .chain(
                            location
                                .iter()
                                .flat_map(|location| value_texts(TextRole::Location, location)),
                        ),
                ),
                Features::Terminology { name, strings, .. } => {
                    first_unrepresentable(written_strings(strings).chain(name_texts(Some(name))))
                }
            };
            (&new.id, found)
        }
        Plan::Set { element, value } => {
            let found = match value {
                Value::Name(name) => first_unrepresentable(name_texts(Some(name))),
                Value::Gid(Some(gid)) => {
                    first_unrepresentable([(crate::xmi::WriteAttribute::Gid, None, Some(*gid))])
                }
                Value::String(attribute, Some(value)) => {
                    first_unrepresentable([(string_attribute(*attribute), None, Some(*value))])
                }
                Value::Text(role, Some(text)) => {
                    first_unrepresentable(value_texts(*role, text.value()))
                }
                _ => None,
            };
            (element.as_ref(), found)
        }
        Plan::Unchanged
        | Plan::Reorder { .. }
        | Plan::Delete(_)
        | Plan::Move { .. }
        | Plan::Copy(_) => return Ok(()),
    };
    match found {
        Some((attribute, position, character)) => Err(EditError::UnrepresentableCharacter {
            element: element.clone(),
            character,
            attribute,
            position,
        }),
        None => Ok(()),
    }
}

/// The string-valued attributes a create gives, for
/// [`check_text`], in the order they are written.
fn written_strings<'a>(
    strings: &'a [(StringAttribute, Option<&'a str>)],
) -> impl Iterator<Item = (crate::xmi::WriteAttribute, Option<usize>, Option<&'a str>)> {
    StringAttribute::ALL.into_iter().map(move |attribute| {
        let value = strings
            .iter()
            .find(|(given, _)| *given == attribute)
            .and_then(|(_, value)| *value);
        (crate::xmi::string_attribute(attribute), None, value)
    })
}

/// The last checks of a create with no reference end of its own: each
/// `ExpressionLangString` of its name and texts has an `expression`, which
/// names a `Term` or an `Expression` (SACM23-BASE-009). A create with ends
/// checks them with its ends, after them in each check.
fn check_new_expressions(document: &Document, features: &Features<'_>) -> Result<(), EditError> {
    match features {
        Features::Name(name) => references::check_expressions(document, [*name], &[]),
        Features::Texts { content, key } => references::check_expressions(
            document,
            content
                .values()
                .iter()
                .chain(key.iter().flat_map(|key| key.values())),
            &[],
        ),
        Features::Asset { name, location, .. } => references::check_expressions(
            document,
            std::iter::once(*name).chain(location.iter().flat_map(|location| location.values())),
            &[],
        ),
        Features::Referencing { .. } | Features::Terminology { .. } => Ok(()),
    }
}

/// The plan of `operation` itself, as [`plan`] runs it.
fn plan_operation<'o>(
    document: &Document,
    operation: &'o Operation,
) -> Result<Plan<'o>, EditError> {
    match operation {
        Operation::CreateAssuranceCasePackage(create) => create.plan(document),
        Operation::CreateAssuranceCasePackageInterface(create) => create.plan(document),
        Operation::CreateAssuranceCasePackageBinding(create) => create.plan(document),
        Operation::CreateArgumentPackage(create) => create.plan(document),
        Operation::CreateArgumentPackageInterface(create) => create.plan(document),
        Operation::CreateArgumentPackageBinding(create) => create.plan(document),
        Operation::CreateClaim(create) => create.plan(document),
        Operation::CreateArgumentReasoning(create) => create.plan(document),
        Operation::CreateAssertedInference(create) => create.plan(document),
        Operation::CreateAssertedContext(create) => create.plan(document),
        Operation::CreateArtifactReference(create) => create.plan(document),
        Operation::CreateArgumentGroup(create) => create.plan(document),
        Operation::CreateAssertedEvidence(create) => create.plan(document),
        Operation::CreateAssertedArtifactSupport(create) => create.plan(document),
        Operation::CreateAssertedArtifactContext(create) => create.plan(document),
        Operation::CreateDescription(create) => create.plan(document),
        Operation::CreateImplementationConstraint(create) => create.plan(document),
        Operation::CreateNote(create) => create.plan(document),
        Operation::CreateTaggedValue(create) => create.plan(document),
        Operation::CreateArtifactPackage(create) => create.plan(document),
        Operation::CreateArtifactPackageInterface(create) => create.plan(document),
        Operation::CreateArtifactPackageBinding(create) => create.plan(document),
        Operation::CreateProperty(create) => create.plan(document),
        Operation::CreateTechnique(create) => create.plan(document),
        Operation::CreateParticipant(create) => create.plan(document),
        Operation::CreateArtifactGroup(create) => create.plan(document),
        Operation::CreateArtifact(create) => create.plan(document),
        Operation::CreateEvent(create) => create.plan(document),
        Operation::CreateResource(create) => create.plan(document),
        Operation::CreateActivity(create) => create.plan(document),
        Operation::CreateArtifactAssetRelationship(create) => create.plan(document),
        Operation::CreateTerminologyPackage(create) => create.plan(document),
        Operation::CreateTerminologyPackageInterface(create) => create.plan(document),
        Operation::CreateTerminologyPackageBinding(create) => create.plan(document),
        Operation::CreateCategory(create) => create.plan(document),
        Operation::CreateTerm(create) => create.plan(document),
        Operation::CreateExpression(create) => create.plan(document),
        Operation::CreateTerminologyGroup(create) => create.plan(document),
        Operation::SetName(set) => set.plan(document),
        Operation::SetGid(set) => set.plan(document),
        Operation::SetFlag(set) => set.plan(document),
        Operation::SetAssertionDeclaration(set) => set.plan(document),
        Operation::SetContent(set) => set.plan(document),
        Operation::SetKey(set) => set.plan(document),
        Operation::SetTaggedValue(set) => set.plan(document),
        Operation::SetVersion(set) => set.plan(document),
        Operation::SetDate(set) => set.plan(document),
        Operation::SetLocation(set) => set.plan(document),
        Operation::SetStartTime(set) => set.plan(document),
        Operation::SetEndTime(set) => set.plan(document),
        Operation::SetValue(set) => set.plan(document),
        Operation::SetExternalReference(set) => set.plan(document),
        Operation::SetReferences(set) => set.plan(document),
        Operation::AddReference(add) => add.plan(document),
        Operation::RemoveReference(remove) => remove.plan(document),
        Operation::ReorderElements(reorder) => reorder.plan(document),
        Operation::MoveElement(moved) => moved.plan(document),
        Operation::CopyElements(copy) => copy.plan(document),
        Operation::DeleteElement(delete) => delete.plan(document),
    }
}

/// An edit operation, as a value: any of the operation structs.
///
/// - **Conversion.** [`apply`] takes an `Operation` or any operation struct, which
///   converts into one with `From`.
/// - **Non-exhaustive.** Later releases add operations, so a `match` on `Operation` needs
///   a wildcard arm.
///
/// # Examples
///
/// ```
/// use sacm::edit::{CreateClaim, Operation};
/// use sacm::model::LangString;
///
/// let operation = Operation::from(CreateClaim::new("arguments", LangString::new("C1")));
/// assert!(matches!(operation, Operation::CreateClaim(_)));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type")
)]
#[non_exhaustive]
pub enum Operation {
    /// Creates an `AssuranceCasePackage`.
    CreateAssuranceCasePackage(CreateAssuranceCasePackage),
    /// Creates an `AssuranceCasePackageInterface`.
    CreateAssuranceCasePackageInterface(CreateAssuranceCasePackageInterface),
    /// Creates an `AssuranceCasePackageBinding`.
    CreateAssuranceCasePackageBinding(CreateAssuranceCasePackageBinding),
    /// Creates an `ArgumentPackage`.
    CreateArgumentPackage(CreateArgumentPackage),
    /// Creates an `ArgumentPackageInterface`.
    CreateArgumentPackageInterface(CreateArgumentPackageInterface),
    /// Creates an `ArgumentPackageBinding`.
    CreateArgumentPackageBinding(CreateArgumentPackageBinding),
    /// Creates a `Claim`.
    CreateClaim(CreateClaim),
    /// Creates an `ArgumentReasoning`.
    CreateArgumentReasoning(CreateArgumentReasoning),
    /// Creates an `AssertedInference`.
    CreateAssertedInference(CreateAssertedInference),
    /// Creates an `AssertedContext`.
    CreateAssertedContext(CreateAssertedContext),
    /// Creates an `ArtifactReference`.
    CreateArtifactReference(CreateArtifactReference),
    /// Creates an `ArgumentGroup`.
    CreateArgumentGroup(CreateArgumentGroup),
    /// Creates an `AssertedEvidence`.
    CreateAssertedEvidence(CreateAssertedEvidence),
    /// Creates an `AssertedArtifactSupport`.
    CreateAssertedArtifactSupport(CreateAssertedArtifactSupport),
    /// Creates an `AssertedArtifactContext`.
    CreateAssertedArtifactContext(CreateAssertedArtifactContext),
    /// Creates a `Description`.
    CreateDescription(CreateDescription),
    /// Creates an `ImplementationConstraint`.
    CreateImplementationConstraint(CreateImplementationConstraint),
    /// Creates a `Note`.
    CreateNote(CreateNote),
    /// Creates a `TaggedValue`.
    CreateTaggedValue(CreateTaggedValue),
    /// Creates an `ArtifactPackage`.
    CreateArtifactPackage(CreateArtifactPackage),
    /// Creates an `ArtifactPackageInterface`.
    CreateArtifactPackageInterface(CreateArtifactPackageInterface),
    /// Creates an `ArtifactPackageBinding`.
    CreateArtifactPackageBinding(CreateArtifactPackageBinding),
    /// Creates a `Property`.
    CreateProperty(CreateProperty),
    /// Creates a `Technique`.
    CreateTechnique(CreateTechnique),
    /// Creates a `Participant`.
    CreateParticipant(CreateParticipant),
    /// Creates an `ArtifactGroup`.
    CreateArtifactGroup(CreateArtifactGroup),
    /// Creates an `Artifact`.
    CreateArtifact(CreateArtifact),
    /// Creates an `Event`.
    CreateEvent(CreateEvent),
    /// Creates a `Resource`.
    CreateResource(CreateResource),
    /// Creates an `Activity`.
    CreateActivity(CreateActivity),
    /// Creates an `ArtifactAssetRelationship`.
    CreateArtifactAssetRelationship(CreateArtifactAssetRelationship),
    /// Creates a `TerminologyPackage`.
    CreateTerminologyPackage(CreateTerminologyPackage),
    /// Creates a `TerminologyPackageInterface`.
    CreateTerminologyPackageInterface(CreateTerminologyPackageInterface),
    /// Creates a `TerminologyPackageBinding`.
    CreateTerminologyPackageBinding(CreateTerminologyPackageBinding),
    /// Creates a `Category`.
    CreateCategory(CreateCategory),
    /// Creates a `Term`.
    CreateTerm(CreateTerm),
    /// Creates an `Expression`.
    CreateExpression(CreateExpression),
    /// Creates a `TerminologyGroup`.
    CreateTerminologyGroup(CreateTerminologyGroup),
    /// Sets the name of a `ModelElement`.
    SetName(SetName),
    /// Sets or removes the `gid` of an element.
    SetGid(SetGid),
    /// Sets a flag of an element: `isCitation`, `isAbstract` or `isCounter`.
    SetFlag(SetFlag),
    /// Sets the `assertionDeclaration` of an Assertion.
    SetAssertionDeclaration(SetAssertionDeclaration),
    /// Sets or removes the `content` of a `UtilityElement`.
    SetContent(SetContent),
    /// Sets the `key` of a `TaggedValue`.
    SetKey(SetKey),
    /// Sets the content of the `TaggedValue` with a plain key, creating it if
    /// absent.
    SetTaggedValue(SetTaggedValue),
    /// Sets or removes the `version` of an `Artifact`.
    SetVersion(SetVersion),
    /// Sets or removes the `date` of an `Artifact` or an `Event`.
    SetDate(SetDate),
    /// Sets or removes the `location` of a `Resource`.
    SetLocation(SetLocation),
    /// Sets or removes the `startTime` of an `Activity`.
    SetStartTime(SetStartTime),
    /// Sets or removes the `endTime` of an `Activity`.
    SetEndTime(SetEndTime),
    /// Sets the `value` of a `Term` or an `Expression`.
    SetValue(SetValue),
    /// Sets or removes the `externalReference` of a `Term`.
    SetExternalReference(SetExternalReference),
    /// Replaces the ids a reference end of an element holds.
    SetReferences(SetReferences),
    /// Appends an id to a reference end of an element.
    AddReference(AddReference),
    /// Removes every occurrence of an id from a reference end of an element.
    RemoveReference(RemoveReference),
    /// Reorders the children of an element, or the roots.
    ReorderElements(ReorderElements),
    /// Moves an element under another parent, or to the roots.
    MoveElement(MoveElement),
    /// Copies elements of another document.
    ///
    /// Not serialized: with the `serde` feature, serializing it is an error, and its tag
    /// does not deserialize (ADR 0030 amendment 2).
    #[cfg_attr(feature = "serde", serde(skip))]
    CopyElements(CopyElements),
    /// Deletes an element under explicit policies.
    DeleteElement(DeleteElement),
}

impl Operation {
    /// The operation's stable name, for logs: the name of its struct, such as
    /// `"CreateClaim"`.
    ///
    /// - **Stable,** like a diagnostic [`Code`]: a name never changes and is never given
    ///   to another operation.
    /// - **The names** of this release are those of the [`Operation`] variants, each the
    ///   name of its struct.
    /// - **One name per struct.** Its forms, such as [`SetGid::new`] and
    ///   [`SetGid::remove`], share it.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{CreateClaim, Operation};
    /// use sacm::model::LangString;
    ///
    /// let operation = Operation::from(CreateClaim::new("arguments", LangString::new("C1")));
    /// assert_eq!(operation.operation_name(), "CreateClaim");
    /// ```
    #[must_use]
    pub fn operation_name(&self) -> &'static str {
        match self {
            Self::CreateAssuranceCasePackage(_) => "CreateAssuranceCasePackage",
            Self::CreateAssuranceCasePackageInterface(_) => "CreateAssuranceCasePackageInterface",
            Self::CreateAssuranceCasePackageBinding(_) => "CreateAssuranceCasePackageBinding",
            Self::CreateArgumentPackage(_) => "CreateArgumentPackage",
            Self::CreateArgumentPackageInterface(_) => "CreateArgumentPackageInterface",
            Self::CreateArgumentPackageBinding(_) => "CreateArgumentPackageBinding",
            Self::CreateClaim(_) => "CreateClaim",
            Self::CreateArgumentReasoning(_) => "CreateArgumentReasoning",
            Self::CreateAssertedInference(_) => "CreateAssertedInference",
            Self::CreateAssertedContext(_) => "CreateAssertedContext",
            Self::CreateArtifactReference(_) => "CreateArtifactReference",
            Self::CreateArgumentGroup(_) => "CreateArgumentGroup",
            Self::CreateAssertedEvidence(_) => "CreateAssertedEvidence",
            Self::CreateAssertedArtifactSupport(_) => "CreateAssertedArtifactSupport",
            Self::CreateAssertedArtifactContext(_) => "CreateAssertedArtifactContext",
            Self::CreateDescription(_) => "CreateDescription",
            Self::CreateImplementationConstraint(_) => "CreateImplementationConstraint",
            Self::CreateNote(_) => "CreateNote",
            Self::CreateTaggedValue(_) => "CreateTaggedValue",
            Self::CreateArtifactPackage(_) => "CreateArtifactPackage",
            Self::CreateArtifactPackageInterface(_) => "CreateArtifactPackageInterface",
            Self::CreateArtifactPackageBinding(_) => "CreateArtifactPackageBinding",
            Self::CreateProperty(_) => "CreateProperty",
            Self::CreateTechnique(_) => "CreateTechnique",
            Self::CreateParticipant(_) => "CreateParticipant",
            Self::CreateArtifactGroup(_) => "CreateArtifactGroup",
            Self::CreateArtifact(_) => "CreateArtifact",
            Self::CreateEvent(_) => "CreateEvent",
            Self::CreateResource(_) => "CreateResource",
            Self::CreateActivity(_) => "CreateActivity",
            Self::CreateArtifactAssetRelationship(_) => "CreateArtifactAssetRelationship",
            Self::CreateTerminologyPackage(_) => "CreateTerminologyPackage",
            Self::CreateTerminologyPackageInterface(_) => "CreateTerminologyPackageInterface",
            Self::CreateTerminologyPackageBinding(_) => "CreateTerminologyPackageBinding",
            Self::CreateCategory(_) => "CreateCategory",
            Self::CreateTerm(_) => "CreateTerm",
            Self::CreateExpression(_) => "CreateExpression",
            Self::CreateTerminologyGroup(_) => "CreateTerminologyGroup",
            Self::SetName(_) => "SetName",
            Self::SetGid(_) => "SetGid",
            Self::SetFlag(_) => "SetFlag",
            Self::SetAssertionDeclaration(_) => "SetAssertionDeclaration",
            Self::SetContent(_) => "SetContent",
            Self::SetKey(_) => "SetKey",
            Self::SetTaggedValue(_) => "SetTaggedValue",
            Self::SetVersion(_) => "SetVersion",
            Self::SetDate(_) => "SetDate",
            Self::SetLocation(_) => "SetLocation",
            Self::SetStartTime(_) => "SetStartTime",
            Self::SetEndTime(_) => "SetEndTime",
            Self::SetValue(_) => "SetValue",
            Self::SetExternalReference(_) => "SetExternalReference",
            Self::SetReferences(_) => "SetReferences",
            Self::AddReference(_) => "AddReference",
            Self::RemoveReference(_) => "RemoveReference",
            Self::ReorderElements(_) => "ReorderElements",
            Self::MoveElement(_) => "MoveElement",
            Self::CopyElements(_) => "CopyElements",
            Self::DeleteElement(_) => "DeleteElement",
        }
    }
}

/// Converts each operation struct that is not a create into its
/// [`Operation`] variant of the same name.
macro_rules! operation_from {
    ($($name:ident),* $(,)?) => {
        $(
            impl From<$name> for Operation {
                fn from(operation: $name) -> Self {
                    Self::$name(operation)
                }
            }
        )*
    };
}

operation_from!(
    SetName,
    SetGid,
    SetFlag,
    SetAssertionDeclaration,
    SetContent,
    SetKey,
    SetTaggedValue,
    SetLocation,
    SetReferences,
    AddReference,
    RemoveReference,
    ReorderElements,
    MoveElement,
    CopyElements,
    DeleteElement,
);

impl From<CreateAssuranceCasePackage> for Operation {
    fn from(operation: CreateAssuranceCasePackage) -> Self {
        Self::CreateAssuranceCasePackage(operation)
    }
}

impl From<CreateArgumentPackage> for Operation {
    fn from(operation: CreateArgumentPackage) -> Self {
        Self::CreateArgumentPackage(operation)
    }
}

impl From<CreateClaim> for Operation {
    fn from(operation: CreateClaim) -> Self {
        Self::CreateClaim(operation)
    }
}

impl From<CreateArgumentReasoning> for Operation {
    fn from(operation: CreateArgumentReasoning) -> Self {
        Self::CreateArgumentReasoning(operation)
    }
}

impl From<CreateArtifactReference> for Operation {
    fn from(operation: CreateArtifactReference) -> Self {
        Self::CreateArtifactReference(operation)
    }
}

impl From<CreateArgumentGroup> for Operation {
    fn from(operation: CreateArgumentGroup) -> Self {
        Self::CreateArgumentGroup(operation)
    }
}

impl From<CreateDescription> for Operation {
    fn from(operation: CreateDescription) -> Self {
        Self::CreateDescription(operation)
    }
}

impl From<CreateImplementationConstraint> for Operation {
    fn from(operation: CreateImplementationConstraint) -> Self {
        Self::CreateImplementationConstraint(operation)
    }
}

impl From<CreateNote> for Operation {
    fn from(operation: CreateNote) -> Self {
        Self::CreateNote(operation)
    }
}

impl From<CreateTaggedValue> for Operation {
    fn from(operation: CreateTaggedValue) -> Self {
        Self::CreateTaggedValue(operation)
    }
}

/// What an applied operation did: the operation, and the ids it created,
/// changed and deleted.
///
/// [`apply`] returns one for every operation it applies. The three lists are what
/// [`semantic_compare`](crate::compare::semantic_compare) of the document before and
/// after would report; `apply` does not run it (ADR 0019 amendment 1).
///
/// | List | Its ids are |
/// |---|---|
/// | [`created_ids`](Self::created_ids) | in the document after, not before |
/// | [`changed_ids`](Self::changed_ids) | in both, with a difference in the element itself |
/// | [`deleted_ids`](Self::deleted_ids) | in the document before, not after |
///
/// What each operation lists:
///
/// - **A create** lists its element as created. The parent is not changed: its features
///   and the order of its existing children are as before. It is in
///   [`operation`](Self::operation).
/// - **A set, reference operation or child reorder** changes its element (for a reorder,
///   the parent) when the value differs from the one held, or when it replaces a
///   preserved attribute of the feature's name ([`SetFlag`], [`SetReferences`]). It
///   creates nothing, but for a [`SetTaggedValue`] that finds no tagged value.
/// - **A referenced element** is not changed by the reference that names it.
/// - **A [`DeleteElement`]** deletes the element, what it contains and what its policies
///   delete, and changes each surviving element that loses a reference to one of them.
/// - **A deleted element's parent** is not changed, unless a preserved child of it comes
///   after a deleted child and so moves to a lower index (ADR 0033).
///
/// How to tell that an edit changed nothing:
///
/// - **[`is_empty`](Self::is_empty)** is true: the three lists are empty. A set of the
///   value held, a move to the place held and a copy of no element change nothing.
/// - **Except the root order.** A change of the order of the roots lists no id either,
///   since that order belongs to no element. [`is_empty`](Self::is_empty) says how to
///   tell then.
///
/// A refused operation returns an [`EditError`] instead:
///
/// - **Log it** with [`EditError::code`].
/// - **Its name.** `apply` takes the operation by value, so keep a clone to log the
///   refused operation's name.
///
/// # Examples
///
/// An audit-log line:
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
/// use sacm::model::{Document, ElementId};
///
/// let mut doc = Document::new();
/// let arguments = CreateArgumentPackage::root("Arguments").with_id("arguments");
/// edit::apply(&mut doc, arguments)?;
/// // No with_id: the claim's id is generated.
/// let claim = CreateClaim::new("arguments", "The system is safe");
/// let mutation = edit::apply(&mut doc, claim)?;
///
/// let ids = |ids: &[ElementId]| ids.iter().map(ElementId::as_str).collect::<Vec<_>>().join(",");
/// let line = format!(
///     "{} created=[{}] changed=[{}] deleted=[{}]",
///     mutation.operation().operation_name(),
///     ids(mutation.created_ids()),
///     ids(mutation.changed_ids()),
///     ids(mutation.deleted_ids()),
/// );
/// assert_eq!(line, "CreateClaim created=[claim_1] changed=[] deleted=[]");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct Mutation {
    operation: Operation,
    #[cfg_attr(feature = "serde", serde(rename = "created_ids"))]
    created: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(rename = "changed_ids"))]
    changed: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(rename = "deleted_ids"))]
    deleted: Vec<ElementId>,
}

impl Mutation {
    /// The mutation of `operation`, with the ids its plan computed.
    fn new(operation: Operation, outcome: Outcome) -> Self {
        let Outcome {
            created,
            changed,
            deleted,
        } = outcome;
        Self {
            operation,
            created,
            changed,
            deleted,
        }
    }

    /// The operation, exactly as given to [`apply`].
    ///
    /// A generated id is not filled in: for an operation without `with_id`, its `id()` is
    /// still `None`, and the id is in [`created_ids`](Self::created_ids).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, Operation};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let operation = CreateArgumentPackage::root(LangString::new("Arguments"));
    /// let mutation = edit::apply(&mut doc, operation.clone())?;
    /// assert_eq!(mutation.operation(), &Operation::from(operation));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// The ids of the elements created, in creation order, with the
    /// operation's own element first.
    ///
    /// - **Same as** the ids `semantic_compare(before, after)` reports as only in after.
    /// - **A create** creates exactly its own element; its parent is not changed.
    /// - **A [`CopyElements`]** creates its copies in pre-order, the i-th being the copy
    ///   of [`CopyElements::source_ids`]`[i]`.
    #[must_use]
    pub fn created_ids(&self) -> &[ElementId] {
        &self.created
    }

    /// The id of the one element the operation created: `Some` exactly when
    /// [`created_ids`](Self::created_ids) holds one id.
    ///
    /// A create gives its element's id. An operation that creates nothing, or a
    /// [`CopyElements`] that creates several, gives `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetName};
    /// use sacm::model::Document;
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root("Arguments").with_id("p"))?;
    /// let made = edit::apply(&mut doc, CreateClaim::new("p", "C1"))?;
    /// let claim = made.created_id().expect("a create creates one element").clone();
    /// assert_eq!(claim.as_str(), "claim_1");
    ///
    /// let renamed = edit::apply(&mut doc, SetName::new(&claim, "C2"))?;
    /// assert_eq!(renamed.created_id(), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn created_id(&self) -> Option<&ElementId> {
        match self.created.as_slice() {
            [one] => Some(one),
            _ => None,
        }
    }

    /// The ids of the existing elements changed, in the document's
    /// [pre-order](crate::model::Document::elements) after the operation.
    ///
    /// They are the ids, in both before and after, for which
    /// `semantic_compare(before, after)` reports a difference of the element itself, such
    /// as its kind, parent, name or child order.
    ///
    /// | Operation | Gives |
    /// |---|---|
    /// | a create, a [`CopyElements`] | none: a new child does not change its parent |
    /// | an operation that changes an element | that element, or none when it sets the value held |
    /// | [`ReorderElements::new`] | the parent, or none when the order is the one held |
    /// | [`ReorderElements::roots`] | none: the order of the roots belongs to no element |
    /// | [`DeleteElement`] | each surviving element that lost an id of a deleted element, and each whose preserved child moved to a lower index (ADR 0033) |
    /// | [`MoveElement`] | the element when its parent changes, and each parent whose child order or preserved placement changes; none for a root moved among the roots |
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetFlag};
    /// use sacm::model::{Document, ElementId, Flag, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    ///
    /// let set = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true))?;
    /// assert_eq!(set.changed_ids(), [ElementId::from("c1")]);
    /// let again = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true))?;
    /// assert!(again.changed_ids().is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn changed_ids(&self) -> &[ElementId] {
        &self.changed
    }

    /// The ids of the elements deleted, in the document's
    /// [pre-order](crate::model::Document::elements) before the operation.
    ///
    /// - **Same as** the ids `semantic_compare(before, after)` reports as only in before.
    /// - **Only a [`DeleteElement`] deletes elements:** the element, everything it
    ///   contains, and what its policies delete. [`DeleteElement`] lists them by policy.
    #[must_use]
    pub fn deleted_ids(&self) -> &[ElementId] {
        &self.deleted
    }

    /// Whether the mutation lists no id: the operation created, changed and deleted no
    /// element.
    ///
    /// - **Empty means nothing changed,** except for the order of the roots: the operation
    ///   set the value held, moved an element to the place it holds, or copied no element.
    /// - **A change of the root order is always empty,** although the document changed:
    ///   that order belongs to no element. This is a [`ReorderElements::roots`], and a
    ///   [`MoveElement::root`] of a root.
    /// - **To learn whether the roots moved,** compare the document with a clone taken
    ///   before (`==`), or use [`compare::changes`](crate::compare::changes), whose
    ///   [`Changes::is_empty`](crate::compare::Changes::is_empty) counts the root order.
    /// - **Batches.** [`BatchMutation::is_empty`] answers the same for a batch.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, SetName};
    /// use sacm::model::Document;
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root("Arguments").with_id("p"))?;
    /// assert!(edit::apply(&mut doc, SetName::new("p", "Arguments"))?.is_empty());
    /// assert!(!edit::apply(&mut doc, SetName::new("p", "Argument"))?.is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.changed.is_empty() && self.deleted.is_empty()
    }
}

/// Why [`apply`] refused an operation. The document is unchanged.
///
/// - **Code.** Each refusal has a catalog [`Code`], from [`code`](Self::code).
/// - **`Display`** writes one line that starts in lower case. Ids are quoted by `{:?}`, so
///   a line break in one never breaks the line.
/// - **Order.** [`apply`](apply#errors) lists the checks of each operation in order; the
///   first that fails is the refusal.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let error = edit::apply(&mut doc, CreateClaim::new("arguments", LangString::new("C1"))).unwrap_err();
/// assert_eq!(error.to_string(), "the document has no element \"arguments\" to be the parent");
/// assert_eq!(error.code().as_str(), "SACM-CMD-001");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// The parent is not in the document (SACM-CMD-001).
    ///
    /// Create the parent first, or correct its id. Ids are compared exactly, case included.
    #[non_exhaustive]
    ParentNotFound {
        /// The parent's id, as given.
        parent: ElementId,
    },
    /// No SACM composite role of the parent's class holds an element of the new kind
    /// (SACM-CMD-005): a Claim directly in an AssuranceCasePackage, say.
    ///
    /// - **Fix.** Choose a parent whose class can hold the new kind: a Claim goes in an
    ///   ArgumentPackage, and an ArgumentPackage in an AssuranceCasePackage or another
    ///   ArgumentPackage.
    /// - **Ask first.** [`ElementKind::may_contain`](crate::model::ElementKind::may_contain)
    ///   answers this before applying.
    #[non_exhaustive]
    InvalidParent {
        /// The parent's id.
        parent: ElementId,
        /// The parent's kind.
        parent_kind: ElementKind,
        /// The kind of the element that was to be created.
        kind: ElementKind,
    },
    /// The id given with `with_id` is not an XML `NCName` (SACM-ID-002).
    ///
    /// - **An `NCName`** starts with a letter or `_` and continues with letters, digits,
    ///   `-`, `.` or `_`; it has no spaces and no `:`.
    /// - **Or** leave out `with_id` to have an id generated.
    #[non_exhaustive]
    InvalidId {
        /// The id, as given.
        id: ElementId,
    },
    /// The id given with `with_id` is already in the document (SACM-CMD-004).
    ///
    /// - **In use** as an element's id, or as the `xmi:id` of a name, a `content`, a `key`,
    ///   a `location` or a `value`, which identify no element
    ///   ([`SacmElement::name_id`](crate::model::SacmElement::name_id),
    ///   [`SacmElement::content_id`](crate::model::SacmElement::content_id)).
    /// - **Fix.** Ids are unique across the whole document, not per parent. Choose another
    ///   id, or leave out `with_id` to have one generated.
    /// - **Ask first.** [`Document::is_id_in_use`] answers this before applying.
    /// - **A [`CopyElements`]** also refuses with it for an `xmi:id` of the preserved
    ///   content it copies: one that is not free in the target (in use, or named by a
    ///   reference), or that names an id the copy changes or drops. `id` is that id.
    #[non_exhaustive]
    DuplicateId {
        /// The id, as given.
        id: ElementId,
    },
    /// A reference end is given fewer ids than SACM requires (SACM-MULT-001).
    ///
    /// | Refused | SACM |
    /// |---|---|
    /// | an asserted relationship with no source | `source:ArgumentAsset[1..*]`, §11.13 |
    /// | an asserted relationship with no target | `target:ArgumentAsset[1]`, §11.13 |
    /// | an interface with no `implements` | §9.3, §10.5, §11.6, §12.5 |
    /// | a binding with fewer than two distinct participants | §9.4, §10.6, §11.5, §12.4 |
    /// | an `ArtifactAssetRelationship` with no source or no target | §12.14 |
    /// | an `ExpressionLangString` with no `expression`, as the end `expression` | §8.4 |
    ///
    /// - **Edits too.** A [`SetReferences::clear`] of such an end, or a
    ///   [`RemoveReference`] of the last source or of the target, would leave it so.
    /// - **`given`** is how many distinct ids the end would hold (ADR 0039 D3).
    /// - **Fix.** Give at least `required` ids.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, EditError};
    /// use sacm::model::{Document, ElementId, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let no_sources: Vec<ElementId> = Vec::new();
    /// let inference = CreateAssertedInference::new("p", LangString::new("I1"), no_sources, "c1");
    /// let error = edit::apply(&mut doc, inference).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::TooFewReferences { end: "source", given: 0, required: 1, .. }
    /// ));
    /// assert_eq!(error.to_string(), "the source is given 0 ids, and SACM requires at least 1");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    TooFewReferences {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// How many distinct values it would hold: ids, and cross-document
        /// references (ADR 0039 D3, ADR 0040).
        given: usize,
        /// How many it needs at least.
        required: usize,
    },
    /// A reference end is given more ids than SACM allows (SACM-MULT-001).
    ///
    /// - **When.** Two ids on a single-valued end, such as `target`, `reasoning` or
    ///   `citedElement`, or an [`AddReference`] to one that holds an id already.
    /// - **The same id twice** is two ids.
    /// - **Fix.** Give at most `allowed` ids, or replace the one held with
    ///   [`SetReferences`].
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetReferences};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let two = SetReferences::new("c1", ReferenceEnd::CitedElement, ["c1", "c1"]);
    /// let error = edit::apply(&mut doc, two).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::TooManyReferences { end: "citedElement", given: 2, allowed: 1, .. }
    /// ));
    /// assert_eq!(error.to_string(), "the citedElement is given 2 ids, and SACM allows at most 1");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    TooManyReferences {
        /// The end, by its XMI name, such as `target`.
        end: &'static str,
        /// How many values it would hold, cross-document references included.
        given: usize,
        /// How many it holds at most.
        allowed: usize,
    },
    /// A referenced id holds XML white space (space, TAB, LF or CR), so it cannot be
    /// written as one id (SACM-ID-002).
    ///
    /// - **Why.** Written to a reference attribute, `a b` would read back as the two ids
    ///   `a` and `b`.
    /// - **Where it comes from.** A file can give an element such an id:
    ///   [`xmi::read`](crate::xmi::read) keeps an id as written, and
    ///   [`validation::validate`](crate::validation::validate) reports it (SACM23-VAL-020).
    /// - **Which id.** The first such id, in the order of
    ///   [`ReferenceNotFound`](Self::ReferenceNotFound).
    /// - **New ids only.** [`SetReferences`] and [`AddReference`] check only the ids the
    ///   end does not hold already.
    /// - **Fix.** Give the referenced element an `NCName` id in the file first.
    ///
    /// ```
    /// use sacm::edit::{self, CreateAssertedInference, EditError};
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="a b"/>
    /// </sacm:ArgumentPackage>"#;
    /// let mut doc = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let name = sacm::model::LangString::new("I1");
    /// let error = edit::apply(&mut doc, CreateAssertedInference::new("p", name, ["a b"], "c1")).unwrap_err();
    /// assert!(matches!(error, EditError::InvalidReference { end: "source", .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the source \"a b\" holds white space, so it cannot be written as one id"
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    InvalidReference {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// A referenced id names no element of the document (SACM-CMD-001).
    ///
    /// - **Any end:** a source, the target or the `reasoning` of an asserted relationship,
    ///   a `structure`, a `referencedArtifactElement` or a group member, say.
    /// - **No dangling reference.** An edit never makes one (D-VAL-LAYER). The `xmi:id` of
    ///   a name, `content`, `key`, `location` or `value` names no element.
    /// - **Which id.** The first such id: the sources in order, then the target, the
    ///   reasoning, the structure, the `referencedArtifactElement` ids in order and the
    ///   group members in order.
    /// - **New ids only.** [`SetReferences`] and [`AddReference`] check only the ids the
    ///   end does not hold already, in list order. So a dangling id read from a file can
    ///   be kept while the end is edited.
    /// - **Fix.** Create the referenced element first, or correct the id. Ids are compared
    ///   exactly, case included.
    #[non_exhaustive]
    ReferenceNotFound {
        /// The end, by its XMI name, such as `target`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// A referenced id names an element of a kind its end's declared type does not admit
    /// (SACM-REF-002).
    ///
    /// - **Such as** a `structure` that is no `ArgumentPackage`, a source or target that
    ///   is no `ArgumentAsset`, a `reasoning` that is no `ArgumentReasoning`, a
    ///   `referencedArtifactElement` that is a `UtilityElement`, or a group member that is
    ///   no `ArgumentationElement`.
    /// - **No wrongly typed reference.** An edit never makes one (D-VAL-LAYER, ADR 0028).
    /// - **Which id.** The first such id, in the order of
    ///   [`ReferenceNotFound`](Self::ReferenceNotFound), once every id is known to name an
    ///   element.
    /// - **New ids only.** [`SetReferences`] and [`AddReference`] check only the ids the
    ///   end does not hold already.
    /// - **Fix.** Reference an element of the end's type instead.
    /// - **Narrower rules are content.** §11.15, §11.17 and §11.18 ask that the ends of an
    ///   `AssertedEvidence`, `AssertedArtifactSupport` or `AssertedArtifactContext` be
    ///   `ArtifactReference`s. [`validation::validate`](crate::validation::validate)
    ///   reports them (SACM23-VAL-016); no edit refuses them.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArgumentReasoning, CreateClaim, EditError};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let reasoning = CreateArgumentReasoning::new("p", LangString::new("R1")).with_structure("c1");
    /// let error = edit::apply(&mut doc, reasoning).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::WrongReferenceType {
    ///         end: "structure",
    ///         kind: ElementKind::Claim,
    ///         expected: "ArgumentPackage",
    ///         ..
    ///     }
    /// ));
    /// assert_eq!(
    ///     error.to_string(),
    ///     r#"the structure "c1" names a Claim, and the structure must be an ArgumentPackage"#
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    WrongReferenceType {
        /// The end, by its XMI name, such as `structure`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
        /// The kind of the element the id names.
        kind: ElementKind,
        /// The end's declared type, by its SACM class name, such as
        /// `ArgumentPackage` or `ArgumentAsset`.
        expected: &'static str,
    },
    /// The element to edit is not in the document (SACM-CMD-001).
    ///
    /// - **Which element.** The element of a set, reference, delete or move operation, the
    ///   parent of a [`ReorderElements::new`], or an id given to [`CopyElements::new`] or
    ///   [`CopyElements::root`] that the source lacks.
    /// - **Fix.** Correct the id. Ids are compared exactly, case included, and the `xmi:id`
    ///   of a name, `content`, `key`, `location` or `value` names no element.
    ///
    /// ```
    /// use sacm::edit::{self, EditError, SetName};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let error = edit::apply(&mut doc, SetName::new("c1", LangString::new("C1"))).unwrap_err();
    /// assert!(matches!(error, EditError::ElementNotFound { .. }));
    /// assert_eq!(error.to_string(), "the document has no element \"c1\" to edit");
    /// ```
    #[non_exhaustive]
    ElementNotFound {
        /// The element's id, as given.
        element: ElementId,
    },
    /// The element's kind has no such feature (SACM-CMD-008).
    ///
    /// | Feature | Refused on |
    /// |---|---|
    /// | `name` | a `UtilityElement` (§8.6 gives names to `ModelElement`s) |
    /// | `isCounter` | a kind that is no `AssertedRelationship` (§11.13) |
    /// | `assertionDeclaration` | a kind that is no Assertion (§11.10) |
    /// | `content` | a kind that is no `UtilityElement` (§8.7) |
    /// | `key` | a kind that is no `TaggedValue` (§8.12) |
    /// | `taggedValue` | a `UtilityElement`, which holds none (§8.6) |
    /// | a reference end | a kind that does not own it, such as a `source` on a Claim |
    ///
    /// - **Never here.** Every element has a `gid`, `isCitation`, `isAbstract`,
    ///   `citedElement` and `abstractForm`.
    /// - **Fix.** Edit an element whose kind has the feature.
    /// - **An end of the same name.** When the element has one, the Display names the
    ///   class whose end it lacks (ADR 0034 amendment 2). The `feature` is the XMI name
    ///   all the same.
    ///
    /// | Operation | Display |
    /// |---|---|
    /// | `source` on an `ArtifactAssetRelationship` | `the ArtifactAssetRelationship "rel" has no source of an AssertedRelationship` |
    /// | [`ArtifactTarget`](crate::model::ReferenceEnd::ArtifactTarget) on an `AssertedInference` | `the AssertedInference "i1" has no target of an ArtifactAssetRelationship` |
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetFlag};
    /// use sacm::model::{Document, ElementKind, Flag, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsCounter, true)).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::FeatureNotApplicable { kind: ElementKind::Claim, feature: "isCounter", .. }
    /// ));
    /// assert_eq!(error.to_string(), "the Claim \"c1\" has no isCounter");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    FeatureNotApplicable {
        /// The element's id.
        element: ElementId,
        /// The element's kind.
        kind: ElementKind,
        /// The feature, by its XMI name, such as `name`, `isCounter`, `assertionDeclaration`,
        /// `content`, `key` or `taggedValue`, or the name of a reference end, such as `source`.
        feature: &'static str,
    },
    /// A reference edit names a derived end (SACM-CMD-008): `ReferenceEnd::Expression`,
    /// which the element's `ExpressionLangStrings` hold. Set the name or text instead.
    ///
    /// - **Who refuses.** [`SetReferences`], [`AddReference`] and [`RemoveReference`], on
    ///   every kind, after an unknown element ([`ElementNotFound`](Self::ElementNotFound))
    ///   (SACM23-BASE-009, ADR 0038 D3).
    /// - **Why.** The end's ids are the `expression`s of the element's name and texts.
    /// - **Fix.** Set them with
    ///   [`LangString::with_expression`](crate::model::LangString::with_expression) and
    ///   [`SetName`], [`SetContent`], [`SetKey`] or [`SetLocation`].
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetReferences};
    /// use sacm::model::{Document, ElementKind, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let set = SetReferences::new("c1", ReferenceEnd::Expression, ["blade"]);
    /// let error = edit::apply(&mut doc, set).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::DerivedEnd { kind: ElementKind::Claim, end: "expression", .. }
    /// ));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the expression of the Claim \"c1\" is derived from its name and texts: set it with \
    ///      SetName, SetContent, SetKey or SetLocation"
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    DerivedEnd {
        /// The element's id.
        element: ElementId,
        /// The element's kind.
        kind: ElementKind,
        /// The end, by its XMI name: `expression`.
        end: &'static str,
    },
    /// [`RemoveReference`] names an id the end does not hold (SACM-CMD-001).
    ///
    /// Ids are compared exactly, case included. Remove an id the end holds.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, RemoveReference};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, RemoveReference::new("c1", ReferenceEnd::MetaClaim, "m1")).unwrap_err();
    /// assert!(matches!(error, EditError::ReferenceNotHeld { end: "metaClaim", .. }));
    /// assert_eq!(error.to_string(), "the metaClaim holds no id \"m1\" to remove");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    ReferenceNotHeld {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// [`RemoveReference::new_href`] names an href the end does not hold
    /// (SACM-CMD-001); hrefs are compared as strings.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, RemoveReference};
    /// use sacm::model::{Document, Href, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root("Arguments").with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", "C1").with_id("c1"))?;
    /// let href = Href::new("o.xmi#m1")?;
    /// let remove = RemoveReference::new_href("c1", ReferenceEnd::MetaClaim, href);
    /// let error = edit::apply(&mut doc, remove).unwrap_err();
    /// assert!(matches!(error, EditError::HrefNotHeld { end: "metaClaim", .. }));
    /// assert_eq!(error.to_string(), "the metaClaim holds no href \"o.xmi#m1\" to remove");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    HrefNotHeld {
        /// The end, by its XMI name, such as `participantPackage`.
        end: &'static str,
        /// The href, as given.
        href: Href,
    },
    /// [`ReorderElements`] names an id that is not a direct child of the parent, or, for
    /// [`ReorderElements::roots`], not a root (SACM-CMD-001).
    ///
    /// - **Such as** a grandchild, another element's child, the parent itself, the id of
    ///   a name, or an id the document does not hold.
    /// - **Which id.** The first such id, in list order.
    /// - **Fix.** Name only the parent's own children, or only roots.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, ReorderElements};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, ReorderElements::roots(["c1"])).unwrap_err();
    /// assert!(matches!(error, EditError::ChildNotFound { parent: None, .. }));
    /// assert_eq!(error.to_string(), "the document has no root \"c1\"");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    ChildNotFound {
        /// The parent's id, or `None` for the roots.
        parent: Option<ElementId>,
        /// The id, as given.
        child: ElementId,
    },
    /// [`ReorderElements`] does not give one order (SACM-CMD-009): its list is empty, or
    /// names an element more than once.
    ///
    /// - **Which id.** The first id named again, in list order.
    /// - **Fix.** Name each child, or each root, to move exactly once.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, ReorderElements};
    /// use sacm::model::{Document, ElementId, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    ///
    /// let empty = edit::apply(&mut doc, ReorderElements::new("p", Vec::<ElementId>::new())).unwrap_err();
    /// assert!(matches!(empty, EditError::InvalidOrder { repeated: None, .. }));
    /// assert_eq!(empty.to_string(), "the new order names no element");
    ///
    /// let twice = edit::apply(&mut doc, ReorderElements::new("p", ["c1", "c1"])).unwrap_err();
    /// assert!(matches!(twice, EditError::InvalidOrder { repeated: Some(ref id), .. } if id.as_str() == "c1"));
    /// assert_eq!(twice.to_string(), "the new order names \"c1\" more than once");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    InvalidOrder {
        /// The first id named more than once, or `None` for an empty list.
        repeated: Option<ElementId>,
    },
    /// A [`DeleteElement`] under [`ReferenceDeletePolicy::RejectIfReferenced`] would leave
    /// a reference dangling (SACM-CMD-002).
    ///
    /// - **Dangling.** An element outside the deleted element's subtree holds the id of a
    ///   deleted element.
    /// - **Which reference.** The first, in the order of [`Preview::dangling_references`].
    /// - **Fix.** Choose
    ///   [`DeleteReferencingRelationships`](ReferenceDeletePolicy::DeleteReferencingRelationships)
    ///   or [`ScrubReferences`](ReferenceDeletePolicy::ScrubReferences), or remove the
    ///   reference first.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement, EditError, SetReferences};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C2")).with_id("c2"))?;
    /// edit::apply(&mut doc, SetReferences::new("c2", ReferenceEnd::MetaClaim, ["c1"]))?;
    /// let error = edit::apply(&mut doc, DeleteElement::new("c1")).unwrap_err();
    /// assert!(matches!(error, EditError::Referenced { end: "metaClaim", .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the metaClaim of \"c2\" names \"c1\", and the delete would leave it dangling"
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    Referenced {
        /// The id of the element that holds the reference.
        holder: ElementId,
        /// The end, by its XMI name, such as `metaClaim`.
        end: &'static str,
        /// The id the reference names: an element the delete would remove.
        id: ElementId,
    },
    /// [`apply_preview`] was given a [`Preview`] taken at another revision of the document
    /// (SACM-CMD-003).
    ///
    /// - **Why.** The document changed after the preview was taken, or the preview is of
    ///   another document.
    /// - **Fix.** Nothing changed. Preview the operation again.
    #[non_exhaustive]
    PreviewExpired {
        /// The revision the preview was taken at.
        previewed: Revision,
        /// The document's revision now.
        current: Revision,
    },
    /// A [`DeleteElement`] under [`PackageDeletePolicy::RejectIfNonEmpty`] names an element
    /// that contains elements other than `UtilityElement`s (SACM-CMD-006).
    ///
    /// - **Not counted:** descriptions, implementation constraints, notes and tagged
    ///   values.
    /// - **Counted:** each child element of preserved content anywhere in the subtree
    ///   (ADR 0033).
    /// - **Fix.** Choose [`DeleteRecursively`](PackageDeletePolicy::DeleteRecursively), or
    ///   delete the contents first.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement, EditError};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")))?;
    /// let error = edit::apply(&mut doc, DeleteElement::new("p")).unwrap_err();
    /// assert!(matches!(error, EditError::NotEmpty { contained: 1, .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the element \"p\" contains 1 element that is no UtilityElement, and the delete is not recursive"
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    NotEmpty {
        /// The id of the element the delete would remove with its contents.
        ///
        /// It is the element to delete, or an interface or binding the delete removes
        /// because it is left below a bound (LIB-EDIT-019).
        element: ElementId,
        /// How many elements it contains, at any depth, that are not `UtilityElement`s.
        ///
        /// The preserved child elements of every element of its subtree, itself included,
        /// count too.
        contained: usize,
    },
    /// A [`DeleteElement`] would leave a reference dangling across a package boundary
    /// (SACM-CMD-007).
    ///
    /// - **Under** [`CrossPackageReferencePolicy::RejectIfExternalReferencesExist`].
    /// - **Across a boundary.** The holder's nearest package is not that of the element
    ///   the reference names.
    /// - **Which reference.** The first dangling reference a policy refuses, in the order
    ///   of [`Preview::dangling_references`].
    /// - **Fix.** Choose
    ///   [`ApplyReferencePolicy`](CrossPackageReferencePolicy::ApplyReferencePolicy) with
    ///   a reference policy that handles it, or remove the reference first.
    #[non_exhaustive]
    CrossPackageReference {
        /// The id of the element that holds the reference.
        holder: ElementId,
        /// The end, by its XMI name, such as `metaClaim`.
        end: &'static str,
        /// The id the reference names: an element the delete would remove.
        id: ElementId,
    },
    /// A [`MoveElement::root`] or a [`CopyElements::root`] would make a root of an element
    /// that is no package (SACM-CMD-005).
    ///
    /// Every root of a document is a package
    /// ([`ElementKind::is_package`](crate::model::ElementKind::is_package)). Move or copy
    /// the element under a package instead.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, MoveElement};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, MoveElement::root("c1")).unwrap_err();
    /// assert!(matches!(error, EditError::InvalidRoot { kind: ElementKind::Claim, .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "element \"c1\" of kind Claim cannot be a root, which must be a package"
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    InvalidRoot {
        /// The element's id; for a copy, its id in the source.
        element: ElementId,
        /// The element's kind.
        kind: ElementKind,
    },
    /// A [`MoveElement`] would move an element into itself or into an element it contains
    /// (SACM-CMD-005).
    ///
    /// Choose a parent outside the element's subtree.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, EditError, MoveElement};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("P")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::new("p", LangString::new("Q")).with_id("q"))?;
    /// let error = edit::apply(&mut doc, MoveElement::new("p", "q")).unwrap_err();
    /// assert!(matches!(error, EditError::ParentInSubtree { .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "element \"p\" cannot be moved into \"q\", which is the element itself or inside it"
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    ParentInSubtree {
        /// The id of the element to move.
        element: ElementId,
        /// The parent given: the element or an element inside it.
        parent: ElementId,
    },
    /// A [`MoveElement::with_position`] is past the end (SACM-CMD-009).
    ///
    /// - **The position** is the element's index after the move, from 0 to the number of
    ///   the parent's other children (or the other roots).
    /// - **Fix.** Give at most `children`, or leave the position out to move after the
    ///   last.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, MoveElement};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C2")).with_id("c2"))?;
    /// let error = edit::apply(&mut doc, MoveElement::new("c1", "p").with_position(2)).unwrap_err();
    /// assert!(matches!(error, EditError::PositionOutOfRange { position: 2, children: 1, .. }));
    /// assert_eq!(error.to_string(), "the position 2 is past the end: \"p\" has 1 other child");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    PositionOutOfRange {
        /// The parent's id, or `None` for the roots.
        parent: Option<ElementId>,
        /// The position given.
        position: usize,
        /// How many children the parent has besides the element (or how
        /// many other roots): the largest position allowed.
        children: usize,
    },
    /// A [`MoveElement::new`] or a [`CopyElements::new`] would make a child of an element
    /// that keeps a type attribute as preserved content (SACM-CMD-005, LIB-COMPAT-019).
    ///
    /// - **The attribute** is an `xmi:type` or `xsi:type` that a read kept on a root
    ///   because it names another class than the root's element name (D-ROOT-NAME,
    ///   ADR 0033).
    /// - **Why.** On a child a type attribute gives the class (XMI 2.5.1 rule 2g), so the
    ///   element could not be written with both.
    /// - **Fix.** Keep it a root, or move or copy its children instead.
    ///
    /// ```
    /// use sacm::edit::{self, EditError, MoveElement};
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:p="urn:example:profile">
    ///   <sacm:AssuranceCasePackage xmi:id="case"><name content="Case"/></sacm:AssuranceCasePackage>
    ///   <sacm:ArgumentPackage xmi:type="p:Bundle" xmi:id="a"><name content="A"/></sacm:ArgumentPackage>
    /// </xmi:XMI>"#;
    /// let mut doc = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let error = edit::apply(&mut doc, MoveElement::new("a", "case")).unwrap_err();
    /// assert!(matches!(error, EditError::KeptRootType { .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "element \"a\" keeps the type attribute xmi:type=\"p:Bundle\", which only a root can carry, so it cannot be a child"
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    KeptRootType {
        /// The element's id; for a copy, its id in the source.
        element: ElementId,
        /// The kept type attribute's name as held, such as `xmi:type`.
        attribute: String,
        /// Its value exactly as held, such as `p:Bundle`.
        value: String,
    },
    /// A text the operation would store holds a character XML 1.0 cannot represent
    /// (SACM-XML-003, LIB-EDIT-025).
    ///
    /// - **Which characters.** Those outside XML's `Char` production, such as U+0000,
    ///   another C0 control other than TAB, LF and CR, U+FFFE or U+FFFF.
    /// - **Why.** No file can carry it, so no edit stores it. Remove the character.
    /// - **Last.** Every other refusal comes first.
    /// - **Writer's order.** The texts are checked in the order
    ///   [`xmi::write`](crate::xmi::write) checks them.
    /// - **First character.** The first such character of the first text holding one is
    ///   named, as
    ///   [`WriteError::UnrepresentableCharacter`](crate::xmi::WriteError::UnrepresentableCharacter)
    ///   names it.
    /// - **Only what is stored.** A remove, or a set of the value held, is not checked.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, EditError};
    /// use sacm::model::{Document, LangString};
    /// use sacm::xmi::WriteAttribute;
    ///
    /// let mut doc = Document::new();
    /// let name = LangString::new("bad\u{1}");
    /// let error = edit::apply(&mut doc, CreateArgumentPackage::root(name).with_id("arguments"))
    ///     .unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::UnrepresentableCharacter {
    ///         attribute: WriteAttribute::NameContent,
    ///         character: '\u{1}',
    ///         position: None,
    ///         ..
    ///     }
    /// ));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the name's content given to the element \"arguments\" holds U+0001, which XML 1.0 \
    ///      cannot represent"
    /// );
    /// assert_eq!(doc.element_count(), 0); // unchanged
    /// ```
    #[non_exhaustive]
    UnrepresentableCharacter {
        /// The element that would hold the text.
        ///
        /// - **A set:** the element it changes.
        /// - **A create:** the new element, with its given id or the id it would be
        ///   generated.
        /// - **[`SetTaggedValue`]:** the tagged value it sets or would create.
        element: ElementId,
        /// The first such character.
        character: char,
        /// The text that holds it.
        attribute: crate::xmi::WriteAttribute,
        /// The value's index, from 0, for the six *Value* attributes; `None`
        /// otherwise.
        position: Option<usize>,
    },
}

impl EditError {
    /// The catalog code of the refusal.
    ///
    /// | Code | Refusals |
    /// |---|---|
    /// | SACM-CMD-001 | [`ParentNotFound`](Self::ParentNotFound), [`ReferenceNotFound`](Self::ReferenceNotFound), [`ElementNotFound`](Self::ElementNotFound), [`ReferenceNotHeld`](Self::ReferenceNotHeld), [`HrefNotHeld`](Self::HrefNotHeld), [`ChildNotFound`](Self::ChildNotFound) |
    /// | SACM-CMD-002 | [`Referenced`](Self::Referenced) |
    /// | SACM-CMD-003 | [`PreviewExpired`](Self::PreviewExpired) |
    /// | SACM-CMD-004 | [`DuplicateId`](Self::DuplicateId) |
    /// | SACM-CMD-005 | [`InvalidParent`](Self::InvalidParent), [`InvalidRoot`](Self::InvalidRoot), [`ParentInSubtree`](Self::ParentInSubtree), [`KeptRootType`](Self::KeptRootType) |
    /// | SACM-CMD-006 | [`NotEmpty`](Self::NotEmpty) |
    /// | SACM-CMD-007 | [`CrossPackageReference`](Self::CrossPackageReference) |
    /// | SACM-CMD-008 | [`FeatureNotApplicable`](Self::FeatureNotApplicable), [`DerivedEnd`](Self::DerivedEnd) |
    /// | SACM-CMD-009 | [`InvalidOrder`](Self::InvalidOrder), [`PositionOutOfRange`](Self::PositionOutOfRange) |
    /// | SACM-ID-002 | [`InvalidId`](Self::InvalidId), [`InvalidReference`](Self::InvalidReference) |
    /// | SACM-MULT-001 | [`TooFewReferences`](Self::TooFewReferences), [`TooManyReferences`](Self::TooManyReferences) |
    /// | SACM-REF-002 | [`WrongReferenceType`](Self::WrongReferenceType) |
    /// | SACM-XML-003 | [`UnrepresentableCharacter`](Self::UnrepresentableCharacter) |
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::ParentNotFound { .. }
            | Self::ReferenceNotFound { .. }
            | Self::ElementNotFound { .. }
            | Self::ReferenceNotHeld { .. }
            | Self::HrefNotHeld { .. }
            | Self::ChildNotFound { .. } => Code::CMD_TARGET_NOT_FOUND,
            Self::InvalidParent { .. }
            | Self::InvalidRoot { .. }
            | Self::ParentInSubtree { .. }
            | Self::KeptRootType { .. } => Code::CMD_INVALID_PARENT,
            Self::InvalidId { .. } | Self::InvalidReference { .. } => Code::ID_INVALID,
            Self::DuplicateId { .. } => Code::CMD_DUPLICATE_ID,
            Self::TooFewReferences { .. } | Self::TooManyReferences { .. } => {
                Code::MULTIPLICITY_VIOLATION
            }
            Self::WrongReferenceType { .. } => Code::REF_WRONG_TYPE,
            Self::FeatureNotApplicable { .. } | Self::DerivedEnd { .. } => {
                Code::CMD_FEATURE_NOT_APPLICABLE
            }
            Self::InvalidOrder { .. } | Self::PositionOutOfRange { .. } => Code::CMD_INVALID_ORDER,
            Self::Referenced { .. } => Code::CMD_DELETE_REFERENCED,
            Self::PreviewExpired { .. } => Code::CMD_PREVIEW_EXPIRED,
            Self::NotEmpty { .. } => Code::CMD_PACKAGE_NOT_EMPTY,
            Self::CrossPackageReference { .. } => Code::CMD_EXTERNAL_REFERENCES,
            Self::UnrepresentableCharacter { .. } => Code::XML_UNREPRESENTABLE_CHARACTER,
        }
    }
}

/// The class that owns the reference end named `feature` that an element of
/// `kind` lacks, when the kind has another end of that name: the
/// `FeatureNotApplicable` Display names it, so that `the AssertedInference
/// "i1" has no target` does not read as if the inference had no target
/// (ADR 0034 amendment 2). `None` when the kind has no end of that name.
fn other_class_of_end(kind: ElementKind, feature: &str) -> Option<&'static str> {
    let named = || {
        crate::model::ReferenceEnd::ALL
            .iter()
            .filter(move |end| end.name() == feature)
    };
    if !named().any(|end| end.applies_to(kind)) {
        return None;
    }
    named()
        .find(|end| !end.applies_to(kind))
        .map(|end| end.first_owner().name())
}

impl fmt::Display for EditError {
    /// Writes the refusal on one line, as described on [`EditError`].
    #[expect(clippy::too_many_lines, reason = "one arm per EditError variant")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentNotFound { parent } => write!(
                f,
                "the document has no element {:?} to be the parent",
                parent.as_str()
            ),
            Self::InvalidParent {
                parent,
                parent_kind,
                kind,
            } => write!(
                f,
                "element {:?} of kind {parent_kind} cannot contain an element of kind {kind}",
                parent.as_str()
            ),
            Self::InvalidId { id } => {
                write!(f, "the element id {:?} is not an XML NCName", id.as_str())
            }
            Self::DuplicateId { id } => write!(
                f,
                "the document already has an element with id {:?}",
                id.as_str()
            ),
            Self::TooFewReferences {
                end,
                given,
                required,
            } => write!(
                f,
                "the {end} is given {given} ids, and SACM requires at least {required}"
            ),
            Self::TooManyReferences {
                end,
                given,
                allowed,
            } => write!(
                f,
                "the {end} is given {given} ids, and SACM allows at most {allowed}"
            ),
            Self::ElementNotFound { element } => write!(
                f,
                "the document has no element {:?} to edit",
                element.as_str()
            ),
            Self::FeatureNotApplicable {
                element,
                kind,
                feature,
            } => {
                write!(f, "the {kind} {:?} has no {feature}", element.as_str())?;
                match other_class_of_end(*kind, feature) {
                    Some(owner) => write!(f, " of {} {owner}", article(owner)),
                    None => Ok(()),
                }
            }
            Self::DerivedEnd { element, kind, end } => write!(
                f,
                "the {end} of the {kind} {:?} is derived from its name and texts: set it with \
                 SetName, SetContent, SetKey or SetLocation",
                element.as_str()
            ),
            Self::ReferenceNotHeld { end, id } => {
                write!(f, "the {end} holds no id {:?} to remove", id.as_str())
            }
            Self::HrefNotHeld { end, href } => {
                write!(f, "the {end} holds no href {:?} to remove", href.as_str())
            }
            Self::ChildNotFound {
                parent: Some(parent),
                child,
            } => write!(
                f,
                "the element {:?} has no child {:?}",
                parent.as_str(),
                child.as_str()
            ),
            Self::ChildNotFound {
                parent: None,
                child,
            } => write!(f, "the document has no root {:?}", child.as_str()),
            Self::InvalidOrder { repeated: None } => f.write_str("the new order names no element"),
            Self::InvalidOrder {
                repeated: Some(repeated),
            } => write!(
                f,
                "the new order names {:?} more than once",
                repeated.as_str()
            ),
            Self::InvalidReference { end, id } => write!(
                f,
                "the {end} {:?} holds white space, so it cannot be written as one id",
                id.as_str()
            ),
            Self::ReferenceNotFound { end, id } => write!(
                f,
                "the document has no element {:?} for the {end} to reference",
                id.as_str()
            ),
            Self::WrongReferenceType {
                end,
                id,
                kind,
                expected,
            } => write!(
                f,
                "the {end} {:?} names {} {kind}, and the {end} must be {} {expected}",
                id.as_str(),
                article(kind.name()),
                article(expected)
            ),
            Self::Referenced { holder, end, id } => write!(
                f,
                "the {end} of {:?} names {:?}, and the delete would leave it dangling",
                holder.as_str(),
                id.as_str()
            ),
            Self::PreviewExpired { .. } => {
                f.write_str("the document has changed since the preview was taken")
            }
            Self::NotEmpty { element, contained } => write!(
                f,
                "the element {:?} contains {contained} {} that {} no UtilityElement, and the \
                 delete is not recursive",
                element.as_str(),
                if *contained == 1 {
                    "element"
                } else {
                    "elements"
                },
                if *contained == 1 { "is" } else { "are" }
            ),
            Self::CrossPackageReference { holder, end, id } => write!(
                f,
                "the {end} of {:?} names {:?} in another package, and the delete would leave it \
                 dangling",
                holder.as_str(),
                id.as_str()
            ),
            Self::InvalidRoot { element, kind } => write!(
                f,
                "element {:?} of kind {kind} cannot be a root, which must be a package",
                element.as_str()
            ),
            Self::ParentInSubtree { element, parent } => write!(
                f,
                "element {:?} cannot be moved into {:?}, which is the element itself or inside it",
                element.as_str(),
                parent.as_str()
            ),
            Self::PositionOutOfRange {
                parent,
                position,
                children,
            } => {
                write!(f, "the position {position} is past the end: ")?;
                let (noun, plural) = if let Some(parent) = parent {
                    write!(f, "{:?} has", parent.as_str())?;
                    ("child", "children")
                } else {
                    f.write_str("the document has")?;
                    ("root", "roots")
                };
                let noun = if *children == 1 { noun } else { plural };
                write!(f, " {children} other {noun}")
            }
            Self::KeptRootType {
                element,
                attribute,
                value,
            } => write!(
                f,
                "element {:?} keeps the type attribute {attribute}={value:?}, which only a root \
                 can carry, so it cannot be a child",
                element.as_str()
            ),
            Self::UnrepresentableCharacter {
                element,
                character,
                attribute,
                position,
            } => {
                write!(f, "the {attribute}")?;
                if let Some(position) = position {
                    write!(f, " at index {position}")?;
                }
                write!(
                    f,
                    " given to the element {:?} holds U+{:04X}, which XML 1.0 cannot represent",
                    element.as_str(),
                    u32::from(*character)
                )
            }
        }
    }
}

impl std::error::Error for EditError {}

/// The indefinite article before the SACM class name `name`: `an` before a
/// vowel, `a` otherwise.
fn article(name: &str) -> &'static str {
    if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}
