# NTE Gacha Exporter

[繁體中文](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/README.md) | English

Captures NTE packets through Windows pktmon, exports limited board, standard board, and fork-lottery records, and generates JSON/CSV.

## Highlights

- Desktop UI and Rust CLI operation.
- Auto paging support for capture.
- Exports data in JSON/CSV format.
- Bundled localized output names: `de`, `en`, `es`, `fr`, `ja`, `ko`, `ru`, `zh-CN`, `zh-Hans`, and `zh-Hant`.
- Optional assets pack for item and banner images in the desktop GUI. The app works without it.

## Quick Start

1. Download the latest Windows portable package release zip from [GitHub Releases](https://github.com/Anong0u0/nte_gacha_exporter/releases). Do not download the Source code zip as the app package.
2. Extract the whole folder.
3. Open `nte-gacha-exporter.exe`.
4. To show GUI images, use `Settings` -> `Assets Pack` -> `Check assets` and `Download assets`.

The desktop UI stores data under portable `data/` and can export JSON/CSV. For CLI commands, when no output path is specified, exported files are written to `output/` under the extracted directory.

Releases also include `nte-assets-pack-<version>-<maphash>.zip` and `nte-assets-pack-manifest.json`. Normal users do not need to manage these files manually; the desktop app downloads a compatible pack for the selected update channel.

## Requirements

- Windows 10/11.
- Live capture requires administrator permission and uses the built-in pktmon runtime.
- The NTE game must be running.
- Auto paging requires administrator permission, the game window visible in the foreground, and the gacha page opened manually with F3. 1920x1080 is recommended. Other resolutions may be inaccurate.

## Usage

Open `nte-gacha-exporter.exe`, then configure output, language, and capture options. For the common path, keep the defaults and start capture.

CLI capture asks for administrator permission when needed. Before auto paging, keep the game on the gacha home screen with the lower-left file icon and fork-lottery entry visible. When the workflow finishes, capture stops and outputs are written. If auto paging fails, automatic clicking stops while capture remains available.

CLI:

```powershell
.\nte-gacha-exporter-cli.exe replay .\output\raw-260611-153012.jsonl --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe capture --output-raw --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe capture --auto-page --output-raw
.\nte-gacha-exporter-cli.exe doctor
.\nte-gacha-exporter-cli.exe maps build --assets-root D:\NTE_Assets --locale zh-Hant
```

## Output

Public JSON contains export info and `nte.list` records:

```json
{
  "info": {
    "schema": "nte-gacha-exporter-export",
    "schema_version": "4.0",
    "privacy": "sanitized"
  },
  "nte": {
    "list": [
      {
        "record_id": "b4b36f5d...",
        "source_order": 0,
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:15",
        "pool_id": "CardPool_Character",
        "pool_name": "王牌一代目",
        "item_id": "Fashion_vehicle_1010_V008",
        "item_name": "改裝件·萌虎來襲-塗裝",
        "count": 1,
        "roll_label": "3",
        "roll_points": 3
      }
    ]
  }
}
```

`record_id` is an opaque hash used for import dedupe. `source_order` preserves capture order for records with the same timestamp. CSV headers are localized by the selected language. Do not publish raw capture files unless you have reviewed their contents.

## Assets Pack

The main app does not bundle image assets. The assets pack contains only images referenced by the bundled maps. It is built from a pinned [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) commit, and the release manifest records app version, maps hash, source commit, zip sha256, and file count.

The installed pack lives under `data/assets-pack/current` in the portable directory. The GUI serves only `assets/*.webp` through the internal `nteasset` protocol and does not enable arbitrary filesystem reads. Removing the pack only disables the installed images; records, import/export, and updates keep working.

## Troubleshooting

### `pktmon capture requires Windows`

Run the tool on Windows. Linux is supported for development and raw replay tests only, not live capture.

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

The CI gate can be run separately:

```powershell
cargo fmt --all --check
cargo fmt --manifest-path tools/agent-smoke/Cargo.toml --check
cargo test --workspace
cargo test --manifest-path tools/agent-smoke/Cargo.toml
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path tools/agent-smoke/Cargo.toml --all-targets -- -D warnings
pushd apps\desktop
bun install --frozen-lockfile
bun run typecheck
bun run build
popd
```

The daily quality gate can be split for focused debugging:

```powershell
cargo xtask check-long-code
cargo machete
pushd tools\agent-smoke
cargo machete
popd
pushd apps\desktop
bun install --frozen-lockfile
bun run knip --reporter compact
bun run typecheck -- --noUnusedLocals --noUnusedParameters --pretty false
popd
```

The Rust workspace lives at the repo root, the GUI app lives under `apps/desktop/`, core crates live under `crates/`, and repo automation lives under `tools/xtask`.
Map resources live under `crates/nte-assets/resources/maps`; rebuild them from `NTE_Assets` with the Rust CLI:

```powershell
cargo run -p nte-gacha-exporter-cli --bin nte-gacha-exporter-cli -- maps build --assets-root .local\NTE_Assets
```

Assets pack build:

```powershell
cargo run -p nte-gacha-exporter-cli --bin nte-gacha-exporter-cli -- assets pack build --assets-root D:\path\NTE_Assets --out dist\nte-assets-pack.zip
```

Windows release package:

```powershell
powershell.exe -ExecutionPolicy Bypass -File packaging\build-win-release.ps1
```

## Credits

- [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) for exported game assets and localization data.
- [rj0217/sniffbridge](https://github.com/rj0217/sniffbridge) by rj0217 for the MIT-licensed Windows PID/interface detection approach; only the minimal logic needed by this exporter is embedded.

## License

[MIT](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/LICENSE)
