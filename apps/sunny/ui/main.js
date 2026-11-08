// Sunny UI — komunikasi dengan backend Rust via Tauri command API.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const queryEl = document.getElementById("query");
const listEl = document.getElementById("list");

let items = []; // item gabungan: {kind:"calc"|"app", ...}
let selected = 0;

function hideWindow() {
  invoke("hide_main_window");
}

function iconElFor(item) {
  const icon = document.createElement("div");
  icon.className = "icon";
  if (item.kind === "calc") {
    icon.classList.add("calc");
    icon.textContent = "=";
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
    div.textContent = queryEl.value
      ? "Tidak ada yang cocok."
      : "Sunny siap. Ketik aplikasi atau rumus (mis. 12*7)";
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
    } else {
      name.textContent = item.name;
    }
    const sub = document.createElement("div");
    sub.className = "sub";
    sub.textContent = item.kind === "calc" ? "Enter: salin" : "Start Menu";

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
  return s.replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[c]);
}

async function refresh() {
  const q = queryEl.value.trim();
  items = [];

  // Kalkulator: hasil ekspresi = item pertama (index 0)
  if (q) {
    const value = await invoke("calc", { query: q });
    if (value != null) {
      items.push({ kind: "calc", expr: q, value });
    }
  }

  if (!q) {
    const apps = await invoke("list_apps");
    items.push(...apps.map((app) => ({ ...app, kind: "app" })));
  } else {
    const hits = await invoke("search_apps", { query: q });
    items.push(...hits.map((hit) => ({ ...hit.item, kind: "app" })));
  }

  selected = 0;
  render();
}

async function activate(i) {
  const item = items[i];
  if (!item) return;
  if (item.kind === "calc") {
    const ok = navigator.clipboard
      ?.writeText(item.value)
      .catch(() => invoke("copy_to_clipboard", { text: item.value }));
    await ok;
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

// ── Ikon siap di background → segarkan daftar ──
listen("sunny:apps-updated", () => refresh());

// ── Input & keyboard ───────────────────────────
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
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (items[selected]) activate(selected);
  } else if (e.key === "Escape") {
    e.preventDefault();
    hideWindow();
  }
});

refresh();
