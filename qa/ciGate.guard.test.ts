import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("CI runs the full gate on macOS with no secrets, signing, build or upload", () => {
  const yml = src(".github/workflows/gate.yml");
  assert.match(yml, /runs-on: macos-latest/);
  const steps = [
    "uses: actions/checkout@v4",
    "uses: actions/setup-node@v4",
    "node-version: 26",
    "cache: npm",
    "uses: dtolnay/rust-toolchain@stable",
    "uses: Swatinem/rust-cache@v2",
    "workspaces: src-tauri",
    "run: npm ci",
    "run: cargo fetch --manifest-path src-tauri/Cargo.toml",
    "run: zsh ./scripts/gate.sh\n",
  ];
  let at = -1;
  for (const step of steps) {
    const i = yml.indexOf(step);
    assert.ok(i > at, step);
    at = i;
  }
  for (const word of ["secrets.", "sign", "upload", "tauri build", "npm run build", "--quick"]) {
    assert.equal(yml.includes(word), false, word);
  }
});

test("npm test is the qa scan and the full gate runs it", () => {
  const pkg = JSON.parse(src("package.json"));
  assert.equal(pkg.scripts.test, "node --test qa/*.test.ts");
  const gate = src("scripts/gate.sh");
  const full = gate.slice(gate.indexOf("if (( ! quick )); then"));
  assert.match(full, /run_npm_check\n {2}run_qa\nfi/);
  assert.match(gate, /node --test "\$\{tests\[@\]\}"/);
});
