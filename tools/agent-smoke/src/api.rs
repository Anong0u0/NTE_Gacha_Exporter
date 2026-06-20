use std::{
    collections::BTreeSet,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::report::{AgentElement, ApiResponse, ExpectTextOutput};

pub fn health(addr: &str) -> Result<Value> {
    request("GET", addr, "/health", None, Duration::from_secs(5))
}

pub fn snapshot(addr: &str) -> Result<Value> {
    action(addr, "snapshot", "", Value::Null, 5000)
}

pub fn wait_health(addr: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match health(addr) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(250));
    }
    match last_error {
        Some(error) => Err(error).context("agent smoke health did not become ready"),
        None => bail!("agent smoke health did not become ready"),
    }
}

pub fn eval_js(addr: &str, script: &str, timeout_ms: u64) -> Result<Value> {
    request(
        "POST",
        addr,
        "/eval",
        Some(json!({ "script": script, "timeout_ms": timeout_ms })),
        Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(2)),
    )
}

pub fn action(
    addr: &str,
    action_name: &str,
    agent_id: &str,
    value: Value,
    timeout_ms: u64,
) -> Result<Value> {
    request(
        "POST",
        addr,
        "/action",
        Some(json!({
            "action": action_name,
            "agent_id": agent_id,
            "value": value,
            "timeout_ms": timeout_ms,
        })),
        Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(2)),
    )
}

pub fn click_nav(addr: &str, view: &str) -> Result<()> {
    action(
        addr,
        "click_agent",
        &format!("nav-{view}"),
        Value::Null,
        5000,
    )?;
    thread::sleep(Duration::from_millis(500));
    Ok(())
}

pub fn agent_elements(snapshot: &Value) -> Vec<AgentElement> {
    snapshot
        .get("agentIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentElement {
                id: item.get("id")?.as_str()?.to_string(),
                tag: item
                    .get("tag")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text: item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                disabled: item
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
}

pub fn find_agent_element(snapshot: &Value, agent_id: &str) -> Option<AgentElement> {
    agent_elements(snapshot)
        .into_iter()
        .find(|element| element.id == agent_id)
}

pub fn wait_agent_element(addr: &str, agent_id: &str, timeout: Duration) -> Result<AgentElement> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = snapshot(addr)?;
        if let Some(element) = find_agent_element(&snapshot, agent_id) {
            return Ok(element);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("agent id not visible before timeout: {agent_id}")
}

pub fn expect_text(addr: &str, text: &str, timeout: Duration) -> Result<ExpectTextOutput> {
    let started_at = Instant::now();
    let deadline = started_at + timeout;
    while Instant::now() < deadline {
        let snapshot = snapshot(addr)?;
        if snapshot
            .get("bodyText")
            .and_then(Value::as_str)
            .is_some_and(|body| body.contains(text))
        {
            return Ok(ExpectTextOutput {
                text: text.to_string(),
                matched: true,
                elapsed_ms: started_at.elapsed().as_millis(),
            });
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("text not visible before timeout: {text}")
}

pub fn print_agent_elements_plain(elements: &[AgentElement]) {
    for element in elements {
        print_agent_element_plain(element);
    }
}

pub fn print_agent_element_plain(element: &AgentElement) {
    let state = if element.disabled {
        "disabled"
    } else {
        "enabled"
    };
    println!(
        "{}\t{}\t{}\t{}",
        element.id,
        element.tag,
        state,
        plain_cell(&element.text)
    );
}

pub fn wait_agent(addr: &str, agent_id: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
        let ids = snapshot
            .get("agentIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        if ids.contains(agent_id) {
            return Ok(snapshot);
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("agent id not visible: {agent_id}")
}

pub fn wait_text(addr: &str, text: &str, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let snapshot = action(addr, "snapshot", "", Value::Null, 5000)?;
        if snapshot
            .get("bodyText")
            .and_then(Value::as_str)
            .is_some_and(|body| body.contains(text))
        {
            return Ok(snapshot);
        }
        thread::sleep(Duration::from_millis(500));
    }
    bail!("text not visible: {text}")
}

pub fn assert_agent_ids(snapshot: &Value, expected: &[&str]) -> Result<()> {
    let ids = snapshot
        .get("agentIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let missing = expected
        .iter()
        .copied()
        .filter(|id| !ids.contains(id))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        bail!("missing agent ids: {}", missing.join(", "))
    }
}

fn request(
    method: &str,
    addr: &str,
    path: &str,
    body: Option<Value>,
    timeout: Duration,
) -> Result<Value> {
    let url = format!("http://{addr}{path}");
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let response = match (method, body) {
        ("GET", None) => agent.get(&url).call(),
        ("POST", Some(body)) => agent
            .post(&url)
            .set("Content-Type", "application/json; charset=utf-8")
            .send_string(&body.to_string()),
        _ => bail!("unsupported request shape: {method} {path}"),
    }
    .map_err(|error| anyhow!("{error}"))?;
    let text = response
        .into_string()
        .map_err(|error| anyhow!("invalid API response body: {error}"))?;
    let envelope: ApiResponse = serde_json::from_str(&text)
        .map_err(|error| anyhow!("invalid API json response: {error}; body={text}"))?;
    if envelope.ok {
        Ok(envelope.result.unwrap_or(Value::Null))
    } else {
        bail!(
            envelope
                .error
                .unwrap_or_else(|| "agent API failed".to_string())
        )
    }
}

fn plain_cell(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if matches!(character, '\t' | '\r' | '\n') {
                ' '
            } else {
                character
            }
        })
        .collect()
}
