use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use kairos_cli::{Project, run_headless, update_translation_file};

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
    /// Create or update a language translation file.
    Translate {
        project: PathBuf,
        #[arg(long)]
        lang: String,
    },
    /// Play a project in a window, or as a transcript with `--headless`.
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
    let root = match &cli.command {
        Command::Check { project } | Command::Run { project, .. } | Command::Translate { project, .. } => project,
    };
    let project = Project::load(root)?;
    for (file, d) in &project.diagnostics {
        eprintln!("{}:{}: {}", file.display(), d.line, d.message);
    }
    let mut has_errors = !project.diagnostics.is_empty();
    if matches!(&cli.command, Command::Check { .. }) {
        for (path, message) in project.asset_diagnostics()? {
            eprintln!("{}: {message}", path.display());
            has_errors = true;
        }
    }
    if has_errors {
        return Ok(ExitCode::FAILURE);
    }
    match cli.command {
        Command::Check { .. } => {
            for (lang, ids) in &project.stale_translations {
                eprintln!("warning: tl/{lang}.toml: {} stale translation(s)", ids.len());
            }
            println!("{}: no problems found", project.manifest.name);
        }
        Command::Translate { lang, .. } => {
            update_translation_file(&project, &lang)?;
            println!("updated tl/{lang}.toml");
        }
        Command::Run { headless: false, .. } => kairos_player::run(project)?,
        Command::Run { inputs, .. } => {
            let inputs = inputs.map(std::fs::read_to_string).transpose()?.unwrap_or_default();
            print!("{}", run_headless(&project, &inputs)?);
        }
    }
    Ok(ExitCode::SUCCESS)
}
