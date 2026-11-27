// Sunny UI — komunikasi dengan backend Rust via Tauri command API.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const queryEl = document.getElementById("query");
const listEl = document.getElementById("list");
const detailEl = document.getElementById("detail");
const hintsEl = document.getElementById("hints");
const clipHeadEl = document.getElementById("clipHead");
const clipCountEl = document.getElementById("clipCount");
const clipFilterEl = document.getElementById("clipFilter");
const pauseBtnEl = document.getElementById("pauseBtn");
const pauseLabelEl = document.getElementById("pauseLabel");
const pauseIcoEl = document.getElementById("pauseIco");
const settingsEl = document.getElementById("settings");
const settingsBodyEl = document.getElementById("settingsBody");
const settingsBackEl = document.getElementById("settingsBack");
const settingsSavedEl = document.getElementById("settingsSaved");
const settingsTabsEl = document.getElementById("settingsTabs");
const settingsSearchEl = document.getElementById("settingsSearch");
const backdropEl = document.getElementById("backdrop");
const winMinEl = document.getElementById("winMin");
const winMaxEl = document.getElementById("winMax");
const winCloseEl = document.getElementById("winClose");
const modeBackEl = document.getElementById("modeBack");
const actionsEl = document.getElementById("actions");
const actionsBtnEl = document.getElementById("actionsBtn");
const launcherStatusEl = document.getElementById("launcherStatus");
const qrPreviewEl = document.getElementById("qrPreview");
const qrPromptEl = document.getElementById("qrPrompt");
const qrCopyEl = document.getElementById("qrCopy");
const videoPanelEl = document.getElementById("videoPanel");
const videoSetupEl = document.getElementById("videoSetup");
const videoQualityEl = document.getElementById("videoQuality");
const videoStartEl = document.getElementById("videoStart");
const videoCancelEl = document.getElementById("videoCancel");
const videoProgressEl = document.getElementById("videoProgress");
const videoStatusEl = document.getElementById("videoStatus");
const videoFileEl = document.getElementById("videoFile");
const videoRevealEl = document.getElementById("videoReveal");
const videoRetryEl = document.getElementById("videoRetry");

// Satu bundel UI dipakai dua jendela: launcher (`main`) dan Settings
// (`settings`). Query string membedakan peran keduanya.
const IS_SETTINGS_WINDOW =
  new URLSearchParams(location.search).get("window") === "settings";
if (IS_SETTINGS_WINDOW) document.body.classList.add("settings-window");

let mode = "apps"; // "apps" | "emoji" | "clip" | "files" | "qr" | "video"
let settingsOpen = false;
let capturingHotkey = false;

// Preferensi (default aman; ditimpa oleh settings_get saat startup).
let SETTINGS = {
  open_at_login: false,
  show_in_tray: true,
  hotkey: "Alt+Space",
  font: "sonora",
  font_size: 14,
  opacity: 100,
  blur_ui: false,
  window_mode: "expanded",
  show_footer: true,
  backspace_back: true,
  vim_nav: false,
  include_files: true,
  file_results: 8,
  theme: "system",
  accent: "#f08c28",
  omniget_cli_path: "",
};
let SETTINGS_ERROR = "";
let saveTimer = null;
let lastPreviewAt = 0;
let confirmedSettings = { ...SETTINGS };
let settingsRevision = 0;
let settingsBroadcastSeq = 0;
let pendingSettings = new Map();
let settingsSavePromise = null;
let settingsSaveRequested = false;
let settingsRenderPending = false;
const settingsSaveErrors = new Map();

let ALL_EMOJI = []; // diisi async saat startup
invoke("emoji_all").then((data) => {
  ALL_EMOJI = data;
  refresh();
});
const settingsReady = invoke("settings_get")
  .then((s) => {
    // A broadcast received during hydration is newer than the startup snapshot.
    if (!settingsBroadcastSeq) acceptSettings(s);
    if (!IS_SETTINGS_WINDOW && mode === "apps") refresh();
  })
  .catch((error) => {
    SETTINGS_ERROR = `Could not load settings: ${error}`;
    refreshSettingsView();
  });
let items = [];
let selected = 0;
let launcherHome = { favorites: [], suggestions: [] };
let homeSeq = 0;
let actionsOpen = false;
let actionSelected = 0;
let favoriteBusy = false;
let activating = false; // Prevent repeated external launches until reopened or an error.

// ── State khusus mode clipboard ──
let paused = false; // watcher pause?
let clipType = "all"; // filter dropdown: "all" | "text"
let clipTotal = 0; // jumlah entri sebelum difilter (untuk "74 items")

// ── State khusus mode file ──
let fileIndexReady = false;
let fileTotal = 0;
let refreshSeq = 0; // penjaga balapan hasil pencarian async
let searchTimer = null;
let qrReadyText = "";
let videoInstallation = null;
let videoChecking = false;
let videoCheckSeq = 0;
let videoSubmitting = false;
let videoCancelling = false;
let omnigetSetupBusy = false;
let omnigetSetupMessage = "Download the official Windows CLI (~25 MB). OmniGet prepares additional engines on first use.";
let omnigetSetupPercent = null;
let omnigetSetupView = null;
let videoState = { id: 0, revision: 0, phase: "idle", percent: null, file_path: null, message: "Paste a video URL to begin." };

// Perintah internal — hanya muncul saat dicari (bukan di daftar default).
const COMMANDS = [
  { kind: "cmd", cmd: "clip", icon: "📋", name: "Clipboard History",
    keys: "clipboard history riwayat clipboard papan klip" },
  { kind: "cmd", cmd: "emoji", icon: "🙂", name: "Emoji Picker",
    keys: "emoji picker emotikon" },
  { kind: "cmd", cmd: "files", icon: "📁", name: "Search Files",
    keys: "search files file folder find dokument dokumen berkas" },
  { kind: "cmd", cmd: "qr", icon: "▦", name: "QR Code Generator",
    keys: "qr code qrcode qr generator barcode" },
  { kind: "cmd", cmd: "video", icon: "↓", name: "Video Download",
    keys: "video download downloader omniget youtube audio" },
  { kind: "cmd", cmd: "settings", icon: "⚙️", name: "Settings",
    keys: "settings preferences setelan pengaturan" },
];

const MODE_LABEL = {
  apps: "Search apps…",
  emoji: "Search emoji…",
  clip: "Browse clipboard history…",
  files: "Search files and folders…",
  qr: "Enter text or URL for QR code…",
  video: "Paste a video URL to download…",
};
queryEl.placeholder = MODE_LABEL[mode];

/* ── Quick Web Search ──────────────────────────────────── */

// Engine list comes from the backend so Settings and the launcher cannot drift.
// A local fallback keeps `?` working during startup, before the call resolves.
const FALLBACK_ENGINES = [
  { id: "google", name: "Google", template: "https://www.google.com/search?q={query}" },
];
let SEARCH_ENGINES = [];

invoke("search_engines")
  .then((engines) => {
    if (Array.isArray(engines) && engines.length) SEARCH_ENGINES = engines;
  })
  .catch(() => {});

function engineList() {
  return SEARCH_ENGINES.length ? SEARCH_ENGINES : FALLBACK_ENGINES;
}

function currentEngine() {
  const id = SETTINGS.search_engine || "google";
  const list = engineList();
  return list.find((engine) => engine.id === id) || list[0] || null;
}

/* ── Backdrop desktop (untuk opacity background-only) ──── */

function setBackdrop(url) {
  if (!backdropEl) return;
  // Failed captures must clear the previous desktop, not leave a stale image.
  const valid = typeof url === "string" && /^data:image\/png;base64,[A-Za-z0-9+/=\r\n]+$/.test(url);
  backdropEl.style.backgroundImage = valid ? `url("${url}")` : "none";
}
let backdropFresh = false;
// Saat start, snapshot mungkin belum selesai diambil → coba beberapa kali.
let backdropTries = 0;
function loadBackdrop() {
  // Jendela Settings punya snapshot sendiri (area tangkapnya berbeda).
  const cmd = IS_SETTINGS_WINDOW ? "settings_backdrop_get" : "backdrop_get";
  invoke(cmd)
    .then((url) => {
      setBackdrop(url);
      if (!url && backdropTries++ < 8) setTimeout(loadBackdrop, 250);
    })
    .catch(() => {});
}
loadBackdrop();
listen("sunny:backdrop", (e) => {
  if (IS_SETTINGS_WINDOW) return;
  backdropFresh = true;
  setBackdrop(e.payload);
});
listen("sunny:settings-backdrop", (e) => {
  if (!IS_SETTINGS_WINDOW) return;
  setBackdrop(e.payload);
});

function hideWindow() {
  if (IS_SETTINGS_WINDOW) {
    invoke("settings_close").catch(() => {});
    return;
  }
  invoke("hide_main_window").catch((error) => {
    // A failed hide must not leave every later launch blocked by the guard.
    activating = false;
    showLauncherError(`Could not hide Sunny: ${error}`);
  });
}

/* ── Util tampilan ala Raycast ─────────────────────────── */

