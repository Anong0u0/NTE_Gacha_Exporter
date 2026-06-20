use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, Runtime};

const ENABLE_ENV: &str = "NTE_AGENT_SMOKE";
const ADDR_ENV: &str = "NTE_AGENT_SMOKE_ADDR";
const DEFAULT_ADDR: &str = "127.0.0.1:17365";

#[derive(Debug, Deserialize)]
struct EvalRequest {
    script: String,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct ActionRequest {
    action: String,
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    value: Value,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

pub(crate) fn maybe_start<R: Runtime>(app: &tauri::App<R>) {
    if env::var(ENABLE_ENV).ok().as_deref() != Some("1") {
        return;
    }

    let addr = env::var(ADDR_ENV).unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let handle = app.handle().clone();
    thread::spawn(move || {
        if let Err(error) = serve(handle, &addr) {
            eprintln!("agent smoke server failed: {error}");
        }
    });
}

fn serve<R: Runtime>(handle: AppHandle<R>, addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let handle = handle.clone();
                thread::spawn(move || handle_connection(handle, stream));
            }
            Err(error) => eprintln!("agent smoke connection failed: {error}"),
        }
    }
    Ok(())
}

fn handle_connection<R: Runtime>(handle: AppHandle<R>, mut stream: TcpStream) {
    let result = read_request(&mut stream)
        .and_then(|request| route_request(handle, request))
        .unwrap_or_else(error_response);
    let _ = stream.write_all(result.as_bytes());
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;

    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            break None;
        }
        buffer.extend_from_slice(&chunk[..count]);
        if let Some(index) = find_header_end(&buffer) {
            break Some(index);
        }
        if buffer.len() > 1024 * 1024 {
            return Err("request header too large".to_string());
        }
    }
    .ok_or_else(|| "invalid HTTP request".to_string())?;

    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|error| format!("invalid request header utf8: {error}"))?;
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
    }

    let body = buffer
        .get(body_start..body_start + content_length)
        .unwrap_or_default()
        .to_vec();
    Ok(HttpRequest { method, path, body })
}

fn route_request<R: Runtime>(handle: AppHandle<R>, request: HttpRequest) -> Result<String, String> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => ok_response(json!({
            "schema": "nte-agent-smoke",
            "version": 1,
            "pid": std::process::id(),
            "app_version": env!("CARGO_PKG_VERSION"),
            "portable_root": crate::state::portable_root()
                .ok()
                .map(|path| path.display().to_string()),
        })),
        ("POST", "/eval") => {
            let payload: EvalRequest =
                serde_json::from_slice(&request.body).map_err(|error| error.to_string())?;
            ok_response(eval_js(&handle, payload.script, payload.timeout_ms)?)
        }
        ("POST", "/action") => {
            let payload: ActionRequest =
                serde_json::from_slice(&request.body).map_err(|error| error.to_string())?;
            ok_response(run_action(&handle, payload)?)
        }
        _ => Err(format!(
            "unknown route: {} {}",
            request.method, request.path
        )),
    }
}

fn run_action<R: Runtime>(handle: &AppHandle<R>, request: ActionRequest) -> Result<Value, String> {
    let script = match request.action.as_str() {
        "snapshot" => snapshot_script(),
        "click_agent" => click_script(&request.agent_id),
        "set_input_agent" | "select_agent" => set_input_script(&request.agent_id, &request.value),
        other => return Err(format!("unknown action: {other}")),
    };
    eval_js(handle, script, request.timeout_ms)
}

fn eval_js<R: Runtime>(
    handle: &AppHandle<R>,
    script: String,
    timeout_ms: u64,
) -> Result<Value, String> {
    let window = handle
        .get_webview_window("main")
        .ok_or_else(|| "main webview not found".to_string())?;
    let wrapped = format!(
        r#"
        (() => {{
          try {{
            const value = (() => {{ {script} }})();
            return {{ ok: true, value }};
          }} catch (error) {{
            return {{
              ok: false,
              error: String(error && error.stack ? error.stack : error),
            }};
          }}
        }})()
        "#
    );

    let (tx, rx) = mpsc::channel();
    window
        .eval_with_callback(wrapped, move |value| {
            let _ = tx.send(value);
        })
        .map_err(|error| error.to_string())?;

    let raw = rx
        .recv_timeout(Duration::from_millis(timeout_ms.max(100)))
        .map_err(|_| "eval timeout".to_string())?;
    let response: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("eval response json error: {error}; raw={raw}"))?;
    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(response.get("value").cloned().unwrap_or(Value::Null))
    } else {
        Err(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("eval failed")
            .to_string())
    }
}

fn snapshot_script() -> String {
    r#"
    return {
      title: document.title,
      location: String(window.location.href),
      bodyText: document.body ? document.body.innerText.slice(0, 5000) : "",
      agentIds: Array.from(document.querySelectorAll("[data-agent-id]")).map((el) => ({
        id: el.getAttribute("data-agent-id"),
        tag: el.tagName.toLowerCase(),
        text: (el.innerText || el.value || "").slice(0, 200),
        disabled: Boolean(el.disabled),
      })),
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
      },
    };
    "#
    .to_string()
}

fn click_script(agent_id: &str) -> String {
    let agent_id_json = serde_json::to_string(agent_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
        const agentId = {agent_id_json};
        const el = Array.from(document.querySelectorAll("[data-agent-id]"))
          .find((node) => node.getAttribute("data-agent-id") === agentId);
        if (!el) throw new Error("agent element not found: " + agentId);
        el.scrollIntoView({{ block: "center", inline: "center" }});
        el.click();
        return {{ clicked: agentId }};
        "#
    )
}

fn set_input_script(agent_id: &str, value: &Value) -> String {
    let agent_id_json = serde_json::to_string(agent_id).unwrap_or_else(|_| "\"\"".to_string());
    let value_json = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    format!(
        r#"
        const agentId = {agent_id_json};
        const el = Array.from(document.querySelectorAll("[data-agent-id]"))
          .find((node) => node.getAttribute("data-agent-id") === agentId);
        if (!el) throw new Error("agent element not found: " + agentId);
        const value = {value_json};
        el.scrollIntoView({{ block: "center", inline: "center" }});
        el.value = String(value ?? "");
        el.dispatchEvent(new Event("input", {{ bubbles: true }}));
        el.dispatchEvent(new Event("change", {{ bubbles: true }}));
        return {{ set: agentId, value: el.value }};
        "#
    )
}

fn ok_response(value: Value) -> Result<String, String> {
    response(
        200,
        &ApiResponse {
            ok: true,
            result: Some(value),
            error: None,
        },
    )
}

fn error_response(error: String) -> String {
    response(
        500,
        &ApiResponse::<Value> {
            ok: false,
            result: None,
            error: Some(error),
        },
    )
    .unwrap_or_else(|_| {
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string()
    })
}

fn response<T: Serialize>(status: u16, body: &T) -> Result<String, String> {
    let reason = if status == 200 {
        "OK"
    } else {
        "Internal Server Error"
    };
    let body = serde_json::to_string(body).map_err(|error| error.to_string())?;
    Ok(format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn default_timeout_ms() -> u64 {
    5000
}
