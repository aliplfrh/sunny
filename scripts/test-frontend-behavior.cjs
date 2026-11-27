// Isolated frontend regression tests. No native app, OS input, or npm dependencies.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { spawnSync } = require("node:child_process");

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
  insertBefore(node, before) { node.parentNode = this; this.children.splice(this.children.indexOf(before), 0, node); }
  remove() { if (this.parentNode) this.parentNode.children.splice(this.parentNode.children.indexOf(this), 1); }
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
  getContext() {
    return { fillStyle: "", fillRect() {} };
  }
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

async function openVideo(f) {
  await f.search("Video Download");
  const i = f.run('items.findIndex((item) => item.cmd === "video")');
  assert.ok(i >= 0, "Video Download command is searchable");
  await f.run(`activate(${i})`);
  await f.settle();
}

const videoIdle = { id: 0, phase: "idle", percent: null, file_path: null, message: "Paste a video URL to begin." };

test("Settings has explicit CLI download and requirements checks, with no automatic setup", async () => {
  const f = await fixture({ settingsWindow: true });
  const buttons = f.nodes.settingsBody.querySelectorAll("button");
  assert.ok(buttons.some((b) => b.textContent === "Download OmniGet CLI"));
  assert.ok(buttons.some((b) => b.textContent === "Check requirements"));
  assert.equal(f.calls.some((c) => c.command === "video_setup"), false);
});

test("CLI setup is single-flight, shows progress, saves its path and never launches an installer", async () => {
  let finish;
  const f = await fixture({ settingsWindow: true, responses: {
    video_setup: () => new Promise((resolve) => { finish = resolve; }),
    settings_update: ({ settings }) => settings,
  } });
  const button = f.nodes.settingsBody.querySelectorAll("button").find((b) => b.textContent === "Download OmniGet CLI");
  assert.ok(button);
  const pending = button.click();
  await f.settle();
  await button.click();
  assert.equal(f.calls.filter((c) => c.command === "video_setup").length, 1);
  f.events["sunny:omniget-setup"]({ payload: { message: "Downloading verified package…", percent: 40 } });
  assert.equal(f.run("omnigetSetupMessage"), "Downloading verified package…");
  finish({ available: true, path: "C:\\Sunny\\tools\\omniget-cli.exe", missing_tools: ["yt-dlp", "FFmpeg"] });
  await pending;
  await f.settle();
  assert.equal(f.calls.find((c) => c.command === "settings_update").args.settings.omniget_cli_path, "C:\\Sunny\\tools\\omniget-cli.exe");
  assert.match(f.run("omnigetSetupMessage"), /yt-dlp/);
  assert.equal(f.calls.some((c) => ["open_path", "run_app", "video_start"].includes(c.command)), false);
});

test("failed setup does not change preferences and can be retried", async () => {
  const f = await fixture({ settingsWindow: true, settings: { omniget_cli_path: "C:\\Existing\\omniget-cli.exe" }, responses: {
    video_setup: () => { throw new Error("Checksum mismatch"); },
  } });
  const button = f.nodes.settingsBody.querySelectorAll("button").find((b) => b.textContent === "Download OmniGet CLI");
  assert.ok(button);
  await button.click();
  await f.settle();
  assert.match(f.run("omnigetSetupMessage"), /Checksum mismatch/);
  assert.equal(f.run("omnigetSetupBusy"), false);
  assert.equal(f.run("SETTINGS.omniget_cli_path"), "C:\\Existing\\omniget-cli.exe");
  assert.equal(f.calls.some((c) => c.command === "settings_update"), false);
  await button.click();
  assert.equal(f.calls.filter((c) => c.command === "video_setup").length, 2);
});

