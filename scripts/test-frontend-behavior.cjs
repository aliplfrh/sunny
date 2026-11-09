// Isolated frontend regression tests. No native app, OS input, or npm dependencies.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ui = path.join(__dirname, "../apps/sunny/ui");
const source = fs.readFileSync(path.join(ui, "main.js"), "utf8");
const html = fs.readFileSync(path.join(ui, "index.html"), "utf8");
const css = fs.readFileSync(path.join(ui, "style.css"), "utf8");

class Element {
  constructor(tag = "div") {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.attributes = {};
    this.className = "";
    this.dataset = {};
    this.value = "";
    this.hidden = false;
    this.listeners = {};
    this.style = { setProperty(key, value) { this[key] = value; } };
    this.classList = {
      contains: (name) => this.className.split(/\s+/).includes(name),
      toggle: (name, on) => {
        const names = new Set(this.className.split(/\s+/).filter(Boolean));
        const add = on === undefined ? !names.has(name) : on;
        if (add) names.add(name); else names.delete(name);
        this.className = [...names].join(" ");
      },
      add: (name) => this.classList.toggle(name, true),
      remove: (name) => this.classList.toggle(name, false),
    };
  }
  set innerHTML(value) { this.children = []; this.markup = value; }
  get innerHTML() { return this.markup || ""; }
  append(...children) { children.forEach((node) => { node.parentNode = this; this.children.push(node); }); }
  appendChild(node) { this.append(node); return node; }
  replaceChildren(...children) { this.children = []; this.append(...children); }
  setAttribute(key, value) { this.attributes[key] = String(value); if (key === "id") this.id = value; }
  getAttribute(key) { return this.attributes[key] ?? null; }
  removeAttribute(key) { delete this.attributes[key]; }
  addEventListener(name, callback) { (this.listeners[name] ||= []).push(callback); }
  contains(node) { return node === this || this.children.some((child) => child.contains(node)); }
  querySelectorAll(selector) {
    const selectors = selector.split(",").map((part) => part.trim());
    const match = (node) => selectors.some((part) => part.startsWith(".")
      ? node.classList.contains(part.slice(1))
      : node.tagName === part.toUpperCase());
    return this.children.flatMap((node) => [...(match(node) ? [node] : []), ...node.querySelectorAll(selector)]);
  }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  focus() { this.focused = true; }
  blur() { this.focused = false; }
  scrollIntoView() {}
  click() {
    const event = { target: this, preventDefault() {}, stopPropagation() {} };
    for (const callback of this.listeners.click || []) callback(event);
    return this.onclick?.(event);
  }
}

