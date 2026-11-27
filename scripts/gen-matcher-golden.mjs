// gen-matcher-golden.mjs
//
// Generates crates/sun-core/tests/fixtures/matcher-golden.json by RUNNING the
// verified reference harness (scripts/raycast-matcher-reference.mjs).
//
// Every expected value in the fixture comes from executing the transcription,
// never from hand calculation. Run with:
//
//   node scripts/gen-matcher-golden.mjs
//
// Determinism: `meta.generatedAt` is preserved from a previous run when the
// source bundle and its hash are unchanged, so re-running produces
// byte-identical output.

import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { normalize, matchScore, admit } from "./raycast-matcher-reference.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "..");
const OUT = path.join(REPO, "crates", "sun-core", "tests", "fixtures", "matcher-golden.json");

const BUNDLE =
  "C:\\Program Files\\WindowsApps\\Raycast.Raycast_2.14.0.0_x64__qypenmj9wpt2a\\Raycast\\frontend\\main-window-BA9jIdSt.js";
const SOURCE = "raycast 2.14.0 main-window-BA9jIdSt.js";

// ---------------------------------------------------------------------------
// Case corpus
//
// [text, query, note] — `note` documents intent; it is NOT written to the JSON.
//
// IMPORTANT (verified against the real bundle): the DP bridge is a RUNNING MAX
// over the previous row's columns `[d, col-2]`, so a gap of ANY length can be
// bridged for a flat -1 per bridged query character. "Gap >= 2 does not match"
// is FALSE. Cases below are annotated with what actually happens.
// ---------------------------------------------------------------------------

