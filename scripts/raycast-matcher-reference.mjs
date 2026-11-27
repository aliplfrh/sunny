// raycast-matcher-reference.mjs
//
// Faithful, dependency-free transcription of the fuzzy matcher that ships in
// Raycast 2.14.0 (frontend/main-window-BA9jIdSt.js). Every block below keeps
// the original minified identifier in a comment so it can be diffed against
// the bundle byte-for-byte.
//
//   Vf   -> WEIGHTS        weight enum
//   Hf   -> LIMITER        Uint8Array(128) limiter byte table
//   sae  -> BRIDGE_MIN     the `2` in `u > sae`
//   oae  -> DpTable        two-row rolling DP storage
//   cae  -> EMPTY_BOUNDS   shared scratch returned by lae() when no boundary
//   lae  -> wordBoundaries camelCase detection
//   Wf   -> normalize()
//   mae  -> weightAt()     weight of a match at a given text index
//   Kf   -> matchScore()   the scorer
//   Gf   -> admit()        tier gate
//   qf   -> rank()         numeric rank
//
// NOTHING here is "improved". Odd edge cases (empty query yielding a NEG_INF
// score, `first` weight applying only at text index 0, the `c < 2` early
// return, the shared scratch array) are reproduced deliberately.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** -(2**53-1): the original spells this out inline everywhere. */
const NEG_INF = -(2 ** 53 - 1);

/** `var Vf=function(e){return e[e.first=4]=...}({})` */
const WEIGHTS = {
  first: 4,
  wordBeginning: 3,
  limiter: 1,
  default: 2,
};

/**
 * `Hf=new Uint8Array(128);for(let e of[`\t`,`\n`,` `,`-`,`.`,`/`,`(`,`)`,`[`,`]`])Hf[e.charCodeAt(0)]=1;`
 *
 * Verified against the bundle bytes at offset 185556. NOTE: the table is
 * exactly ten characters. It does NOT contain NBSP (U+00A0) — a non-breaking
 * space is *not* a limiter in Raycast 2.14.0, it is an ordinary character.
 */
const LIMITER = new Uint8Array(128);
for (const ch of ["\t", "\n", " ", "-", ".", "/", "(", ")", "[", "]"]) {
  LIMITER[ch.charCodeAt(0)] = 1;
}

/**
 * `var sae=2` — only run the cheap greedy subsequence pre-pass when the query
 * has MORE than this many meaningful (non-limiter) characters.
 */
const SUBSEQUENCE_CHECK_MIN = 2;

// ---------------------------------------------------------------------------
// oae -> DpTable
// ---------------------------------------------------------------------------

/**
 * `oae=class{width;rows;storage;...}`
 *
 * Two rolling rows inside one flat array. `rowIndex` is `(rows % 2) * width`,
 * so `rows` doubles as "index of the row currently being written". `ignoreRow`
 * decrements `rows`, which makes the next `startRow` reuse the same slot and
 * makes `getPreviousRowColumn` look at the last *kept* row.
 */
class DpTable {
  constructor() {
    this.width = 0;
    this.rows = -1;
    this.storage = Array(128);
  }

  reset(width) {
    this.width = width;
    this.rows = -1;
    const needed = 2 * width;
    if (this.storage.length < needed) {
      this.storage = Array(Math.max(needed, this.storage.length * 2));
    }
    for (let i = 0; i < needed; i++) {
      this.storage[i] = NEG_INF;
    }
  }

  startRow() {
    this.rows += 1;
  }

  ignoreRow() {
    --this.rows;
  }

  setColumn(column, value) {
    this.storage[this.rowIndex + column] = value;
  }

  getPreviousRowColumn(column) {
    // Deliberately `(this.rows - 1) % 2 * this.width`. When `rows === 0` this
    // is `-width`, i.e. a negative index -> `undefined`, which every caller
    // treats as "no predecessor". Reproduced as-is.
    const base = ((this.rows - 1) % 2) * this.width;
    return this.storage[base + column];
  }

  maxScore() {
    const start = this.rowIndex;
    let best = NEG_INF;
    const end = start + this.width;
    for (let i = start; i < end; i++) {
      const v = this.storage[i];
      if (v > best) best = v;
    }
    return best;
  }

  get rowIndex() {
    return (this.rows % 2) * this.width;
  }
}

/** `Uf=new oae` — a single module-level instance, reset on every call. */
const Uf = new DpTable();

