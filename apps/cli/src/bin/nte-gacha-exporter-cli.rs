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
use nte_assets::{build_asset_maps, find_assets_root};
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
}

#[derive(Subcommand)]
enum MapsCommand {
    List,
    Build(MapBuildArgs),
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

fn replay(args: ReplayArgs) -> CliResult<()> {
    let defaults = DefaultPaths::new();
    let json = args.output.json.clone().unwrap_or(defaults.json);
    let csv = args.output.csv.clone().unwrap_or(defaults.csv);
    export_raw_replay(&args.raw_jsonl, &args.output.locale, &json, Some(&csv))?;
    println!("records={}", count_public_records(&json)?);
    print_paths(&json, Some(&csv), None);
    Ok(())
}

fn capture(args: CaptureArgs) -> CliResult<()> {
    if relaunch_capture_as_admin()? {
        return Ok(());
    }
    let locale = args.output.locale.clone();
    let verbose = args.verbose;
    let pid = match args.pid {
        Some(pid) => pid,
        None => find_process_pid(EXE_NAME)
            .map_err(CliError::from_error)?
            .ok_or_else(|| CliError::new(3, format!("{EXE_NAME} not found")))?,
    };
    let ports = candidate_ports(pid).map_err(CliError::from_error)?;
    if ports.is_empty() {
        return Err(CliError::new(
            3,
            format!("no candidate ports found for pid={pid}"),
        ));
    }

    let defaults = DefaultPaths::new();
    let json = args.output.json.clone().unwrap_or(defaults.json);
    let csv = args.output.csv.clone().unwrap_or(defaults.csv);
    let output_raw = args.output_raw.map(|path| {
        if path.as_os_str().is_empty() {
            defaults.raw
        } else {
            path
        }
    });
    let stop = Arc::new(AtomicBool::new(false));
    install_ctrlc(Arc::clone(&stop))?;
    let q_listener = start_q_listener(Arc::clone(&stop))?;
    let has_q_listener = q_listener.is_some();
    let _q_listener = q_listener;

    if args.auto_page {
        run_auto_capture(AutoCaptureContext {
            pid,
            ports,
            output_raw,
            json,
            csv,
            locale,
            verbose,
            stop,
            has_q_listener,
        })
    } else {
        if has_q_listener {
            println!("Press q or Ctrl+C to stop.");
        } else {
            println!("Press Ctrl+C to stop.");
        }
        let result = capture_live(
            CaptureOptions {
                pid,
                exe: EXE_NAME.to_string(),
                ports,
                raw_out: output_raw.clone(),
                max_packets: 0,
                max_decoded: 0,
                on_progress: Some(progress_callback(&locale, verbose)),
            },
            stop,
        )
        .map_err(capture_error)?;
        export_capture_rows(&result.rows, &locale, &json, Some(&csv))?;
        println!("records={}", result.rows.len());
        print_paths(&json, Some(&csv), output_raw.as_deref());
        Ok(())
    }
}

struct AutoCaptureContext {
    pid: u32,
    ports: Vec<u16>,
    output_raw: Option<PathBuf>,
    json: PathBuf,
    csv: PathBuf,
    locale: String,
    verbose: bool,
    stop: Arc<AtomicBool>,
    has_q_listener: bool,
}

fn run_auto_capture(context: AutoCaptureContext) -> CliResult<()> {
    let AutoCaptureContext {
        pid,
        ports,
        output_raw,
        json,
        csv,
        locale,
        verbose,
        stop,
        has_q_listener,
    } = context;

    if has_q_listener {
        println!("Press Esc to stop auto. Press q or Ctrl+C to stop capture.");
    } else {
        println!("Press Esc to stop auto. Press Ctrl+C to stop capture.");
    }
    let capture_stop = Arc::clone(&stop);
    let capture_raw = output_raw.clone();
    let progress = progress_callback(&locale, verbose);
    let handle = thread::spawn(move || {
        capture_live(
            CaptureOptions {
                pid,
                exe: EXE_NAME.to_string(),
                ports,
                raw_out: capture_raw,
                max_packets: 0,
                max_decoded: 0,
                on_progress: Some(progress),
            },
            capture_stop,
        )
    });

    let mut options = AutoPageOptions::new(pid, Arc::clone(&stop));
    options.full_update = true;
    options.non_interactive = true;
    options.tooltip = false;
    options.on_status = Some(Arc::new(print_auto_status));
    let auto_result = run_auto_page(options);
    println!(
        "auto_page={} message={}",
        auto_result.status, auto_result.message
    );
    if auto_result.succeeded() {
        stop.store(true, Ordering::SeqCst);
    }

    let capture_result = handle
        .join()
        .map_err(|_| CliError::new(2, "capture worker panicked"))?
        .map_err(capture_error)?;
    export_capture_rows(&capture_result.rows, &locale, &json, Some(&csv))?;
    println!("records={}", capture_result.rows.len());
    print_paths(&json, Some(&csv), output_raw.as_deref());
    if auto_result.succeeded() {
        Ok(())
    } else {
        Err(CliError::new(2, auto_result.message))
    }
}

fn doctor() -> CliResult<()> {
    let report = capture_doctor(EXE_NAME).map_err(CliError::from_error)?;
    println!(
        "Windows: {}",
        if report.windows { "ok" } else { "unavailable" }
    );
    println!("Admin: {}", if report.admin { "ok" } else { "required" });
    println!("Process: {} {:?}", report.exe, report.pid);
    println!("Ports: {:?}", report.ports);
    for note in &report.notes {
        println!("{note}");
    }
    if report.windows && report.admin && report.pid.is_some() && !report.ports.is_empty() {
        Ok(())
    } else {
        Err(CliError::new(3, "capture environment is not ready"))
    }
}

fn maps_build(args: MapBuildArgs) -> CliResult<()> {
    let assets_root =
        find_assets_root(args.assets_root.as_deref()).map_err(CliError::from_error)?;
    let out_dir = args.out_dir.unwrap_or_else(default_maps_output_dir);
    fs::create_dir_all(&out_dir).map_err(CliError::from_error)?;

    for build in
        build_asset_maps(&assets_root, args.locale.as_deref()).map_err(CliError::from_error)?
    {
        let out = out_dir.join(format!("{}.json", build.locale));
        let bytes = serde_json::to_vec_pretty(&build.map).map_err(CliError::from_error)?;
        fs::write(&out, bytes).map_err(CliError::from_error)?;
        println!(
            "{}: items={} pools={} labels={}",
            build.locale, build.item_count, build.pool_count, build.label_count
        );
    }
    Ok(())
}

fn default_maps_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/nte-assets/resources/maps")
}

