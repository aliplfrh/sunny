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

let mode = "apps"; // "apps" | "emoji" | "clip"
let ALL_EMOJI = []; // diisi async saat startup
invoke("emoji_all").then((data) => {
  ALL_EMOJI = data;
  refresh();
});
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
listen("sunny:shown", () => queryEl.focus());
window.addEventListener("focus", () => queryEl.focus());
queryEl.focus();

/* ── Data berubah di background → segarkan ─────────────── */
listen("sunny:apps-updated", () => refresh());
listen("sunny:clip-updated", () => {
  if (mode === "clip") refresh();
});

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
