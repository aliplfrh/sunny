# Sunny Roadmap

Full port of Vicinae (C++23/Qt) → Rust + Tauri v2. Windows-first, GPL-3.0.

## Phase 0 — Foundation (current)

- [x] License decision: full port → GPL-3.0
- [x] UI stack decision: Tauri v2 (WebView2) for Raycast-like appearance
- [x] Platform decision: Windows first
- [x] Repo, LICENSE, NOTICE, README, ROADMAP, PORT-MAP
- [ ] Scaffold Cargo workspace (`crates/sun-core`, `crates/sun-platform`)
- [ ] Scaffold Tauri application (`apps/sunny`)
- [ ] Shallow clone Vicinae repo as porting reference (do not modify)
- [ ] Raycast-style overlay window: frameless, always-on-top, centered on active monitor, global hotkey (default: Alt+Space)

## Phase 1 — MVP (usable launcher)

- [ ] Fuzzy search engine (ported concept from Vicinae) + frequency & recency-based ranking
- [ ] Windows application index (Start Menu + installed apps)
- [ ] Calculator
- [ ] Raycast-style list view UI (display + keyboard bindings ↑↓, Enter, Esc, Tab)
- [ ] Application launching (proper process spawning)
- [ ] Autorun on startup (optional)

## Phase 2 — Core Vicinae Features

- [ ] Clipboard history (daemon + encrypted/optional storage)
- [ ] Text expander / snippet
- [ ] Emoji picker
- [ ] File search (fast index à la Everything/ripgrep)
- [ ] Window switcher

## Phase 3 — Extension System

- [ ] Design Sunny's own extension API
- [ ] Raycast script command subset compatibility
- [ ] (Optional/gradual) Raycast React extension compatibility

## Phase 4 — Parity with Vicinae

- [ ] Extension store
- [ ] AI chat / dictation (integration stub)
- [ ] Volume control, font browser, etc.
- [ ] Linux (wlr/X11) and macOS

## Porting Principles

1. **UI is rewritten, not translated**: QML → Raycast-like web UI; C++ logic → idiomatic Rust (safe, no line-by-line 1:1 equivalence).
2. **Vicinae references are mapped** in `docs/PORT-MAP.md` — each module lists its upstream source file.
3. **Windows first**, with abstraction in `sun-platform` so Linux/macOS can follow without a major refactor.
4. Each phase must yield a *usable* application, not a half-finished one.