// ---------------------------------------------------------------------------
// lae -> wordBoundaries, Wf -> normalize
// ---------------------------------------------------------------------------

/**
 * `cae=new Uint8Array` — a module-level scratch buffer. `lae` returns this
 * (length 0) whenever the text has no camelCase boundary, and only allocates a
 * real array on the first boundary it finds. Indexing the empty array yields
 * `undefined`, which is falsy, so behaviour matches a zero-filled array.
 */
const EMPTY_BOUNDS = new Uint8Array();

/**
 * `function lae(e)` — camelCase boundary detection.
 *
 * A boundary at index n means: text[n-1] is [a-z0-9] AND text[n] is [A-Z].
 * Note it runs on the *original* (case-preserving) string, not the lowercased
 * one. Also note it never marks index 0.
 */
function wordBoundaries(text) {
  let bounds = EMPTY_BOUNDS;
  for (let n = 1; n < text.length; n++) {
    const prev = text.charCodeAt(n - 1);
    const cur = text.charCodeAt(n);
    if (
      ((prev >= 97 && prev <= 122) || (prev >= 48 && prev <= 57)) &&
      cur >= 65 &&
      cur <= 90
    ) {
      if (bounds === EMPTY_BOUNDS) {
        bounds = new Uint8Array(text.length);
      }
      bounds[n] = 1;
    }
  }
  return bounds;
}

/**
 * `function Wf(e){return{normalizedText:e.toLowerCase(),wordBoundaries:lae(e)}}`
 *
 * `wordBoundaries` is computed on the original casing, `normalizedText` is
 * `toLowerCase()`. For exotic characters whose lowercase expands (e.g. U+0130
 * -> "i̇") the boundary array can be shorter than `normalizedText`; that is the
 * original behaviour.
 */
export function normalize(text) {
  return {
    normalizedText: text.toLowerCase(),
    wordBoundaries: wordBoundaries(text),
  };
}

// ---------------------------------------------------------------------------
// mae -> weightAt
// ---------------------------------------------------------------------------

/**
 * `function mae(e,t,n)` — weight for a match at text index `t`.
 *
 * wordBeginning (3) when the position starts a word — either a camelCase
 * boundary, or the previous character is a limiter — AND the current character
 * is not itself a limiter. Otherwise default (2).
 */
function weightAt(normalizedText, index, bounds) {
  if (index <= 0) {
    throw Error(`Index must be greater than zero`);
  }
  const startsWord =
    bounds[index] || LIMITER[normalizedText.charCodeAt(index - 1)];
  const isLimiter = LIMITER[normalizedText.charCodeAt(index)];
  return startsWord && !isLimiter ? WEIGHTS.wordBeginning : WEIGHTS.default;
}

// ---------------------------------------------------------------------------
// Kf -> matchScore
// ---------------------------------------------------------------------------

/**
 * `function Kf(e,t,n)` — the scorer.
 *
 * @param {{normalizedText: string, wordBoundaries: Uint8Array}} target
 * @param {string} query — already normalized (lowercased) by the caller
 * @param {{literalQueryDots?: boolean}} [options]
 * @returns {"exact" | "unmatched" | {score: number, skipped: number}}
 */
