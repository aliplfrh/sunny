// Isolated UI tests: no WebView, desktop input, registry, or keyboard hooks.
const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

function fixture(error = "") {
  const source = fs.readFileSync(path.join(__dirname, "../apps/sunny/ui/main.js"), "utf8");
  const start = source.indexOf("let hotkeyBtnRef = null;");
  const end = source.indexOf("/* ── Tab panel Settings", start);
  assert.ok(start >= 0 && end > start);
  const calls = [];
  const context = {
    SETTINGS: { hotkey: "Alt+Space" }, SETTINGS_ERROR: "", capturingHotkey: false,
    renderSettings() {},
    onSettingChange: async (patch) => {
      calls.push(patch.hotkey);
      Object.assign(context.SETTINGS, patch);
      return error;
    },
    el: (tag, className, textContent) => ({
      tag, className, textContent, children: [], attributes: {},
      classList: { add() {}, remove() {}, toggle() {} },
      append(...children) { this.children.push(...children); },
      setAttribute(key, value) { this.attributes[key] = value; },
    }),
  };
  vm.runInNewContext(source.slice(start, end) + "\nglobalThis.api = { hotkeyControl, handleHotkeyCapture, accelFromEvent };", context);
  return { context, calls, api: context.api };
}

test("explicit Win+Space preset saves the canonical accelerator", async () => {
  const { api, context, calls } = fixture();
  const control = api.hotkeyControl();
  const preset = control.children[1];
  assert.equal(preset.type, "button");
  assert.equal(preset.textContent, "Win + Space");
  await preset.onclick();
  assert.equal(context.SETTINGS.hotkey, "Super+Space");
  assert.deepEqual(calls, ["Super+Space"]);
});

test("backend rejection restores the old value without another settings write", async () => {
  const { api, context, calls } = fixture("Unavailable");
  await api.hotkeyControl().children[1].onclick();
  assert.equal(context.SETTINGS.hotkey, "Alt+Space");
  assert.equal(context.SETTINGS_ERROR, "Unavailable");
  assert.equal(calls.length, 1);
});

test("saved Super accelerator is displayed as Win", () => {
  const { api, context } = fixture();
  context.SETTINGS.hotkey = "Super+Space";
  assert.equal(api.hotkeyControl().children[0].textContent, "Win + Space");
});

test("Escape cancels recording without saving", () => {
  const { api, context, calls } = fixture();
  api.hotkeyControl().children[0].onclick();
  assert.equal(context.capturingHotkey, true);
  api.handleHotkeyCapture({ key: "Escape", preventDefault() {}, stopPropagation() {} });
  assert.equal(context.capturingHotkey, false);
  assert.equal(calls.length, 0);
});

test("Tab cancels recording and keeps native keyboard focus navigation", () => {
  const { api, context } = fixture();
  api.hotkeyControl().children[0].onclick();
  let prevented = false;
  api.handleHotkeyCapture({ key: "Tab", preventDefault() { prevented = true; }, stopPropagation() {} });
  assert.equal(context.capturingHotkey, false);
  assert.equal(prevented, false);
});