test("a lost setup response unlocks the button instead of hanging forever", async () => {
  const f = await fixture({ settingsWindow: true, responses: {
    video_setup: () => new Promise(() => {}),
  } });
  const button = f.nodes.settingsBody.querySelectorAll("button").find((b) => b.textContent === "Download OmniGet CLI");
  assert.ok(button);
  const pending = button.click();
  await f.settle();
  assert.equal(f.run("omnigetSetupBusy"), true);
  await f.runTimers(180000);
  assert.equal(f.run("omnigetSetupBusy"), false);
  assert.match(f.run("omnigetSetupMessage"), /stopped responding/);
  await pending;
  assert.equal(f.calls.some((c) => c.command === "settings_update"), false);
  assert.equal(button.disabled, false);
  assert.equal(button.textContent, "Download OmniGet CLI");
});

test("requirements check distinguishes a found CLI from missing video engines without downloading", async () => {
  const f = await fixture({ settingsWindow: true, responses: {
    video_check: { available: true, path: "C:\\OmniGet\\omniget-cli.exe", missing_tools: ["FFmpeg", "FFprobe"] },
  } });
  const button = f.nodes.settingsBody.querySelectorAll("button").find((b) => b.textContent === "Check requirements");
  assert.ok(button);
  await button.click();
  await f.settle();
  assert.match(f.run("omnigetSetupMessage"), /FFmpeg.*FFprobe/);
  assert.equal(f.calls.some((c) => c.command === "video_setup"), false);
});

test("Video Download opens inside Sunny without launching a tab or fetching a URL", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: true, path: "C:\\OmniGet\\omniget-cli.exe" } } });
  await openVideo(f);
  assert.equal(f.run("mode"), "video");
  assert.equal(f.nodes.videoPanel.hidden, false);
  assert.equal(f.nodes.list.hidden, true);
  await f.search("https://example.com/video?");
  assert.equal(f.calls.some((c) => ["video_start", "web_search", "open_path", "run_app"].includes(c.command)), false);
});

test("video download starts once on Enter, retains focus, and sends the selected quality", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: true } } });
  await openVideo(f);
  f.nodes.videoQuality.value = "720";
  let finish;
  f.handlers.video_start = () => new Promise((resolve) => { finish = resolve; });
  await f.search("https://example.com/video");
  await f.key("Enter");
  await f.key("Enter", { repeat: true });
  await f.nodes.videoStart.click();
  assert.equal(f.calls.filter((c) => c.command === "video_start").length, 1);
  const args = f.calls.find((c) => c.command === "video_start").args;
  assert.equal(args.url, "https://example.com/video");
  assert.equal(args.quality, "720");
  finish({ ...videoIdle, id: 1, phase: "running", message: "Starting…" });
  await f.settle();
  assert.equal(f.calls.some((c) => c.command === "hide_main_window"), false);
});

test("video controls use native keyboard behavior and progress cannot overwrite a newer job", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: true } } });
  await openVideo(f);
  await f.key("Enter", { target: { tagName: "SELECT" } });
  assert.equal(f.calls.some((c) => c.command === "video_start"), false);
  f.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 2, phase: "running", percent: 64, message: "Downloading…" } });
  f.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 1, phase: "complete", percent: 100 } });
  assert.equal(f.nodes.videoProgress.value, 64);
  assert.equal(f.nodes.videoCancel.hidden, false);
  await f.nodes.videoCancel.click();
  assert.equal(f.calls.find((c) => c.command === "video_cancel").args.id, 2);
});

test("video failures and a missing OmniGet installation stay visible inside the panel", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: false, path: null } } });
  await openVideo(f);
  assert.match(f.nodes.videoSetup.textContent, /omniget-cli\.exe/i);
  assert.equal(f.nodes.videoStart.disabled, true);
  f.handlers.video_check = { available: true };
  await f.nodes.videoRetry.click();
  await f.search("https://example.com/video");
  f.handlers.video_start = () => { throw new Error("Could not start OmniGet"); };
  await f.key("Enter");
  assert.match(f.nodes.videoStatus.textContent, /Could not start OmniGet/);
  assert.equal(f.nodes.videoStart.disabled, false);
});

