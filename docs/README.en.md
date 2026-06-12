# NTE Gacha Exporter | NTE Gacha Record Exporter

[繁體中文](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/README.md) | [English](README.en.md)

Captures NTE packets through Npcap, exports limited board, standard board, and fork-lottery records, and generates JSON/CSV.

## Highlights

- Interactive operation with auto paging support for capture.
- Exports data in JSON/CSV format.
- Bundled localized output names: `de`, `en`, `es`, `fr`, `ja`, `ko`, `ru`, `zh-CN`, `zh-Hans`, and `zh-Hant`.

## Quick Start

1. Download the latest release zip from [GitHub Releases](https://github.com/Anong0u0/nte_gacha_exporter/releases).
2. Extract the whole folder.
3. Open `nte-gacha.exe`.

Press `1` to start the capture/export flow. Press `L` to switch Traditional Chinese/English and `Q` to quit.
When no output path is specified, exported files are written to `output/` under the extracted directory.

## Requirements

- [Npcap](https://npcap.com/#download) must be installed.
 - Ensure `Install Npcap in WinPcap API-compatible mode` is ticked
 - Note for WiFi users: During Npcap installation, ensure `Support raw 802.11 traffic (and monitor mode) for wireless adapters` is ticked.
- The NTE game must be running.
- Auto paging requires administrator permission, the game window visible in the foreground, and the gacha page opened manually with F3. 1920x1080 is recommended. Other resolutions may be inaccurate.

## Usage

Open `nte-gacha.exe`, then configure output, language, and capture options. For the common path, keep the defaults and start capture.

Auto paging asks for administrator permission when needed. Before starting, keep the game on the gacha home screen with the lower-left file icon and fork-lottery entry visible. When the workflow finishes, capture stops and outputs are written. If auto paging fails, automatic clicking stops while capture remains available.

## Output

Public JSON contains export info and `nte.list` records:

```json
{
  "info": {
    "schema": "nte-gacha-export",
    "schema_version": "1.0",
    "privacy": "sanitized"
  },
  "nte": {
    "list": [
      {
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:15",
        "pool_name": "王牌一代目",
        "item_name": "改裝件·萌虎來襲-塗裝",
        "count": 1
      }
    ]
  }
}
```

CSV headers are localized by the selected language. Do not publish raw capture files unless you have reviewed their contents.

## Troubleshooting

### `live capture requires Windows + Npcap`

Make sure Windows is being used and Npcap is installed, then reopen the tool.

### `HTGame.exe` is not found

Start NTE, keep the game running, then reopen `nte-gacha.exe`.

### No records are written

Open the in-game gacha history screen so the game sends relevant packets. If records are still missing, switch network environment or restart the game and try again.

## Development

```powershell
poetry install --extras live
poetry run pytest
poetry run ruff check .
poetry run python packaging\nuitka\build.py
```

Package metadata lives in `pyproject.toml`. Run `poetry run nte-gacha`.

## Credits

- [Npcap](https://npcap.com/) for Windows packet capture.
- [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) for exported game assets and localization data.
- [rj0217/sniffbridge](https://github.com/rj0217/sniffbridge) by rj0217 for the MIT-licensed Windows PID/interface detection approach; only the minimal logic needed by this exporter is embedded.

## License

[MIT](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/LICENSE)
