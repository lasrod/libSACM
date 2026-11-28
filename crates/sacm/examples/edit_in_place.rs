//! Reads a document, edits it (a name, one language value of a description,
//! a tagged value), shows one refused edit, and prints the saved XMI.
//!
//! Run it with `cargo run -p sacm --example edit_in_place`.

use std::io::{self, Write as _};

use sacm::edit::{self, DeleteElement, Mutation, SetContent, SetName, SetTaggedValue};
use sacm::model::{ElementId, LangString, SacmElement};
use sacm::xmi::{self, ReadOptions, WriteOptions};

/// The document to edit. A program that edits a file reads its bytes with
/// `std::fs::read`, and saves the text it gets with `std::fs::write`.
const INPUT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
  <name lang="en" content="Arguments"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
    <name lang="en" content="The brake stops the vehicle"/>
    <description xmi:id="d1">
      <content>
        <value lang="en" content="Within 40 m from 100 km/h."/>
        <value lang="sv" content="Inom 40 m."/>
      </content>
    </description>
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
    <name lang="en" content="The brake passed its tests"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2" target="c1">
    <name lang="en" content="Tested, so safe"/>
  </argumentElement>
</sacm:ArgumentPackage>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stdout().lock();
    let mut document = xmi::read(INPUT.as_bytes(), &ReadOptions::default())?.into_document();

    // 1. Rename the Claim. Every edit is an operation given to `edit::apply`,
    //    which answers with the ids it created, changed and deleted.
    let name = LangString::new("The brake stops the vehicle in time").with_lang("en");
    let renamed = edit::apply(&mut document, SetName::new("c1", name))?;
    writeln!(out, "1. set the name of c1: {}", summary(&renamed))?;

    // 2. Set one language value of the Description. A content is replaced
    //    whole, so take the one held, change the Swedish value in place and
    //    set the result. The other values, and every id, stay as they are.
    let mut content = document
        .get("d1")
        .and_then(SacmElement::content)
        .cloned()
        .ok_or("the Description d1 has no content")?;
    let swedish = content
        .iter_mut()
        .find(|value| value.lang() == Some("sv"))
        .ok_or("the Description d1 has no Swedish value")?;
    swedish.set_content(Some("Inom 40 m från 100 km/h.".to_owned()));
    let described = edit::apply(&mut document, SetContent::new("d1", content))?;
    writeln!(
        out,
        "2. set the Swedish value of d1: {}",
        summary(&described)
    )?;

    // 3. Set a tagged value by its key. The Claim has none with this key, so
    //    one is created.
    let tagged = edit::apply(
        &mut document,
        SetTaggedValue::new("c1", "status", "reviewed"),
    )?;
    writeln!(
        out,
        "3. set the tagged value \"status\" of c1: {}",
        summary(&tagged)
    )?;

    // 4. A refused edit. The AssertedInference i1 names c2 as its source, and
    //    a delete refuses by default to remove an element that is referenced.
    //    The refusal has a stable code, and the document is as it was.
    let before = document.clone();
    match edit::apply(&mut document, DeleteElement::new("c2")) {
        Ok(deleted) => writeln!(out, "4. delete c2: {}", summary(&deleted))?,
        Err(refusal) => writeln!(out, "4. delete c2: refused, {}: {refusal}", refusal.code())?,
    }
    let unchanged = if document == before { "yes" } else { "no" };
    writeln!(out, "   the document is unchanged: {unchanged}")?;

    // Save, in strict mode by default, and print the result.
    let text = xmi::write(&document, &WriteOptions::default())?;
    writeln!(out)?;
    write!(out, "{text}")?;
    if !text.ends_with('\n') {
        writeln!(out)?;
    }
    Ok(())
}

/// What `mutation` did, on one line: the ids it created, changed and deleted.
fn summary(mutation: &Mutation) -> String {
    let ids = |ids: &[ElementId]| {
        let ids: Vec<&str> = ids.iter().map(ElementId::as_str).collect();
        format!("[{}]", ids.join(", "))
    };
    format!(
        "created {}, changed {}, deleted {}",
        ids(mutation.created_ids()),
        ids(mutation.changed_ids()),
        ids(mutation.deleted_ids())
    )
}
