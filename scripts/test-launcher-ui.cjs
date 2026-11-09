// Tests launcher reopen behavior without opening windows or sending desktop keys.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

function fixture(isSettings = false) {
  const source = fs.readFileSync(path.join(__dirname, "../apps/sunny/ui/main.js"), "utf8");
  const start = source.indexOf("function resetLauncherSearch()");
  const end = source.indexOf('window.addEventListener("focus"', start);
  assert.ok(start >= 0 && end > start, "launcher has a dedicated reset on reopen");
  let refreshes = 0;
  let renders = 0;
  let focuses = 0;
  const events = {};
  const context = {
    IS_SETTINGS_WINDOW: isSettings, settingsOpen: false,
    mode: "clip", selected: 12, items: [{ name: "old result" }], refreshSeq: 5,
    queryEl: { value: "whats", placeholder: "Clipboard", focus() { focuses++; } },
    MODE_LABEL: { apps: "Search apps..." },
    backdropFresh: true,
    listen(name, handler) { events[name] = handler; },
    refresh() { refreshes++; }, render() { renders++; },
  };
  vm.runInNewContext(source.slice(start, end) + "\nglobalThis.reset = resetLauncherSearch;", context);
  return { context, events, focuses: () => focuses, refreshes: () => refreshes, renders: () => renders };
}

test("reopening clears query, previous results, selection, and specialized mode", () => {
  for (const mode of ["apps", "clip", "emoji", "files"]) {
    const { context, refreshes, renders } = fixture();
    context.mode = mode;
    context.reset();
    assert.equal(context.queryEl.value, "");
    assert.equal(context.queryEl.placeholder, "Search apps...");
    assert.equal(context.mode, "apps");
    assert.equal(context.selected, 0);
    assert.equal(context.items.length, 0);
    assert.equal(refreshes(), 1);
    assert.equal(renders(), 1);
  }
});

test("reset invalidates searches still awaiting backend results", () => {
  const { context } = fixture();
  const pendingSequence = context.refreshSeq;
  context.reset();
  assert.ok(context.refreshSeq > pendingSequence);
});

test("launcher events do not clear settings-window state", () => {
  const { context, refreshes } = fixture(true);
  context.reset();
  assert.equal(context.queryEl.value, "whats");
  assert.equal(context.items.length, 1);
  assert.equal(refreshes(), 0);
});

test("native shown event actually resets the launcher and focuses its search field", () => {
  const { context, events, focuses } = fixture();
  events["sunny:shown"]();
  assert.equal(context.queryEl.value, "");
  assert.equal(context.items.length, 0);
  assert.equal(focuses(), 1);
});

test("a delayed response from the old query cannot overwrite a reopened launcher", async () => {
  const { context } = fixture();
  const source = fs.readFileSync(path.join(__dirname, "../apps/sunny/ui/main.js"), "utf8");
  const start = source.indexOf("async function refresh()");
  const end = source.indexOf("/* ── Aktivasi baris", start);
  assert.ok(start >= 0 && end > start);
  let resolveOldQuery;
  context.mode = "apps";
  context.SETTINGS = { emoji_prefix: ":", include_files: false };
  context.COMMANDS = [];
  context.invoke = (command) => {
    if (command === "calc") return new Promise((resolve) => { resolveOldQuery = resolve; });
    assert.equal(command, "list_apps");
    return Promise.resolve([{ name: "Fresh application" }]);
  };
  vm.runInNewContext(source.slice(start, end), context);
  const oldSearch = context.refresh();
  context.reset();
  await new Promise(setImmediate);
  resolveOldQuery("old calculator result");
  await oldSearch;
  assert.equal(context.items.length, 1);
  assert.equal(context.items[0].name, "Fresh application");
  assert.equal(context.queryEl.value, "");
});