export function matchScore(target, query, options) {
  const { normalizedText: r, wordBoundaries: i } = target;
  const a = query;
  const literalDots = options?.literalQueryDots === true;

  if (r === a) return "exact";

  const s = r.length;
  const c = a.length;

  // Count the limiters in the query; `u` is the number of "meaningful" chars.
  let limiterCount = 0;
  for (let e = 0; e < c; e++) {
    const t = a.charCodeAt(e);
    if (LIMITER[t] && (!literalDots || t !== 46)) limiterCount++;
  }
  const u = c - limiterCount;

  if (u > s) return "unmatched";

  // `if(u>sae)` — a cheap greedy subsequence pre-pass. This only *rejects*; it
  // never accepts. It exists purely to skip the DP for hopeless queries.
  if (u > SUBSEQUENCE_CHECK_MIN) {
    let pos = 0;
    for (let e = 0; e < c; e++) {
      const t = a.charCodeAt(e);
      if (LIMITER[t] && (!literalDots || t !== 46)) {
        // A query limiter consumes a text limiter if one is sitting right here.
        if (pos < s && LIMITER[r.charCodeAt(pos)]) pos++;
        continue;
      }
      let found = false;
      while (pos < s) {
        if (r.charCodeAt(pos) === t) {
          pos++;
          found = true;
          break;
        }
        pos++;
      }
      if (!found) return "unmatched";
    }
  }

  Uf.reset(s);

  let lastMatchColumn = -1; // `d` — first (leftmost) match column of the previous row
  let skipped = 0; // `f`
  let matchedRows = 0; // `p`

  for (let e = 0; e < c; e++) {
    Uf.startRow();
    const t = a.charCodeAt(e);
    const isQueryLimiter = LIMITER[t] && (!literalDots || t !== 46);
    const budget = u - 1 - matchedRows; // `l`
    let firstHit = -1; // `m`
    const columnStart = lastMatchColumn + 1; // `h`
    const columnEnd = s - budget; // `g`
    // `_` — best score carried forward from a previous row. NOTE: this is a
    // RUNNING MAX over `getPreviousRowColumn(col-2)` for every col scanned so
    // far in this row, not a single-column lookback. Combined with the `-1`
    // penalty it means a gap of ANY length can be bridged (costing 1 per
    // bridged query character, not per skipped text character). This is the
    // real behaviour; see the report for the `vsc` / `cpd` consequences.
    let bridge = NEG_INF;

    for (let col = columnStart; col < columnEnd; col++) {
      // `if(d>=0&&e-2>=d){let t=Uf.getPreviousRowColumn(e-2);t>_&&(_=t)}`
      // Running max (see the note on `bridge` above).
      if (lastMatchColumn >= 0 && col - 2 >= lastMatchColumn) {
        const v = Uf.getPreviousRowColumn(col - 2);
        if (v > bridge) bridge = v;
      }

      const textChar = r.charCodeAt(col);
      const equal = textChar === t;
      // A query limiter may consume a *different* text limiter, at weight 1.
      const limiterSwap = !equal && isQueryLimiter && LIMITER[textChar];

      if (equal || limiterSwap) {
        if (lastMatchColumn < 0) {
          // First query character: `first` weight applies only at column 0.
          const w = limiterSwap
            ? WEIGHTS.limiter
            : col === 0
              ? WEIGHTS.first
              : weightAt(r, col, i);
          Uf.setColumn(col, w);
        } else {
          const w = limiterSwap ? WEIGHTS.limiter : weightAt(r, col, i);
          let best = NEG_INF;
          const diag = Uf.getPreviousRowColumn(col - 1);
          if (diag > NEG_INF) best = diag + w;
          if (bridge > NEG_INF) best = Math.max(best, bridge + w - 1);
          Uf.setColumn(col, best);
        }
        if (firstHit < 0) firstHit = col;
      }
    }

    if (firstHit >= 0) {
      lastMatchColumn = firstHit;
      matchedRows += 1;
    } else if (isQueryLimiter) {
      // The query limiter found no text limiter: skip it, and drop this DP row
      // so it does not become the "previous row" of the next one.
      skipped += 1;
      if (e < c - 1) Uf.ignoreRow();
    } else {
      return "unmatched";
    }
  }

  return { score: Uf.maxScore(), skipped };
}

// ---------------------------------------------------------------------------
// Gf -> admit
// ---------------------------------------------------------------------------

/**
 * `function Gf(e,t,n)` — tier gate.
 *
 * @param {"exact" | "unmatched" | {score:number, skipped:number}} match
 * @param {number} queryLength
 * @param {"low" | "medium" | "high"} tier
 * @returns {boolean | undefined} `undefined` for an unknown tier (as original)
 */
export function admit(match, queryLength, tier) {
  if (match === "unmatched") return false;
  if (match === "exact") return true;
  switch (tier) {
    case "low":
      return true;
    case "medium":
      return match.score >= 1.5 * (queryLength - match.skipped - 2) + 4;
    case "high":
      return match.score > 2 * (queryLength - match.skipped);
  }
}

// ---------------------------------------------------------------------------
// qf -> rank
// ---------------------------------------------------------------------------

/**
 * `function qf(e){return e===`unmatched`?-(2**53-1):e===`exact`?2**53-1:e.score}`
 */
export function rank(match) {
  if (match === "unmatched") return NEG_INF;
  if (match === "exact") return 2 ** 53 - 1;
  return match.score;
}

// ---------------------------------------------------------------------------
// Extras (exported for inspection / test-vector generation)
// ---------------------------------------------------------------------------

export const LIMITER_CHARS = ["\t", "\n", " ", "-", ".", "/", "(", ")", "[", "]"];
export const NEG_INF_SCORE = NEG_INF;
export { weightAt };
