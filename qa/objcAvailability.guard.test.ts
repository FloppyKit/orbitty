import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const tauriRoot = join(repoRoot, "src-tauri");

function walkNative(dir: string, acc: string[] = []): string[] {
  for (const ent of readdirSync(dir, { withFileTypes: true })) {
    if (ent.name === "target" || ent.name === "node_modules") continue;
    const next = join(dir, ent.name);
    if (ent.isDirectory()) walkNative(next, acc);
    else if (ent.isFile() && /\.(m|mm|c)$/.test(ent.name)) acc.push(next);
  }
  return acc;
}

/** Drop // and block comments. Newlines stay so later line numbers match the file. */
function stripComments(src: string): string {
  let out = "";
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    const d = src[i + 1];
    if (c === "/" && d === "/") {
      while (i < src.length && src[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && d === "*") {
      i += 2;
      while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) {
        if (src[i] === "\n") out += "\n";
        i++;
      }
      if (i < src.length) i += 2;
      continue;
    }
    if (c === "@" && d === '"') {
      out += '@"';
      i += 2;
      while (i < src.length) {
        out += src[i];
        if (src[i] === "\\") {
          i++;
          if (i < src.length) out += src[i++];
          continue;
        }
        if (src[i] === '"') {
          i++;
          break;
        }
        i++;
      }
      continue;
    }
    if (c === '"') {
      out += c;
      i++;
      while (i < src.length) {
        out += src[i];
        if (src[i] === "\\") {
          i++;
          if (i < src.length) out += src[i++];
          continue;
        }
        if (src[i] === '"') {
          i++;
          break;
        }
        i++;
      }
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function availabilityHits(stripped: string): { line: number; text: string }[] {
  const hits: { line: number; text: string }[] = [];
  const lines = stripped.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].includes("@available(") || lines[i].includes("__builtin_available(")) {
      hits.push({ line: i + 1, text: lines[i].trim() });
    }
  }
  return hits;
}

test("G1: no @available( or __builtin_available( in src-tauri native code", () => {
  const files = walkNative(tauriRoot);
  assert.ok(
    files.length >= 4,
    `expected at least 4 native sources under src-tauri, found ${files.length}: ${files.join(", ")}`,
  );
  const problems: string[] = [];
  for (const file of files) {
    const rel = relative(repoRoot, file);
    for (const hit of availabilityHits(stripComments(readFileSync(file, "utf8")))) {
      problems.push(`${rel}:${hit.line}: ${hit.text}`);
    }
  }
  assert.deepEqual(problems, [], problems.join("\n"));
});

test("G2: window_thumb_mac.m runtime OS and class check before ScreenCaptureKit, status 3 fallback", () => {
  const raw = readFileSync(join(tauriRoot, "src/windows/window_thumb_mac.m"), "utf8");
  const stripped = stripComments(raw);
  assert.equal(stripped.includes("isOperatingSystemAtLeastVersion"), true);
  assert.match(stripped, /\{\s*14\s*,\s*0\s*,\s*0\s*\}/);
  assert.equal(stripped.includes('NSClassFromString(@"SCScreenshotManager")'), true);
  assert.equal(stripped.includes("orbitty_thumb_finish(ctx, cb, 3, nil, 0, 0)"), true);

  const sig = stripped.indexOf("void orbitty_thumb_capture");
  assert.notEqual(sig, -1, "orbitty_thumb_capture");
  const body = stripped.slice(sig);
  const sckAt = body.indexOf("SCShareableContent getShareableContent");
  assert.notEqual(sckAt, -1, "SCShareableContent getShareableContent");
  const guardNeedles = [
    "orbitty_thumb_sck_ok(",
    "isOperatingSystemAtLeastVersion",
    'NSClassFromString(@"SCScreenshotManager")',
  ];
  const guardAt = Math.min(
    ...guardNeedles.map((needle) => {
      const at = body.indexOf(needle);
      return at === -1 ? Number.POSITIVE_INFINITY : at;
    }),
  );
  assert.ok(Number.isFinite(guardAt), "SCK guard missing inside orbitty_thumb_capture");
  assert.ok(guardAt < sckAt, "SCK guard must appear before SCShareableContent getShareableContent");
});

test("G3: window_thumb_mac.m availability pragmas are balanced and narrow", () => {
  const raw = readFileSync(join(tauriRoot, "src/windows/window_thumb_mac.m"), "utf8");
  const allowed = new Set(["-Wunguarded-availability-new", "-Wunguarded-availability"]);
  let depth = 0;
  let pushes = 0;
  let pops = 0;
  const lines = raw.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const ignored = line.match(/#pragma\s+clang\s+diagnostic\s+ignored\s+"(-W[^"]*)"/);
    if (ignored) {
      assert.ok(allowed.has(ignored[1]), `${i + 1}: ignored ${ignored[1]}`);
    }
    if (/#pragma\s+clang\s+diagnostic\s+push\b/.test(line)) {
      pushes++;
      depth++;
    }
    if (/#pragma\s+clang\s+diagnostic\s+pop\b/.test(line)) {
      assert.ok(depth > 0, `pop without a preceding push at line ${i + 1}`);
      pops++;
      depth--;
    }
  }
  assert.equal(pushes, pops, `push ${pushes} pop ${pops}`);
  assert.equal(depth, 0);
});