test("late video status snapshots cannot restore running state after completion", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: true } } });
  await openVideo(f);
  f.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 1, revision: 4, phase: "complete", file_path: "C:\\Downloads\\video.mp4", percent: 100, message: "Saved." } });
  f.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 1, revision: 0, phase: "running" } });
  assert.equal(f.nodes.videoReveal.hidden, false);
  assert.equal(f.nodes.videoCancel.hidden, true);
  await f.nodes.videoReveal.click();
  await f.settle();
  assert.equal(f.calls.find((c) => c.command === "video_reveal").args.id, 1);
  assert.equal(f.calls.some((c) => c.command === "open_path"), false);
});

test("changing the CLI path live rechecks installation and URL input has its own accessible label", async () => {
  const f = await fixture({ responses: { video_status: videoIdle, video_check: { available: false } } });
  await openVideo(f);
  const before = f.calls.filter((c) => c.command === "video_check").length;
  f.handlers.video_check = { available: true };
  f.events["sunny:settings-updated"]({ payload: { omniget_cli_path: "C:\\OmniGet\\omniget-cli.exe" } });
  await f.settle();
  assert.equal(f.calls.filter((c) => c.command === "video_check").length, before + 1);
  await f.search("https://example.com/video");
  assert.equal(f.nodes.videoStart.disabled, false);
  assert.equal(f.nodes.query.getAttribute("aria-label"), "Video URL");
  assert.equal(f.nodes.query.getAttribute("role"), null);
  await f.key("Escape");
  assert.equal(f.nodes.query.getAttribute("role"), "combobox");
});

test("leaving video mode does not cancel an active download and reopening restores it", async () => {
  const status = { ...videoIdle, id: 4, phase: "running", percent: 30, message: "Downloading…" };
  const f = await fixture({ responses: { video_status: status, video_check: { available: true } } });
  await openVideo(f);
  await f.key("Escape");
  assert.equal(f.run("mode"), "apps");
  assert.equal(f.nodes.videoPanel.hidden, true);
  assert.equal(f.calls.some((c) => c.command === "video_cancel"), false);
  await openVideo(f);
  assert.equal(f.nodes.videoProgress.value, 30);
  assert.equal(f.nodes.videoCancel.hidden, false);
});

test("video event text is rendered as text and Settings never checks or starts OmniGet", async () => {
  const f = await fixture({ settingsWindow: true });
  assert.equal(f.calls.some((c) => c.command.startsWith("video_")), false);
  f.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 1, phase: "running", message: "<img src=x onerror=alert(1)>" } });
  assert.equal(f.calls.some((c) => c.command.startsWith("video_")), false);
  const launcher = await fixture({ responses: { video_status: videoIdle, video_check: { available: true } } });
  await openVideo(launcher);
  launcher.events["sunny:video-progress"]({ payload: { ...videoIdle, id: 1, phase: "failed", message: "<img src=x onerror=alert(1)>" } });
  assert.equal(launcher.nodes.videoStatus.textContent, "<img src=x onerror=alert(1)>");
  assert.equal(launcher.nodes.videoStatus.children.length, 0);
});

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

test("file rows offer Copy Full Path and Show in Explorer without closing the launcher", async () => {
  const copied = [];
  const f = await fixture({ responses: {
    search_apps: [],
    search_files: { ready: true, total: 1, items: [{ item: { name: "Report.pdf", path: "C:\\Users\\me\\Downloads\\Report.pdf" } }] },
  } });
  f.context.navigator.clipboard.writeText = async (text) => { copied.push(text); };
  await f.search("report");
  assert.equal(f.run("items[0].kind"), "file");

  await f.key("K", { ctrlKey: true, shiftKey: true });
  assert.equal(f.nodes.actions.hidden, false);
  assert.deepEqual(f.nodes.actions.querySelectorAll("button").map((b) => b.textContent),
    ["Open File", "Copy Full Path", "Show in Explorer"]);

  await f.nodes.actions.querySelectorAll("button")[1].click();
  await f.settle();
  assert.deepEqual(copied, ["C:\\Users\\me\\Downloads\\Report.pdf"]);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);

  await f.key("K", { ctrlKey: true, shiftKey: true });
  await f.nodes.actions.querySelectorAll("button")[2].click();
  await f.settle();
  assert.equal(f.calls.filter((call) => call.command === "reveal_path").pop().args.path,
    "C:\\Users\\me\\Downloads\\Report.pdf");
});

