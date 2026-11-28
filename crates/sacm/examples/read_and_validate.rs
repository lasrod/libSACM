//! Reads an XMI document in strict and in tolerant mode, validates it, and
//! prints each diagnostic with its code and position.
//!
//! Run it with `cargo run -p sacm --example read_and_validate`.

use std::io::{self, Write};

use sacm::diagnostic::{self, Diagnostic};
use sacm::model::ElementId;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions};

/// The document to read. A program that opens a file reads its bytes with
/// `std::fs::read` instead.
///
/// It has three faults:
///
/// - **`reviewer`.** The Claim `c1` has an attribute that is not SACM 2.3.
/// - **No name.** The Claim `c2` has no name.
/// - **`c9`.** The `AssertedInference` names a source the document lacks.
const INPUT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name lang="en" content="Braking case"/>
  <argumentPackage xmi:id="arguments">
    <name lang="en" content="Arguments"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1" reviewer="QA">
      <name lang="en" content="The brake stops the vehicle"/>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"/>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2 c9" target="c1">
      <name lang="en" content="Tested, so safe"/>
    </argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::stdout().lock();
    for mode in [Mode::Strict, Mode::Tolerant] {
        writeln!(out, "== {mode} read ==")?;
        report(&mut out, mode)?;
        writeln!(out)?;
    }
    Ok(())
}

/// Reads [`INPUT`] in `mode`, validates what was read, and writes what both
/// found to `out`.
fn report(out: &mut impl Write, mode: Mode) -> io::Result<()> {
    let options = ReadOptions::default().with_mode(mode);
    let output = match xmi::read(INPUT.as_bytes(), &options) {
        Ok(output) => output,
        // A refusal has a code too, and a position when one place is at fault.
        Err(refusal) => {
            write!(out, "refused: {}", refusal.code())?;
            if let Some(location) = refusal.location() {
                write!(out, " at {location}")?;
            }
            return writeln!(out, ": {refusal}");
        }
    };

    // What the read found: content that is not SACM 2.3 where it stands.
    writeln!(out, "read: {} diagnostic(s)", output.diagnostics().len())?;
    for found in output.diagnostics() {
        write_diagnostic(out, found)?;
    }

    // What the SACM 2.3 rules say of the model. A read never runs them.
    let findings = validation::validate(output.document());
    writeln!(out, "validate: {} diagnostic(s)", findings.len())?;
    for found in &findings {
        write_diagnostic(out, found)?;
    }

    // A file conforms when a strict read and `validate` both find no Error.
    let conforms =
        mode == Mode::Strict && !output.has_errors() && !diagnostic::has_errors(&findings);
    writeln!(out, "conforms: {}", if conforms { "yes" } else { "no" })
}

/// Writes one diagnostic: its severity, code, position and affected ids on
/// one line, then its message.
fn write_diagnostic(out: &mut impl Write, found: &Diagnostic) -> io::Result<()> {
    write!(out, "  {} {}", found.severity(), found.code())?;
    if let Some(location) = found.location() {
        write!(out, " at {location}")?;
    }
    let ids: Vec<&str> = found.affected().iter().map(ElementId::as_str).collect();
    if !ids.is_empty() {
        write!(out, " [{}]", ids.join(", "))?;
    }
    writeln!(out)?;
    writeln!(out, "    {}", found.message())
}
