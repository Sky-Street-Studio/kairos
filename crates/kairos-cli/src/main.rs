use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kairos_cli::{Project, run_headless};

#[derive(Parser)]
#[command(name = "kairos", version, about = "Kairos visual novel engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report script errors.
    Check { project: PathBuf },
    /// Play a project. Only `--headless` exists until the player lands.
    Run {
        project: PathBuf,
        #[arg(long)]
        headless: bool,
        /// File of 1-based menu choices, one per line.
        #[arg(long)]
        inputs: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let (Command::Check { project } | Command::Run { project, .. }) = &cli.command;
    let project = Project::load(project)?;
    for (file, d) in &project.diagnostics {
        eprintln!("{}:{}: {}", file.display(), d.line, d.message);
    }
    if !project.diagnostics.is_empty() {
        return Ok(ExitCode::FAILURE);
    }
    match cli.command {
        Command::Check { .. } => println!("{}: no problems found", project.manifest.name),
        Command::Run { headless: false, .. } => {
            return Err("the graphical player is not built yet; pass --headless".into());
        }
        Command::Run { inputs, .. } => {
            let inputs = inputs.map(std::fs::read_to_string).transpose()?.unwrap_or_default();
            print!("{}", run_headless(&project, &inputs)?);
        }
    }
    Ok(ExitCode::SUCCESS)
}
