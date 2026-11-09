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

let mode = "apps"; // "apps" | "emoji" | "clip"
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
};
let SETTINGS_ERROR = "";
let saveTimer = null;
let lastPreviewAt = 0;

let ALL_EMOJI = []; // diisi async saat startup
invoke("emoji_all").then((data) => {
  ALL_EMOJI = data;
  refresh();
});
invoke("settings_get")
  .then((s) => {
    SETTINGS = s;
    applyAppearance();
  })
  .catch(() => {});
let items = [];
let selected = 0;

// ── State khusus mode clipboard ──
let paused = false; // watcher pause?
let clipType = "all"; // filter dropdown: "all" | "text"
let clipTotal = 0; // jumlah entri sebelum difilter (untuk "74 items")

// Perintah internal — hanya muncul saat dicari (bukan di daftar default).
const COMMANDS = [
  { kind: "cmd", cmd: "clip", icon: "📋", name: "Clipboard History",
    keys: "clipboard history riwayat clipboard papan klip" },
  { kind: "cmd", cmd: "emoji", icon: "🙂", name: "Emoji Picker",
    keys: "emoji picker emotikon" },
  { kind: "cmd", cmd: "settings", icon: "⚙️", name: "Settings",
    keys: "settings preferences setelan pengaturan" },
];

const MODE_LABEL = {
  apps: "Search apps…",
  emoji: "Search emoji…",
  clip: "Browse clipboard history…",
};
queryEl.placeholder = MODE_LABEL[mode];

function hideWindow() {
  invoke("hide_main_window");
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
    icon.classList.add(item.kind === "clip" && item.kind2 === "image" ? "type-image" : "type-text");
    icon.textContent = item.kind2 === "image" ? "🖼" : "T";
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

function renderFooter() {
  if (mode === "clip") {
    hintsEl.innerHTML = `
      <span class="hint-strong"><img class="hint-icon" src="sunny-icon.png" alt="" draggable="false" />Clipboard History</span>
      <span class="spacer"></span>
      <span><kbd>↵</kbd> Paste to active window</span>
      <span><kbd>Ctrl</kbd><kbd>B</kbd> Actions</span>
      <span><kbd>Ctrl</kbd><kbd>D</kbd> Delete</span>`;
  } else if (mode === "emoji") {
    hintsEl.innerHTML = `
      <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
      <span><kbd>↵</kbd> copy</span>
      <span><kbd>⌫</kbd> back</span>`;
  } else {
    hintsEl.innerHTML = `
      <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
      <span><kbd>↵</kbd> select</span>
      <span><kbd>:</kbd> emoji</span>
      <span><kbd>Ctrl</kbd><kbd>,</kbd> settings</span>
      <span><kbd>Alt</kbd>+<kbd>Space</kbd> hide</span>`;
  }
}

function render() {
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
      : queryEl.value ? "Nothing matches."
      : "Sunny is ready. Type an app name, an expression (12*7), ':' for emoji, or search a command.";
    listEl.appendChild(div);
  }

  items.forEach((item, i) => {
    const row = document.createElement("div");
    row.className = "item" + (i === selected ? " selected" : "");
    row.setAttribute("role", "option");

    row.append(iconElFor(item));

    const name = document.createElement("div");
    name.className = "name";
    if (item.kind === "calc") {
      name.innerHTML = `${escapeHtml(item.expr)} <b class="val">= ${escapeHtml(item.value)}</b>`;
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
    } else {
      const sub = document.createElement("div");
      sub.className = "sub";
      if (item.kind === "calc") sub.textContent = "Enter: copy";
      else if (item.kind === "emoji") sub.textContent = "Enter: copy";
      else if (item.kind === "cmd") sub.textContent = "Enter: open";
      else sub.textContent = "Start Menu";
      row.appendChild(sub);
    }

    row.onclick = () => activate(i);
    row.onmousemove = () => {
      if (selected !== i) {
        selected = i;
        [...listEl.querySelectorAll(".item")].forEach((el, j) =>
          el.classList.toggle("selected", j === i)
        );
        if (mode === "clip") renderDetail(items[i]);
      }
    };
    listEl.appendChild(row);
  });

  if (isClip) {
    renderDetail(items[selected] || null);
  } else {
    detailEl.hidden = true;
    detailEl.innerHTML = "";
  }
  renderFooter();
}