const CASES = [
  // 1. exact, and case-insensitive exact -------------------------------
  ["settings", "settings", "exact"],
  ["Settings", "settings", "case-insensitive exact (normalize lowercases both sides)"],
  ["Visual Studio Code", "visual studio code", "case-insensitive exact, spaces"],
  ["Cargo.lock", "cargo.lock", "exact, text contains a limiter"],
  ["", "", "empty text + empty query => exact, because r === a"],
  ["İstanbul", "i̇stanbul", "exact after toLowerCase() expands U+0130 to 'i'+U+0307"],
  ["İstanbul", "istanbul", "NOT exact: 'i'+U+0307 != 'i', so it falls through to the DP"],
  ["istanbul", "istanbul", "ASCII baseline for the case above"],

  // 2. prefix matches, lengths 1..5, short and long --------------------
  ["Photos", "p", "prefix len 1, short name"],
  ["Photos", "ph", "prefix len 2, short name"],
  ["Photos", "pho", "prefix len 3, short name"],
  ["Photos", "phot", "prefix len 4, short name"],
  ["Photos", "photo", "prefix len 5, short name"],
  ["Visual Studio Code", "v", "prefix len 1, long name"],
  ["Visual Studio Code", "vi", "prefix len 2, long name"],
  ["Visual Studio Code", "vis", "prefix len 3, long name"],
  ["Visual Studio Code", "visu", "prefix len 4, long name"],
  ["Visual Studio Code", "visua", "prefix len 5, long name"],

  // 3. contiguous interior matches -------------------------------------
  ["Visual Studio Code", "studio", "contiguous interior, after space"],
  ["TextEditor", "editor", "contiguous interior, after camelCase boundary"],
  ["report-final.pdf", "final", "contiguous interior, after '-'"],
  ["node_modules", "modules", "contiguous interior, after '_' (NOT a limiter => weight 2)"],
  ["Settings", "tti", "contiguous interior, mid-word"],

  // 4. word boundary after each limiter --------------------------------
  // Each of these compares against the same tail so the weight difference is
  // isolated. 'cd' after a limiter costs wordBeginning(3)+default(2)=5.
  ["ab-cd", "cd", "boundary after '-'"],
  ["ab.cd", "cd", "boundary after '.'"],
  ["ab/cd", "cd", "boundary after '/'"],
  ["ab(cd", "cd", "boundary after '('"],
  ["ab)cd", "cd", "boundary after ')'"],
  ["ab[cd", "cd", "boundary after '['"],
  ["ab]cd", "cd", "boundary after ']'"],
  ["ab cd", "cd", "boundary after ' '"],
  ["ab\tcd", "cd", "boundary after TAB"],
  ["ab\ncd", "cd", "boundary after NEWLINE"],
  ["ab_cd", "cd", "boundary after '_' — '_' is NOT a limiter"],
  ["ab\u00a0cd", "cd", "boundary after NBSP — NBSP is NOT a limiter in 2.14.0"],

  // 5. camelCase boundaries --------------------------------------------
  ["TextEditor", "te", "t at col 0 (first=4) + e at camelCase boundary (3)"],
  ["visualStudioCode", "code", "camelCase boundary, contiguous interior"],
  ["visualStudioCode", "vsc", "three camelCase hits across gaps"],
  ["TextEditor", "tor", "camelCase, contiguous interior"],
  ["myFileName", "fn", "camelCase, 1-char gap"],
  ["MP3Tag", "tag", "camelCase after a DIGIT (lae accepts [0-9][A-Z])"],

  // 6. separators inside the QUERY -------------------------------------
  ["app", "a-p-p", "query limiters with no text limiters => both skipped"],
  ["app", "app", "plain baseline for the case above"],
  ["texteditor", "text-editor", "query '-' has no text limiter => skipped"],
  ["text-editor", "text-editor", "exact with limiter in both"],
  ["texteditor", "text editor", "query space, text has no limiter => skipped"],
  ["abc", "a b", "single query limiter skipped"],
  ["abcde", "a e", "query limiter skipped mid-query"],

  // 7. queries longer than the text ------------------------------------
  ["ab", "abc", "meaningful query length (3) > text length (2)"],
  ["x", "xy", "1-char text, 2-char query"],
  ["a", "aa", "repeated char, query longer"],

  // 8. non-subsequence --------------------------------------------------
  ["ba", "ab", "classic non-subsequence"],
  ["abcdef", "fed", "reverse order"],
  ["7-Zip", "z7", "non-subsequence across a limiter"],
  ["settings", "setx", "trailing char absent"],
  ["node_modules", "mn", "non-subsequence initials"],
  ["café", "cafe", "é is not e => no subsequence"],
  ["a", "b", "single char, different"],
  ["control panel display", "cpdx", "'x' absent => unmatched"],
  ["IMG_2024_001.jpg", "jpg2", "'2' precedes 'jpg' => reverse order => unmatched"],
  ["node_modules", "nmx", "'x' absent => unmatched"],
  ["abcdefghij", "jih", "reverse order after the first char => unmatched"],

  // 9. gap behaviour — ANY gap length bridges for a flat -1 per query char
  ["abcde", "ace", "two 1-char gaps"],
  ["abcdefghij", "ace", "two 1-char gaps (b, d skipped)"],
  ["abxcde", "ace", "2-char gap a..c and 1-char gap c..e"],
  ["abcdefghij", "adg", "2-char gaps both sides — STILL MATCHES (score 6)"],
  ["abcdefghij", "aj", "9-char gap — STILL MATCHES (score 5)"],
  ["visual studio code", "vsc", "v->s crosses 6 chars, s->c crosses 6 — matches, score 8"],
  ["visual studio code", "vscode", "long jumps, score 14"],
  ["control panel display", "cpd", "c->p 6 chars, p->d 8 chars — matches, score 8"],
  ["task manager", "tm", "initials across a space, score 6"],
  ["snipping tool", "st", "initials across a space, score 6"],
  ["node_modules", "nm", "initials across '_', score 5"],
  ["visual studio code", "cd", "c->d adjacent in 'code', score 4"],
  ["mp3tag", "mpt", "m->p gap 2, p->t gap 1, score 7"],

  // 10. empty query ------------------------------------------------------
  ["settings", "", "empty query => scored with score === -(2**53-1), NOT 'unmatched'"],
  ["Visual Studio Code", "", "empty query, long text"],
  ["a", "", "empty query, 1-char text"],

  // 11. realistic Windows app names -------------------------------------
  ["Microsoft Edge", "edge", "interior after space"],
  ["Microsoft Edge", "me", "initials"],
  ["Windows Terminal", "terminal", "interior after space"],
  ["Windows Terminal", "wt", "initials"],
  ["Task Manager", "task", "prefix"],
  ["Task Manager", "tm", "initials across space"],
  ["Task Manager", "manager", "interior after space"],
  ["File Explorer", "fe", "initials"],
  ["File Explorer", "explorer", "interior after space"],
  ["Snipping Tool", "st", "initials across space"],
  ["Snipping Tool", "snip", "prefix"],
  ["Calculator", "calc", "prefix"],
  ["Control Panel", "cp", "initials"],
  ["Control Panel", "panel", "interior after space"],
  ["Settings", "set", "prefix"],
  ["Photos", "pho", "prefix"],

  // 12. realistic file names --------------------------------------------
  ["report-final.pdf", "report", "prefix before limiter"],
  ["report-final.pdf", "rf", "initials across '-'"],
  ["report-final.pdf", "pdf", "extension after '.'"],
  ["IMG_2024_001.jpg", "img", "prefix"],
  ["IMG_2024_001.jpg", "jpg", "extension after '.'"],
  ["IMG_2024_001.jpg", "2024", "digits inside, contiguous"],
  ["node_modules", "nm", "initials across '_' (matches: bridge is unbounded)"],
  ["node_modules", "node", "prefix"],
  ["Cargo.lock", "cl", "initials across '.'"],
  ["Cargo.lock", "lock", "after '.'"],

  // 13. digits and mixed alphanumeric -----------------------------------
  ["7-Zip", "7z", "digit, limiter, letter"],
  ["7-Zip", "zip", "after '-'"],
  ["MP3Tag", "mp3", "digit+letters, camelCase boundary after '3'"],
  ["Visual Studio 2022", "2022", "trailing digits after space"],
  ["Visual Studio 2022", "vs2022", "initials + digits across gaps"],
  ["Visual Studio 2022", "vs", "initials"],

  // 14. first query char lands mid-word — `first` weight is COLUMN-0 ONLY
  ["abcdef", "cd", "first char at col 2 => default(2), NOT first(4); score 4"],
  ["Visual Studio Code", "isual", "first char at col 1 => default weight"],
  ["Settings", "ettings", "first char at col 1 => default weight"],
  ["report-final.pdf", "eport", "first char at col 1 => default weight"],
  ["aXbXcX", "bcd", "'d' is absent => unmatched"],
  ["cdefgh", "cd", "same 'cd' but at col 0 => first(4)+default(2) = 6; compare with 'abcdef'"],
  ["abcDef", "def", "first query char lands ON a camelCase boundary => wordBeginning(3), not first(4)"],
  ["abCdef", "cdef", "first char on a boundary, then 3 contiguous => 3+2+2+2 = 9"],
  ["abCdef", "cd", "'c' at col 2 is the camelCase boundary => 3+2 = 5, vs 4 for plain 'abcdef'"],
  ["abCdef", "bc", "first char at col 1 => default(2); 'c' IS the camelCase boundary => +3, total 5"],

  // 15. query limiter consumes a text limiter ---------------------------
  ["x-y", "x y", "query space consumes text '-' at limiter weight 1"],
  ["report-final.pdf", "report final", "query space consumes text '-'"],
  ["Visual Studio Code", "visual-studio", "query '-' consumes text space"],
  ["a.b", "a-b", "query '-' consumes text '.'"],
  ["a-b", "a.b", "query '.' consumes text '-'"],
  ["node_modules", "node modules", "text has '_' not a limiter => query space is skipped"],
];

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------

