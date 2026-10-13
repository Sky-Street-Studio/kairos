use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
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
    /// Export a checked project for the web or host desktop.
    Export {
        project: PathBuf,
        #[arg(long, value_enum)]
        target: ExportTarget,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        player_pkg: Option<PathBuf>,
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

#[derive(Clone, Copy, ValueEnum)]
enum ExportTarget {
    Web,
    Desktop,
}

fn main() -> ExitCode {
    if std::env::args_os().len() == 1 {
        return match kairos_player::run_bundled() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if let Command::Export { project, target, out, player_pkg } = &cli.command {
        let target = match target {
            ExportTarget::Web => kairos_project::export::ExportTarget::Web,
            ExportTarget::Desktop => kairos_project::export::ExportTarget::Desktop,
        };
        let player = std::env::current_exe()?;
        let result = kairos_project::export::export(project, out, target, Some(&player), player_pkg.as_deref())?;
        println!("exported to {}", result.display());
        return Ok(ExitCode::SUCCESS);
    }
    let root = match &cli.command {
        Command::Check { project } | Command::Run { project, .. } | Command::Translate { project, .. } => project,
        Command::Export { .. } => unreachable!(),
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
        Command::Export { .. } => unreachable!(),
    }
    Ok(ExitCode::SUCCESS)
}