/* ── Settings: penerapan + panel ───────────────────────── */

function applyAppearance() {
  document.documentElement.style.setProperty("--fs", SETTINGS.font_size + "px");
  document.body.classList.toggle("font-system", SETTINGS.font === "system");
  document.body.classList.toggle("blur-ui", !!SETTINGS.blur_ui);
  document.body.classList.toggle("compact", SETTINGS.window_mode === "compact");
}

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

function stepperControl(start, min, max, onChange) {
  let value = start;
  const wrap = el("div", "stepper");
  const minus = el("button", null, "−");
  const plus = el("button", null, "+");
  const val = el("span", "val");
  minus.type = plus.type = "button";
  const sync = () => {
    val.textContent = `${value} px`;
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

function segmentControl(options, value, onChange) {
  const wrap = el("div", "segment");
  for (const [val, label] of options) {
    const b = el("button", val === value ? "on" : "", label);
    b.type = "button";
    b.onclick = () => {
      [...wrap.children].forEach((c) => c.classList.toggle("on", c === b));
      onChange(val);
    };
    wrap.appendChild(b);
  }
  return wrap;
}

/* Hotkey capture — minimal satu modifier (atau tombol F1–F24). */
let hotkeyBtnRef = null;

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
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    capturingHotkey = false;
    if (hotkeyBtnRef) {
      hotkeyBtnRef.classList.remove("capturing");
      hotkeyBtnRef.textContent = SETTINGS.hotkey.replace(/\+/g, " + ");
    }
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
  const prev = SETTINGS.hotkey;
  SETTINGS.hotkey = accel;
  if (hotkeyBtnRef) {
    hotkeyBtnRef.classList.remove("capturing");
    hotkeyBtnRef.textContent = accel.replace(/\+/g, " + ");
  }
  const err = await onSettingChange({}, true);
  if (err) {
    SETTINGS.hotkey = prev;
    SETTINGS_ERROR = err;
    invoke("settings_update", { settings: SETTINGS }).catch(() => {});
    renderSettings();
  }
}

function hotkeyControl() {
  const btn = el("button", "hotkey-btn", SETTINGS.hotkey.replace(/\+/g, " + "));
  btn.type = "button";
  btn.onclick = () => {
    if (capturingHotkey) return;
    capturingHotkey = true;
    btn.classList.add("capturing");
    btn.textContent = "Press a shortcut…";
  };
  hotkeyBtnRef = btn;
  return btn;
}

function renderSettings() {
  settingsBodyEl.innerHTML = "";
  hotkeyBtnRef = null;

  if (SETTINGS_ERROR) {
    settingsBodyEl.appendChild(el("div", "settings-error", SETTINGS_ERROR));
  }

  settingsBodyEl.append(
    settingsSection("General", [
      settingsRow(
        "Open at Login",
        "Launch Sunny automatically when you sign in.",
        toggleControl(SETTINGS.open_at_login, (v) =>
          onSettingChange({ open_at_login: v }, true)
        )
      ),
      settingsRow(
        "Show in System Tray",
        "Keep the Sunny icon in the notification area.",
        toggleControl(SETTINGS.show_in_tray, (v) =>
          onSettingChange({ show_in_tray: v }, true)
        )
      ),
      settingsRow(
        "Sunny Hotkey",
        "Global shortcut to show or hide the launcher.",
        hotkeyControl()
      ),
    ]),
    settingsSection("Appearance", [
      settingsRow(
        "Font",
        "Sonora ships Inter — the default Sunny typeface.",
        fontControl()
      ),
      settingsRow(
        "Font Size",
        "Base text size across the launcher.",
        stepperControl(SETTINGS.font_size, 12, 20, (v) =>
          onSettingChange({ font_size: v })
        )
      ),
    ]),
    settingsSection("Window", [
      settingsRow(
        "Opacity",
        "Overall window transparency.",
        sliderControl(SETTINGS.opacity, 40, 100, (v) =>
          onSettingChange({ opacity: v })
        )
      ),
      settingsRow(
        "Blur UI",
        "Frosted-glass backdrop behind the launcher.",
        toggleControl(SETTINGS.blur_ui, (v) =>
          onSettingChange({ blur_ui: v }, true)
        )
      ),
      settingsRow(
        "Window Mode",
        "Raycast-style density of the launcher window.",
        segmentControl(
          [["compact", "Raycast Compact"], ["expanded", "Raycast Expanded"]],
          SETTINGS.window_mode,
          (v) => onSettingChange({ window_mode: v }, true)
        )
      ),
    ])
  );
}

function openSettings() {
  settingsOpen = true;
  capturingHotkey = false;
  mode = "apps";
  queryEl.value = "";
  queryEl.blur();
  document.body.classList.add("settings-open");
  settingsEl.hidden = false;
  invoke("settings_resize", { open: true }).catch(() => {});
  renderSettings();
}

function closeSettings() {
  settingsOpen = false;
  capturingHotkey = false;
  document.body.classList.remove("settings-open");
  settingsEl.hidden = true;
  applyAppearance();
  invoke("settings_resize", { open: false }).catch(() => {});
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

async function saveSettings() {
  try {
    SETTINGS = await invoke("settings_update", { settings: SETTINGS });
    SETTINGS_ERROR = "";
    applyAppearance();
    flashSaved();
    return "";
  } catch (err) {
    SETTINGS_ERROR = String(err);
    renderSettings();
    return SETTINGS_ERROR;
  }
}

function onSettingChange(patch, immediate = false) {
  Object.assign(SETTINGS, patch);
  SETTINGS_ERROR = "";
  applyAppearance();

  // Preview native (opacity/ukuran jendela) — throttle agar slider tetap ringan.
  const now = performance.now();
  if (immediate || now - lastPreviewAt > 70) {
    lastPreviewAt = now;
    invoke("settings_preview", { settings: SETTINGS }).catch(() => {});
  }

  clearTimeout(saveTimer);
  if (immediate) return saveSettings();
  saveTimer = setTimeout(saveSettings, 300);
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

async function refresh() {
  if (settingsOpen) return;
  const raw = queryEl.value;
  const q = raw.trim();

  // Promosi/demosi mode via awalan ":" (ala Slack/Discord)
  if (mode !== "clip") {
    if (raw.startsWith(":")) {
      if (mode !== "emoji") {
        mode = "emoji";
        queryEl.placeholder = MODE_LABEL.emoji;
      }
    } else if (mode === "emoji") {
      mode = "apps";
      queryEl.placeholder = MODE_LABEL.apps;
    }
  }

  items = [];

  if (mode === "emoji") {
    const needle = q.replace(/^:+/, "").trim();
    const pool = ALL_EMOJI.filter(
      (e) => !needle ||
        e.name.toLowerCase().includes(needle.toLowerCase()) ||
        e.keywords.toLowerCase().includes(needle.toLowerCase())
    ).slice(0, 60);
    items.push(...pool.map((e) => ({ ...e, kind: "emoji" })));
  } else if (mode === "clip") {
    const clips = await invoke("clip_history");
    clipTotal = clips.length;
    const typeOK = (c) =>
      clipType === "all" || (clipType === "text" ? c.kind === "text" : c.kind !== "text");
    const filtered = clips.filter(
      (c) =>
        typeOK(c) &&
        (!q || String(c.text).toLowerCase().includes(q.toLowerCase()))
    );
    items.push(...filtered.slice(0, 200).map((c) => ({
      ...c,
      kind: "clip",
      kind2: c.kind || "text",
    })));
  } else {
    // mode apps — hasil kalkulator di urutan pertama bila kueri valid
    if (q) {
      const value = await invoke("calc", { query: q });
      if (value != null) items.push({ kind: "calc", expr: q, value });
    }
    if (!q) {
      // Daftar default hanya aplikasi — perintah internal tidak lagi "menempel"
      // di atas; muncul hanya bila dicari (Rows: "Clipboard History", "Emoji").
      const apps = await invoke("list_apps");
      items.push(...apps.map((app) => ({ ...app, kind: "app" })));
    } else {
      const cmds = COMMANDS
        .filter((c) => fuzzy(q, c.keys))
        .map(({ keys, ...c }) => c);
      const str = await invoke("search_apps", { query: q });
      items.push(...cmds, ...str.map((hit) => ({ ...hit.item, kind: "app" })));
    }
  }

  selected = 0;
  render();
}

/* ── Aktivasi baris ────────────────────────────────────── */

async function activate(i) {
  const item = items[i];
  if (!item) return;
  if (item.kind === "calc") {
    const value = item.value;
    await navigator.clipboard?.writeText(value).catch(() => invoke("copy_to_clipboard", { text: value }));
    hideWindow();
  } else if (item.kind === "cmd") {
    if (item.cmd === "settings") {
      openSettings();
      return;
    }
    mode = item.cmd;
    queryEl.value = "";
    queryEl.placeholder = MODE_LABEL[mode];
    refresh();
  } else if (item.kind === "clip") {
    // Salin + tempel ke jendela aktif sebelumnya (ala Raycast)
    await invoke("paste_to_active_window", { text: item.text });
  } else if (item.kind === "emoji") {
    await navigator.clipboard?.writeText(item.emoji).catch(() => invoke("copy_to_clipboard", { text: item.emoji }));
    hideWindow();
  } else {
    invoke("run_app", { path: item.path });
    hideWindow();
  }
}

/* ── Fokus otomatis ────────────────────────────────────── */
listen("sunny:shown", () => {
  if (!settingsOpen) queryEl.focus();
});
window.addEventListener("focus", () => {
  if (!settingsOpen) queryEl.focus();
});
queryEl.focus();

/* ── Data berubah di background → segarkan ─────────────── */
listen("sunny:apps-updated", () => refresh());
listen("sunny:clip-updated", () => {
  if (mode === "clip") refresh();
});
/* Menu tray → buka panel Settings */
listen("sunny:open-settings", () => openSettings());
settingsBackEl.addEventListener("click", closeSettings);

queryEl.addEventListener("input", refresh);
clipFilterEl.addEventListener("change", () => {
  clipType = clipFilterEl.value;
  refresh();
});
pauseBtnEl.addEventListener("click", () => {
  invoke("clip_toggle_pause").then((v) => {
    paused = v;
    render();
  });
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

  // Ctrl+, → Settings (pintasan ala Raycast).
  if (e.ctrlKey && !e.altKey && (e.key === "," || e.key === "<")) {
    e.preventDefault();
    openSettings();
    return;
  }

  const max = Math.max(items.length - 1, 0);
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selected = Math.min(selected + 1, max);
    render();
    listEl.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selected = Math.max(selected - 1, 0);
    render();
    listEl.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
  } else if (e.key === "Backspace" && !queryEl.value && mode !== "apps") {
    e.preventDefault();
    mode = "apps";
    queryEl.placeholder = MODE_LABEL.apps;
    refresh();
  } else if (mode === "clip" && e.ctrlKey && e.key.toLowerCase() === "d") {
    // Hapus entri terpilih
    e.preventDefault();
    const item = items[selected];
    if (item?.kind === "clip") {
      invoke("clip_delete", { id: item.id }).then(refresh);
    }
  } else if (mode === "clip" && e.ctrlKey && e.key.toLowerCase() === "b") {
    // "Actions": salin entri tanpa paste (aksi alternatif)
    e.preventDefault();
    const item = items[selected];
    if (item?.kind === "clip") {
      invoke("copy_to_clipboard", { text: item.text }).then(hideWindow);
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    activate(selected);
  } else if (e.key === "Escape") {
    e.preventDefault();
    hideWindow();
  }
});

refresh();
