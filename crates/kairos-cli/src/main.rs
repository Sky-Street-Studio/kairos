use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use kairos_cli::{Project, run_headless, update_translation_file};
use kairos_project::routes::{RouteTestOptions, test_routes};

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
    /// Export a checked project for the web or a desktop platform.
    Export {
        project: PathBuf,
        #[arg(long, value_enum)]
        target: ExportTarget,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        player_pkg: Option<PathBuf>,
        #[arg(long)]
        player_binary: Option<PathBuf>,
        #[arg(long)]
        pack_assets: bool,
    },
    /// Explore every reachable route and report problems.
    TestRoutes {
        project: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value_t = 1000)]
        max_depth: usize,
        #[arg(long, default_value_t = 3)]
        max_loop_revisits: usize,
        #[arg(long, default_value_t = 10000)]
        max_routes: usize,
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
    #[value(name = "windows-x64")]
    WindowsX64,
    #[value(name = "linux-x64")]
    LinuxX64,
    Macos,
}

fn is_host_target(target: kairos_project::export::ExportTarget) -> bool {
    match target {
        kairos_project::export::ExportTarget::Desktop => true,
        kairos_project::export::ExportTarget::WindowsX64 => cfg!(all(target_os = "windows", target_arch = "x86_64")),
        kairos_project::export::ExportTarget::LinuxX64 => cfg!(all(target_os = "linux", target_arch = "x86_64")),
        kairos_project::export::ExportTarget::MacOS => cfg!(target_os = "macos"),
        kairos_project::export::ExportTarget::Web => false,
    }
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
    if let Command::Export { project, target, out, player_pkg, player_binary, pack_assets } = &cli.command {
        let target = match target {
            ExportTarget::Web => kairos_project::export::ExportTarget::Web,
            ExportTarget::Desktop => kairos_project::export::ExportTarget::Desktop,
            ExportTarget::WindowsX64 => kairos_project::export::ExportTarget::WindowsX64,
            ExportTarget::LinuxX64 => kairos_project::export::ExportTarget::LinuxX64,
            ExportTarget::Macos => kairos_project::export::ExportTarget::MacOS,
        };
        let player = match player_binary {
            Some(path) => Some(path.clone()),
            None if is_host_target(target) => Some(std::env::current_exe()?),
            None => None,
        };
        let result = kairos_project::export::export(
            project,
            out,
            target,
            player.as_deref(),
            player_pkg.as_deref(),
            *pack_assets,
        )?;
        println!("exported to {}", result.display());
        return Ok(ExitCode::SUCCESS);
    }
    let root = match &cli.command {
        Command::Check { project }
        | Command::TestRoutes { project, .. }
        | Command::Run { project, .. }
        | Command::Translate { project, .. } => project,
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
        Command::TestRoutes { json, max_depth, max_loop_revisits, max_routes, .. } => {
            let report = test_routes(&project, RouteTestOptions { max_depth, max_loop_revisits, max_routes })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for label in &report.unreachable_labels {
                    println!("unreachable label: {label}");
                }
                for issue in report.dead_ends.iter().chain(&report.missing_assets).chain(&report.unresolved_choices) {
                    println!("{}:{}: {}", issue.label, issue.line, issue.message);
                }
                for cap in &report.truncated {
                    println!("truncated: {cap}");
                }
                if !report.has_problems() {
                    println!(
                        "{}: {} routes explored, no problems found",
                        project.manifest.name, report.routes_explored
                    );
                }
            }
            has_errors = report.has_problems();
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
    Ok(if has_errors { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}