function escapeHtml(s) {
  const map = {
    "&": "\u0026amp;",
    "<": "\u0026lt;",
    ">": "\u0026gt;",
    '"': "\u0026quot;",
    "'": "\u0026#39;",
  };
  return String(s).replace(/[&<>"']/g, (c) => map[c]);
}

function relTime(ts) {
  const diff = Math.max(0, Date.now() - ts);
  const s = Math.floor(diff / 1000);
  if (s < 5) return "just now";
  if (s < 60) return `${s} seconds ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return m === 1 ? "1 minute ago" : `${m} minutes ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return h === 1 ? "1 hour ago" : `${h} hours ago`;
  const d = Math.floor(h / 24);
  return d === 1 ? "1 day ago" : `${d} days ago`;
}

function fmtSize(b) {
  if (!b && b !== 0) return "—";
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / (1024 * 1024)).toFixed(2)} MB`;
}

function fmtDate(ts) {
  const d = new Date(ts);
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const mon = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  const p = (n) => String(n).padStart(2, "0");
  return `${days[d.getDay()]} ${mon[d.getMonth()]} ${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())} ${d.getFullYear()}`;
}

function clipTitle(c) {
  if (c.kind === "image") return `Image (${c.width || "?"}×${c.height || "?"})`;
  const first = String(c.text).split("\n").find((l) => l.trim()) || "";
  return first.trim();
}

/* ── Ikon baris hasil ──────────────────────────────────── */

const IMAGE_EXT = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff"];
const VIDEO_EXT = ["mp4", "mkv", "mov", "avi", "webm", "wmv", "m4v"];
const AUDIO_EXT = ["mp3", "wav", "flac", "m4a", "ogg", "aac", "wma"];
const DOC_EXT = ["pdf", "doc", "docx", "txt", "md", "rtf", "odt", "pages"];
const SHEET_EXT = ["xls", "xlsx", "csv", "ods", "numbers"];
const ARCHIVE_EXT = ["zip", "rar", "7z", "tar", "gz", "bz2", "xz"];
const CODE_EXT = ["rs", "js", "ts", "tsx", "jsx", "py", "go", "c", "cpp", "h", "java", "json", "toml", "yaml", "yml", "html", "css", "sh", "ps1"];

function fileGlyph(item) {
  if (item.is_dir) return "📁";
  const ext = (item.name.split(".").pop() || "").toLowerCase();
  if (IMAGE_EXT.includes(ext)) return "🖼";
  if (VIDEO_EXT.includes(ext)) return "🎬";
  if (AUDIO_EXT.includes(ext)) return "🎵";
  if (ext === "pdf") return "📕";
  if (SHEET_EXT.includes(ext)) return "📊";
  if (DOC_EXT.includes(ext)) return "📄";
  if (ARCHIVE_EXT.includes(ext)) return "🗜";
  if (CODE_EXT.includes(ext)) return "🧾";
  if (["exe", "msi", "bat", "cmd", "lnk"].includes(ext)) return "⚙";
  return "📄";
}

/** Folder induk, dengan home dipendekkan menjadi "~" ala Raycast. */
function parentDir(path) {
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  const dir = i > 0 ? path.slice(0, i) : path;
  const user = dir.match(/^[A-Za-z]:\\Users\\[^\\]+/);
  if (user) return "~" + dir.slice(user[0].length);
  return dir;
}

/** One-line preview of generated lorem text; full text is copied on Enter. */
function loremPreview(text) {
  const flat = String(text).replace(/\s+/g, " ").trim();
  return flat.length > 96 ? flat.slice(0, 96).trimEnd() + "…" : flat;
}

function iconElFor(item) {
  const icon = document.createElement("div");
  icon.className = "icon";
  if (item.kind === "calc") {
    icon.classList.add("calc");
    icon.textContent = "=";
  } else if (item.kind === "cmd") {
    icon.textContent = item.icon;
  } else if (item.kind === "emoji") {
    icon.classList.add("emoji");
    icon.textContent = item.emoji;
  } else if (item.kind === "clip") {
    icon.classList.add(item.kind2 === "image" ? "type-image" : "type-text");
    icon.textContent = item.kind2 === "image" ? "🖼" : "T";
  } else if (item.kind === "file") {
    icon.classList.add("file");
    icon.textContent = fileGlyph(item);
  } else if (item.kind === "lorem") {
    icon.classList.add("calc");
    icon.textContent = "¶";
  } else if (item.kind === "web") {
    icon.classList.add("calc");
    icon.textContent = "🔎";
  } else if (item.icon) {
    const img = document.createElement("img");
    img.src = item.icon;
    img.alt = "";
    img.draggable = false;
    icon.appendChild(img);
  } else {
    icon.textContent = "🚀"; // fallback bila ikon belum tersedia
  }
  return icon;
}

/* ── Render: list + panel detail + header + footer ─────── */

function renderDetail(c) {
  if (!c) {
    detailEl.hidden = true;
    detailEl.innerHTML = "";
    return;
  }
  detailEl.hidden = false;

  const isImg = c.kind2 === "image";
  const preview = document.createElement("div");
  preview.className = "preview";
  if (isImg && c.dataUrl) {
    const img = document.createElement("img");
    img.className = "shot";
    img.src = c.dataUrl;
    preview.appendChild(img);
  } else {
    const full = document.createElement("div");
    full.className = "clip-full";
    full.textContent = c.text;
    preview.appendChild(full);
  }

  const meta = document.createElement("div");
  meta.className = "meta";
  const rows = [
    ["Type", isImg ? `Image<span class="tag">🏷</span>` : `Plain Text<span class="tag">🏷</span>`, false],
    ["Size", fmtSize(c.size), false],
    ["Copied at", fmtDate(c.id), false],
    ["MD5", escapeHtml(c.hash || "—"), true],
  ];
  for (const [k, v, mono] of rows) {
    const row = document.createElement("div");
    row.className = "row";
    const key = document.createElement("span");
    key.className = "k";
    key.textContent = k;
    const val = document.createElement("span");
    val.className = "v" + (mono ? " mono" : "");
    val.innerHTML = v;
    row.append(key, val);
    meta.appendChild(row);
  }

  detailEl.innerHTML = "";
  detailEl.append(preview, meta);
}

/* Footer labels reflect actual actions; emoji has no implicit entry shortcut. */
const FOOTER_HINTS = {
  clip: { title: "Clipboard History", icon: true, actions: [
    ["Enter", "Paste"], ["Ctrl+B", "Copy"], ["Ctrl+D", "Delete"],
  ] },
  emoji: { title: "Emoji Picker", actions: [["Enter", "Copy"]], back: true },
  video: { title: "Video Download", actions: [["Enter", "Download"]], back: true },
  qr: { title: "QR Code Generator", actions: [["Enter", "Copy Image"]], back: true },
  files: { title: "Search Files", icon: true, actions: [["Enter", "Open"], ["Ctrl+R", "Reveal"]], actionsMenu: true },
};

function footerHintKeys(keys) {
  return keys.split("+").map((key) => `<kbd>${key}</kbd>`).join("");
}

function footerActionsButton() {
  const modifier = SETTINGS.vim_nav ? "<kbd>Ctrl</kbd><kbd>Shift</kbd>" : "<kbd>Ctrl</kbd>";
  const disabled = hasActions(items[selected]) ? "" : " disabled";
  return `<button class="hint-action hint-button" type="button" data-actions aria-haspopup="menu" aria-controls="actions" aria-expanded="${actionsOpen}"${disabled}>${modifier}<kbd>K</kbd> Actions</button>`;
}

function renderFooter() {
  const hints = FOOTER_HINTS[mode] ?? { title: "Sunny", icon: true, actions: [["Enter", "Open"]], actionsMenu: true };
  const icon = hints.icon ? '<img class="hint-icon" src="sunny-icon.png" alt="" draggable="false" />' : "";
  const actions = hints.actions.map(([keys, label]) =>
    `<span class="hint-action">${footerHintKeys(keys)} ${label}</span>`).join('<span class="hint-sep"></span>');
  const trailing = hints.back
    ? '<button class="hint-action hint-button" type="button" data-back>Back <kbd>Esc</kbd></button>'
    : hints.actionsMenu ? footerActionsButton() : "";
  hintsEl.innerHTML = `
      <span class="hint-strong">${icon}${hints.title}</span>
      <span class="spacer"></span>
      ${actions}${trailing ? `<span class="hint-sep"></span>${trailing}` : ""}`;
  hintsEl.querySelector("[data-actions]")?.addEventListener("click", toggleActions);
  hintsEl.querySelector("[data-back]")?.addEventListener("click", backToApps);
}

/* Move the highlight without rebuilding rows, so icons are not re-decoded. */
function syncSelectionDom() {
  [...listEl.querySelectorAll(".item")].forEach((el, j) => {
    el.classList.toggle("selected", j === selected);
    el.setAttribute("aria-selected", String(j === selected));
  });
  const item = items[selected];
  if (item) queryEl.setAttribute("aria-activedescendant", `result-${selected}`);
  else queryEl.removeAttribute("aria-activedescendant");
  const canAct = hasActions(item);
  actionsBtnEl.hidden = !canAct;
  actionsBtnEl.disabled = !canAct;
  const title = item ? actionsTitle(item) : "Actions";
  actionsBtnEl.title = `${title} (Ctrl+K)`;
  actionsBtnEl.setAttribute("aria-label", title);
  renderFooter();
  if (mode === "clip") renderDetail(item || null);
}

function render() {
  modeBackEl.hidden = mode === "apps";
  listEl.hidden = mode === "qr" || mode === "video";
  videoPanelEl.hidden = mode !== "video";
  if (mode === "video") renderVideo();
  qrPromptEl.hidden = mode !== "qr" || !!qrReadyText;
  qrPreviewEl.hidden = mode !== "qr" || !qrReadyText;
  qrCopyEl.disabled = !qrReadyText;
  queryEl.setAttribute("aria-controls", mode === "video" ? "videoPanel" : mode === "qr" ? "qrPreview" : "list");
  queryEl.setAttribute("aria-expanded", String(mode !== "qr" && mode !== "video"));
  queryEl.setAttribute("aria-label", mode === "video" ? "Video URL" : "Search Sunny");
  if (mode === "video") {
    queryEl.removeAttribute("role");
    queryEl.removeAttribute("aria-expanded");
    queryEl.removeAttribute("aria-autocomplete");
    queryEl.setAttribute("aria-describedby", "videoSetup videoStatus");
  } else {
    queryEl.setAttribute("role", "combobox");
    queryEl.setAttribute("aria-autocomplete", "list");
    queryEl.removeAttribute("aria-describedby");
  }
  const canAct = hasActions(items[selected]);
  actionsBtnEl.hidden = !canAct;
  actionsBtnEl.disabled = !canAct;
  actionsBtnEl.title = `${actionsTitle(items[selected])} (Ctrl+K)`;
  actionsBtnEl.setAttribute("aria-label", actionsTitle(items[selected]));
  const isClip = mode === "clip";
  clipHeadEl.hidden = !isClip;
  clipFilterEl.hidden = !isClip;
  if (isClip) {
    clipCountEl.textContent = `${clipTotal} items`;
    pauseLabelEl.textContent = paused ? "Resume clipboard" : "Pause clipboard";
    pauseIcoEl.textContent = paused ? "▶" : "⏸";
    pauseBtnEl.classList.toggle("paused", paused);
  }

  listEl.innerHTML = "";
  const empty = !items.length;
  if (empty) {
    const div = document.createElement("div");
    div.className = "empty";
    div.textContent =
      isClip ? (clipTotal ? "Nothing matches the filter." : "Clipboard history is empty.")
      : mode === "emoji" ? "No matching emoji."
      : mode === "files" ? (!fileIndexReady
          ? "Indexing your files in the background…"
          : (fileTotal ? (queryEl.value ? "No matching files or folders." : "Type to search files and folders.")
                       : "No files were found to index."))
      : queryEl.value ? "Nothing matches."
      : "Sunny is ready. Search for an app, a calculation, or a command such as Emoji Picker.";
    listEl.appendChild(div);
  }

  let groupEl = listEl;
  let groupName = "";
  items.forEach((item, i) => {
    if (item.group && item.group !== groupName) {
      groupName = item.group;
      groupEl = document.createElement("div");
      groupEl.className = "result-group";
      groupEl.setAttribute("role", "group");
      const heading = document.createElement("h2");
      heading.className = "result-heading";
      heading.id = `result-group-${i}`;
      heading.textContent = groupName;
      groupEl.setAttribute("aria-labelledby", heading.id);
      groupEl.appendChild(heading);
      listEl.appendChild(groupEl);
    }
    const row = document.createElement("div");
    row.className = "item" + (i === selected ? " selected" : "");
    row.setAttribute("role", "option");
    row.id = `result-${i}`;
    row.setAttribute("aria-selected", String(i === selected));

    row.append(iconElFor(item));

    const name = document.createElement("div");
    name.className = "name";
    if (item.kind === "calc") {
      name.innerHTML = `${escapeHtml(item.expr)} <b class="val">= ${escapeHtml(item.value)}</b>`;
    } else if (item.kind === "lorem") {
      name.textContent = loremPreview(item.text);
    } else if (item.kind === "web") {
      name.textContent = `Search ${item.engineName} for “${item.text}”`;
    } else if (item.kind === "clip") {
      name.className += " clip";
      name.textContent = clipTitle(item);
    } else if (item.kind === "cmd") {
      name.textContent = item.name;
    } else if (item.kind === "emoji") {
      name.innerHTML = `${escapeHtml(item.emoji)} <span class="sub-inline">${escapeHtml(item.name)}</span>`;
    } else {
      name.textContent = item.name;
    }

    row.append(name);

    if (item.kind === "clip") {
      const t = document.createElement("span");
      t.className = "time";
      t.textContent = relTime(item.id);
      row.appendChild(t);
    } else if (item.kind === "file") {
      const sub = document.createElement("div");
      sub.className = "sub file-path";
      sub.textContent = parentDir(item.path);
      sub.title = item.path;
      row.appendChild(sub);
    } else {
      const sub = document.createElement("div");
      sub.className = "sub";
      if (item.kind === "calc") sub.textContent = "Enter: copy";
      else if (item.kind === "lorem") sub.textContent = "Enter: copy";
      else if (item.kind === "web") sub.textContent = "Enter: open in browser";
      else if (item.kind === "emoji") sub.textContent = "Enter: copy";
      else if (item.kind === "cmd") sub.textContent = "Enter: open";
      else sub.textContent = item.category || "Application";
      row.appendChild(sub);
    }

    row.onclick = () => activate(i);
    row.oncontextmenu = (e) => {
      if (!hasActions(item)) return;
      e.preventDefault();
      closeActions(false);
      selected = i;
      render();
      toggleActions();
    };
    row.onmousemove = () => {
      if (actionsOpen) return;
      if (selected !== i) {
        selected = i;
        syncSelectionDom();
      }
    };
    groupEl.appendChild(row);
  });
  if (items[selected]) queryEl.setAttribute("aria-activedescendant", `result-${selected}`);
  else queryEl.removeAttribute("aria-activedescendant");

  if (isClip) {
    renderDetail(items[selected] || null);
  } else {
    detailEl.hidden = true;
    detailEl.innerHTML = "";
  }
  renderFooter();
}

/* App actions are shared by the footer, toolbar, context menu, and keyboard. */
function showLauncherError(error = "") {
  launcherStatusEl.textContent = error ? String(error) : "";
  launcherStatusEl.hidden = !error;
  launcherStatusEl.classList.remove("info");
}

/** Non-error confirmation that never hides itself behind an error style. */
function showLauncherNote(text) {
  launcherStatusEl.textContent = text;
  launcherStatusEl.hidden = false;
  launcherStatusEl.classList.add("info");
}

/** Copy through the WebView API, falling back to the native clipboard. */
async function copyText(text) {
  const copied = await navigator.clipboard?.writeText(text).then(() => true).catch(() => false);
  if (copied) return true;
  try {
    await invoke("copy_to_clipboard", { text });
    return true;
  } catch (error) {
    showLauncherError(`Could not copy: ${error}`);
    return false;
  }
}

function isFavorite(path) {
  const key = String(path).toLowerCase();
  return launcherHome.favorites.some((app) => app.path.toLowerCase() === key);
}

/* Actions exist only for rows that have real commands behind them. */
function hasActions(item) {
  return item?.kind === "app" || item?.kind === "file";
}

function actionsTitle(item) {
  return item?.kind === "file" ? "File actions" : "App actions";
}

/** Copy a full path without hiding the launcher, so nearby rows stay usable. */
async function copyFilePath(item) {
  closeActions();
  try {
    const text = String(item.path);
    const copied = await navigator.clipboard?.writeText(text).then(() => true).catch(() => false);
    if (!copied) await invoke("copy_to_clipboard", { text });
    showLauncherNote(`Copied path: ${text}`);
  } catch (error) {
    showLauncherError(`Could not copy the path: ${error}`);
  }
}

/** Show the folder with the file selected; a missing folder surfaces its error. */
function revealFilePath(item) {
  closeActions();
  invoke("reveal_path", { path: item.path })
    .then(() => showLauncherError())
    .catch((error) => showLauncherError(`Could not show this file: ${error}`));
}

async function refreshHome() {
  if (IS_SETTINGS_WINDOW || favoriteBusy) return launcherHome;
  const seq = ++homeSeq;
  const home = await invoke("launcher_home");
  if (seq === homeSeq) launcherHome = home;
  return launcherHome;
}

function closeActions(restoreFocus = true) {
  actionsOpen = false;
  actionsEl.hidden = true;
  actionsBtnEl.setAttribute("aria-expanded", "false");
  renderFooter();
  if (restoreFocus) queryEl.focus();
}

function focusAction(index) {
  const buttons = [...actionsEl.querySelectorAll("button")];
  actionSelected = Math.max(0, Math.min(index, buttons.length - 1));
  buttons.forEach((button, i) => {
    button.tabIndex = i === actionSelected ? 0 : -1;
    button.classList.toggle("selected", i === actionSelected);
  });
  buttons[actionSelected]?.focus();
}

async function setAppFavorite(item) {
  if (favoriteBusy) return;
  favoriteBusy = true;
  ++homeSeq; // Invalidate older home requests before the mutation.
  showLauncherError();
  closeActions();
  try {
    launcherHome = await invoke("app_set_favorite", { path: item.path, favorite: !isFavorite(item.path) });
    ++homeSeq;
    await refresh();
  } catch (error) {
    showLauncherError(error);
  } finally {
    favoriteBusy = false;
  }
}

function toggleActions() {
  if (actionsOpen) {
    closeActions();
    return;
  }
  const item = items[selected];
  if (!hasActions(item) || favoriteBusy) return;
  actionsEl.innerHTML = "";
  const actions = item.kind === "file"
    ? [
        ["Open File", () => { closeActions(); activate(items.indexOf(item)); }],
        ["Copy Full Path", () => copyFilePath(item)],
        ["Show in Explorer", () => revealFilePath(item)],
      ]
    : [
        ["Open Application", () => { closeActions(); activate(items.indexOf(item)); }],
        [isFavorite(item.path) ? "Remove from Favorites" : "Add to Favorites", () => setAppFavorite(item)],
      ];
  for (const [label, action] of actions) {
    const button = el("button", "action-item", label);
    button.type = "button";
    button.setAttribute("role", "menuitem");
    button.onclick = action;
    actionsEl.appendChild(button);
  }
  actionsOpen = true;
  actionsEl.hidden = false;
  actionsEl.setAttribute("aria-label", actionsTitle(item));
  actionsBtnEl.setAttribute("aria-expanded", "true");
  renderFooter();
  focusAction(0);
}

function backToApps() {
  closeActions(false);
  qrReadyText = "";
  mode = "apps";
  queryEl.value = "";
  queryEl.placeholder = MODE_LABEL.apps;
  refresh();
  queryEl.focus();
}

/* ── Settings: penerapan + panel ───────────────────────── */

/** Tema efektif: "system" mengikuti preferensi Windows (dark/light). */
function effectiveTheme() {
  if (SETTINGS.theme === "light" || SETTINGS.theme === "dark") {
    return SETTINGS.theme;
  }
  return window.matchMedia?.("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

function applyAppearance() {
  document.documentElement.style.setProperty("--fs", SETTINGS.font_size + "px");
  // Opacity = alpha tint background saja. Teks/ikon di atas sheet tetap pekat.
  const pct = Math.min(100, Math.max(40, Number(SETTINGS.opacity) || 100));
  document.documentElement.style.setProperty("--bg-tint", (pct / 100).toFixed(2));
  applyAccent(SETTINGS.accent);
  const theme = effectiveTheme();
  document.body.classList.toggle("theme-light", theme === "light");
  document.body.classList.toggle("theme-dark", theme === "dark");
  document.body.classList.toggle("font-system", SETTINGS.font === "system");
  document.body.classList.toggle("blur-ui", !!SETTINGS.blur_ui);
  document.body.classList.toggle("compact", SETTINGS.window_mode === "compact");
  document.body.classList.toggle("no-footer", !SETTINGS.show_footer);
}

/* ── Warna aksen ───────────────────────────────────────── */

const DEFAULT_ACCENT = "#f08c28";

// Preset yang enak dipakai di tema gelap maupun terang.
const ACCENT_PRESETS = [
  ["Sunny", "#f08c28"],
  ["Amber", "#e8a33d"],
  ["Coral", "#f2685a"],
  ["Rose", "#e8557f"],
  ["Violet", "#9b6cf0"],
  ["Indigo", "#5b7cf5"],
  ["Sky", "#3aa8e8"],
  ["Teal", "#2bb5a0"],
  ["Green", "#3fae5a"],
  ["Graphite", "#8a8a94"],
];

function normalizeHex(input) {
  const s = String(input || "").trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{3}$/.test(s)) {
    return "#" + s.split("").map((c) => c + c).join("").toLowerCase();
  }
  if (/^[0-9a-fA-F]{6}$/.test(s)) return "#" + s.toLowerCase();
  return null;
}

function hexToRgb(hex) {
  const h = normalizeHex(hex) || DEFAULT_ACCENT;
  return [
    parseInt(h.slice(1, 3), 16),
    parseInt(h.slice(3, 5), 16),
    parseInt(h.slice(5, 7), 16),
  ];
}

/// Terapkan aksen ke CSS variable. Turunannya (soft/border) dihitung di sini
/// supaya CSS tidak perlu tahu format warnanya.
function applyAccent(hex) {
  const color = normalizeHex(hex) || DEFAULT_ACCENT;
  const [r, g, b] = hexToRgb(color);
  const root = document.documentElement.style;
  root.setProperty("--accent", `rgba(${r}, ${g}, ${b}, 0.92)`);
  root.setProperty("--accent-rgb", `${r}, ${g}, ${b}`);
  root.setProperty("--accent-soft", `rgba(${r}, ${g}, ${b}, 0.18)`);
  root.setProperty("--accent-border", `rgba(${r}, ${g}, ${b}, 0.55)`);
}

// Ikuti perubahan tema Windows saat setelan "System" aktif.
window
  .matchMedia?.("(prefers-color-scheme: light)")
  ?.addEventListener?.("change", () => {
    if (SETTINGS.theme === "system") applyAppearance();
  });

function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text != null) node.textContent = text;
  return node;
}

function settingsSection(title, rows) {
  const sec = el("section", "settings-section");
  sec.appendChild(el("h3", null, title));
  const group = el("div", "settings-group");
  group.append(...rows);
  sec.appendChild(group);
  return sec;
}

function settingsRow(label, desc, control) {
  const row = el("div", "settings-row");
  const left = el("div", "label");
  left.appendChild(el("div", "t", label));
  if (desc) left.appendChild(el("div", "d", desc));
  const right = el("div", "control");
  right.appendChild(control);
  const controls = [control, ...control.querySelectorAll("button, input, select")]
    .filter((node) => ["BUTTON", "INPUT", "SELECT"].includes(node.tagName));
  controls.forEach((node, i) => {
    node.dataset.settingsFocus = `${label}-${i}`;
    if (node.getAttribute("aria-label")) return;
    const text = node.textContent?.trim();
    if (node.tagName === "BUTTON" && (text === "+" || text === "−")) {
      node.setAttribute("aria-label", `${text === "+" ? "Increase" : "Decrease"} ${label}`);
    } else if (node.tagName !== "BUTTON" || !text) {
      node.setAttribute("aria-label", label);
    }
  });
  row.append(left, right);
  return row;
}

function toggleControl(on, onChange) {
  const btn = el("button", "switch" + (on ? " on" : ""));
  btn.type = "button";
  btn.setAttribute("role", "switch");
  btn.setAttribute("aria-checked", String(on));
  btn.onclick = () => {
    const next = !btn.classList.contains("on");
    btn.classList.toggle("on", next);
    btn.setAttribute("aria-checked", String(next));
    onChange(next);
  };
  return btn;
}

function fontControl() {
  const sel = el("select", "select");
  for (const [value, label] of [["sonora", "Sonora (Inter)"], ["system", "System"]]) {
    const o = el("option", null, label);
    o.value = value;
    sel.appendChild(o);
  }
  sel.value = SETTINGS.font;
  sel.onchange = () => onSettingChange({ font: sel.value }, true);
  return sel;
}

/** Engine picker for Quick Web Search; falls back to a plain value if the
    backend list has not arrived yet. */
function searchEngineControl() {
  const sel = el("select", "select");
  const list = engineList();
  for (const engine of list) {
    const option = el("option", null, engine.name);
    option.value = engine.id;
    sel.appendChild(option);
  }
  sel.value = SETTINGS.search_engine || "google";
  sel.onchange = () => onSettingChange({ search_engine: sel.value }, true);
  return sel;
}

function stepperControl(start, min, max, onChange, unit = "px") {
  let value = start;
  const wrap = el("div", "stepper");
  const minus = el("button", null, "-");
  const plus = el("button", null, "+");
  const val = el("span", "val");
  minus.type = plus.type = "button";
  const sync = () => {
    val.textContent = `${value} ${unit}`;
    minus.disabled = value <= min;
    plus.disabled = value >= max;
  };
  minus.onclick = () => {
    if (value > min) {
      value--;
      sync();
      onChange(value);
    }
  };
  plus.onclick = () => {
    if (value < max) {
      value++;
      sync();
      onChange(value);
    }
  };
  sync();
  wrap.append(minus, val, plus);
  return wrap;
}

function sliderControl(start, min, max, onChange) {
  const wrap = el("div", "slider-wrap");
  const input = document.createElement("input");
  input.type = "range";
  input.min = String(min);
  input.max = String(max);
  input.value = String(start);
  const pct = el("span", "pct", `${start}%`);
  input.oninput = () => {
    pct.textContent = `${input.value}%`;
    onChange(Number(input.value));
  };
  wrap.append(input, pct);
  return wrap;
}

/* Pemilih warna aksen: preset swatch + input hex + color picker native. */
function accentControl(onChange) {
  const wrap = el("div", "accent-control");

  const swatches = el("div", "accent-swatches");
  const paint = () => {
    [...swatches.children].forEach((b) => {
      b.classList.toggle("on", normalizeHex(b.dataset.color) === normalizeHex(SETTINGS.accent));
    });
    hexInput.value = normalizeHex(SETTINGS.accent) || DEFAULT_ACCENT;
    native.value = normalizeHex(SETTINGS.accent) || DEFAULT_ACCENT;
  };

  for (const [name, color] of ACCENT_PRESETS) {
    const b = el("button", "accent-swatch");
    b.type = "button";
    b.title = name;
    b.setAttribute("aria-label", name);
    b.dataset.color = color;
    b.style.setProperty("--sw", color);
    b.onclick = () => {
      SETTINGS.accent = color;
      applyAccent(color);
      paint();
      onChange(color);
    };
    swatches.appendChild(b);
  }

  const pickers = el("div", "accent-pickers");

  const native = document.createElement("input");
  native.type = "color";
  native.className = "accent-native";
  native.title = "Pick a custom color";
  native.oninput = () => {
    SETTINGS.accent = native.value;
    applyAccent(native.value);
    hexInput.value = native.value;
    paint();
    onChange(native.value);
  };

  const hexInput = document.createElement("input");
  hexInput.type = "text";
  hexInput.className = "accent-hex";
  hexInput.spellcheck = false;
  hexInput.maxLength = 7;
  hexInput.placeholder = "#f08c28";
  hexInput.oninput = () => {
    const hex = normalizeHex(hexInput.value);
    if (!hex) return; // biarkan user selesai mengetik
    SETTINGS.accent = hex;
    applyAccent(hex);
    native.value = hex;
    onChange(hex);
  };
  hexInput.onblur = () => paint();

  const reset = el("button", "link-btn", "Reset");
  reset.type = "button";
  reset.onclick = () => {
    SETTINGS.accent = DEFAULT_ACCENT;
    applyAccent(DEFAULT_ACCENT);
    paint();
    onChange(DEFAULT_ACCENT);
  };

  pickers.append(native, hexInput, reset);
  wrap.append(swatches, pickers);
  paint();
  return wrap;
}

function segmentControl(options, value, onChange) {
  const wrap = el("div", "segment");
  for (const [val, label] of options) {
    const b = el("button", val === value ? "on" : "", label);
    b.type = "button";
    b.setAttribute("aria-pressed", String(val === value));
    b.onclick = () => {
      [...wrap.children].forEach((c) => {
        c.classList.toggle("on", c === b);
        c.setAttribute("aria-pressed", String(c === b));
      });
      onChange(val);
    };
    wrap.appendChild(b);
  }
  return wrap;
}

/* Hotkey capture — minimal satu modifier (atau tombol F1–F24). */
let hotkeyBtnRef = null;
let savingHotkey = false;

function hotkeyLabel(accel) {
  return accel.replace(/\bSuper\b/gi, "Win").replace(/\+/g, " + ");
}

function cancelHotkeyCapture() {
  capturingHotkey = false;
  if (hotkeyBtnRef) {
    hotkeyBtnRef.classList.remove("capturing");
    hotkeyBtnRef.textContent = hotkeyLabel(SETTINGS.hotkey);
  }
}

function accelFromEvent(e) {
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");

  let key = e.key;
  const named = {
    " ": "Space",
    Spacebar: "Space",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Escape: "Escape",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    Insert: "Insert",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
  };
  if (named[key]) key = named[key];
  else if (/^f\d{1,2}$/i.test(key)) key = key.toUpperCase();
  else if (/^[a-z]$/i.test(key)) key = key.toUpperCase();
  else if (key.length !== 1) return null;

  const fnKey = /^F\d{1,2}$/.test(key);
  if (!mods.length && !fnKey) return null;
  return [...mods, key].join("+");
}

function handleHotkeyCapture(e) {
  // Let keyboard users leave the recorder and reach the explicit preset.
  if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) {
    cancelHotkeyCapture();
    return;
  }
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    cancelHotkeyCapture();
    return;
  }
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;

  const accel = accelFromEvent(e);
  if (!accel) {
    if (hotkeyBtnRef) hotkeyBtnRef.textContent = "Add Ctrl / Alt / Shift…";
    return;
  }
  capturingHotkey = false;
  setHotkey(accel);
}

async function setHotkey(accel) {
  if (savingHotkey) return;
  savingHotkey = true;
  cancelHotkeyCapture();
  const prev = SETTINGS.hotkey;
  try {
    const err = await onSettingChange({ hotkey: accel }, true);
    if (err) {
      // The backend keeps the previous binding on failure; don't write it again.
      SETTINGS.hotkey = prev;
      SETTINGS_ERROR = err;
    }
  } finally {
    savingHotkey = false;
    renderSettings();
  }
}

function hotkeyControl() {
  const wrap = el("div", "hotkey-control");

  const btn = el("button", "hotkey-btn", hotkeyLabel(SETTINGS.hotkey));
  btn.type = "button";
  btn.setAttribute("aria-label", "Record Sunny hotkey");
  btn.onclick = () => {
    if (capturingHotkey || savingHotkey) return;
    capturingHotkey = true;
    btn.classList.add("capturing");
    btn.textContent = "Press a shortcut…";
  };

  // Windows can consume this combo before WebView receives it. No recording needed.
  const winSpace = el("button", "hotkey-win", "Win + Space");
  winSpace.type = "button";
  winSpace.title = "Use Win + Space (Windows may also switch input languages)";
  winSpace.onclick = () => setHotkey("Super+Space");

  wrap.append(btn, winSpace);
  hotkeyBtnRef = btn;
  return wrap;
}

/* ── Tab panel Settings (ala Raycast) ──────────────────── */

// Ikon tab: SVG garis seragam (stroke 1.6, 18×18) agar senada satu sama lain.
// Launcher memakai ikon Sunny sendiri, sesuai permintaan.
const TAB_ICONS = {
  general:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  shortcuts:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="20" height="12" rx="2.5"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8"/></svg>',
  keyboard:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="20" height="14" rx="2.5"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M9 16h6"/></svg>',
  advanced:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a4 4 0 0 0 5 5l-9.4 9.4a2.1 2.1 0 0 1-3-3z"/><path d="M14.7 6.3 17 4l3 3-2.3 2.3"/></svg>',
  about:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></svg>',
};

const SETTINGS_TABS = [
  { id: "general", label: "General", icon: TAB_ICONS.general },
  { id: "launcher", label: "Launcher", icon: "sunny", sunny: true },
  { id: "shortcuts", label: "Shortcuts", icon: TAB_ICONS.shortcuts },
  { id: "keyboard", label: "Keyboard", icon: TAB_ICONS.keyboard },
  { id: "advanced", label: "Advanced", icon: TAB_ICONS.advanced },
  { id: "about", label: "About Sunny", icon: TAB_ICONS.about },
];

let settingsTab = "general";
let settingsFilter = "";

/** Baris yang cocok dengan filter pencarian setelan. */
function rowMatches(label, desc) {
  if (!settingsFilter) return true;
  const hay = `${label} ${desc || ""}`.toLowerCase();
  return hay.includes(settingsFilter);
}

/** Bungkus baris; kembalikan null bila tersaring keluar. */
function maybeRow(label, desc, control) {
  return rowMatches(label, desc) ? settingsRow(label, desc, control) : null;
}

function settingsSectionFiltered(title, rows) {
  const kept = rows.filter(Boolean);
  if (!kept.length) return null;
  return settingsSection(title, kept);
}

function textValue(text, mono = false) {
  return el("span", "value" + (mono ? " mono" : ""), text);
}

function omnigetPathControl() {
  const input = el("input", "omniget-path");
  input.type = "text";
  input.value = SETTINGS.omniget_cli_path || "";
  input.placeholder = "Auto-detect omniget-cli.exe";
  input.maxLength = 32768;
  input.spellcheck = false;
  input.setAttribute("aria-label", "OmniGet CLI executable path");
  input.dataset.settingsFocus = "omniget_cli_path";
  input.addEventListener("input", () => onSettingChange({ omniget_cli_path: input.value }));
  input.addEventListener("blur", flushPendingSave);
  return input;
}

function omnigetRequirementSummary(installation) {
  if (!installation?.available) return "OmniGet CLI not found. Download it here, or set its existing path above.";
  const missing = Array.isArray(installation.missing_tools) ? installation.missing_tools : [];
  return missing.length
    ? `CLI found. Not found in common locations: ${missing.join(", ")}. OmniGet prepares missing engines on first download; custom overrides may already provide them.`
    : "CLI found. Common download engines were detected; site support and login requirements still depend on OmniGet.";
}

function updateOmnigetSetupControl() {
  if (!omnigetSetupView) return;
  const { wrap, download, check, status, progress } = omnigetSetupView;
  wrap.setAttribute("aria-busy", String(omnigetSetupBusy));
  download.disabled = check.disabled = omnigetSetupBusy;
  download.textContent = omnigetSetupBusy ? "Please wait…" : "Download OmniGet CLI";
  status.textContent = omnigetSetupMessage;
  progress.hidden = !omnigetSetupBusy;
  if (Number.isFinite(omnigetSetupPercent) && omnigetSetupPercent !== null) progress.value = omnigetSetupPercent;
  else progress.removeAttribute("value");
}

function omnigetSetupControl() {
  const wrap = el("div", "omniget-setup");
  const actions = el("div", "omniget-setup-actions");
  const download = linkButton("Download OmniGet CLI", downloadOmnigetCli);
  const check = linkButton("Check requirements", checkOmnigetRequirements);
  const status = el("p", "omniget-setup-status");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const progress = el("progress");
  progress.max = 100;
  progress.setAttribute("aria-label", "OmniGet CLI download progress");
  actions.append(download, check);
  wrap.append(actions, progress, status);
  omnigetSetupView = { wrap, download, check, status, progress };
  updateOmnigetSetupControl();
  return wrap;
}

async function checkOmnigetRequirements() {
  if (omnigetSetupBusy) return;
  omnigetSetupBusy = true;
  omnigetSetupPercent = null;
  omnigetSetupMessage = "Checking local requirements…";
  updateOmnigetSetupControl();
  try {
    flushPendingSave();
    // Flush is debounced; await any write already in flight before checking its path.
    if (settingsSavePromise) await settingsSavePromise;
    omnigetSetupMessage = omnigetRequirementSummary(await invoke("video_check"));
  } catch (error) { omnigetSetupMessage = String(error); }
  finally { omnigetSetupBusy = false; updateOmnigetSetupControl(); }
}

const OMNIGET_SETUP_TIMEOUT_MS = 180000;

async function downloadOmnigetCli() {
  if (omnigetSetupBusy) return;
  omnigetSetupBusy = true;
  omnigetSetupPercent = 0;
  omnigetSetupMessage = "Downloading and verifying the official OmniGet CLI. No installer will be run…";
  updateOmnigetSetupControl();
  // The backend has its own deadline; this only covers a lost event/response so
  // the button can never stay locked on "Please wait…".
  let watchdog = 0;
  const timeout = new Promise((resolve) => {
    watchdog = setTimeout(() => resolve("timeout"), OMNIGET_SETUP_TIMEOUT_MS);
  });
  try {
    const outcome = await Promise.race([invoke("video_setup"), timeout]);
    if (outcome === "timeout") {
      omnigetSetupMessage = "OmniGet setup stopped responding. Nothing was installed - try again.";
      return;
    }
    const installation = outcome;
    if (!installation?.available || typeof installation.path !== "string" || !installation.path) throw new Error("The downloaded CLI could not be detected.");
    const error = await onSettingChange({ omniget_cli_path: installation.path }, true);
    if (error) throw new Error(`CLI downloaded, but its path could not be saved: ${error}`);
    omnigetSetupMessage = `OmniGet CLI installed. ${omnigetRequirementSummary(installation)}`;
  } catch (error) { omnigetSetupMessage = String(error); }
  finally {
    clearTimeout(watchdog);
    omnigetSetupBusy = false;
    omnigetSetupPercent = null;
    updateOmnigetSetupControl();
  }
}

listen("sunny:omniget-setup", (e) => {
  if (!omnigetSetupBusy || !e.payload || typeof e.payload.message !== "string") return;
  omnigetSetupMessage = e.payload.message;
  omnigetSetupPercent = typeof e.payload.percent === "number" && Number.isFinite(e.payload.percent)
    ? Math.min(100, Math.max(0, e.payload.percent)) : null;
  updateOmnigetSetupControl();
});

/** Status indeks file yang selalu segar (dibaca ulang saat tab dibuka). */
function indexStatusValue() {
  const span = textValue(fileIndexReady ? `${fileTotal} files` : "Indexing…");
  if (!fileIndexReady) {
    invoke("search_files", { query: "" })
      .then((r) => {
        fileIndexReady = !!r.ready;
        fileTotal = r.total || 0;
        span.textContent = fileIndexReady ? `${fileTotal} files` : "Indexing…";
      })
      .catch(() => {});
  }
  return span;
}

function linkButton(label, onClick) {
  const b = el("button", "link-btn", label);
  b.type = "button";
  b.onclick = onClick;
  return b;
}

function aboutCard() {
  const card = el("div", "about-card");
  const img = document.createElement("img");
  img.src = "sunny-icon.png";
  img.alt = "";
  img.draggable = false;
  const text = el("div");
  text.appendChild(el("div", "about-name", "Sunny"));
  text.appendChild(
    el(
      "div",
      "about-tag",
      "A fast, lightweight launcher for Windows. GPL-3.0 port of Vicinae's core features."
    )
  );
  card.append(img, text);
  return card;
}

function renderSettingsTabs() {
  settingsTabsEl.innerHTML = "";
  for (const tab of SETTINGS_TABS) {
    const b = el("button", "settings-tab" + (tab.id === settingsTab ? " on" : ""));
    b.type = "button";
    b.setAttribute("role", "tab");
    b.setAttribute("aria-selected", String(tab.id === settingsTab));
    const ico = el("span", "tab-ico");
    if (tab.sunny) {
      // Launcher memakai ikon Sunny agar senada dengan branding aplikasi.
      const img = document.createElement("img");
      img.src = "sunny-icon.png";
      img.alt = "";
      img.draggable = false;
      ico.appendChild(img);
    } else {
      ico.innerHTML = tab.icon;
    }
    b.append(ico, el("span", "tab-label", tab.label));
    b.onclick = () => {
      settingsTab = tab.id;
      renderSettings();
    };
    settingsTabsEl.appendChild(b);
  }
}

function renderSettings(force = false) {
  const active = document.activeElement;
  if (!force && (capturingHotkey || (active && settingsBodyEl.contains(active) && ["INPUT", "SELECT"].includes(active.tagName)))) {
    settingsRenderPending = true;
    return;
  }
  settingsRenderPending = false;
  settingsBodyEl.innerHTML = "";
  hotkeyBtnRef = null;
  renderSettingsTabs();

  if (SETTINGS_ERROR) {
    settingsBodyEl.appendChild(el("div", "settings-error", SETTINGS_ERROR));
  }

  const sections = [];

  if (settingsTab === "general") {
    sections.push(
      settingsSectionFiltered("General", [
        maybeRow(
          "Open at Login",
          "Launch Sunny automatically when you sign in.",
          toggleControl(SETTINGS.open_at_login, (v) =>
            onSettingChange({ open_at_login: v }, true)
          )
        ),
        maybeRow(
          "Show in System Tray",
          "Keep the Sunny icon in the notification area.",
          toggleControl(SETTINGS.show_in_tray, (v) =>
            onSettingChange({ show_in_tray: v }, true)
          )
        ),
        maybeRow(
          "Sunny Hotkey",
          "Record a shortcut, or choose Win + Space. Windows may also switch input languages; Sunny never blocks keys.",
          hotkeyControl()
        ),
        maybeRow(
          "OmniGet CLI Path",
          "Local path for Video Download. Download the CLI below, or leave empty to auto-detect an existing installation.",
          omnigetPathControl()
        ),
        maybeRow(
          "OmniGet Download & Setup",
          "User-requested download from getomniget.com, SHA-256 checked before extraction. Only the CLI is installed; no GUI or installer is launched.",
          omnigetSetupControl()
        ),
      ])
    );
  } else if (settingsTab === "launcher") {
    sections.push(
      settingsSectionFiltered("Appearance", [
        maybeRow(
          "Theme",
          "Follow Windows, or force a light or dark launcher.",
          segmentControl(
            [["system", "System"], ["light", "Light"], ["dark", "Dark"]],
            SETTINGS.theme,
            (v) => onSettingChange({ theme: v }, true)
          )
        ),
        maybeRow(
          "Accent Color",
          "Highlight color for selections, switches, and active controls.",
          accentControl((v) => onSettingChange({ accent: v }, true))
        ),
        maybeRow(
          "Font",
          "Sonora ships Inter - the default Sunny typeface.",
          fontControl()
        ),
        maybeRow(
          "Font Size",
          "Base text size across the launcher.",
          stepperControl(SETTINGS.font_size, 12, 20, (v) =>
            onSettingChange({ font_size: v })
          )
        ),
      ]),
      settingsSectionFiltered("Window", [
        maybeRow(
          "Opacity",
          "Background opacity. Text, icons, and controls stay fully opaque.",
          sliderControl(SETTINGS.opacity, 40, 100, (v) =>
            onSettingChange({ opacity: v })
          )
        ),
        maybeRow(
          "Blur UI",
          "Blur only the desktop snapshot, never text or icons. Visible below 100% opacity.",
          toggleControl(SETTINGS.blur_ui, (v) =>
            onSettingChange({ blur_ui: v }, true)
          )
        ),
        maybeRow(
          "Window Mode",
          "Raycast-style density of the launcher window.",
          segmentControl(
            [["compact", "Raycast Compact"], ["expanded", "Raycast Expanded"]],
            SETTINGS.window_mode,
            (v) => onSettingChange({ window_mode: v }, true)
          )
        ),
        maybeRow(
          "Show Footer Hints",
          "Display the keyboard hint bar at the bottom.",
          toggleControl(SETTINGS.show_footer, (v) =>
            onSettingChange({ show_footer: v }, true)
          )
        ),
      ]),
      settingsSectionFiltered("Search", [
        maybeRow(
          "Include Files in Root Search",
          "Show matching files directly in the main search, without opening Search Files.",
          toggleControl(SETTINGS.include_files, (v) =>
            onSettingChange({ include_files: v }, true)
          )
        ),
        maybeRow(
          "File Results",
          "How many file matches to show below Results in the main search.",
          stepperControl(SETTINGS.file_results, 0, 30, (v) =>
            onSettingChange({ file_results: v }, true), "files"
          )
        ),
        maybeRow(
          "Quick Web Search",
          "End a query with ? to search the web, for example “rust ownership?”.",
          searchEngineControl()
        ),
      ])
    );
  } else if (settingsTab === "shortcuts") {
    sections.push(
      settingsSectionFiltered("Global", [
        maybeRow(
          "Show / Hide Sunny",
          "Win + Space activates after release and may also switch the Windows input language.",
          hotkeyControl()
        ),
      ]),
      settingsSectionFiltered("In the Launcher", [
        maybeRow("Open Settings", "Jump straight to this panel.", textValue("Ctrl + ,")),
        maybeRow("Emoji Picker", "Search for the command, then open it.", textValue("Type “emoji” → Enter")),
        maybeRow("QR Code Generator", "Search for QR Code Generator, enter text or a URL, then copy its image.", textValue("Type “qr code” → Enter")),
        maybeRow("Copy QR Image", "In the QR Code Generator view, copy the generated image to the clipboard.", textValue("Enter")),
        maybeRow("Video Download", "Open the in-box OmniGet panel, paste a URL, choose quality, then press Enter to download.", textValue("Type “video download” → Enter")),
        maybeRow("Video Controls", "Tab to quality, Download, Cancel or Show in Explorer. Esc returns to search; downloads continue until cancelled or Sunny exits.", textValue("Tab / Enter / Esc")),
        maybeRow("Actions", "Open actions for the selected app or file. Ctrl+K is also available when Vim navigation is off.", textValue("Ctrl + Shift + K")),
        maybeRow("File Path", "Copy the selected file's full path without closing the launcher.", textValue("Ctrl + Shift + C")),
        maybeRow("Back to App Search", "Leave a command view. Backspace also works when its search field is empty and enabled in Keyboard.", textValue("Esc")),
        maybeRow("Clipboard History", "Search for the command by name.", textValue("Type “clipboard”")),
        maybeRow("Search Files", "Search for the command by name.", textValue("Type “files”")),
        maybeRow("Close Launcher", "Hide the window.", textValue("Esc")),
      ]),
      settingsSectionFiltered("Clipboard", [
        maybeRow("Paste to Active Window", "Copies and pastes into the previous app.", textValue("Enter")),
        maybeRow("Copy Without Pasting", "Keeps the launcher open.", textValue("Ctrl + B")),
        maybeRow("Delete Entry", "Removes the selected history item.", textValue("Ctrl + D")),
      ]),
      settingsSectionFiltered("Files", [
        maybeRow("Open File", "Uses the default application.", textValue("Enter")),
        maybeRow("Reveal in Explorer", "Selects the file in File Explorer, in Files mode or root search.", textValue("Ctrl + R")),
        maybeRow("Copy Full Path", "Copies the selected file's path without pasting it.", textValue("Ctrl + Shift + C")),
      ])
    );
  } else if (settingsTab === "keyboard") {
    sections.push(
      settingsSectionFiltered("Navigation", [
        maybeRow("Move Selection", "Up and down the result list.", textValue("↑ / ↓")),
        maybeRow("Vim Navigation", "Also use Ctrl+J / Ctrl+K to move.", toggleControl(SETTINGS.vim_nav, (v) =>
          onSettingChange({ vim_nav: v }, true)
        )),
        maybeRow("Activate Selection", "Open the highlighted result.", textValue("Enter")),
        maybeRow("Back to App Search", "Use Backspace in an empty command search to return to apps.", toggleControl(SETTINGS.backspace_back, (v) =>
          onSettingChange({ backspace_back: v }, true)
        )),
      ])
    );
  } else if (settingsTab === "advanced") {
    sections.push(
      settingsSectionFiltered("Data", [
        maybeRow(
          "Settings File",
          "Stored as JSON in your local app data folder.",
          linkButton("Open Folder", () =>
            invoke("settings_dir").then((dir) => invoke("open_path", { path: dir })).catch(() => {})
          )
        ),
        maybeRow(
          "Reset All Settings",
          "Restore every option to its default value.",
          linkButton("Reset…", resetSettings)
        ),
      ]),
      settingsSectionFiltered("Index", [
        maybeRow(
          "File Index",
          "Built in the background at launch from your user folders.",
          indexStatusValue()
        ),
        maybeRow(
          "Rebuild Index",
          "Rescan your folders now.",
          linkButton("Rebuild", () => {
            invoke("rebuild_file_index").catch(() => {});
            renderSettings();
          })
        ),
      ])
    );
  } else if (settingsTab === "about") {
    sections.push(aboutCard());
    sections.push(
      settingsSectionFiltered("Updates", [
        maybeRow(
          "Version",
            installedVersion && installedVersion.release_tag.replace(/^v/, "") !== installedVersion.version
              ? `Installed Sunny build. Internal version ${installedVersion.version}.`
              : "Installed Sunny build.",
            textValue(installedVersion?.release_tag || versionState)
        ),
        maybeRow(
          "Check for Updates",
          "Look for a newer release on GitHub.",
          updateControl()
        ),
      ])
    );
    sections.push(
      settingsSectionFiltered("About", [
        maybeRow("License", "Derivative work of Vicinae.", textValue("GPL-3.0")),
        maybeRow("Font", "Bundled from the Sonora project.", textValue("Inter (OFL)")),
        maybeRow("Emoji Data", "Generated from gemoji.", textValue("MIT")),
        maybeRow("Platform", "Windows 10 / 11.", textValue("WebView2")),
      ])
    );
  }

  const kept = sections.filter(Boolean);
  if (!kept.length) {
    settingsBodyEl.appendChild(
      el("div", "settings-empty", "No settings match your search.")
    );
  } else {
    settingsBodyEl.append(...kept);
  }
}

/* ── Auto-update (backend: tauri-plugin-updater) ───────── */

// Installed metadata comes from the running binary, never the latest remote release.
let installedVersion = null;
let versionState = "Loading…";
if (IS_SETTINGS_WINDOW) {
  invoke("app_version")
    .then((info) => {
      if (!info || typeof info.version !== "string" || !info.version) throw new Error("Unavailable");
      installedVersion = {
        version: info.version,
        release_tag: typeof info.release_tag === "string" && info.release_tag ? info.release_tag : info.version,
      };
    })
    .catch(() => { versionState = "Unavailable"; })
    .finally(() => {
      if (settingsOpen && settingsTab === "about") renderSettings();
    });
}
let updateState = "idle"; // idle | checking | available | downloading | uptodate | error
let updateInfo = null;
let updateError = "";
let updateProgress = 0;

function updateControl() {
  const wrap = el("div", "update-control");
  const status = el("span", "value", updateStatusText());
  const btn = el("button", "link-btn", updateButtonLabel());
  btn.type = "button";
  btn.disabled = updateState === "checking" || updateState === "downloading";
  btn.onclick = () => {
    if (updateState === "available") {
      installUpdate();
    } else {
      checkForUpdates();
    }
  };
  wrap.append(status, btn);
  return wrap;
}

function updateStatusText() {
  switch (updateState) {
    case "checking":
      return "Checking…";
    case "available":
      return `v${updateInfo?.version || "?"} available`;
    case "downloading":
      return updateProgress ? `Downloading… ${updateProgress}%` : "Downloading…";
    case "uptodate":
      return "Up to date";
    case "error":
      return updateError || "Update check failed";
    default:
      return "Not checked yet";
  }
}

function updateButtonLabel() {
  if (updateState === "available") return "Install & Restart";
  if (updateState === "downloading") return "Installing…";
  if (updateState === "checking") return "Checking…";
  return "Check Now";
}

async function checkForUpdates() {
  updateState = "checking";
  updateError = "";
  renderSettings();
  try {
    const found = await invoke("update_check");
    if (found) {
      updateInfo = found;
      updateState = "available";
    } else {
      updateState = "uptodate";
    }
  } catch (err) {
    updateState = "error";
    updateError = friendlyUpdateError(err);
  }
  renderSettings();
}

/// Pesan error updater cukup teknis; terjemahkan kasus yang paling sering.
function friendlyUpdateError(err) {
  const raw = String(err || "");
  if (/valid release JSON|404|not found/i.test(raw)) {
    return "No published release found yet";
  }
  if (/network|connect|dns|timed? ?out/i.test(raw)) {
    return "Network unavailable";
  }
  if (/signature/i.test(raw)) {
    return "Update signature is invalid";
  }
  return raw.slice(0, 120) || "Update check failed";
}

async function installUpdate() {
  if (!updateInfo) return;
  updateState = "downloading";
  updateProgress = 0;
  renderSettings();
  try {
    await invoke("update_install");
    // Di Windows proses ditutup oleh installer; jaga-jaga bila tidak.
    await window.__TAURI__.process.relaunch();
  } catch (err) {
    updateState = "error";
    updateError = friendlyUpdateError(err);
    renderSettings();
  }
}

listen("sunny:update-progress", (e) => {
  updateProgress = Number(e.payload) || 0;
  const span = settingsBodyEl.querySelector(".update-control .value");
  if (span) span.textContent = `Downloading… ${updateProgress}%`;
});

async function resetSettings() {
  try {
    clearTimeout(saveTimer);
    saveTimer = null;
    await saveSettings();
    const settings = await invoke("settings_reset");
    acceptSettings(settings);
    SETTINGS_ERROR = "";
    refreshSettingsView();
    flashSaved();
  } catch (err) {
    SETTINGS_ERROR = String(err);
    renderSettings();
  }
}

function openSettings() {
  // Di jendela launcher: buka jendela Settings terpisah (ala Raycast).
  if (!IS_SETTINGS_WINDOW) {
    invoke("settings_open").catch(() => {});
    return;
  }
  settingsOpen = true;
  capturingHotkey = false;
  mode = "apps";
  queryEl.value = "";
  queryEl.blur();
  settingsFilter = "";
  settingsSearchEl.value = "";
  document.body.classList.add("settings-open");
  settingsEl.hidden = false;
  // Jendela terpisah: tombol kembali berperan sebagai penutup jendela.
  settingsBackEl.textContent = "×";
  settingsBackEl.title = "Close (Esc)";
  renderSettings();
  settingsSearchEl.focus();
}

function closeSettings() {
  flushPendingSave();
  settingsOpen = false;
  capturingHotkey = false;
  document.body.classList.remove("settings-open");
  settingsEl.hidden = true;
  applyAppearance();
  if (IS_SETTINGS_WINDOW) {
    invoke("settings_close").catch(() => {});
    return;
  }
  queryEl.value = "";
  refresh();
  queryEl.focus();
}

let savedTimer = null;
function flashSaved() {
  settingsSavedEl.textContent = "Saved";
  settingsSavedEl.classList.add("show");
  clearTimeout(savedTimer);
  savedTimer = setTimeout(() => settingsSavedEl.classList.remove("show"), 1200);
}

function rebaseSettings() {
  SETTINGS = { ...confirmedSettings };
  for (const [key, patch] of pendingSettings) SETTINGS[key] = patch.value;
  applyAppearance();
}

function refreshSettingsView(force = false) {
  if (!settingsOpen) return;
  const active = document.activeElement;
  const editing = active && settingsBodyEl.contains(active) && ["INPUT", "SELECT"].includes(active.tagName);
  if (!force && (editing || capturingHotkey)) {
    settingsRenderPending = true;
    return;
  }
  const focusKey = active?.dataset?.settingsFocus;
  const scrollTop = settingsBodyEl.scrollTop;
  settingsRenderPending = false;
  renderSettings(force);
  if (focusKey) {
    const control = [...settingsBodyEl.querySelectorAll("button, input, select")]
      .find((node) => node.dataset.settingsFocus === focusKey);
    control?.focus({ preventScroll: true });
  }
  settingsBodyEl.scrollTop = scrollTop;
}

function acceptSettings(settings) {
  const previousCli = SETTINGS.omniget_cli_path;
  confirmedSettings = { ...confirmedSettings, ...settings };
  rebaseSettings();
  refreshSettingsView();
  if (!IS_SETTINGS_WINDOW && mode === "video" && previousCli !== SETTINGS.omniget_cli_path) checkVideoInstallation();
}

function saveSettings() {
  const requestedRevision = settingsRevision;
  settingsSaveRequested = true;
  if (!settingsSavePromise) settingsSavePromise = (async () => {
    // Do not replace still-loading persisted fields with startup defaults.
    await settingsReady;
    while (settingsSaveRequested && pendingSettings.size) {
      settingsSaveRequested = false;
      const submitted = new Map(pendingSettings);
      const settings = { ...confirmedSettings };
      for (const [key, patch] of submitted) settings[key] = patch.value;
      const broadcastsAtSend = settingsBroadcastSeq;
      try {
        const saved = await invoke("settings_update", { settings });
        if (settingsBroadcastSeq === broadcastsAtSend) {
          confirmedSettings = { ...confirmedSettings, ...saved };
        } else {
          // Preserve unrelated changes received while this write was in flight.
          for (const key of submitted.keys()) confirmedSettings[key] = saved[key];
        }
        for (const [key, patch] of submitted) {
          if (pendingSettings.get(key)?.revision === patch.revision) pendingSettings.delete(key);
        }
        rebaseSettings();
        SETTINGS_ERROR = "";
        refreshSettingsView();
        if (!pendingSettings.size) flashSaved();
      } catch (error) {
        // Roll back only this failed revision, leaving newer edits queued.
        for (const [key, patch] of submitted) {
          if (pendingSettings.get(key)?.revision === patch.revision) pendingSettings.delete(key);
        }
        rebaseSettings();
        SETTINGS_ERROR = String(error);
        for (const patch of submitted.values()) settingsSaveErrors.set(patch.revision, SETTINGS_ERROR);
        // Keep error attribution bounded while overlapping callers finish.
        while (settingsSaveErrors.size > 32) settingsSaveErrors.delete(settingsSaveErrors.keys().next().value);
        invoke("settings_preview", { settings: { ...SETTINGS } }).catch(() => {});
        refreshSettingsView(true);
      }
    }
  })().finally(() => { settingsSavePromise = null; });
  return settingsSavePromise.then(() => settingsSaveErrors.get(requestedRevision) || "");
}

/** Simpan segera perubahan yang masih menunggu debounce (mis. saat panel ditutup). */
function flushPendingSave() {
  if (!saveTimer) return;
  clearTimeout(saveTimer);
  saveTimer = null;
  saveSettings();
}

function onSettingChange(patch, immediate = false) {
  const revision = ++settingsRevision;
  for (const [key, value] of Object.entries(patch)) pendingSettings.set(key, { value, revision });
  rebaseSettings();
  SETTINGS_ERROR = "";
  settingsSavedEl.classList.remove("show");

  // Preview native (opacity/ukuran jendela) — throttle agar slider tetap ringan.
  const now = performance.now();
  if (immediate || now - lastPreviewAt > 70) {
    lastPreviewAt = now;
    invoke("settings_preview", { settings: { ...SETTINGS } }).catch(() => {});
  }

  clearTimeout(saveTimer);
  if (immediate) {
    saveTimer = null;
    return saveSettings();
  }
  saveTimer = setTimeout(() => {
    saveTimer = null;
    saveSettings();
  }, 300);
  return Promise.resolve("");
}

/* ── Pemuatan data per mode ────────────────────────────── */

const fuzzy = (needle, hay) => {
  // pencocokan longgar sederhana: semua karakter urut muncul
  const n = needle.toLowerCase(), h = hay.toLowerCase();
  let i = 0;
  for (const ch of n) {
    i = h.indexOf(ch, i);
    if (i === -1) return false;
    i++;
  }
  return true;
};

/* Commands are search-only. With root file search enabled, the dedicated
   "Search Files" command would duplicate the Files section, so it only appears
   when the query actually names it. */
function filesCommandVisible(query) {
  if (!SETTINGS.include_files) return true;
  const text = String(query || "").toLowerCase().trim();
  if (!text) return false;
  return ["file", "files", "search files", "search file"].some((name) => text.includes(name));
}

/* ── Optional OmniGet download panel (no remote page or iframe) ── */
function renderVideo() {
  if (IS_SETTINGS_WINDOW || mode !== "video") return;
  const active = videoState.phase === "running";
  const busy = active || videoSubmitting;
  videoSetupEl.textContent = videoChecking ? "Checking for OmniGet CLI…"
    : videoInstallation?.available ? "OmniGet CLI found. First use may prepare yt-dlp, FFmpeg and a JavaScript runtime. Downloads stay here, without a browser tab."
    : "OmniGet CLI not found. Open Settings → General → OmniGet Download & Setup, or set the path to an existing omniget-cli.exe.";
  videoStartEl.disabled = busy || !videoInstallation?.available || !queryEl.value.trim();
  videoStartEl.textContent = videoSubmitting ? "Starting…" : "Download";
  videoQualityEl.disabled = busy;
  videoCancelEl.hidden = !active;
  videoCancelEl.disabled = videoCancelling;
  videoCancelEl.textContent = videoCancelling ? "Cancelling…" : "Cancel";
  videoProgressEl.hidden = !active && videoState.phase !== "complete";
  if (typeof videoState.percent === "number" && Number.isFinite(videoState.percent)) {
    videoProgressEl.value = Math.max(0, Math.min(100, videoState.percent));
  } else videoProgressEl.removeAttribute("value");
  videoStatusEl.textContent = videoState.message || "Paste a video URL to begin.";
  videoStatusEl.classList.toggle("error", videoState.phase === "failed");
  videoFileEl.textContent = videoState.phase === "complete" ? videoState.file_path || "" : "";
  videoFileEl.hidden = !videoFileEl.textContent;
  videoRevealEl.hidden = videoState.phase !== "complete" || !videoState.file_path;
  videoRetryEl.disabled = videoChecking;
}

function acceptVideoStatus(status) {
  if (IS_SETTINGS_WINDOW || !status || !Number.isSafeInteger(status.id) || status.id < videoState.id) return;
  if (!["idle", "running", "complete", "failed", "cancelled"].includes(status.phase)) return;
  if (status.id === videoState.id && (status.revision || 0) < (videoState.revision || 0)) return;
  videoState = { ...status };
  if (status.phase !== "running") videoCancelling = false;
  renderVideo();
}

async function checkVideoInstallation() {
  if (IS_SETTINGS_WINDOW) return;
  const seq = ++videoCheckSeq;
  videoChecking = true;
  renderVideo();
  try {
    const installation = await invoke("video_check");
    if (seq === videoCheckSeq) videoInstallation = installation;
  } catch {
    if (seq === videoCheckSeq) videoInstallation = null;
  } finally {
    if (seq === videoCheckSeq) { videoChecking = false; renderVideo(); }
  }
}

async function loadVideo() {
  if (IS_SETTINGS_WINDOW) return;
  const results = await Promise.allSettled([checkVideoInstallation(), invoke("video_status")]);
  if (results[1].status === "fulfilled") acceptVideoStatus(results[1].value);
  else if (mode === "video") { videoStatusEl.textContent = `Could not load download status: ${results[1].reason}`; }
}

async function startVideo() {
  if (IS_SETTINGS_WINDOW || mode !== "video" || videoSubmitting || videoState.phase === "running" || !videoInstallation?.available || !queryEl.value.trim()) return;
  videoSubmitting = true;
  const url = queryEl.value.trim();
  const quality = videoQualityEl.value || "best";
  renderVideo();
  try {
    acceptVideoStatus(await invoke("video_start", { url, quality }));
  } catch (error) {
    if (videoState.phase !== "running") videoState = { ...videoState, phase: "failed", message: String(error), file_path: null };
  } finally {
    videoSubmitting = false;
    renderVideo();
  }
}

async function cancelVideo() {
  if (IS_SETTINGS_WINDOW || videoState.phase !== "running" || videoCancelling) return;
  videoCancelling = true;
  renderVideo();
  try { await invoke("video_cancel", { id: videoState.id }); }
  catch (error) {
    videoCancelling = false;
    if (mode === "video") showLauncherError(error);
    renderVideo();
  }
}

listen("sunny:video-progress", (e) => acceptVideoStatus(e.payload));
videoStartEl?.addEventListener("click", startVideo);
videoCancelEl?.addEventListener("click", cancelVideo);
videoRetryEl?.addEventListener("click", checkVideoInstallation);
videoRevealEl?.addEventListener("click", () => {
  if (!IS_SETTINGS_WINDOW && videoState.phase === "complete") invoke("video_reveal", { id: videoState.id }).catch(showLauncherError);
});

/** Draw square modules with a four-module white quiet zone, without any network request. */
function renderQr(modules) {
  const quiet = 4;
  const scale = 4;
  const size = (modules.length + quiet * 2) * scale;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  canvas.dataset.size = String(modules.length);
  canvas.setAttribute("role", "img");
  canvas.setAttribute("aria-label", "QR code for the entered text");
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas is unavailable for the QR preview.");
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, size, size);
  ctx.fillStyle = "#000";
  modules.forEach((row, y) => row.forEach((dark, x) => {
    if (dark) ctx.fillRect((x + quiet) * scale, (y + quiet) * scale, scale, scale);
  }));
  qrPreviewEl.querySelector("canvas")?.remove();
  qrPreviewEl.insertBefore(canvas, qrCopyEl);
}

async function copyQr() {
  if (mode !== "qr" || !qrReadyText || qrReadyText !== queryEl.value) return;
  try {
    await invoke("qr_copy", { text: qrReadyText });
    showLauncherNote("QR image copied to clipboard.");
  } catch (error) {
    showLauncherError(error);
  }
  queryEl.focus();
}

/* Commands are shared by search and settings; keep this helper next to them. */
async function refresh() {
  if (IS_SETTINGS_WINDOW || settingsOpen) return;
  clearTimeout(searchTimer);
  searchTimer = null;
  const seq = ++refreshSeq;
  const q = queryEl.value.trim();
  const nextItems = [];
  closeActions(false);
  showLauncherError();

  try {
    if (mode === "video") {
      // Typing never starts OmniGet or contacts the pasted URL.
    } else if (mode === "qr") {
      qrReadyText = "";
      if (q) {
        const modules = await invoke("qr_generate", { text: queryEl.value });
        if (seq !== refreshSeq) return;
        renderQr(modules);
        qrReadyText = queryEl.value;
      }
    } else if (mode === "emoji") {
      const needle = q.toLowerCase();
      const pool = ALL_EMOJI.filter(
        (e) => !needle || e.name.toLowerCase().includes(needle) || e.keywords.toLowerCase().includes(needle)
      ).slice(0, 60);
      nextItems.push(...pool.map((e) => ({ ...e, kind: "emoji" })));
    } else if (mode === "clip") {
      const clips = await invoke("clip_history");
      if (seq !== refreshSeq) return;
      clipTotal = clips.length;
      const typeOK = (c) => clipType === "all" || (clipType === "text" ? c.kind === "text" : c.kind !== "text");
      const filtered = clips.filter((c) => typeOK(c) && (!q || String(c.text).toLowerCase().includes(q.toLowerCase())));
      nextItems.push(...filtered.slice(0, 200).map((c) => ({ ...c, kind: "clip", kind2: c.kind || "text" })));
    } else if (mode === "files") {
      const res = await invoke("search_files", { query: q });
      if (seq !== refreshSeq) return;
      fileIndexReady = !!res.ready;
      fileTotal = res.total || 0;
      nextItems.push(...res.items.map((hit) => ({ ...hit.item, kind: "file" })));
    } else if (!q) {
      const home = await refreshHome();
      if (seq !== refreshSeq) return;
      nextItems.push(
        ...home.favorites.map((app) => ({ ...app, kind: "app", group: "Favorites" })),
        ...home.suggestions.map((app) => ({ ...app, kind: "app", group: "Suggestions" }))
      );
    } else if (q.endsWith("?") || q.startsWith("?")) {
      // Explicit web intent never scans local apps/files or waits for IPC.
      const engine = currentEngine();
      const searchText = q.replace(/^\?+|\?+$/g, "").trim();
      if (engine && Array.from(searchText).length >= 2) {
        nextItems.push({
          kind: "web", group: "Results", engine: engine.id,
          engineName: engine.name, text: searchText, query: q,
        });
      }
    } else {
      const fileLimit = Math.max(0, Math.min(30, Number(SETTINGS.file_results) || 0));
      // Attach error handling immediately: files may reject before apps finish.
      const fileRequest = SETTINGS.include_files && fileLimit > 0 && q.length >= 2
        ? invoke("search_files", { query: q, limit: fileLimit }).then(
            (value) => ({ value }), (error) => ({ error }))
        : null;
      const results = await Promise.allSettled([
        invoke("calc", { query: q }),
        /^(lorem|ipsum)(\s|$)/i.test(q) ? invoke("lorem", { query: q }) : Promise.resolve(null),
        invoke("search_apps", { query: q }),
      ]);
      if (seq !== refreshSeq) return;
      const [calculation, lorem, apps] = results;
      if (calculation.status === "fulfilled" && calculation.value != null) {
        nextItems.push({ kind: "calc", expr: q, value: calculation.value, group: "Results" });
      }
      if (lorem.status === "fulfilled" && lorem.value != null) {
        nextItems.push({ kind: "lorem", expr: q, text: lorem.value, group: "Results" });
      }
      const cmds = COMMANDS
        .filter((c) => fuzzy(q, c.keys))
        .filter((c) => c.cmd !== "files" || filesCommandVisible(q))
        .map(({ keys, ...c }) => ({ ...c, group: "Results" }));
      const str = apps.status === "fulfilled" ? apps.value : [];
      nextItems.push(...cmds, ...str.map((hit) => ({ ...hit.item, kind: "app", group: "Results" })));
      for (const result of results) {
        if (result.status === "rejected") showLauncherError(result.reason);
      }
      if (fileRequest) {
        // Publish apps atomically before slow files; backing items always match DOM.
        items = [...nextItems];
        selected = 0;
        render();
        const result = await fileRequest;
        if (seq !== refreshSeq) return;
        if (result.error) {
          showLauncherError(result.error);
          return;
        }
        const res = result.value;
        fileIndexReady = !!res.ready;
        fileTotal = res.total || 0;
        const files = res.items
          .slice(0, fileLimit)
          .map((hit) => ({ ...hit.item, kind: "file", group: "Files" }));
        nextItems.push(...files);
        // Files are appended only. Preserve a user's app selection and its index.
        items = nextItems;
        render();
        return;
      }
    }
  } catch (error) {
    if (seq !== refreshSeq) return;
    showLauncherError(error);
  }

  if (seq !== refreshSeq) return;
  items = nextItems;
  selected = 0;
  render();
}

/* ── Aktivasi baris ────────────────────────────────────── */

async function activate(i) {
  const item = items[i];
  if (!item) return;
  const external = ["web", "file", "app"].includes(item.kind);
  if (external && activating) return;
  if (external) activating = true;
  if (item.kind === "calc") {
    if (await copyText(item.value)) hideWindow();
  } else if (item.kind === "lorem") {
    if (await copyText(item.text)) hideWindow();
  } else if (item.kind === "web") {
    try {
      await invoke("web_search", { query: item.query, engineId: item.engine });
      hideWindow();
    } catch (error) {
      activating = false;
      showLauncherError(error);
    }
  } else if (item.kind === "cmd") {
    if (item.cmd === "settings") {
      openSettings();
      return;
    }
    mode = item.cmd;
    qrReadyText = "";
    queryEl.value = "";
    queryEl.placeholder = MODE_LABEL[mode];
    refresh();
    if (mode === "video") loadVideo();
  } else if (item.kind === "clip") {
    // Copy, then paste into the window that had focus before Sunny opened.
    try {
      await invoke("paste_to_active_window", { text: item.text });
    } catch (error) {
      showLauncherError(`Could not paste: ${error}`);
    }
  } else if (item.kind === "emoji") {
    if (await copyText(item.emoji)) hideWindow();
  } else if (item.kind === "file") {
    try {
      await invoke("open_path", { path: item.path });
      hideWindow();
    } catch (error) {
      activating = false;
      showLauncherError(error);
    }
  } else {
    try {
      await invoke("run_app", { path: item.path });
      hideWindow();
    } catch (error) {
      activating = false;
      showLauncherError(error);
    }
  }
}

/* ── Fokus otomatis ────────────────────────────────────── */
function resetLauncherSearch() {
  if (IS_SETTINGS_WINDOW || settingsOpen) return;
  activating = false;
  qrReadyText = "";
  // Invalidate in-flight requests before clearing anything, so they cannot restore
  // the previous query's results after the launcher has reopened.
  refreshSeq++;
  queryEl.value = "";
  queryEl.placeholder = MODE_LABEL.apps;
  mode = "apps";
  selected = 0;
  items = [];
  render();
  refresh();
}

listen("sunny:shown", () => {
  if (IS_SETTINGS_WINDOW) return;
  resetLauncherSearch();
  // Event `sunny:backdrop` biasanya sudah membawa snapshot baru; fetch hanya
  // bila event terlewat (mis. saat WebView belum selesai dimuat).
  if (!backdropFresh) {
    invoke("backdrop_get").then(setBackdrop).catch(() => {});
  }
  backdropFresh = false;
  if (!settingsOpen) queryEl.focus();
});
window.addEventListener("focus", () => {
  if (!settingsOpen) queryEl.focus();
});
if (!IS_SETTINGS_WINDOW) queryEl.focus();

// Perubahan yang masih menunggu debounce tetap disimpan saat jendela
// disembunyikan/ditutup — ini penyebab setelan kadang "hilang".
window.addEventListener("blur", flushPendingSave);
window.addEventListener("beforeunload", flushPendingSave);

/* ── Data berubah di background → segarkan ─────────────── */
listen("sunny:apps-updated", async () => {
  if (IS_SETTINGS_WINDOW) return;
  try {
    if (mode !== "apps" || queryEl.value.trim()) await refreshHome();
    await refresh();
  } catch (error) {
    showLauncherError(error);
  }
});
listen("sunny:clip-updated", () => {
  if (mode === "clip") refresh();
});
listen("sunny:launcher-warning", (e) => {
  if (IS_SETTINGS_WINDOW) return;
  showLauncherError(e.payload);
});
listen("sunny:files-updated", () => {
  if (mode === "files" || (mode === "apps" && SETTINGS.include_files && queryEl.value.trim())) refresh();
});
/* Setelan diubah di jendela Settings → terapkan juga di jendela ini.
   Tanpa ini, tema/opacity baru hanya terlihat setelah restart. */
listen("sunny:settings-updated", (e) => {
  if (!e.payload) return;
  ++settingsBroadcastSeq;
  acceptSettings(e.payload);
  if (mode === "apps") refresh();
});
/* Menu tray → buka panel Settings */
listen("sunny:open-settings", () => openSettings());
settingsBackEl.addEventListener("click", closeSettings);
settingsSearchEl.addEventListener("input", () => {
  settingsFilter = settingsSearchEl.value.trim().toLowerCase();
  renderSettings();
});

queryEl.addEventListener("input", () => {
  if (mode === "video") { renderVideo(); return; }
  // Invalidate immediately, not when the timer fires. Enter cannot activate a
  // result belonging to the previous query during the short typing debounce.
  ++refreshSeq;
  clearTimeout(searchTimer);
  closeActions(false);
  items = [];
  qrReadyText = "";
  selected = 0;
  render();
  const q = queryEl.value.trim();
  if (mode === "qr" && q) searchTimer = setTimeout(refresh, 80);
  else if (mode === "emoji" || !q || q.startsWith("?") || q.endsWith("?")) refresh();
  else searchTimer = setTimeout(refresh, 40);
});
modeBackEl?.addEventListener("click", backToApps);
qrCopyEl?.addEventListener("click", copyQr);
actionsBtnEl?.addEventListener("click", toggleActions);
document.addEventListener("pointerdown", (e) => {
  if (actionsOpen && !actionsEl.contains(e.target) && !actionsBtnEl.contains(e.target) && !hintsEl.contains(e.target)) {
    closeActions(false);
  }
});
document.addEventListener("focusout", () => {
  if (settingsRenderPending) setTimeout(() => refreshSettingsView(), 0);
});
clipFilterEl.addEventListener("change", () => {
  clipType = clipFilterEl.value;
  refresh();
});
pauseBtnEl.addEventListener("click", () => {
  invoke("clip_toggle_pause").then((v) => {
    paused = v;
    render();
  }).catch(showLauncherError);
});

window.addEventListener("keydown", (e) => {
  // Sedang merekam hotkey baru → tangkap semua tombol.
  if (capturingHotkey) {
    handleHotkeyCapture(e);
    return;
  }

  // Panel Settings terbuka: Esc kembali ke launcher.
  if (settingsOpen) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeSettings();
    }
    return;
  }

  if (actionsOpen) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const count = actionsEl.querySelectorAll("button").length;
      focusAction((actionSelected + (e.key === "ArrowDown" ? 1 : count - 1)) % count);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      focusAction(e.key === "Home" ? 0 : actionsEl.querySelectorAll("button").length - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (!e.repeat) actionsEl.querySelectorAll("button")[actionSelected]?.click();
    } else if (e.key === "Escape" || e.key === "Tab" || (e.ctrlKey && e.key.toLowerCase() === "k")) {
      e.preventDefault();
      closeActions();
    }
    return;
  }

  // Ctrl+, → Settings (pintasan ala Raycast).
  if (e.ctrlKey && !e.altKey && (e.key === "," || e.key === "<")) {
    e.preventDefault();
    openSettings();
    return;
  }

  if (e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "k" && (e.shiftKey || !SETTINGS.vim_nav)) {
    e.preventDefault();
    toggleActions();
    return;
  }

  if (mode === "video") {
    if (e.key === "Escape" || (e.key === "Backspace" && e.target === queryEl && !queryEl.value && SETTINGS.backspace_back)) {
      e.preventDefault();
      backToApps();
    } else if (e.key === "Enter" && e.target === queryEl) {
      e.preventDefault();
      if (!e.repeat) startVideo();
    }
    // Tab, arrows and Enter on buttons/selects retain their native behavior.
    return;
  }

  // Let native buttons activate normally instead of opening the selected row.
  if (e.key === "Enter" && e.target?.tagName === "BUTTON") return;

  const max = Math.max(items.length - 1, 0);
  // Ctrl+J / Ctrl+K (opsional, Settings → Keyboard → Vim Navigation).
  const vimDown = SETTINGS.vim_nav && e.ctrlKey && e.key.toLowerCase() === "j";
  const vimUp = SETTINGS.vim_nav && e.ctrlKey && e.key.toLowerCase() === "k";
  if (e.key === "ArrowDown" || vimDown) {
    e.preventDefault();
    selected = Math.min(selected + 1, max);
    syncSelectionDom();
    listEl.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  } else if (e.key === "ArrowUp" || vimUp) {
    e.preventDefault();
    selected = Math.max(selected - 1, 0);
    syncSelectionDom();
    listEl.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  } else if (
    e.key === "Backspace" &&
    !queryEl.value &&
    mode !== "apps" &&
    SETTINGS.backspace_back
  ) {
    e.preventDefault();
    backToApps();
  } else if (mode === "clip" && e.ctrlKey && e.key.toLowerCase() === "d") {
    // Hapus entri terpilih
    e.preventDefault();
    const item = items[selected];
    if (item?.kind === "clip") {
      invoke("clip_delete", { id: item.id }).then(refresh).catch(showLauncherError);
    }
  } else if (mode === "clip" && e.ctrlKey && e.key.toLowerCase() === "b") {
    // "Actions": salin entri tanpa paste (aksi alternatif)
    e.preventDefault();
    const item = items[selected];
    if (item?.kind === "clip") {
      copyText(item.text).then((ok) => { if (ok) hideWindow(); });
    }
  } else if (items[selected]?.kind === "file" && e.ctrlKey && !e.altKey && !e.metaKey && e.shiftKey && e.key.toLowerCase() === "c") {
    e.preventDefault();
    copyFilePath(items[selected]);
  } else if (items[selected]?.kind === "file" && e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey && e.key.toLowerCase() === "r") {
    // Tampilkan file terpilih di File Explorer (berlaku juga di pencarian utama).
    e.preventDefault();
    revealFilePath(items[selected]);
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (!e.repeat) {
      if (mode === "qr") copyQr();
      else activate(selected);
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    if (IS_SETTINGS_WINDOW) {
      closeSettings();
    } else if (mode !== "apps") {
      backToApps();
    } else {
      hideWindow();
    }
  }
});

// Jendela Settings: langsung tampilkan panelnya saat dimuat.
if (IS_SETTINGS_WINDOW) {
  openSettings();
  // Kontrol titlebar kustom (jendela frameless).
  const appWindow = window.__TAURI__.window.getCurrentWindow();
  winMinEl?.addEventListener("click", () => appWindow.minimize());
  winMaxEl?.addEventListener("click", () => appWindow.toggleMaximize());
  winCloseEl?.addEventListener("click", () => invoke("settings_close").catch(() => {}));
} else {
  refresh();
}
