# Agent Smoke

IPC-enabled desktop debug and smoke tool.

## Fixed Paths

| Purpose | Path |
| --- | --- |
| Agent app | `target/agent-smoke/app-current` |
| Agent data/assets/profile root | `target/agent-smoke/app-current/data` |
| Latest smoke report | `target/agent-smoke/latest-report.json` |
| Latest smoke run | `target/agent-smoke/latest-run.txt` |

## Main Flow

| Need | Command |
| --- | --- |
| Build IPC app | `cargo agent build` |
| Launch IPC app | `cargo agent launch` |
| Run automated smoke | `cargo smoke` |

## Agent Commands

| Need | Command |
| --- | --- |
| Health | `cargo agent health` |
| Snapshot | `cargo agent snapshot` |
| List ids | `cargo agent ids` |
| List ids as rows | `cargo agent ids --plain` |
| Inspect one id | `cargo agent inspect --agent-id settings-import-raw` |
| Wait for id | `cargo agent wait --agent-id view-settings --timeout-secs 30` |
| Wait for text | `cargo agent expect-text "Import completed" --timeout-secs 30` |
| Click id | `cargo agent click --agent-id nav-settings` |
| Set input/select | `cargo agent set --agent-id profile-create-input --value "smoke_profile"` |
| Evaluate JS | `cargo agent eval --script "return document.body.innerText"` |
| Capture window | `cargo agent screenshot --title "NTE Gacha Exporter" --out target\agent-smoke\manual.png` |

## Behavior

- `cargo agent build` preserves `target/agent-smoke/app-current/data`.
- Asset packs belong under `target/agent-smoke/app-current/data`.
- `cargo agent launch` uses `127.0.0.1:17365` by default.
- If that port is owned by an NTE process, launch closes/replaces it; non-NTE owners fail.
- `cargo smoke` uses isolated staged data, closes the app, removes the staged portable copy, and keeps latest evidence.
- Public release builds do not include IPC.