test("file actions are reachable by shortcut and never open an unrelated row", async () => {
  const copied = [];
  const f = await fixture({ responses: {
    search_apps: [],
    search_files: { ready: true, total: 1, items: [{ item: { name: "Notes.txt", path: "C:\\Temp\\Notes.txt" } }] },
  } });
  f.context.navigator.clipboard.writeText = async (text) => { copied.push(text); };
  await f.search("notes");
  await f.key("C", { ctrlKey: true, shiftKey: true });
  assert.deepEqual(copied, ["C:\\Temp\\Notes.txt"]);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
  await f.key("r", { ctrlKey: true });
  assert.equal(f.calls.filter((call) => call.command === "reveal_path").pop().args.path, "C:\\Temp\\Notes.txt");
});

test("rows without file or app actions never open the actions menu", async () => {
  const f = await fixture();
  await f.search("emoji");
  const index = f.run('items.findIndex((item) => item.cmd === "emoji")');
  await f.run(`activate(${index})`);
  await f.settle();
  await f.key("K", { ctrlKey: true, shiftKey: true });
  assert.equal(f.nodes.actions.hidden, true);
});

test("a repeated activation cannot open the browser twice", async () => {
  const f = await fixture();
  let release;
  f.handlers.web_search = () => new Promise((resolve) => { release = () => resolve("https://example.com"); });
  await f.search("Siapa itu jokowi?");
  assert.equal(f.run("items[0].kind"), "web");
  f.calls.length = 0;

  // Enter held down, or a second click, while the first launch is still in flight.
  const first = f.run("activate(0)");
  const second = f.run("activate(0)");
  await f.settle();
  release();
  await Promise.all([first, second]);

  assert.equal(f.calls.filter((call) => call.command === "web_search").length, 1);
});

test("holding Enter does not launch the same row repeatedly", async () => {
  const f = await fixture();
  f.handlers.open_path = () => new Promise(() => {});
  f.handlers.search_apps = [];
  f.handlers.search_files = { ready: true, total: 1, items: [{ item: { name: "Notes.txt", path: "C:\\Temp\\Notes.txt" } }] };
  await f.search("notes");
  f.calls.length = 0;

  // Auto-repeat sends several Enter events with repeat set.
  for (let i = 0; i < 3; i++) await f.key("Enter", { repeat: true });
  await f.key("Enter");

  assert.equal(f.calls.filter((call) => call.command === "open_path").length, 1);
});

test("a failed web launch is visible and may be retried", async () => {
  const f = await fixture();
  let attempts = 0;
  f.handlers.web_search = () => { if (++attempts === 1) throw new Error("No browser"); };
  await f.search("find this?");
  await f.run("activate(0)");
  assert.match(f.nodes.launcherStatus.textContent, /No browser/);
  await f.run("activate(0)");
  assert.equal(attempts, 2);
});

test("a successful web launch stays guarded until the launcher is shown again", async () => {
  const f = await fixture();
  await f.search("first search?");
  await f.run("activate(0)");
  await f.run("activate(0)");
  assert.equal(f.calls.filter((call) => call.command === "web_search").length, 1);
  f.events["sunny:shown"]();
  await f.settle();
  await f.search("next search?");
  await f.run("activate(0)");
  assert.equal(f.calls.filter((call) => call.command === "web_search").length, 2);
});

test("a failed file open keeps the launcher visible and permits another attempt", async () => {
  const f = await fixture({ responses: {
    search_apps: [],
    search_files: { ready: true, total: 1, items: [{ item: { name: "note", path: "C:\\note.txt" } }] },
    open_path: () => { throw new Error("No handler"); },
  } });
  await f.search("note");
  const index = f.run('items.findIndex((item) => item.kind === "file")');
  assert.ok(index >= 0);
  await f.run(`activate(${index})`);
  assert.match(f.nodes.launcherStatus.textContent, /No handler/);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
  await f.run(`activate(${index})`);
  assert.equal(f.calls.filter((call) => call.command === "open_path").length, 2);
});

