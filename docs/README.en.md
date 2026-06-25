# NTE Gacha Exporter

[繁體中文](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/README.md) | English

Captures NTE packets through Windows pktmon, exports limited board, standard board, and fork-lottery records, and generates JSON/CSV.

## Highlights

- GUI and gacha analysis.
- Auto paging support for capture.
- Exports data in JSON/CSV format.
- Bundled localized output names: `de`, `en`, `es`, `fr`, `ja`, `ko`, `ru`, `zh-CN`, `zh-Hans`, and `zh-Hant`.

## Quick Start

1. Download the latest Windows portable package zip from [GitHub Releases](https://github.com/Anong0u0/nte_gacha_exporter/releases). Do not download the Source code zip as the app package.
2. Extract the whole folder.
3. Open `nte-gacha-exporter.exe`.

## Requirements

- Windows 10/11.
- Administrator permission is required.
- The NTE game must be running.
- Auto paging requires the game window to be visible in the foreground and the gacha page opened manually with F3. 1920x1080 is recommended. Other resolutions may be inaccurate.

## Usage

Open `nte-gacha-exporter.exe`, then click `Update Data` in the upper-right corner.

Before auto paging, keep the game on the F3 gacha home screen with the lower-left file icon and fork-lottery entry visible. When the workflow finishes, it stops and updates the analysis data.

CLI:

```powershell
.\nte-gacha-exporter-cli.exe capture --output-raw --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe replay .\output\raw-260611-153012.jsonl --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe doctor
```

## Output

Public JSON contains export info and `nte.list` records:

```json
{
  "info": {
    "schema": "nte-gacha-export",
    "schema_version": "2.0",
    "locale": "en"
  },
  "nte": {
    "list": [
      {
        "record_id": "02539eac...",
        "source_order": 0,
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:15",
        "pool_id": "CardPool_Character",
        "pool_name": "The Ichi-daime",
        "banner_id": "monopoly_limited_Nanali",
        "item_id": "Fashion_vehicle_1010_V008",
        "item_name": "Mod Parts·Tiger Incoming! - Livery",
        "rarity": 5,
        "count": 1,
        "roll_points": 2,
        "roll_label": "2"
      }
    ]
  }
}
```

CSV headers are localized by the selected language. Do not publish raw capture files unless you have reviewed their contents.

## Troubleshooting

### `pktmon capture requires administrator privilege`

Reopen the tool with administrator permission.

### `HTGame.exe` is not found

Start NTE, keep the game running, then reopen `nte-gacha-exporter.exe`.

### No records are written

Open the in-game gacha history screen so the game sends relevant packets. If records are still missing, switch network environment or restart the game and try again.

## Development

```powershell
cargo xtask ci
cargo xtask quality
```

Windows release package:

```powershell
cargo run -p nte-gacha-exporter-cli --bin nte-gacha-exporter-cli -- assets pack build --assets-root D:\path\NTE_Assets --out dist\nte-assets-pack.zip
powershell.exe -ExecutionPolicy Bypass -File packaging\build-win-release.ps1 -AssetsPackZip dist\nte-assets-pack.zip
```

## Credits

- [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) for exported game assets and localization data.

## License

[MIT](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/LICENSE)
