use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::Parser;
use serde_json::Value;

mod api;
mod cli;
mod report;
mod runtime;
mod smoke;
mod util;
mod window;

use api::{
    action, agent_elements, eval_js, expect_text, find_agent_element, health,
    print_agent_element_plain, print_agent_elements_plain, snapshot, wait_agent_element,
};
use cli::{Cli, CommandKind, SmokeOptions};
use report::AgentIdsOutput;
use runtime::{run_agent_build, run_agent_launch};
use smoke::run_smoke;
use util::{print_json, write_json, write_png};
use window::{capture_window, find_window, image_metrics};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Build { force } => run_agent_build(force),
        CommandKind::Launch {
            addr,
            timeout_secs,
            out,
        } => {
            let output = run_agent_launch(&addr, Duration::from_secs(timeout_secs))?;
            if let Some(out) = out {
                write_json(out, &output)?;
            } else {
                print_json(&output)?;
            }
            Ok(())
        }
        CommandKind::Smoke {
            sample,
            out_dir,
            addr,
            timeout_secs,
        } => run_smoke(SmokeOptions {
            sample,
            out_dir,
            addr,
            timeout: Duration::from_secs(timeout_secs),
        }),
        CommandKind::Health { addr } => print_value(health(&addr)?),
        CommandKind::Snapshot { addr } => print_value(snapshot(&addr)?),
        CommandKind::Ids { addr, plain } => {
            let ids = agent_elements(&snapshot(&addr)?);
            if plain {
                print_agent_elements_plain(&ids);
            } else {
                print_json(&AgentIdsOutput { ids })?;
            }
            Ok(())
        }
        CommandKind::Inspect {
            addr,
            agent_id,
            plain,
        } => {
            let element = find_agent_element(&snapshot(&addr)?, &agent_id)
                .ok_or_else(|| anyhow!("agent id not visible: {agent_id}"))?;
            if plain {
                print_agent_element_plain(&element);
            } else {
                print_json(&element)?;
            }
            Ok(())
        }
        CommandKind::Wait {
            addr,
            agent_id,
            timeout_secs,
        } => print_json(&wait_agent_element(
            &addr,
            &agent_id,
            Duration::from_secs(timeout_secs),
        )?),
        CommandKind::ExpectText {
            addr,
            text,
            timeout_secs,
        } => print_json(&expect_text(
            &addr,
            &text,
            Duration::from_secs(timeout_secs),
        )?),
        CommandKind::Click { addr, agent_id } => {
            print_value(action(&addr, "click_agent", &agent_id, Value::Null, 5000)?)
        }
        CommandKind::Set {
            addr,
            agent_id,
            value,
        } => print_value(action(
            &addr,
            "set_input_agent",
            &agent_id,
            Value::String(value),
            5000,
        )?),
        CommandKind::Eval {
            addr,
            script,
            timeout_ms,
        } => print_value(eval_js(&addr, &script, timeout_ms)?),
        CommandKind::Screenshot { pid, title, out } => {
            let window = find_window(pid, title.as_deref(), None, Duration::from_secs(10))?;
            let image = capture_window(&window)?;
            write_png(&out, &image)?;
            print_json(&report::ScreenshotReport {
                name: "screenshot".to_string(),
                path: out.display().to_string(),
                window,
                metrics: image_metrics(&image),
            })
        }
    }
}

fn print_value(value: Value) -> Result<()> {
    print_json(&value)
}
