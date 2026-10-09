// Guard: every free identifier in a moved page module resolves, or is a
// browser/JS global on the allowlist below. A bare page let (embedded) fails.
// RAIL_SCOPE_ROOT points the scan at another tree (the 5d75e85 copies).

import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = require("typescript") as typeof import("typescript");

const ALLOW = new Set([
  "CSS",
  "DragEvent",
  "Element",
  "Event",
  "EventTarget",
  "HTMLDivElement",
  "HTMLElement",
  "HTMLInputElement",
  "HTMLTextAreaElement",
  "Image",
  "KeyboardEvent",
  "MouseEvent",
  "Node",
  "PointerEvent",
  "clearInterval",
  "clearTimeout",
  "document",
  "getComputedStyle",
  "navigator",
  "queueMicrotask",
  "requestAnimationFrame",
  "setInterval",
  "setTimeout",
  "window",
]);

const DIRS = ["src/lib/rail", "src/lib/board", "src/lib/companion"];

function pageModules(root: string): string[] {
  const out: string[] = [];
  for (const rel of DIRS) {
    const dir = path.join(root, rel);
    if (!existsSync(dir)) continue;
    for (const name of readdirSync(dir).sort()) {
      if (/^page.+\.svelte\.ts$/.test(name)) out.push(path.join(dir, name));
    }
  }
  return out;
}

test("moved page modules bind every free identifier", () => {
  const root = process.env.RAIL_SCOPE_ROOT
    ? path.resolve(process.env.RAIL_SCOPE_ROOT)
    : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const files = pageModules(root);
  assert.ok(files.length > 0, `no page modules under ${root}`);
  const wanted = new Set(files.map((f) => path.resolve(f)));

  const opts: import("typescript").CompilerOptions = {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    lib: ["lib.es2022.d.ts"],
    types: [],
    noEmit: true,
    skipLibCheck: true,
    allowImportingTsExtensions: true,
    strict: false,
  };
  const host = ts.createCompilerHost(opts);
  const read = host.getSourceFile.bind(host);
  host.getSourceFile = (fileName, languageVersion, onError, shouldCreateNewSourceFile) => {
    const sf = read(fileName, languageVersion, onError, shouldCreateNewSourceFile);
    if (!sf || !wanted.has(path.resolve(fileName))) return sf;
    const text = readFileSync(fileName, "utf8").replace(/^\/\/ @ts-nocheck\r?\n/, "");
    if (text === sf.text) return sf;
    return ts.createSourceFile(fileName, text, languageVersion, true, ts.ScriptKind.TS);
  };

  const prog = ts.createProgram(files, opts, host);
  const checker = prog.getTypeChecker();
  const hits: string[] = [];
  for (const file of files) {
    const sf = prog.getSourceFile(file);
    if (!sf) {
      hits.push(`${path.relative(root, file)}: missing source`);
      continue;
    }
    const lineShift = readFileSync(file, "utf8").startsWith("// @ts-nocheck") ? 1 : 0;
    for (const d of prog.getSemanticDiagnostics(sf)) {
      if (d.code !== 2304 && d.code !== 2552) continue;
      const msg = ts.flattenDiagnosticMessageText(d.messageText, " ");
      const name = msg.match(/'([^']+)'/)?.[1] ?? "?";
      if (ALLOW.has(name)) continue;
      const line = d.start === undefined ? 0 : sf.getLineAndCharacterOfPosition(d.start).line + 1 + lineShift;
      hits.push(`${path.relative(root, file)}:${line} ${name}`);
    }
    // Object-literal shorthand in an `any` argument is not diagnostic 2304.
    // The value symbol is still unresolved.
    const walk = (node: import("typescript").Node) => {
      if (ts.isShorthandPropertyAssignment(node)) {
        const name = node.name.text;
        if (!ALLOW.has(name) && !checker.getShorthandAssignmentValueSymbol(node)) {
          const line = sf.getLineAndCharacterOfPosition(node.name.getStart()).line + 1 + lineShift;
          hits.push(`${path.relative(root, file)}:${line} ${name}`);
        }
      }
      ts.forEachChild(node, walk);
    };
    walk(sf);
  }
  assert.deepEqual(hits, [], hits.join("\n"));
});