test("QR command opens an offline text-to-image view with an accessible preview", async () => {
  const f = await fixture({ responses: { qr_generate: [[false, true], [true, false]] } });
  await f.search("qr code");
  const index = f.run('items.findIndex((item) => item.cmd === "qr")');
  assert.ok(index >= 0);
  await f.run(`activate(${index})`);
  assert.equal(f.run("mode"), "qr");
  assert.match(f.nodes.query.placeholder, /QR/);
  await f.search("https://example.com/?q=hi&x=2");
  assert.equal(f.calls.filter((call) => call.command === "qr_generate").at(-1).args.text,
    "https://example.com/?q=hi&x=2");
  assert.equal(f.nodes.qrPreview.hidden, false);
  assert.equal(f.nodes.qrPreview.querySelectorAll("canvas").length, 1);
  assert.equal(f.calls.some((call) => call.command === "web_search"), false);
  await f.key("Enter");
  assert.equal(f.calls.filter((call) => call.command === "qr_copy").at(-1).args.text,
    "https://example.com/?q=hi&x=2");
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
  await f.key("Escape");
  assert.equal(f.run("mode"), "apps");
});

test("ordinary file queries do not surface the QR command", async () => {
  const f = await fixture();
  await f.search("note");
  assert.equal(f.run('items.some((item) => item.cmd === "qr")'), false);
});

test("typing QR content coalesces generation until the latest input settles", async () => {
  const f = await fixture({ responses: { qr_generate: [[true]] } });
  await f.search("qr code");
  await f.run('activate(items.findIndex((item) => item.cmd === "qr"))');
  f.nodes.query.value = "old";
  f.nodes.query.listeners.input[0]();
  f.nodes.query.value = "latest";
  f.nodes.query.listeners.input[0]();
  await f.runTimers(80);
  assert.deepEqual(f.calls.filter((call) => call.command === "qr_generate").map((call) => call.args.text),
    ["latest"]);
});

test("QR rejects empty input, reports generation and copy errors, and ignores stale previews", async () => {
  const f = await fixture({ responses: { qr_generate: [[true]] } });
  await f.search("qr code");
  await f.run('activate(items.findIndex((item) => item.cmd === "qr"))');
  await f.search("   ");
  assert.equal(f.nodes.qrPreview.hidden, true);
  assert.equal(f.calls.some((call) => call.command === "qr_generate"), false);

  let release;
  f.handlers.qr_generate = ({ text }) => text === "old" ? new Promise((resolve) => { release = resolve; }) : [[true]];
  f.nodes.query.value = "old";
  const pending = f.run("refresh()");
  await f.search("new");
  release([[false, false]]);
  await pending;
  await f.settle();
  assert.equal(f.nodes.qrPreview.querySelectorAll("canvas")[0].dataset.size, "1");
  f.handlers.qr_copy = () => { throw new Error("Clipboard busy"); };
  await f.key("Enter");
  assert.match(f.nodes.launcherStatus.textContent, /Clipboard busy/);
  f.handlers.qr_generate = () => { throw new Error("Too long"); };
  await f.search("another");
  assert.equal(f.nodes.qrPreview.hidden, true);
  assert.match(f.nodes.launcherStatus.textContent, /Too long/);
});

test("a failed hide does not block later launches", async () => {
  const f = await fixture({ responses: {
    hide_main_window: () => { throw new Error("Window busy"); },
    web_search: () => "https://example.com",
  } });
  await f.search("first search?");
  await f.run("activate(0)");
  assert.match(f.nodes.launcherStatus.textContent, /Window busy/);
  await f.run("activate(0)");
  assert.equal(f.calls.filter((call) => call.command === "web_search").length, 2);
});

test("a copy that fails everywhere is reported instead of losing the text", async () => {
  const f = await fixture({ responses: {
    calc: "42",
    copy_to_clipboard: () => { throw new Error("Clipboard busy"); },
  } });
  f.context.navigator.clipboard.writeText = async () => { throw new Error("denied"); };
  await f.search("6*7");
  assert.equal(f.run("items[0].kind"), "calc");
  await f.run("activate(0)");
  assert.match(f.nodes.launcherStatus.textContent, /Clipboard busy/);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
});