fn progress_callback(
    locale: &str,
    verbose: bool,
) -> Arc<dyn Fn(CaptureProgress) + Send + Sync + 'static> {
    let builder = Arc::new(Mutex::new(CaptureRecordBuilder::new(locale).ok()));
    Arc::new(move |progress: CaptureProgress| {
        if verbose && !progress.new_rows.is_empty() {
            if let Ok(mut guard) = builder.lock() {
                if let Some(builder) = guard.as_mut() {
                    for record in builder.build_records(&progress.new_rows) {
                        println!("{}", record.value);
                    }
                }
            }
        }
        eprint!(
            "\rrecords={} packets={} decoded={} dropped={} duplicates={}",
            progress.row_count,
            progress.counters.packets_seen,
            progress.counters.decoded_packets,
            progress.counters.dropped_packets,
            progress.counters.duplicate_packets
        );
    })
}

fn print_auto_status(status: AutoPageStatus) {
    eprintln!(
        "auto_page: {} pool={} page={}/{} {}",
        status.message,
        status.pool.unwrap_or_else(|| "-".to_string()),
        status
            .current_page
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        status
            .total_pages
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        status.technical_detail
    );
}

fn export_raw_replay(
    raw_jsonl: &Path,
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
) -> CliResult<()> {
    let rows = read_raw_capture(raw_jsonl).map_err(CliError::from_error)?;
    let document = build_capture_document(&rows.rows, locale).map_err(CliError::from_error)?;
    export_document(document, locale, json, csv, Some(raw_jsonl))
}

fn export_capture_rows(
    rows: &[ParsedRow],
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
) -> CliResult<()> {
    let document = build_capture_document(rows, locale).map_err(CliError::from_error)?;
    export_document(document, locale, json, csv, None)
}

fn export_document(
    document: serde_json::Value,
    locale: &str,
    json: &Path,
    csv: Option<&Path>,
    source_path: Option<&Path>,
) -> CliResult<()> {
    let temp = tempfile::tempdir().map_err(CliError::from_error)?;
    let store = JsonStore::open(temp.path()).map_err(CliError::from_error)?;
    let text = serde_json::to_string(&document).map_err(CliError::from_error)?;
    store
        .import_public_document(
            "default",
            &text,
            "raw_jsonl",
            source_path.map(|path| path.to_string_lossy()).as_deref(),
        )
        .map_err(CliError::from_error)?;
    store
        .export_public_json("default", locale, json)
        .map_err(CliError::from_error)?;
    if let Some(csv) = csv {
        store
            .export_csv("default", locale, csv)
            .map_err(CliError::from_error)?;
    }
    Ok(())
}

