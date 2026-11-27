# Raycast UX teardown — findings for Sunny

> **Correction (Ghidra pass).** An earlier version of this document said root
> search and ranking live in `frontend/main-window-*.js`. That file is the
> renderer. The **backend** owns ordering: `backend/index.mjs` (Node) plus the
> Rust N-API addons. See section 0 below, which supersedes the parts of section 1
> that talk about "the renderer scores every candidate".

Source analysed on this machine, read-only, nothing executed from the target:

| Artifact | Path | Notes |
|---|---|---|
| `Raycast.dll` | `%ProgramFiles%\WindowsApps\Raycast.Raycast_2.14.0.0_x64__qypenmj9wpt2a\Raycast\` | .NET 10, IL-only, 6,588 types, 50,567 methods, sha256 `4891d526…fd8de` |
| `frontend\main-window-*.js` | same folder | 5.9 MB renderer; UI only — see section 0 |
| `backend\index.mjs` | same folder | 9.3 MB Node backend — **this is where root search is ordered** |
| `backend\indexer.win32-x64-msvc.node` | same folder | 18.4 MB Rust addon — content search (FTS5 + BM25) |
| `native.dll` | same folder | Rust + uniffi; coordinator/Sentry/telemetry only, **no search** |
| `Raycast.SystemService.exe` | same folder | NTFS MFT/USN **file** indexer, not an app scanner |
| `Raycast.UIAccess.exe` | same folder | shortcuts, snippets, window management — no app enumeration |
| `app_index.db`, `frecency.db`, `settings_v2.db` | `%LocalAppData%\Raycast\` | AES-encrypted; not opened |

There is **no Raycast APK** on this machine. Raycast for Windows is a .NET/WPF
app with a web renderer, not an Electron or Android app, so the APK route did
not apply.

## 0. Where search actually runs (Ghidra + structural pass)

Ghidra could not finish importing the three largest binaries: REA's provider hit
its 330 s startup deadline on `native.dll` (7.7 MB), `SystemService.exe` (4.5 MB)
and `UIAccess.exe` (5.1 MB). A 351 KB control target (`Raycast.exe`) imported in
45 s, so this is a capacity limit, not a broken provider. Conclusions below about
those three therefore rest on **exports, imports, embedded module paths and
strings**, not on decompiled instructions. Every claim is labelled accordingly.

| Binary | Role | Evidence |
|---|---|---|
| `native.dll` | supervisor + OS/telemetry | 866 exports, all uniffi: `Coordinator`, `EventHandler`, `NativeSentryClient`, `TelemetryScrubber`, `SentryFrameResolver`, plus clipboard/displays/confetti. **No** `search`/`rank`/`match`/`score`/`index` export. 68 embedded crates contain **no** `nucleo`, `fuzzy-matcher`, `fst`, `tantivy`, `rusqlite`, `strsim`. Imports contain **no** `IShellLinkW`, no `PackageManager`, no directory enumeration. |
| `Raycast.SystemService.exe` | NTFS **file** indexer | CLI: `usn-read`, `scan`, `install`, `status`. Strings: "No persisted snapshot for drive, enumerating MFT" → "MFT enumerated, ready", "Reading USN journal deltas". Imports: `DeviceIoControl`, `NtDeviceIoControlFile`, `FindFirstFileExW`, `ReadDirectoryChangesW`. **No `Reg*` imports at all**, no `IShellLink`, no `PackageManager`. Snapshot at `%LOCALAPPDATA%\…\NtfsScanner\snapshot.bin.zst`. |
| `Raycast.UIAccess.exe` | shortcuts / snippets / windows | Modules: `shortcuts/src/{low_level_keyboard_hook,snippets/*}`, `ui-access-helper/src/{foreground_tracker,arrange_window}`. Zero hits for `IShellLink`/`PackageManager`/`FindFirstFile`. |
| `Raycast.dll` (managed) | **app enumeration** | `IShellLinkW`, `IShellLinkDataList`, `shell:AppsFolder`, `PackageManager`, 53 `AppUserModelId` symbols, and two error strings: "Failed to enumerate AppUserModelIds from shell:AppsFolder" and "Failed to extract AppUserModelId from IShellLinkW for app {FriendlyName}". So: `shell:AppsFolder` (UWP/Store + shell-visible desktop apps) joined with `.lnk` parsing, keyed on AppUserModelId. |

**Root/command ranking** is Fuse.js 7.0.0 in `backend/index.mjs`:

```js
this.fuse = new uN(this.items, {
  includeScore: true, threshold: .3, location: 0, distance: 100,
  keys: [{name:`commandName`, weight:3}, {name:`displayName`, weight:2},
         {name:`partKey`, weight:2}, {name:`aliasKey`, weight:2},
         {name:`displayPartKey`, weight:1}]
})
```

`threshold: .3` is a hard admission gate. Ordering then uses a **7-element
lexicographic vector**, not a blended score:

```js
f = [ +!exact, +!aliasExact, startsWithLen, aliasStartsWithLen,
      fuseBucket, -descriptionHits, -scoreBoost ]
```

where `fuseBucket = Math.floor(fuseScore * 10)` — score differences are
deliberately flattened into ten buckets. Substring description hits are searched
separately and injected at a fixed bucket.

**File-path scorer** (hand-written, in `index.mjs`): precomputed `charBits`
bitmasks for O(1) subsequence rejection, then `+4` per adjacent query-char pair,
`−(3 + skippedChars)` per non-adjacent step, `+16` per matched char, plus
`max(0, 32 − ⌊len/4⌋)` for short paths, and a separate better-of branch when all
query chars fall inside the filename. Reported score is the rank percentile.

**Content search** is SQLite FTS5 with `bm25()` and a 3-tier cascade (0 = phrase,
1 = all terms, 2 = any term), ordered `c.tier, c.score, d.uri`, with `sqlite-vec`
embeddings, Windows OCR, and an `fst` Levenshtein fallback.

**Frecency decay** — previously listed here as unrecovered:

```js
decayedScore({entry, sampledAtMs}) {
  const n = this.now() - sampledAtMs;
  return n <= 0 ? entry.score : Math.max(1, entry.score * 2 ** (-n / aso));
}
```

with `aso = 864e6` → **10-day half-life, floored at 1**, default `frecency ?? 1`,
stored in `frecency.root_search (item_id, frecency_date, search_terms, opened_at)`.
Frecency is **not** in the 7-element vector; it only breaks ties later.

## 1. Ranking: what actually decides the order

The matcher below is the one recovered from the renderer's own scoring helpers
(`m5e`/`Kf`/`Gf`/`qf`). It is still the right model for a launcher that scores
per-candidate text, and Sunny's port is validated against it — but note from
section 0 that Raycast's *root search* additionally gates on Fuse's `threshold`
and re-orders with the 7-element vector. Treat section 1 as "how a candidate is
scored", not "how the list is ordered".

### The matcher is near-contiguous, not long-gap fuzzy

This is the finding that matters most. The dynamic-programming table only
carries two transitions: stay on the diagonal (consecutive characters), or
bridge **exactly one** skipped character. There is no long-gap transition.

Weights, recovered exactly:

| Event | Weight |
|---|---|
| first query character (wherever it lands) | `4` |
| character at a word boundary | `3` |
| default consecutive match | `2` |
| the query character matched a separator itself | `1` |

A word boundary is `\t`, space, `-`, `.`, `/`, `(`, `)`, `[`, `]`, or a
lowercase/digit followed by an uppercase letter (camelCase).

Because gaps beyond one character cannot be bridged, these are the actual
outcomes:

| Text | Query | Result |
|---|---|---|
| `settings` | `set` | 8 |
| `application` | `app` | 8 |
| `control panel display` | `cont` | 10 |
| `text-editor` | `editor` | 14 |
| `a-p-p` | `app` | 8 (separators are skippable) |
| `visual studio code` | `code` | 10 |
| `control panel display` | `cpd` | **no valid path** |
| `visual studio code` | `vsc` | **no valid path** |

So acronyms like `vsc` do **not** match by fuzzy alignment. Raycast serves them
through the per-command `keywords` array (302 declarations), which is tried only
when the title itself does not clear the bar. That is the mechanism behind the
"precise, not noisy" feel — the launcher is not guessing, it is matching
contiguous text or an explicitly declared alias.

### Sort order

```
title score (desc)  →  searchPriority (desc)
```

`exact` sorts as the maximum value and `unmatched` as the minimum. Beyond score
and priority, a later comparator applies three refinements: favourites first,
then `exact` title, then `exact` path.

### Admission thresholds

Matching is not enough; a candidate must also clear a tier:

| Tier | Rule |
|---|---|
| `low` | any match is accepted |
| `medium` | `score >= 1.5 * (query_len - skipped - 2) + 4` |
| `high` | `score > 2 * (query_len - skipped)` |

Root search uses **high**. This is why weak matches disappear instead of
trailing at the bottom of the list.

### Query normalisation

The query is `trimStart()` → `normalize("NFC")` → `toLowerCase()`, then
transliterated to ASCII before comparison. Titles are lowercased and cached per
item.

### What to port to Sunny

Sunny ranks with `SkimMatcherV2` and keeps whatever score comes back — no
admission gate. The highest-value change is not a new library but two rules:

1. **Near-contiguous matching** (≤1 skipped character) instead of long-gap
   fuzzy. This alone removes most noise, because a scattered subsequence stops
   scoring entirely.
2. **A high-tier admission gate** on the root search, so survivors are few.

Then, in order: per-command `keywords` to recover acronyms, `searchPriority`
as a tiebreak, and `requiresManualSelection` for risky commands.

## 2. Root search is a section engine, not a result list

Every command declares when it may appear and how strictly it must match:

```
rootSearch: { when: "empty" | "searching" | "always",
              matching: "default" | "always" | "url",
              section: "<name>",
              pinned?: n, pinnedWhileSearching?: n,
              alwaysMatching?: bool, maxItems?: n }
```

Observed section names: `apps`, `files`, `calculator`, `calendar`, `weather`,
`flight`, `travelTime`, `color`, `translate`, `updates`, `announcements`,
`preferences`, `walkthroughs`, `credits`, `vocabulary`, `agents`,
`personalization`, `keyboard`, `region`, `focus`, `flyout`, `general`, `tile`.

Counts: `when:"empty"` 15, `when:"searching"` 15, `matching:"always"` 7.
This is how Calculator, Weather and File Search appear *while typing* without
being real search results, and how announcements appear only on an empty box.

**Consequence for Sunny:** a command that always participates in ranking is
noise. Section + `when` + `matching` is what keeps the list short.

## 3. Discovery metadata every command carries

- `keywords`: 302 declarations. Some are functions of the payload, so a row can
  add keywords at runtime.
- `alias` / `aliases`: 135 occurrences; user-assigned, persisted per command.
- `searchPriority`: per-item integer, used as the first tiebreak after score.
- `requiresManualSelection: true`: never auto-selected on the first row — the
  user must move the selection before Enter runs it. Used for risky or
  surprising commands.
- `runOnConsecutiveHotkey`: pressing the hotkey again runs the top item.
- `omitFromFrecencyTracking`: ephemeral rows (indexing progress, transient
  status) never pollute history. 36 uses.
- `ephemeral`: row exists only for this session.

## 4. Frecency

`frecency` + `frecencyDate` are tracked per item; `resetRanking` is a per-command
action (15 uses) so a user can clear one bad habit. Ephemeral and
`omitFromFrecencyTracking` rows are excluded by construction, which is why
progress rows never crowd out real apps.

## 5. Settings

245 preference declarations, each with a shape like:

```
W({ default, storage: "synced" | "local" | "platform",
    preference, ai: "read-write" | "read-only" | "hidden",
    availability: "production" | "internal" | "development" })
```

| Axis | Distribution |
|---|---|
| storage | synced 165, local 124, platform 18 |
| AI exposure | read-write 116, hidden 60, read-only 32 |
| availability | internal 45, production 6, development 9 |

Notable design choices:

- **Every setting declares whether an agent may change it.** Most are
  `read-write`; 60 are hidden from AI entirely; 32 are read-only.
- **Availability gates the whole feature**, not just the row: `internal` rows
  ship but are hidden from production builds.
- Settings live in an extension-shaped registry (`settings`, `commands`,
  `preferences` per extension id), so built-in features and third-party
  extensions use one model.
- The Settings window itself is a separate renderer entry
  (`settings-window-*.js`, 2.1 MB) with its own routes (`/advanced`,
  `/extensions`), not a modal inside the launcher.

## 6. What was ported to Sunny

Implemented in this cycle, in `apps/sunny/ui/main.js` and `crates/sun-core`:

1. **`rootSearch` metadata per command.** `commandVisible()` replaces the old
   `cmd !== "files"` special case: `when`, `matching` (`default` | `named`),
   and `section` are declared on each command. `filesCommandVisible` is gone.
2. **Section integrity.** `mergeSections()` guarantees every section is
   contiguous, so the renderer's consecutive-change grouping emits each heading
   once. A named command no longer splits the app list into two "Results".
3. **`requiresManualSelection`.** Video Download refuses Enter until the user
   has moved the highlight; the row shows a muted marker and the status line
   explains what to press. Clicking a row or the Actions menu still counts.
4. **`keywords` aliases.** Commands carry explicit alias lists matched
   individually; apps get `AppEntry.keywords` (e.g. `vsc`, `code` for Visual
   Studio Code) so an acronym surfaces an app whose name does not contain it.
5. **Usage tiebreak.** `rank_with_usage` puts name matches above alias matches,
   then score, then launch count. A habit can only reorder an exact tie — it can
   never outrank a better match. `usage_count()` is the only history read.
6. **Verified matcher.** `crates/sun-core/src/match_rank.rs` is a port of
   Raycast's matcher, validated against 126 golden vectors generated from the
   shipped bundle (`scripts/raycast-matcher-reference.mjs`,
   `crates/sun-core/tests/fixtures/matcher-golden.json`). It is not yet wired
   into `search_apps`, which still uses `SkimMatcherV2`; adopting it is the next
   step and is a deliberate, separate change.

## 7. What to port next, in order

1. **Adopt the verified matcher in `search_apps`.** `match_rank` is ported and
   proven but still unused; switching `search_apps` and `files::search` to it is
   the remaining half of the ranking work, and needs a before/after comparison on
   a real index.
2. **`pinned` and `maxItems`** on `rootSearch`, now that sections are contiguous.
3. **`omitFromFrecencyTracking`** per row, once rows other than apps are tracked.
4. **Per-command `resetRanking`** action, so a bad habit is fixable.
5. **Settings: an AI/automation exposure field** only if Sunny ever exposes
   settings to an agent; today it has none, so this is deferred.

Not worth copying: the extension marketplace, synced settings, cloud sync,
pro-feature gating, and the `flyout`/`tile` surfaces — all out of scope for
Sunny per `AGENTS.md`.

## 8. Evidence limits

- No member-level CIL was read; the ranking above is read from the shipped
  renderer JavaScript, which is the code that actually runs.
- Frecency half-life and decay were not recovered. Only that ordering uses
  frecency after score and priority.
- `native.dll` was not decompiled beyond its exported symbol names.
- Encrypted databases were not decrypted, and their contents are not part of
  this document.
