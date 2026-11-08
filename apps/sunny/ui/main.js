// Sunny UI — komunikasi dengan backend Rust via Tauri command API.
const { invoke } = window.__TAURI__.core;

const queryEl = document.getElementById("query");
const listEl = document.getElementById("list");

let items = []; // hasil pencarian saat ini
let selected = 0;

function render() {
  listEl.innerHTML = "";
  if (!items.length) {
    const div = document.createElement("div");
    div.className = "empty";
    div.textContent = queryEl.value
      ? "Tidak ada yang cocok."
      : "Sunny siap. Ketik untuk mencari aplikasi.";
    listEl.appendChild(div);
    return;
  }
  items.forEach((hit, i) => {
    const row = document.createElement("div");
    row.className = "item" + (i === selected ? " selected" : "");
    row.setAttribute("role", "option");

    const icon = document.createElement("div");
    icon.className = "icon";
    icon.textContent = "🚀";

    const name = document.createElement("div");
    name.className = "name";
    name.textContent = hit.item.name;

    const sub = document.createElement("div");
    sub.className = "sub";
    sub.textContent = "Start Menu";

    row.append(icon, name, sub);
    row.onclick = () => launch(hit);
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

async function refresh() {
  const q = queryEl.value.trim();
  if (!q) {
    const apps = await invoke("list_apps");
    items = apps.map((item) => ({ item, score: 0 }));
  } else {
    items = await invoke("search_apps", { query: q });
  }
  selected = 0;
  render();
}

function launch(hit) {
  invoke("run_app", { path: hit.item.path });
  window.__TAURI__.window.getCurrentWindow().hide();
}

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
    if (items[selected]) launch(items[selected]);
  }
});

refresh();