const results = CASES.map(([text, query]) => {
  const target = normalize(text);
  const q = query.toLowerCase();
  const m = matchScore(target, q);

  if (m === "exact") {
    return {
      text, query, kind: "exact",
      score: null, skipped: null,
      low: admit(m, q.length, "low"),
      medium: admit(m, q.length, "medium"),
      high: admit(m, q.length, "high"),
    };
  }
  if (m === "unmatched") {
    return {
      text, query, kind: "unmatched",
      score: null, skipped: null,
      low: false, medium: false, high: false,
    };
  }
  return {
    text, query, kind: "scored",
    score: m.score, skipped: m.skipped,
    low: admit(m, q.length, "low"),
    medium: admit(m, q.length, "medium"),
    high: admit(m, q.length, "high"),
  };
});

const bundleSha256 = crypto
  .createHash("sha256")
  .update(fs.readFileSync(BUNDLE))
  .digest("hex");

// Reproducible timestamp: keep the previous one if the source is unchanged.
let generatedAt = new Date().toISOString();
if (fs.existsSync(OUT)) {
  try {
    const prev = JSON.parse(fs.readFileSync(OUT, "utf8"));
    if (prev?.meta?.bundleSha256 === bundleSha256 && prev?.meta?.source === SOURCE) {
      generatedAt = prev.meta.generatedAt;
    }
  } catch {
    // unreadable/invalid previous file -> fall through to "now"
  }
}

const doc = {
  meta: { source: SOURCE, bundleSha256, generatedAt },
  cases: results,
};

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify(doc, null, 2) + "\n", "utf8");

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

const counts = results.reduce((acc, c) => {
  acc[c.kind] = (acc[c.kind] ?? 0) + 1;
  return acc;
}, {});

console.log(`wrote ${OUT}`);
console.log(`cases: ${results.length}  (${JSON.stringify(counts)})`);
console.log(`bundle sha256: ${bundleSha256}`);

// Full table when VERBOSE=1, so intent can be eyeballed against behaviour.
if (process.env.VERBOSE === "1") {
  console.log("\nfull table:");
  CASES.forEach(([text, query, note], idx) => {
    const c = results[idx];
    const sc = c.kind === "scored" ? `${c.score}/${c.skipped}` : "-";
    console.log(
      `${String(idx + 1).padStart(3)} ${JSON.stringify(c.text).padEnd(24)} ` +
      `${JSON.stringify(c.query).padEnd(20)} ${c.kind.padEnd(10)} ${sc.padEnd(20)} ` +
      `${[c.low, c.medium, c.high].map((b) => (b ? "T" : "F")).join("")}  ${note}`
    );
  });
}

const interesting = [
  ["visual studio code", "vsc"],
  ["control panel display", "cpd"],
  ["abcdefghij", "aj"],
  ["abcdefghij", "ace"],
  ["abcdef", "cd"],
  ["ab\u00a0cd", "cd"],
  ["x-y", "x y"],
  ["app", "a-p-p"],
  ["settings", ""],
  ["", ""],
].map(([text, query]) => {
  const c = results.find((r) => r.text === text && r.query === query);
  return c;
}).filter(Boolean);

console.log("\nmost interesting cases:");
for (const c of interesting) {
  const sc = c.kind === "scored" ? `${c.score} (skipped ${c.skipped})` : "-";
  console.log(
    `  ${JSON.stringify(c.text).padEnd(24)} ${JSON.stringify(c.query).padEnd(12)} ` +
    `${c.kind.padEnd(10)} score=${sc.padEnd(20)} low/med/high=${[c.low, c.medium, c.high].map((b) => (b ? "T" : "F")).join("/")}`
  );
}