async function fixture({ settingsWindow = false, settings = {}, responses = {} } = {}) {
  const nodes = Object.fromEntries([...html.matchAll(/id="([^"]+)"/g)].map((m) => [m[1], new Element()]));
  const events = {};
  const listeners = {};
  const calls = [];
  const timers = new Map();
  let nextTimer = 0;
  const defaults = { theme: "dark", font: "sonora", font_size: 14, opacity: 100,
    blur_ui: false, window_mode: "expanded", show_footer: true, accent: "#f08c28",
    include_files: true, file_results: 8, backspace_back: true, vim_nav: false,
    hotkey: "Alt+Space", ...settings };
  const handlers = {
    emoji_all: [{ emoji: "🙂", name: "smile", keywords: "happy" }], settings_get: defaults,
    launcher_home: { favorites: [], suggestions: [{ name: "Fallback", path: "fallback.lnk" }] },
    list_apps: [], search_apps: [], search_files: { ready: true, total: 0, items: [] }, calc: null,
    app_version: { version: "1.0.3", release_tag: "v1.0.2.1" }, ...responses,
  };
  const context = vm.createContext({
    console, URLSearchParams, location: { search: settingsWindow ? "?window=settings" : "" },
    performance: { now: () => 1000 }, navigator: { clipboard: { writeText: async () => {} } },
    setTimeout: (callback, delay) => { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; },
    clearTimeout: (id) => timers.delete(id),
    document: { getElementById: (id) => nodes[id] || null, createElement: (tag) => new Element(tag),
      documentElement: new Element("html"), body: new Element("body"),
      addEventListener(name, cb) { (listeners[name] ||= []).push(cb); } },
    window: { matchMedia: () => ({ matches: false, addEventListener() {} }),
      addEventListener(name, cb) { (listeners[name] ||= []).push(cb); },
      __TAURI__: { core: { invoke: async (command, args) => {
        calls.push({ command, args });
        const handler = handlers[command];
        return typeof handler === "function" ? handler(args) : handler ?? null;
      } }, event: { listen(name, cb) { events[name] = cb; } },
      window: { getCurrentWindow: () => ({}) } } },
  });
  vm.runInContext(source, context);
  const run = (code) => vm.runInContext(code, context);
  const settle = async () => { for (let i = 0; i < 12; i++) await new Promise(setImmediate); };
  await settle();
  const key = async (key, extras = {}) => {
    const e = { key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false,
      target: nodes.query, preventDefault() {}, stopPropagation() {}, ...extras };
    for (const cb of listeners.keydown || []) cb(e);
    await settle();
  };
  const search = async (query) => { nodes.query.value = query; await run("refresh()"); };
  const headings = () => nodes.list.querySelectorAll(".result-heading").map((el) => el.textContent);
  const dispatch = (name, event = {}) => { for (const callback of listeners[name] || []) callback(event); };
  const runTimers = async (delay) => {
    for (const [id, timer] of [...timers]) {
      if (timer.delay === delay) { timers.delete(id); timer.callback(); }
    }
    await settle();
  };
  return { run, nodes, events, calls, handlers, settle, key, search, headings, context, dispatch, runTimers };
}

test("root emoji prefixes never switch mode; selecting the command opens a persistent emoji view", async () => {
  const f = await fixture();
  await f.search(":smile");
  assert.equal(f.run("mode"), "apps");
  await f.search("emoji");
  const index = f.run('items.findIndex((item) => item.cmd === "emoji")');
  assert.ok(index >= 0);
  await f.run(`activate(${index})`);
  await f.settle();
  assert.equal(f.run("mode"), "emoji");
  assert.equal(f.run("items.length"), 1);
  await f.search("happy");
  assert.equal(f.run("mode"), "emoji");
  assert.equal(f.run("items[0].emoji"), "🙂");
  await f.search("");
  await f.key("Backspace");
  assert.equal(f.run("mode"), "apps");
  assert.doesNotMatch(source + html, /Emoji Prefix|Type the prefix|<kbd>:<\/kbd>|emojiPrefixControl/);
});

test("root results and files have visible nonselectable groups in keyboard order", async () => {
  const f = await fixture({ responses: {
    calc: "12", search_apps: [{ item: { name: "App", path: "app.lnk" } }],
    search_files: { ready: true, total: 1, items: [{ item: { name: "Document", path: "doc.txt" } }] },
  } });
  await f.search("emoji");
  assert.deepEqual(f.headings(), ["Results", "Files"]);
  assert.equal(f.nodes.list.querySelectorAll(".item").length, 4);
  assert.equal(f.run("items.length"), 4);
  for (let i = 0; i < 3; i++) await f.key("ArrowDown");
  assert.equal(f.run("items[selected].kind"), "file");
  assert.equal(f.nodes.list.querySelector(".selected").getAttribute("aria-selected"), "true");
  assert.equal(f.nodes.query.getAttribute("aria-activedescendant"), "result-3");
});

test("files-only results start at the top and empty headings are omitted", async () => {
  const f = await fixture({ responses: {
    search_files: { ready: true, total: 1, items: [{ item: { name: "Document", path: "doc.txt" } }] },
  } });
  await f.search("zz");
  assert.deepEqual(f.headings(), ["Files"]);
  assert.equal(f.run("items[0].kind"), "file");
  f.handlers.search_files = { ready: true, total: 0, items: [] };
  await f.search("zz");
  assert.deepEqual(f.headings(), []);
});

test("zero file-results setting does not silently fall back to eight", async () => {
  const f = await fixture({ settings: { file_results: 0 }, responses: {
    search_files: { ready: true, total: 1, items: [{ item: { name: "Document", path: "doc.txt" } }] },
  } });
  await f.search("zz");
  assert.equal(f.run("items.length"), 0);
});

test("home uses ranked payload order, Favorites before Suggestions, and refreshes on reopen/update", async () => {
  const home = { favorites: [{ name: "Pinned", path: "pin.lnk" }],
    suggestions: [{ name: "Zulu most used", path: "z.lnk" }, { name: "Alpha less used", path: "a.lnk" }] };
  const f = await fixture({ responses: { launcher_home: home } });
  assert.deepEqual(f.headings(), ["Favorites", "Suggestions"]);
  assert.deepEqual(Array.from(f.run("items.map((item) => item.name)")), ["Pinned", "Zulu most used", "Alpha less used"]);
  assert.equal(f.calls.some((call) => call.command === "list_apps"), false);
  const before = f.calls.filter((call) => call.command === "launcher_home").length;
  await f.search("old query");
  f.events["sunny:shown"]();
  await f.settle();
  assert.equal(f.nodes.query.value, "");
  assert.deepEqual(f.headings(), ["Favorites", "Suggestions"]);
  f.events["sunny:apps-updated"]();
  await f.settle();
  assert.ok(f.calls.filter((call) => call.command === "launcher_home").length >= before + 2);
});

test("new users see fallback suggestions without an empty Favorites heading", async () => {
  const f = await fixture();
  assert.deepEqual(f.headings(), ["Suggestions"]);
  assert.equal(f.run("items[0].name"), "Fallback");
});

test("favorite actions work by keyboard and mouse without adding controls to rows", async () => {
  const f = await fixture({ responses: {
    app_set_favorite: { favorites: [{ name: "Fallback", path: "fallback.lnk" }], suggestions: [] },
  } });
  await f.key("k", { ctrlKey: true });
  assert.equal(f.nodes.actions.hidden, false);
  assert.equal(f.nodes.actions.querySelectorAll("button")[1].textContent, "Add to Favorites");
  await f.key("ArrowDown");
  await f.key("Enter");
  assert.deepEqual(JSON.parse(JSON.stringify(f.calls.find((call) => call.command === "app_set_favorite").args)),
    { path: "fallback.lnk", favorite: true });
  assert.deepEqual(f.headings(), ["Favorites"]);
  assert.equal(f.nodes.actions.hidden, true);
  f.nodes.list.querySelector(".item").oncontextmenu({ preventDefault() {} });
  assert.equal(f.nodes.actions.querySelectorAll("button")[1].textContent, "Remove from Favorites");
  f.handlers.app_set_favorite = { favorites: [], suggestions: [{ name: "Fallback", path: "fallback.lnk" }] };
  await f.nodes.actions.querySelectorAll("button")[1].click();
  await f.settle();
  assert.deepEqual(f.headings(), ["Suggestions"]);
  assert.equal(f.nodes.list.querySelectorAll("button").length, 0);
});

test("Vim Ctrl+K remains navigation; Ctrl+Shift+K opens Actions and Escape restores focus", async () => {
  const f = await fixture({ settings: { vim_nav: true } });
  await f.key("k", { ctrlKey: true });
  assert.equal(f.nodes.actions.hidden, true);
  await f.key("K", { ctrlKey: true, shiftKey: true });
  assert.equal(f.nodes.actions.hidden, false);
  await f.key("Escape");
  assert.equal(f.nodes.actions.hidden, true);
  assert.equal(f.nodes.query.focused, true);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
});

test("favorite errors are visible and do not discard the home payload", async () => {
  const f = await fixture({ responses: { app_set_favorite: () => { throw new Error("Cannot save favorites"); } } });
  await f.key("k", { ctrlKey: true });
  await f.nodes.actions.querySelectorAll("button")[1].click();
  await f.settle();
  assert.match(f.nodes.launcherStatus.textContent, /Cannot save favorites/);
  assert.equal(f.run("items[0].name"), "Fallback");
});

test("snapshot events are window-specific, tint changes immediately, and settings opacity has no floor", async () => {
  for (const settingsWindow of [false, true]) {
    const f = await fixture({ settingsWindow, settings: { opacity: 40, blur_ui: true, theme: "light" } });
    const own = settingsWindow ? "sunny:settings-backdrop" : "sunny:backdrop";
    const other = settingsWindow ? "sunny:backdrop" : "sunny:settings-backdrop";
    f.events[own]?.({ payload: "data:image/png;base64,AAAA" });
    const image = f.nodes.backdrop.style.backgroundImage;
    assert.match(image, /AAAA/);
    f.events[other]?.({ payload: "data:image/png;base64,BBBB" });
    assert.equal(f.nodes.backdrop.style.backgroundImage, image);
    assert.equal(f.context.document.documentElement.style["--bg-tint"], "0.40");
    assert.equal(f.context.document.body.classList.contains("theme-light"), true);
    assert.equal(f.context.document.body.classList.contains("blur-ui"), true);
  }
  assert.doesNotMatch(css, /max\(var\(--bg-tint\),\s*0\.9\)/);
  assert.doesNotMatch(css, /transform:\s*scale\(1\.08\)/);
  assert.match(css, /body\.blur-ui \.backdrop[\s\S]*?filter:\s*blur\(/);
});

test("About shows the installed friendly release and internal version, never a hardcoded or remote version", async () => {
  const f = await fixture({ settingsWindow: true });
  f.run('settingsTab = "about"; renderSettings();');
  const values = f.nodes.settingsBody.querySelectorAll(".value").map((node) => node.textContent);
  assert.ok(values.includes("v1.0.2.1"));
  const descriptions = f.nodes.settingsBody.querySelectorAll(".d").map((node) => node.textContent);
  assert.ok(descriptions.some((text) => /1\.0\.3/.test(text)));
  assert.doesNotMatch(source, /const APP_VERSION\s*=\s*"1\.0\.0"/);
  assert.equal(f.calls.filter((call) => call.command === "app_version").length, 1);
  assert.equal(f.calls.some((call) => call.command === "update_check"), false);
});

test("About gracefully reports unavailable version metadata", async () => {
  const f = await fixture({ settingsWindow: true, responses: { app_version: () => { throw new Error("Unavailable"); } } });
  f.run('settingsTab = "about"; renderSettings();');
  assert.ok(f.nodes.settingsBody.querySelectorAll(".value").some((node) => node.textContent === "Unavailable"));
});

test("pending searches never mutate rows' backing items before the final atomic render", async () => {
  let resolveApps;
  const f = await fixture();
  f.handlers.calc = "123";
  f.handlers.search_apps = () => new Promise((resolve) => { resolveApps = resolve; });
  f.nodes.query.value = "123";
  const pending = f.run("refresh()");
  await f.settle();
  assert.equal(f.run("items[0].name"), "Fallback");
  assert.equal(f.nodes.list.querySelector(".item").children[1].textContent, "Fallback");
  resolveApps([]);
  await pending;
  assert.equal(f.run("items[0].kind"), "calc");
});

test("settings saves are serialized and old acknowledgments/broadcasts cannot overwrite newer edits", async () => {
  const saves = [];
  const f = await fixture({ settingsWindow: true, responses: {
    settings_update: ({ settings }) => new Promise((resolve) => saves.push({ settings, resolve })),
  } });
  const first = f.run("onSettingChange({ opacity: 60 }, true)");
  await f.settle();
  const second = f.run("onSettingChange({ opacity: 80 }, true)");
  await f.settle();
  assert.equal(saves.length, 1);
  f.events["sunny:settings-updated"]({ payload: { ...saves[0].settings, opacity: 60 } });
  assert.equal(f.run("SETTINGS.opacity"), 80);
  saves[0].resolve(saves[0].settings);
  await f.settle();
  assert.equal(f.run("SETTINGS.opacity"), 80);
  assert.equal(saves.length, 2);
  assert.equal(saves[1].settings.opacity, 80);
  saves[1].resolve(saves[1].settings);
  await Promise.all([first, second]);
  assert.equal(f.run("SETTINGS.opacity"), 80);
});

test("failed optimistic settings roll back to confirmed values without discarding a newer patch", async () => {
  const f = await fixture({ settingsWindow: true, settings: { open_at_login: false }, responses: {
    settings_update: () => { throw new Error("Autostart registry write failed"); },
  } });
  await f.run("onSettingChange({ open_at_login: true, opacity: 60 }, true)");
  assert.equal(f.run("SETTINGS.open_at_login"), false);
  assert.equal(f.run("SETTINGS.opacity"), 100);
  assert.match(f.run("SETTINGS_ERROR"), /Autostart registry write failed/);
});

test("settings hydration updates initial controls; external changes preserve the active input draft", async () => {
  let hydrate;
  const f = await fixture({ settingsWindow: true, responses: {
    settings_get: () => new Promise((resolve) => { hydrate = resolve; }),
  } });
  hydrate({ ...JSON.parse(f.run("JSON.stringify(SETTINGS)")), open_at_login: true });
  await f.settle();
  assert.equal(f.nodes.settingsBody.querySelectorAll(".switch")[0].getAttribute("aria-checked"), "true");
  f.nodes.settingsSearch.value = "unfinished search";
  f.events["sunny:settings-updated"]({ payload: { ...JSON.parse(f.run("JSON.stringify(SETTINGS)")), open_at_login: false } });
  assert.equal(f.nodes.settingsBody.querySelectorAll(".switch")[0].getAttribute("aria-checked"), "false");
  assert.equal(f.nodes.settingsSearch.value, "unfinished search");
  const draft = new Element("input");
  draft.type = "text";
  draft.value = "#12";
  f.nodes.settingsBody.appendChild(draft);
  f.context.document.activeElement = draft;
  f.events["sunny:settings-updated"]({ payload: { ...JSON.parse(f.run("JSON.stringify(SETTINGS)")), opacity: 75 } });
  assert.equal(f.run("SETTINGS.opacity"), 75);
  assert.equal(f.nodes.settingsBody.contains(draft), true);
  assert.equal(draft.value, "#12");
  f.context.document.activeElement = f.nodes.settingsSearch;
  f.dispatch("focusout");
  await f.runTimers(0);
  assert.equal(f.nodes.settingsBody.contains(draft), false);
});

test("capture failures explicitly clear stale snapshots", async () => {
  const f = await fixture();
  f.events["sunny:backdrop"]({ payload: "data:image/png;base64,AAAA" });
  f.events["sunny:backdrop"]({ payload: null });
  assert.equal(f.nodes.backdrop.style.backgroundImage, "none");
});

test("About Loading… rerenders when installed metadata arrives while the tab is open", async () => {
  let resolveVersion;
  const f = await fixture({ settingsWindow: true, responses: {
    app_version: () => new Promise((resolve) => { resolveVersion = resolve; }),
  } });
  f.run('settingsTab = "about"; renderSettings();');
  assert.ok(f.nodes.settingsBody.querySelectorAll(".value").some((node) => node.textContent === "Loading…"));
  resolveVersion({ version: "2.0.0", release_tag: "v2.0.0" });
  await f.settle();
  assert.ok(f.nodes.settingsBody.querySelectorAll(".value").some((node) => node.textContent === "v2.0.0"));
  assert.ok(f.nodes.settingsBody.querySelectorAll(".d").every((node) => !/Internal version/.test(node.textContent)));
});

test("reopen invalidates an old query without losing ranked home results", async () => {
  let resolveCalc;
  const f = await fixture();
  f.handlers.calc = () => new Promise((resolve) => { resolveCalc = resolve; });
  f.nodes.query.value = "old calculation";
  const old = f.run("refresh()");
  await f.settle();
  f.events["sunny:shown"]();
  await f.settle();
  resolveCalc("999");
  await old;
  assert.equal(f.nodes.query.value, "");
  assert.equal(f.run("items[0].name"), "Fallback");
  assert.deepEqual(f.headings(), ["Suggestions"]);
});

test("a newer settings patch survives an older failed save and is persisted next", async () => {
  const saves = [];
  const f = await fixture({ settingsWindow: true, responses: {
    settings_update: ({ settings }) => new Promise((resolve, reject) => saves.push({ settings, resolve, reject })),
  } });
  const old = f.run("onSettingChange({ opacity: 60 }, true)");
  await f.settle();
  const next = f.run("onSettingChange({ opacity: 80 }, true)");
  saves[0].reject(new Error("First save failed"));
  await f.settle();
  assert.equal(f.run("SETTINGS.opacity"), 80);
  assert.equal(saves[1].settings.opacity, 80);
  saves[1].resolve(saves[1].settings);
  await Promise.all([old, next]);
  assert.equal(f.run("SETTINGS.opacity"), 80);
});

test("non-immediate edits remain debounced while an earlier save finishes", async () => {
  let finish;
  let count = 0;
  const f = await fixture({ settingsWindow: true, responses: {
    settings_update: ({ settings }) => {
      count++;
      return count === 1 ? new Promise((resolve) => { finish = () => resolve(settings); }) : settings;
    },
  } });
  const first = f.run("onSettingChange({ opacity: 60 }, true)");
  await f.settle();
  await f.run("onSettingChange({ opacity: 80 })");
  finish();
  await first;
  assert.equal(count, 1);
  assert.equal(f.run("SETTINGS.opacity"), 80);
  await f.runTimers(300);
  assert.equal(count, 2);
  assert.equal(f.run("SETTINGS.opacity"), 80);
});

test("a settings broadcast for an unrelated field is rebased into the next serialized save", async () => {
  const saves = [];
  const f = await fixture({ settingsWindow: true, responses: {
    settings_update: ({ settings }) => new Promise((resolve) => saves.push({ settings, resolve })),
  } });
  const first = f.run("onSettingChange({ opacity: 60 }, true)");
  await f.settle();
  f.events["sunny:settings-updated"]({ payload: { ...saves[0].settings, theme: "light" } });
  const next = f.run("onSettingChange({ opacity: 80 }, true)");
  saves[0].resolve(saves[0].settings);
  await f.settle();
  assert.equal(f.run("SETTINGS.theme"), "light");
  assert.equal(saves[1].settings.theme, "light");
  saves[1].resolve(saves[1].settings);
  await Promise.all([first, next]);
});

test("file-index updates refresh an enabled root query", async () => {
  const f = await fixture();
  await f.search("zz");
  f.handlers.search_files = { ready: true, total: 1, items: [{ item: { name: "New file", path: "new.txt" } }] };
  f.events["sunny:files-updated"]();
  await f.settle();
  assert.equal(f.run("items[0].name"), "New file");
  assert.deepEqual(f.headings(), ["Files"]);
});

test("edits made before settings hydration are rebased over persisted fields before saving", async () => {
  let hydrate;
  const saves = [];
  const f = await fixture({ settingsWindow: true, responses: {
    settings_get: () => new Promise((resolve) => { hydrate = resolve; }),
    settings_update: ({ settings }) => { saves.push(settings); return settings; },
  } });
  const pending = f.run("onSettingChange({ opacity: 80 }, true)");
  assert.equal(saves.length, 0);
  hydrate({ ...JSON.parse(f.run("JSON.stringify(SETTINGS)")), opacity: 60, open_at_login: true });
  await pending;
  assert.equal(saves[0].opacity, 80);
  assert.equal(saves[0].open_at_login, true);
});

test("each immediate setting caller receives its own failure rather than another edit's result", async () => {
  const saves = [];
  const f = await fixture({ settingsWindow: true, responses: {
    settings_update: ({ settings }) => new Promise((resolve, reject) => saves.push({ settings, resolve, reject })),
  } });
  const failed = f.run('onSettingChange({ hotkey: "Ctrl+F12" }, true)');
  await f.settle();
  const succeeded = f.run("onSettingChange({ opacity: 80 }, true)");
  saves[0].reject(new Error("Hotkey registration failed"));
  await f.settle();
  saves[1].resolve(saves[1].settings);
  assert.match(await failed, /Hotkey registration failed/);
  assert.equal(await succeeded, "");
});