test("arrow keys move the highlight without rebuilding result rows", async () => {
  const f = await fixture({ responses: {
    search_apps: [{ item: { name: "One", path: "one.lnk" } }, { item: { name: "Two", path: "two.lnk" } }],
  } });
  await f.search("o");
  const first = f.nodes.list.querySelectorAll(".item")[0];
  await f.key("ArrowDown");
  const rows = f.nodes.list.querySelectorAll(".item");
  assert.equal(rows[0], first);
  assert.equal(rows[1].classList.contains("selected"), true);
  assert.equal(f.run("selected"), 1);
});

test("animated browser launches keep the guard until the launcher is shown", async () => {
  const f = await fixture();
  await f.search("guarded?");
  await f.run("activate(0)");
  await f.run("activate(0)");
  assert.equal(f.calls.filter((call) => call.command === "web_search").length, 1);
});

test("backend warnings reach the launcher status line", async () => {
  const f = await fixture();
  f.events["sunny:launcher-warning"]({ payload: "Index unavailable" });
  assert.match(f.nodes.launcherStatus.textContent, /Index unavailable/);
});

test("clipboard footer names the real copy and delete shortcuts", async () => {
  const f = await fixture({ responses: { clip_history: [] } });
  await f.search("clipboard");
  const index = f.run('items.findIndex((item) => item.cmd === "clip")');
  await f.run(`activate(${index})`);
  await f.settle();
  assert.match(f.nodes.hints.innerHTML, /Ctrl<\/kbd><kbd>B<\/kbd> Copy/);
  assert.match(f.nodes.hints.innerHTML, /Ctrl<\/kbd><kbd>D<\/kbd> Delete/);
});

