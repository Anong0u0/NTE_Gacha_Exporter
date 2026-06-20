# Agent Smoke

Rust release-smoke runner for agent-operated desktop checks.

- Builds are external; this tool only operates an existing portable release.
- The desktop app must be built with `agent-smoke` feature for IPC to exist.
- Output defaults to `target/agent-smoke`.
- Smoke output is compact by default: keep the latest run report/logs/screenshots, remove the per-run portable copy, and preserve `target/agent-smoke/smoke-input-current`.
- Windows supports launch and `PrintWindow` screenshots. Other hosts compile, but Windows-only commands return an error.

## Smoke

Build a smoke-enabled portable without touching public `target\release`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\agent-smoke\build-smoke-portable.ps1
```

Run it:

```powershell
cargo smoke
```

Defaults: `target\agent-smoke\smoke-input-current`, then `dist\nte-gacha-exporter-<version>`, plus `fixtures\sample.raw.jsonl`.

Build and run in one command:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools\agent-smoke\build-smoke-portable.ps1 -RunSmoke
```

Report:

- `target/agent-smoke/latest-report.json`
- `target/agent-smoke/latest-run.txt`
- `<run-dir>\report.json`
- `<run-dir>\logs\*.json`
- `<run-dir>\screenshots\*.png`

Retention:

- Default: `cargo smoke --keep-runs 1`
- Per-run portable copies are removed after the app exits.
- `--keep-app` keeps the app running and also keeps that run's portable copy.
- Use `cargo smoke --keep-portable` to keep `<run-dir>\portable` for debugging.
- Use `cargo smoke --keep-runs 3` to retain more run directories.
- The smoke build cache lives in `target\agent-smoke-build` and is not rotated by this tool.

## Manual Ops

Launch a smoke-enabled portable app first:

```powershell
$env:NTE_AGENT_SMOKE = "1"
$env:NTE_AGENT_SMOKE_ADDR = "127.0.0.1:17365"
dist\nte-gacha-exporter-0.1.1\nte-gacha-exporter.exe
```

Then operate the existing IPC:

```powershell
cargo agent health
cargo agent snapshot
cargo agent ids
cargo agent ids --plain
cargo agent inspect --agent-id import-run
cargo agent wait --agent-id last-import-panel --timeout-secs 30
cargo agent expect-text "Import completed" --timeout-secs 30
cargo agent click --agent-id nav-import_export
cargo agent set --agent-id import-path --value "D:\path\sample.raw.jsonl"
cargo agent eval --script "return document.body.innerText"
cargo agent screenshot --title "NTE Gacha Exporter" --out target\agent-smoke\manual.png
```

`ids` and `inspect` read `data-agent-id` elements only. JSON is the default output; `--plain` prints tab-separated rows.

Common ids:

- `nav-dashboard`
- `nav-records`
- `nav-import_export`
- `nav-settings`
- `view-import-export`
- `import-mode`
- `import-path`
- `import-run`
