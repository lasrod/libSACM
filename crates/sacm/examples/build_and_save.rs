//! Builds a small assurance case in code and writes it as XMI to stdout.
//!
//! Run it with `cargo run -p sacm --example build_and_save`.

use std::io::{self, Write as _};

use sacm::edit::{
    self, CreateArgumentPackage, CreateArtifact, CreateArtifactPackage, CreateArtifactReference,
    CreateAssertedEvidence, CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim,
    CreateDescription,
};
use sacm::model::{Document, LangString};
use sacm::xmi::{self, WriteOptions};
use sacm::{diagnostic, validation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = |text: &str| LangString::new(text).with_lang("en");
    let mut document = Document::new();

    // The packages: an AssuranceCasePackage that holds one ArgumentPackage
    // and one ArtifactPackage. An id given with `with_id` is kept; without
    // it the id is generated.
    let case = CreateAssuranceCasePackage::root(name("Braking case")).with_id("case");
    edit::apply(&mut document, case)?;
    let arguments = CreateArgumentPackage::new("case", name("Arguments")).with_id("arguments");
    edit::apply(&mut document, arguments)?;
    let artifacts = CreateArtifactPackage::new("case", name("Artifacts")).with_id("artifacts");
    edit::apply(&mut document, artifacts)?;

    // The evidence: an Artifact, and the ArtifactReference that cites it in
    // the argument.
    let report = CreateArtifact::new("artifacts", name("Brake test report"))
        .with_id("report")
        .with_version("1.2")
        .with_date("2026-03-01T00:00:00Z");
    edit::apply(&mut document, report)?;
    let cited = CreateArtifactReference::new("arguments", name("The brake test report"))
        .with_id("report_ref")
        .with_referenced_artifact_elements(["report"]);
    edit::apply(&mut document, cited)?;

    // The claims, one with a Description.
    let top = CreateClaim::new("arguments", name("The brake stops the vehicle")).with_id("c1");
    edit::apply(&mut document, top)?;
    let tested = CreateClaim::new("arguments", name("The brake passed its tests")).with_id("c2");
    edit::apply(&mut document, tested)?;
    let text = name("Within 40 m from 100 km/h on a dry road.");
    edit::apply(&mut document, CreateDescription::new("c1", text))?;

    // The asserted relationships: c2 supports c1, and the report supports c2.
    // Each takes its sources and its target when it is created.
    let inference =
        CreateAssertedInference::new("arguments", name("Tested, so safe"), ["c2"], "c1");
    edit::apply(&mut document, inference)?;
    let evidence = CreateAssertedEvidence::new(
        "arguments",
        name("The report shows it"),
        ["report_ref"],
        "c2",
    );
    edit::apply(&mut document, evidence)?;

    // Validate before handing the file on: a strict write checks the schema,
    // not the SACM rules.
    let findings = validation::validate(&document);
    if diagnostic::has_errors(&findings) {
        return Err("the case that was built does not conform to SACM 2.3".into());
    }

    // Write, in strict mode by default. A program that saves a file gives the
    // text to `std::fs::write`; this one prints it, and nothing else.
    let text = xmi::write(&document, &WriteOptions::default())?;
    let mut out = io::stdout().lock();
    write!(out, "{text}")?;
    if !text.ends_with('\n') {
        writeln!(out)?;
    }
    Ok(())
}
