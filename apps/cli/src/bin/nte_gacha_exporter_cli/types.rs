#[cfg(windows)]
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use clap::{Args, CommandFactory, Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use nte_assets::{
    AssetPackBuildOptions, DEFAULT_WEBP_QUALITY, build_asset_maps, build_assets_pack,
    find_assets_root,
};
use nte_automation::{AutoPageOptions, AutoPageStatus, run_auto_page};
use nte_capture::{
    CaptureOptions, CaptureProgress, CaptureRecordBuilder, ParsedRow, build_capture_document,
    candidate_ports, capture_doctor, capture_live, find_process_pid, is_admin, read_raw_capture,
};
use nte_core::available_locales;
use nte_store::JsonStore;

const DEFAULT_LOCALE: &str = "zh-Hant";
const EXE_NAME: &str = "HTGame.exe";

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", error.message);
            error.code
        }
    };
    std::process::exit(code);
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    match cli.command {
        Some(Command::Replay(args)) => replay(args),
        Some(Command::Capture(args)) => capture(args),
        Some(Command::Doctor) => doctor(),
        Some(Command::Maps { command }) => match command {
            MapsCommand::List => {
                for locale in available_locales() {
                    println!("{locale}");
                }
                Ok(())
            }
            MapsCommand::Build(args) => maps_build(args),
        },
        Some(Command::Assets { command }) => match command {
            AssetsCommand::Pack { command } => match command {
                AssetsPackCommand::Build(args) => assets_pack_build(args),
            },
        },
        None => {
            let mut command = Cli::command();
            command.print_help().map_err(CliError::from_error)?;
            println!();
            Err(CliError::new(2, "command is required"))
        }
    }
}

#[derive(Parser)]
#[command(
    name = "nte-gacha-exporter-cli",
    about = "NTE gacha history exporter runtime"
)]
struct Cli {
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Replay(ReplayArgs),
    Capture(CaptureArgs),
    Doctor,
    Maps {
        #[command(subcommand)]
        command: MapsCommand,
    },
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
}

#[derive(Subcommand)]
enum MapsCommand {
    List,
    Build(MapBuildArgs),
}

#[derive(Subcommand)]
enum AssetsCommand {
    Pack {
        #[command(subcommand)]
        command: AssetsPackCommand,
    },
}

#[derive(Subcommand)]
enum AssetsPackCommand {
    Build(AssetsPackBuildArgs),
}

#[derive(Args)]
struct MapBuildArgs {
    #[arg(long)]
    assets_root: Option<PathBuf>,
    #[arg(long)]
    locale: Option<String>,
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[derive(Args)]
struct AssetsPackBuildArgs {
    #[arg(long)]
    assets_root: Option<PathBuf>,
    #[arg(long)]
    maps_dir: Option<PathBuf>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = DEFAULT_WEBP_QUALITY)]
    quality: u8,
}

#[derive(Args)]
struct OutputArgs {
    #[arg(long)]
    json: Option<PathBuf>,
    #[arg(long)]
    csv: Option<PathBuf>,
    #[arg(long, default_value = DEFAULT_LOCALE)]
    locale: String,
}

#[derive(Args)]
struct ReplayArgs {
    raw_jsonl: PathBuf,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Args)]
struct CaptureArgs {
    #[command(flatten)]
    output: OutputArgs,
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    output_raw: Option<PathBuf>,
    #[arg(long)]
    pid: Option<u32>,
    #[arg(long, action = clap::ArgAction::SetTrue)]
    auto_page: bool,
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    verbose: bool,
}
