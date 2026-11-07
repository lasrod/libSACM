//! `sacm` — inspect, validate and convert OMG SACM 2.3 assurance cases.
//!
//! The command-line front end to the [`sacm`] library. Subcommands are added
//! as the library gains the capability behind them; see the functionality
//! matrix (`docs/matrix/functionality-matrix.md`) for what is planned.

use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Inspect, validate and convert OMG SACM 2.3 assurance cases.
#[derive(Debug, Parser)]
#[command(name = "sacm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the tool version and the SACM revision it implements.
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        // A closed pipe (`sacm version | head -0`) is not an error worth reporting.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "sacm: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli, out: &mut impl Write) -> io::Result<()> {
    match cli.command {
        Command::Version => writeln!(out, "sacm {} (SACM {})", sacm::VERSION, sacm::SACM_VERSION),
    }
}