test("every mode keeps its footer title, shortcuts and Back button", async () => {
  const f = await fixture({ responses: { clip_history: [] } });
  const footers = {
    "": { title: "Sunny", actions: ["Open"], back: false, menu: true },
    clipboard: { title: "Clipboard History", actions: ["Paste", "Copy", "Delete"], back: false, menu: false },
    emoji: { title: "Emoji Picker", actions: ["Copy"], back: true, menu: false },
    "qr code": { title: "QR Code Generator", actions: ["Copy Image"], back: true, menu: false },
    "search files": { title: "Search Files", actions: ["Open", "Reveal"], back: false, menu: true },
  };
  for (const [query, expected] of Object.entries(footers)) {
    await f.run("resetLauncherSearch()");
    await f.search(query);
    if (query) {
      const index = f.run('items.findIndex((item) => item.cmd && item.cmd !== "web")');
      await f.run(`activate(${index})`);
    }
    await f.settle();
    const html = f.nodes.hints.innerHTML;
    const label = query || "root";
    assert.match(html, new RegExp(expected.title), `${label} title`);
    for (const action of expected.actions) assert.match(html, new RegExp(` ${action}<`), `${label}: ${action}`);
    assert.equal(/data-back/.test(html), expected.back, `${label} Back button`);
    assert.equal(/data-actions/.test(html), expected.menu, `${label} Actions menu`);
    const trailing = expected.back || expected.menu ? 1 : 0;
    assert.equal((html.match(/hint-sep/g) || []).length, expected.actions.length - 1 + trailing, `${label} separators`);
  }
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

test("Search Files command is hidden while root file search covers it", async () => {
  const f = await fixture({ responses: {
    search_apps: [], search_files: { ready: true, total: 0, items: [] },
  } });
  const filesCommands = () => f.run('items.filter((item) => item.cmd === "files").length');

  // Root file search is on, so a separate command would duplicate its Files section.
  await f.search("fil");
  assert.equal(filesCommands(), 0);
  await f.search("search");
  assert.equal(filesCommands(), 0);

  // Typing the command's own name keeps it reachable.
  await f.search("search files");
  assert.equal(filesCommands(), 1);
  await f.search("files");
  assert.equal(filesCommands(), 1);

  // With root file search off the command is the only route, so it always shows.
  const off = await fixture({ settings: { include_files: false }, responses: { search_apps: [] } });
  await off.search("fil");
  assert.equal(off.run('items.filter((item) => item.cmd === "files").length'), 1);
});

test("file result count is labelled in files, not pixels", async () => {
  assert.match(source, /function stepperControl\(start, min, max, onChange, unit = "px"\)/);
  assert.match(source, /stepperControl\(SETTINGS\.file_results[\s\S]{0,160}?"files"/);
});

test("lorem queries become a copyable result and other queries do not", async () => {
  const f = await fixture({ responses: {
    search_apps: [],
    lorem: (args) => (/^lorem/i.test(args.query) ? "Lorem ipsum dolor sit amet." : null),
  } });

  await f.search("lorem 2 sentences");
  assert.equal(f.run('items.filter((i) => i.kind === "lorem").length'), 1);
  assert.equal(f.run('items.find((i) => i.kind === "lorem").text'), "Lorem ipsum dolor sit amet.");
  assert.deepEqual(f.headings(), ["Results"]);

  await f.search("photoshop");
  assert.equal(f.run('items.filter((i) => i.kind === "lorem").length'), 0);
});

test("a trailing question mark offers a web search in the configured engine", async () => {
  const f = await fixture({ settings: { search_engine: "duckduckgo" }, responses: {
    search_apps: [],
    search_engines: [
      { id: "google", name: "Google", template: "https://www.google.com/search?q={query}", suffix: "" },
      { id: "duckduckgo", name: "DuckDuckGo", template: "https://duckduckgo.com/?q={query}", suffix: "" },
    ],
  } });

  await f.search("rust ownership?");
  const web = f.run('items.find((i) => i.kind === "web")');
  assert.ok(web, "a web search row should be offered");
  assert.equal(web.engine, "duckduckgo");
  assert.equal(web.text, "rust ownership");

  // A leading question mark works too.
  await f.search("?rust ownership");
  assert.equal(f.run('items.find((i) => i.kind === "web").text'), "rust ownership");

  // Without the marker there is no web row.
  await f.search("rust ownership");
  assert.equal(f.run('items.filter((i) => i.kind === "web").length'), 0);

  // A bare marker is not a search.
  await f.search("?");
  assert.equal(f.run('items.filter((i) => i.kind === "web").length'), 0);
});

test("activating a web result opens the browser through the backend", async () => {
  const f = await fixture({ responses: {
    search_apps: [],
    web_search: "https://www.google.com/search?q=rust%20ownership",
  } });
  await f.search("rust ownership?");
  const index = f.run('items.findIndex((i) => i.kind === "web")');
  assert.ok(index >= 0);
  await f.run(`activate(${index})`);
  await f.settle();
  const call = f.calls.filter((c) => c.command === "web_search").pop();
  assert.equal(call.args.query, "rust ownership?");
  assert.equal(call.args.engineId, "google");
});

test("actual web activation arguments pass the real Rust backend parser", async () => {
  for (const query of ["Siapa itu jokowi?", "?c++ & rust #1", "harga 東京??"]) {
    const f = await fixture();
    await f.search(query);
    await f.run('activate(items.findIndex((item) => item.kind === "web"))');
    const call = f.calls.find((entry) => entry.command === "web_search");
    assert.ok(call);
    const result = spawnSync("cargo", ["run", "--quiet", "--offline", "-p", "sun-core",
      "--example", "web_search_contract"], {
      cwd: path.join(__dirname, ".."), input: JSON.stringify(call.args), encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const url = new URL(result.stdout.trim());
    assert.equal(url.searchParams.get("q"), query.trim().replace(/^\?+|\?+$/g, "").trim());
  }
});

test("marked web queries do not wait for or invoke local search", async () => {
  const f = await fixture();
  f.calls.length = 0;
  for (const command of ["calc", "lorem", "search_apps", "search_files"]) {
    f.handlers[command] = () => { throw new Error(`unexpected ${command}`); };
  }
  await f.search("Siapa itu jokowi?");
  assert.equal(f.run("items[0]?.kind"), "web");
  assert.equal(f.calls.length, 0);
  await f.search("?😀");
  assert.equal(f.run("items.length"), 0, "match Rust Unicode character minimum");
});

test("local providers start together and apps render before slow files", async () => {
  const f = await fixture();
  let resolveCalc, resolveFiles;
  f.handlers.calc = () => new Promise((resolve) => { resolveCalc = resolve; });
  f.handlers.search_apps = [
    { item: { name: "Fast app", path: "fast.lnk" } },
    { item: { name: "Second app", path: "second.lnk" } },
  ];
  f.handlers.search_files = () => new Promise((resolve) => { resolveFiles = resolve; });
  f.calls.length = 0;
  f.nodes.query.value = "fast";
  const pending = f.run("refresh()");
  await f.settle();
  assert.ok(f.calls.some((call) => call.command === "search_apps"));
  assert.ok(resolveFiles, "file provider starts concurrently");
  resolveCalc(null);
  await f.settle();
  assert.equal(f.run("items[0].name"), "Fast app");
  // Even after keyboard navigation the file append must not reset selection.
  await f.key("ArrowDown");
  resolveFiles({ ready: true, total: 1, items: [{ item: { name: "File", path: "file.txt" } }] });
  await pending;
  assert.deepEqual(f.headings(), ["Results", "Files"]);
  assert.equal(f.run("items[selected].name"), "Second app");
});

test("file search errors preserve usable app results", async () => {
  const f = await fixture({ responses: {
    search_apps: [{ item: { name: "Fast app", path: "fast.lnk" } }],
    search_files: () => { throw new Error("File search unavailable"); },
  } });
  await f.search("fast");
  assert.equal(f.run("items[0].name"), "Fast app");
  assert.match(f.nodes.launcherStatus.textContent, /File search unavailable/);
});

test("typing bursts coalesce requests and invalidate old rows immediately", async () => {
  const f = await fixture();
  f.calls.length = 0;
  for (const query of ["s", "su", "sun", "sunn", "sunny"]) {
    f.nodes.query.value = query;
    for (const callback of f.nodes.query.listeners.input || []) callback();
  }
  await f.settle();
  assert.equal(f.run("items.length"), 0, "Enter must not launch an old result");
  assert.equal(f.calls.length, 0);
  await f.runTimers(40);
  assert.equal(f.calls.filter((call) => call.command === "search_apps").length, 1);
  assert.equal(f.calls.find((call) => call.command === "search_apps").args.query, "sunny");
});

test("late files cannot replace a newer query, including during debounce", async () => {
  const f = await fixture();
  let resolveFiles;
  f.handlers.search_files = () => new Promise((resolve) => { resolveFiles = resolve; });
  f.nodes.query.value = "old";
  const pending = f.run("refresh()");
  await f.settle();
  f.nodes.query.value = "new";
  for (const callback of f.nodes.query.listeners.input || []) callback();
  resolveFiles({ ready: true, total: 1, items: [{ item: { name: "Stale file", path: "old.txt" } }] });
  await pending;
  assert.equal(f.run("items.length"), 0);
  f.handlers.search_files = { ready: true, total: 1, items: [{ item: { name: "New file", path: "new.txt" } }] };
  await f.runTimers(40);
  assert.equal(f.run("items[0].name"), "New file");
});

test("Enter passes the captured marked query, and browser failures stay visible", async () => {
  const f = await fixture({ responses: {
    web_search: () => { throw new Error("Cannot open default browser"); },
  } });
  await f.search("Siapa itu jokowi?");
  f.calls.length = 0;
  await f.key("Enter");
  assert.equal(f.calls.find((call) => call.command === "web_search").args.query, "Siapa itu jokowi?");
  assert.match(f.nodes.launcherStatus.textContent, /Cannot open default browser/);
  assert.equal(f.calls.some((call) => call.command === "hide_main_window"), false);
});
