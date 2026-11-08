// Sunny UI — komunikasi dengan backend Rust via Tauri command API.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const queryEl = document.getElementById("query");
const listEl = document.getElementById("list");

let mode = "apps"; // "apps" | "emoji" | "clip"
let ALL_EMOJI = []; // diisi async saat startup
invoke("emoji_all").then((data) => {
  ALL_EMOJI = data;
  refresh();
});
let items = [];
let selected = 0;

const MODE_LABEL = { apps: "Cari aplikasi…", emoji: "Cari emoji…", clip: "Filter riwayat clipboard…" };
queryEl.placeholder = MODE_LABEL[mode];

function hideWindow() {
  invoke("hide_main_window");
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

function render() {
  listEl.innerHTML = "";
  if (!items.length) {
    const div = document.createElement("div");
    div.className = "empty";
    div.textContent =
      mode === "emoji" ? "Tidak ada emoji yang cocok."
      : mode === "clip" ? "Riwayat clipboard masih kosong."
      : queryEl.value ? "Tidak ada yang cocok."
      : "Sunny siap. Ketik aplikasi, rumus (12*7), ':' untuk emoji, atau pilih perintah di bawah.";
    listEl.appendChild(div);
    return;
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
      name.textContent = item.text;
    } else if (item.kind === "cmd") {
      name.textContent = item.name;
    } else if (item.kind === "emoji") {
      name.innerHTML = `${escapeHtml(item.emoji)} <span class="sub-inline">${escapeHtml(item.name)}</span>`;
    } else {
      name.textContent = item.name;
    }

    const sub = document.createElement("div");
    sub.className = "sub";
    if (item.kind === "calc") sub.textContent = "Enter: salin";
    else if (item.kind === "clip") sub.textContent = "Enter: salin kembali";
    else if (item.kind === "emoji") sub.textContent = "Enter: salin";
    else if (item.kind === "cmd") sub.textContent = "Enter: buka";
    else sub.textContent = "Start Menu";

    row.append(name, sub);
    row.onclick = () => activate(i);
    row.onmousemove = () => {
      if (selected !== i) {
        selected = i;
        [...listEl.children].forEach((el, j) =>
          el.classList.toggle("selected", j === i)
        );
      }
    };
    listEl.appendChild(row);
  });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[c]);
}

function labelFor(hit) {
  const e = hit.emoji || "";
  return `${e} ${hit.name}`;
}

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
    const filtered = q
      ? clips.filter((c) => c.text.toLowerCase().includes(q.toLowerCase()))
      : clips;
    items.push(...filtered.slice(0, 60).map((c) => ({ ...c, kind: "clip" })));
    if (items.length) items.unshift({ kind: "cmd", cmd: "apps", icon: "◀️", name: "Kembali ke pencarian aplikasi" });
  } else {
    // mode apps — hasil kalkulator di urutan pertama bila kueri valid
    if (q) {
      const value = await invoke("calc", { query: q });
      if (value != null) items.push({ kind: "calc", expr: q, value });
    }
    if (!q) {
      items.push(
        { kind: "cmd", cmd: "clip", icon: "📋", name: "Riwayat Clipboard" },
        { kind: "cmd", cmd: "emoji", icon: "🙂", name: "Emoji Picker (atau ketik ':')" }
      );
      const apps = await invoke("list_apps");
      items.push(...apps.map((app) => ({ ...app, kind: "app" })));
    } else {
      const str = await invoke("search_apps", { query: q });
      items.push(...str.map((hit) => ({ ...hit.item, kind: "app" })));
    }
  }

  selected = 0;
  render();
}

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
    await navigator.clipboard?.writeText(item.text).catch(() => invoke("copy_to_clipboard", { text: item.text }));
    hideWindow();
  } else if (item.kind === "emoji") {
    await navigator.clipboard?.writeText(item.emoji).catch(() => invoke("copy_to_clipboard", { text: item.emoji }));
    hideWindow();
  } else {
    invoke("run_app", { path: item.path });
    hideWindow();
  }
}

// ── Fokus otomatis ─────────────────────────────
listen("sunny:shown", () => queryEl.focus());
window.addEventListener("focus", () => queryEl.focus());
queryEl.focus();

// ── Data berubah di background → segarkan ──────
listen("sunny:apps-updated", () => refresh());
listen("sunny:clip-updated", () => {
  if (mode === "clip") refresh();
});

queryEl.addEventListener("input", refresh);

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
  } else if (e.key === "Enter") {
    e.preventDefault();
    activate(selected);
  } else if (e.key === "Escape") {
    e.preventDefault();
    hideWindow();
  }
});

refresh();
