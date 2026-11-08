# Sunny

*Sunny* is a fast and lightweight desktop launcher / command palette — inspired by [Raycast](https://www.raycast.com/), a full port of [Vicinae](https://github.com/vicinaehq/vicinae) (C++/Qt) to **Rust**.

## Project Status

🚧 **Phase 0 — Foundation** is in progress. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Architecture

| Component               | Description                                                              |
|-------------------------|--------------------------------------------------------------------------|
| `apps/sunny`            | Tauri v2 application — web UI (Raycast-like interface) + Rust backend    |
| `crates/sun-core`       | Search engine (fuzzy), ranking, settings, storage                        |
| `crates/sun-platform`   | Windows platform abstraction: global hotkey, window enumeration, clipboard |
| `crates/sun-extensions` | (Future phase) Extension runtime, Raycast API compatibility              |

## Platform

**Windows** is prioritized first, followed by Linux and macOS.

## Build (Windows)

**Prerequisites:**

1. [Rust](https://rustup.rs) (toolchain `stable-x86_64-pc-windows-msvc`)
2. [Node.js](https://nodejs.org) ≥ 20
3. **Microsoft Visual Studio Build Tools** with the *Desktop development with C++* workload (MSVC linker + Windows SDK). For machines without VS:
