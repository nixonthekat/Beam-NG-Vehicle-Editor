# BeamNG Vehicle Editor

A desktop app for previewing and editing BeamNG `.pc` vehicle configs. Built with **Rust + egui/eframe**.

## Features

- **Grid view** — Vehicle cards with thumbnails (`preview.jpg`, `default.jpg`, or `ui/*`), search, and mod/stock filters
- **Editor** — Live parts/slots editing from the `.pc` JSON `parts` object, config diff preview
- **Engine swap** — Browse engines scanned from `.jbeam` files across your mods folder; assign with one click
- **Backup / restore** — Auto-backup before first edit and before every apply; versioned copies with metadata; restore per backup or restore all
- **Polish** — Dark theme, toasts, undo/redo, `Ctrl+S` / `Ctrl+Z`, async scanning, optional BeamNG launch on apply

## Requirements

- [Rust](https://rustup.rs/) (stable, 2021 edition)
- Windows/macOS/Linux

## Build & run

```bash
cargo build --release
cargo run --release
```

Do **not** run the built `.exe` from the agent/CI shell — build with the commands above and launch locally.

## First-time setup

1. Open **Settings**
2. Set **Mods / vehicles folder** to your BeamNG user content path, for example:
   - Windows: `C:\Users\<you>\Documents\BeamNG.drive\mods`
   - Or a subfolder like `...\mods\unpacked` if you only want unpacked mods
3. Optionally set **BeamNG executable** to launch the game after applying changes
4. Click **Rescan** (or restart the app)

Settings and backups are stored under your OS config directory:

- Windows: `%APPDATA%\beam-ng-vehicle-editor\`
- Linux: `~/.config/beam-ng-vehicle-editor/`
- macOS: `~/Library/Application Support/beam-ng-vehicle-editor/`

Backups live in `backups/<vehicle_key>/` and are **never auto-deleted**.

## Usage

### Grid

- Click a card to open the vehicle in the **Editor**
- Use search and filters (mod name, stock-only, mod-only)

### Editor

- Edit any slot in the parts list (engine slots are marked)
- **Browse Engines** — scans `.jbeam` files for engine-type parts; search and **Assign**
- **Apply Changes** — validates JSON, creates a backup, writes the `.pc` file
- **Restore Latest** — restores the most recent backup for the current vehicle
- **Ctrl+S** — apply, **Ctrl+Z** / **Ctrl+Shift+Z** — undo / redo

### Backups

- Sidebar lists vehicles with backup counts
- Each entry shows timestamp, version, size, and reason
- **Restore** opens a confirmation dialog
- **Restore All** restores the earliest backup per vehicle (with confirmation)

## Project layout

```
src/
  main.rs       — entry point
  app.rs        — eframe App, actions, hotkeys
  state.rs      — central app state
  settings.rs   — persisted paths
  scanner.rs    — .pc vehicle discovery + thumbnails
  config.rs     — .pc JSON load/save/diff
  engine.rs     — .jbeam engine part scanner
  backup.rs     — versioned backup/restore
  gui/          — grid, editor, backups, theme
```

## BeamNG paths reference

| Content | Typical location |
|---------|------------------|
| Mods | `Documents/BeamNG.drive/mods/` |
| Unpacked mods | `Documents/BeamNG.drive/mods/unpacked/<mod>/` |
| Vehicle config | `<mod>/vehicles/<name>/<name>.pc` |
| Thumbnails | `preview.jpg`, `default.jpg`, or `ui/*.jpg` in vehicle folder |
| Parts | `.jbeam` files under mod folders |

## Safety

- JSON is validated before save
- Original file is copied to backups before the first edit and before each apply
- Restore always asks for confirmation (except quick restore latest in editor — use with care)

## License

MIT