fn count_public_records(path: &Path) -> CliResult<usize> {
    let text = std::fs::read_to_string(path).map_err(CliError::from_error)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(CliError::from_error)?;
    Ok(value
        .get("nte")
        .and_then(|nte| nte.get("list"))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len))
}

fn print_paths(json: &Path, csv: Option<&Path>, raw: Option<&Path>) {
    println!("json={}", json.display());
    if let Some(csv) = csv {
        println!("csv={}", csv.display());
    }
    if let Some(raw) = raw {
        println!("private_raw={}", raw.display());
    }
}

fn install_ctrlc(stop: Arc<AtomicBool>) -> CliResult<()> {
    ctrlc::set_handler(move || {
        stop.store(true, Ordering::SeqCst);
    })
    .map_err(CliError::from_error)
}

struct QListener {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for QListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let _ = disable_raw_mode();
    }
}

fn start_q_listener(stop: Arc<AtomicBool>) -> CliResult<Option<QListener>> {
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    enable_raw_mode().map_err(CliError::from_error)?;
    let listener_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        while !listener_stop.load(Ordering::SeqCst) {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            listener_stop.store(true, Ordering::SeqCst);
                            return;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            listener_stop.store(true, Ordering::SeqCst);
                            return;
                        }
                        _ => {}
                    },
                    Ok(_) => {}
                    Err(_) => {
                        listener_stop.store(true, Ordering::SeqCst);
                        return;
                    }
                },
                Ok(false) => {}
                Err(_) => {
                    listener_stop.store(true, Ordering::SeqCst);
                    return;
                }
            }
        }
    });
    Ok(Some(QListener {
        stop,
        handle: Some(handle),
    }))
}

fn relaunch_capture_as_admin() -> CliResult<bool> {
    if !cfg!(windows) || is_admin() {
        return Ok(false);
    }
    println!("Requesting administrator permission for capture.");
    relaunch_current_process_as_admin()?;
    Ok(true)
}

#[cfg(windows)]
fn relaunch_current_process_as_admin() -> CliResult<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(CliError::from_error)?;
    let parameters = build_relaunch_parameters(std::env::args_os().skip(1));
    let directory = std::env::current_dir().map_err(CliError::from_error)?;
    let verb = wide_null(OsStr::new("runas"));
    let file = wide_null(exe.as_os_str());
    let params = wide_null(OsStr::new(&parameters));
    let dir = wide_null(directory.as_os_str());
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            dir.as_ptr(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result <= 32 {
        return Err(CliError::new(
            3,
            format!("administrator relaunch failed: ShellExecuteW={result}"),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn relaunch_current_process_as_admin() -> CliResult<()> {
    Err(CliError::new(3, "administrator relaunch requires Windows"))
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg_attr(not(windows), allow(dead_code))]
fn build_relaunch_parameters(args: impl IntoIterator<Item = OsString>) -> String {
    args.into_iter()
        .map(|arg| windows_quote_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn windows_quote_arg(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if !text.is_empty() && !text.chars().any(|char| char.is_whitespace() || char == '"') {
        return text.into_owned();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for char in text.chars() {
        match char {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(char);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn capture_error(error: impl std::fmt::Display) -> CliError {
    let text = error.to_string();
    if text.contains("Windows") || text.contains("administrator") || text.contains("pktmon") {
        CliError::new(3, text)
    } else {
        CliError::new(2, text)
    }
}

struct DefaultPaths {
    json: PathBuf,
    csv: PathBuf,
    raw: PathBuf,
}

impl DefaultPaths {
    fn new() -> Self {
        let stamp = chrono::Local::now().format("%y%m%d-%H%M%S").to_string();
        let output = PathBuf::from("output");
        Self {
            json: output.join(format!("history-{stamp}.json")),
            csv: output.join(format!("history-{stamp}.csv")),
            raw: output.join(format!("raw-{stamp}.jsonl")),
        }
    }
}

type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
struct CliError {
    code: i32,
    message: String,
}

impl CliError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_error(error: impl std::fmt::Display) -> Self {
        Self::new(2, error.to_string())
    }
}
