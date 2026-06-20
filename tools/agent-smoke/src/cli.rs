use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub const DEFAULT_ADDR: &str = "127.0.0.1:17365";
pub const DEFAULT_OUT_DIR: &str = "target/agent-smoke";
pub const DEFAULT_SAMPLE: &str = "fixtures/sample.raw.jsonl";
pub const DEFAULT_AGENT_APP_ROOT: &str = "target/agent-smoke/app-current";
pub const AGENT_BUILD_SCRIPT: &str = "tools/agent-smoke/build-agent-app.ps1";
pub const DEFAULT_KEEP_RUNS: usize = 1;
pub const APP_TITLE: &str = "NTE Gacha Exporter";

#[derive(Debug, Parser)]
#[command(name = "nte-agent-smoke")]
#[command(about = "Agent-operated release smoke runner for NTE Gacha Exporter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CommandKind,
}

#[derive(Debug, Subcommand)]
pub enum CommandKind {
    Build {
        #[arg(long)]
        skip_install: bool,
    },
    Launch {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long, default_value_t = 30)]
        timeout_secs: u64,
    },
    Smoke {
        #[arg(long)]
        sample: Option<PathBuf>,
        #[arg(long, default_value = DEFAULT_OUT_DIR)]
        out_dir: PathBuf,
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long, default_value_t = 30)]
        timeout_secs: u64,
    },
    Health {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
    },
    Snapshot {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
    },
    Ids {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        plain: bool,
    },
    Inspect {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        plain: bool,
    },
    Wait {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    ExpectText {
        text: String,
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    Click {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
    },
    Set {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        value: String,
    },
    Eval {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
        #[arg(long)]
        script: String,
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
    Screenshot {
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

pub struct SmokeOptions {
    pub sample: Option<PathBuf>,
    pub out_dir: PathBuf,
    pub addr: String,
    pub timeout: std::time::Duration,
}
