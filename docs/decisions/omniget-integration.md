# Optional OmniGet integration

Approved by the maintainer in the 2026-11-18 implementation conversation: searching
**Video Download** opens a downloader inside Sunny's existing box, not a browser tab.
The maintainer confirmed using `tonhowtf/omniget` as an external CLI, and the narrow
exception to the update-only network policy. They subsequently requested an explicit
Settings download/setup option and pushing the integration to GitHub.

## Scope

- Sunny keeps its Rust/Tauri + vanilla JS architecture and two-window model.
- Only the installed `omniget-cli.exe` download subcommand is used, via direct
  process arguments. No shell, web iframe, extension runtime, MCP/agent features,
  unattended installation, or bundled OmniGet code/assets in Sunny's release.
- The CLI is resolved from absolute local PATH entries / standard installation
  directories, or an explicit local absolute path in General settings. The GUI
  executable and network/relative paths are not accepted. Sunny's managed tools
  directory is also checked after an explicit CLI setup.
- Only an explicit Download action contacts the user-supplied HTTP(S) URL.
  Searching, typing, opening the panel and checking installation do not contact it.
- Choose Best / 4K / 1080p / 720p / 480p / Audio only. Files go to Windows Downloads.
- One job runs at a time. Progress and completion stay in memory across launcher
  hide/reopen, including returning to search. Cancel terminates that CLI's Windows
  job and descendants; quitting Sunny also closes the kill-on-close job. Partial
  files may remain. Downloads have a six-hour runtime limit.
- No cookie/token import, DRM bypass, telemetry, or changes to global hotkeys.
  Login-dependent downloads remain OmniGet's responsibility.

## User-requested Settings setup

- **General → OmniGet Download & Setup → Download OmniGet CLI** downloads the
  official Windows x64 v1.0.0 ZIP from `getomniget.com`, verifies its published
  SHA-256, and extracts only `omniget-cli.exe` into
  `%LOCALAPPDATA%\Sunny\tools\omniget`. MCP, gateway and worker executables in the
  archive are not installed. No installer or downloaded executable is run by setup.
- The URL/hash are pinned in `sun-core::omniget_setup`, not selected from arbitrary
  web content. HTTPS, bounded redirects, a 64 MB package limit, a 256 MB extracted
  CLI limit, traversal/symlink/duplicate protection, and atomic replacement apply.
  Failed verification/replacement preserves an existing CLI. The saved path uses
  the normal Settings transaction/broadcast, not a separate configuration file.
- Connection stalls time out after 10 seconds, the transfer has a 120-second total
  deadline, and there is no application retry loop. Setup is single-flight and
  refuses to replace the CLI during an active media download.
- The download runs on a worker thread, never on the UI thread, and a panicking
  command replies with an error instead of leaving the frontend waiting. The UI also
  unlocks the control if no response arrives. v1.1.0 shipped without this and hung
  at "Please wait…" because `reqwest` had no `rustls` provider installed; the provider
  is now installed at startup and before each download.
- **Check requirements** only checks local file presence in common locations; it
  does not execute programs or contact the network, and may not see custom OmniGet
  overrides. Missing yt-dlp, FFmpeg/FFprobe and a JS runtime are reported separately
  from CLI availability. Upstream v1.0.0's engine provisioning can download these
  on the first user-requested video download. The UI warns that first use takes longer;
  CLI presence is not a guarantee that all sites, codecs or logins will work.
- The full OmniGet desktop app is optional. Users may still provide their own CLI.

## Compatibility and verification

Adapter contract inspected from upstream `src-tauri/omniget-cli/src/main.rs` and
`commands/download.rs`: `--json download --output <folder> [--quality <height> |
--audio-only] -- <url>`. Progress output currently contains Rust `Some`/`None`
debug values in optional fields; the parser accepts only the exact progress
prefix in that case and parses completion as strict JSON. Unknown log lines are
ignored; output lines/queues are bounded, and raw diagnostics/URL credentials
are not forwarded to the UI or stored in Sunny logs.

Tests use fabricated outputs and a native child-process fixture, never a real
media download or desktop keyboard injection. A real download with an installed,
compatible OmniGet CLI remains a separate acceptance check.
