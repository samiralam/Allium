# AGENTS.md

This file provides guidance to AI coding agents when working with code in this repository.

## Project Overview

Allium is a custom launcher for Miyoo Mini handheld gaming devices (Mini, Mini+, Mini Flip). It replaces the stock UI with a fast, themeable interface for managing RetroArch-based game emulation. Written in Rust (nightly, edition 2024), targeting ARM Linux (armv7-unknown-linux-gnueabihf).

## Build Commands

```bash
make build                          # Cross-compile for hardware (uses cargo-zigbuild)
make debug                          # Build with debug symbols
make simulator bin=allium-launcher  # Run launcher in desktop simulator
make simulator bin=allium-menu      # Run in-game menu in simulator
make lint                           # rustfmt + clippy
make all                            # Full build (binaries, RetroArch, themes, migrations)
make clean                          # Clean build artifacts
cargo test                          # Run tests
SDCARD_PATH=/path/to/sd make deploy # Deploy to SD card
```

For first-time macOS setup: `./scripts/setup-mac.sh`

## Architecture

### Multi-binary design with shared library

**Daemon (`alliumd`)** — persistent process managing lifecycle of launcher/game/menu, handles power button, volume/brightness hotkeys, and power management (sleep/wake/shutdown).

**Launcher (`allium-launcher`)** — main UI with tabbed views: Recents, Games, Apps, Settings. Handles game discovery, metadata, artwork display, and all settings.

**In-game Menu (`allium-menu`)** — overlay during gameplay for save/load states, disk switching, guide reading. Communicates with RetroArch via UDP socket protocol.

**Utility binaries** — `activity-tracker`, `screenshot`, `screenshot-viewer`, `say`, `show`, `myctl` (hardware control via FFI).

### Shared library (`common` crate)

The `common` crate is the core shared dependency containing:

- **`platform/`** — trait-based hardware abstraction. `miyoo/` for real hardware (evdev input, framebuffer display, GPIO, battery via sysfs), `simulator/` for desktop development (winit + softbuffer). Selected at compile time via Cargo features (`miyoo` or `simulator`).
- **`view/`** — custom UI component system built on async `View` trait. Components: Label, Image, Button, TextBox, Toggle, ScrollList, StatusBar, etc. All views use `async_trait(?Send)`.
- **`display/`** — rendering via `tiny-skia` pixmaps to framebuffer/softbuffer.
- **`database.rs`** — SQLite with `rusqlite_migration`. Stores game metadata, play sessions, FTS for search.
- **`stylesheet.rs`** — theme system with color/font/wallpaper customization and theme inheritance.
- **`locale.rs`** — i18n via Fluent templates.
- **`retroarch.rs`** — UDP IPC protocol for controlling RetroArch (pause, save states, disk management).
- **`command.rs`** — command system for cross-view communication.

### Key data flows

- **Display pipeline:** Platform → Display → tiny-skia Pixmap → Framebuffer/Softbuffer
- **Input pipeline:** evdev/winit → Platform → KeyEvent → View handlers
- **Game launch:** Entry selected → alliumd notified → RetroArch spawned → Menu overlay ready

### Entry system (`allium-launcher/entry/`)

Three entry types: **Game** (ROM files with artwork/metadata), **Directory** (console folder navigation), **App** (`.pak` bundles).

### Environment variables

`ALLIUM_SD_ROOT`, `ALLIUM_BASE_DIR`, `ALLIUM_DATABASE` control runtime paths. Static resources live in `/static/` and deploy to `.allium/`, `Apps/`, `RetroArch/`, `Themes/` on device.

## Cargo Features

- `simulator` — desktop development mode (winit + softbuffer windowed rendering)
- `miyoo` — hardware-specific code (evdev, framebuffer, GPIO, proprietary FFI)

## CI

GitHub Actions runs: rustfmt check, unit tests, `cargo-deny` audit. Releases triggered by version tags; nightly builds from main.
