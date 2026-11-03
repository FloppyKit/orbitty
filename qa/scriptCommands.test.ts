// 27p-c part b: Raycast-format script commands. Header grammar (the pure
// TS mirror of the Rust scan), launcher rows/actions, and source scans
// for the discovery/run guards. Nothing here starts the app.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import {
  HEADER_BYTES,
  MODES,
  emojiOnly,
  headerMap,
  isExecutableMode,
  parseArgument,
  parseScriptHeader,
  skipName,
  type ScriptCommand,
} from "../src/lib/companion/scriptCmds.ts";
import {
  catalogue,
  launcherItems,
  rankItems,
  type LaunchCtx,
} from "../src/lib/companion/launcher.ts";

const ROOT = new URL("../", import.meta.url);

function src(rel: string): string {
  return readFileSync(new URL(rel, ROOT), "utf8");
}

function cmd(over: Partial<ScriptCommand> = {}): ScriptCommand {
  return {
    id: "/tmp/scripts/hello.sh",
    title: "Hello 27p-c",
    subtitle: "Test",
    icon: "\u{1f44b}",
    mode: "compact",
    needsConfirmation: false,
    ...over,
  };
}

function ctx(over: Partial<LaunchCtx> = {}): LaunchCtx {
  return {
    query: "",
    armed: null,
    planets: [{ id: "p-kansas", name: "Kansas" }],
    hosted: "off",
    hostBusy: false,
    agent: "spark",
    agents: [{ id: "spark", label: "Spark" }],
    mode: "spark",
    speak: false,
    sprite: true,
    captions: true,
    head: false,
    prompts: false,
    nextCount: 0,
    smallestTodo: null,
    planetsHidden: false,
    ...over,
  };
}

const HELLO = [
  "#!/bin/bash",
  "# @raycast.schemaVersion 1",
  "# @raycast.title Hello 27p-c",
  "# @raycast.mode compact",
  "# @raycast.icon \u{1f44b}",
  "# @raycast.packageName Test",
  'echo "hello from 27p-c"',
].join("\n");

// ---- header grammar -------------------------------------------------------

test("both # and // headers parse with first value winning", () => {
  const hash = headerMap(HELLO);
  assert.equal(hash.get("schemaVersion"), "1");
  assert.equal(hash.get("title"), "Hello 27p-c");
  const slash = headerMap(
    "// @raycast.schemaVersion 1\n// @raycast.title Two\n// @raycast.mode silent\n",
  );
  assert.equal(slash.get("title"), "Two");
  assert.equal(headerMap("# @raycast.title A\n# @raycast.title B\n").get("title"), "A");
  // A shebang is a comment but has no @raycast key.
  assert.equal(hash.has("!"), false);
});

test("a valid hash script becomes a row", () => {
  const out = parseScriptHeader(HELLO, { name: "hello.sh", folder: "scripts" });
  assert.ok("cmd" in out);
  assert.equal(out.cmd.title, "Hello 27p-c");
  assert.equal(out.cmd.mode, "compact");
  assert.equal(out.cmd.icon, "\u{1f44b}");
  assert.equal(out.cmd.subtitle, "Test");
  assert.equal(out.cmd.needsConfirmation, false);
  assert.equal(out.cmd.argument1, undefined);
});

test("every required key is required", () => {
  const base = { name: "x.sh", folder: "scripts" };
  const cases = [
    ["# @raycast.title X\n# @raycast.mode compact\n", "schemaVersion"],
    ["# @raycast.schemaVersion 1\n# @raycast.mode compact\n", "title"],
    ["# @raycast.schemaVersion 1\n# @raycast.title X\n", "mode"],
    ["# @raycast.schemaVersion 2\n# @raycast.title X\n# @raycast.mode compact\n", "schemaVersion"],
    ["# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode loud\n", "mode"],
    ["# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.platform windows\n", "platform"],
  ] as const;
  for (const [text, reason] of cases) {
    const out = parseScriptHeader(text, base);
    assert.ok("skip" in out, text);
    assert.equal(out.skip, reason);
  }
  assert.deepEqual([...MODES], ["silent", "compact", "fullOutput", "inline"]);
});

test("template, hidden and non-executable files are skipped", () => {
  assert.equal(skipName(".hidden.sh"), "hidden");
  assert.equal(skipName("hello.template.sh"), "template");
  assert.equal(skipName("hello.sh"), null);
  const out = parseScriptHeader(HELLO, { name: "a.template.sh", folder: "scripts" });
  assert.ok("skip" in out);
  assert.equal(isExecutableMode(0o755), true);
  assert.equal(isExecutableMode(0o644), false);
});

test("binary junk and a header past 4 KiB are skipped", () => {
  assert.ok("skip" in parseScriptHeader("\u0000\u0001nope", { name: "j.sh", folder: "s" }));
  let text = "";
  while (text.length <= HEADER_BYTES) text += "# filler filler filler filler\n";
  text += HELLO;
  const head = text.slice(0, HEADER_BYTES);
  assert.equal(headerMap(head).has("schemaVersion"), false);
  assert.ok("skip" in parseScriptHeader(head, { name: "late.sh", folder: "s" }));
});

test("bad argument1 JSON keeps the row but drops the argument", () => {
  const bad = `${HELLO}\n# @raycast.argument1 {nope\n`;
  const out = parseScriptHeader(bad, { name: "hello.sh", folder: "scripts" });
  assert.ok("cmd" in out);
  assert.equal(out.cmd.argument1, undefined);
  assert.equal(parseArgument("{nope"), null);
  assert.equal(parseArgument('["text"]'), null);
  assert.equal(parseArgument('{"type":"number"}'), null);
  const arg = parseArgument('{ "type": "text", "placeholder": "Query", "optional": true }');
  assert.deepEqual(arg, { kind: "text", placeholder: "Query", optional: true });
});

test("icons are emoji only; paths and URLs are ignored", () => {
  assert.equal(emojiOnly("\u{1f44b}"), "\u{1f44b}");
  assert.equal(emojiOnly("/tmp/icon.png"), undefined);
  assert.equal(emojiOnly("https://example.test/i.png"), undefined);
  assert.equal(emojiOnly("A"), undefined);
  const path = parseScriptHeader(
    "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.icon /tmp/i.png\n",
    { name: "x.sh", folder: "scripts" },
  );
  assert.ok("cmd" in path);
  assert.equal(path.cmd.icon, undefined);
});

test("subtitle falls back packageName -> description -> folder", () => {
  const text =
    "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n# @raycast.description Says hi\n";
  const out = parseScriptHeader(text, { name: "x.sh", folder: "scripts" });
  assert.ok("cmd" in out);
  assert.equal(out.cmd.subtitle, "Says hi");
  const bare = parseScriptHeader(
    "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode compact\n",
    { name: "x.sh", folder: "scripts" },
  );
  assert.ok("cmd" in bare);
  assert.equal(bare.cmd.subtitle, "scripts");
});

// ---- launcher rows --------------------------------------------------------

test("catalogue merges script rows with title, subtitle and emoji glyph", () => {
  const rows = catalogue(ctx({ scripts: [cmd()] }));
  const row = rows.find((r) => r.id === "script:/tmp/scripts/hello.sh");
  assert.ok(row);
  assert.equal(row.title, "Hello 27p-c");
  assert.equal(row.subtitle, "Test");
  assert.equal(row.glyph, "\u{1f44b}");
  assert.equal(row.kind, "command");
  assert.deepEqual(row.run, {
    t: "script",
    id: "/tmp/scripts/hello.sh",
    title: "Hello 27p-c",
  });
});

test("no folder or an empty list adds no script rows", () => {
  assert.equal(catalogue(ctx()).some((r) => r.id.startsWith("script:")), false);
  assert.equal(catalogue(ctx({ scripts: [] })).some((r) => r.id.startsWith("script:")), false);
});

test("a script row ranks like any title and never hides the Ask row", () => {
  const rows = catalogue(ctx({ scripts: [cmd()] }));
  const ranked = rankItems("hello", rows);
  assert.equal(ranked[0]?.title, "Hello 27p-c");
  const items = launcherItems({ ...ctx({ scripts: [cmd()] }), query: "hello" });
  assert.equal(items[0]?.kind, "ask");
  assert.ok(items.some((r) => r.run.t === "script"));
});

// ---- Rust side source scans ----------------------------------------------

test("Rust discovery reads 4 KiB headers and never spawns", () => {
  const rust = src("src-tauri/src/companion/script_cmds.rs");
  assert.match(rust, /pub const HEADER_BYTES: u64 = 4096;/);
  assert.match(rust, /pub const MAX_FILES: usize = 200;/);
  assert.match(rust, /take\(HEADER_BYTES\)/);
  const scan = rust.slice(rust.indexOf("pub fn list_scripts_in"));
  const scanBody = scan.slice(0, scan.indexOf("fn expand_home"));
  assert.equal(scanBody.includes("Command::"), false, "discovery must not spawn");
  assert.match(rust, /#\[cfg\(test\)\]\nmod tests \{/);
});

test("Rust run is a direct spawn, one argv entry, 30 s kill, 64 KiB cap", () => {
  const rust = src("src-tauri/src/companion/script_cmds.rs");
  assert.match(rust, /RUN_TIMEOUT: Duration = Duration::from_secs\(30\)/);
  assert.match(rust, /OUTPUT_CAP: usize = 64 \* 1024/);
  assert.match(rust, /Command::new\(&path\)/);
  assert.match(rust, /cmd\.arg\(arg\)/);
  assert.match(rust, /\.current_dir\(&cwd\)/);
  assert.equal(/sh\s+-c|bash\s+-c|Command::new\("\/bin\/sh"\)/.test(rust), false);
  assert.match(rust, /child\.kill\(\)/);
  assert.match(rust, /parent\(\) != Some\(root\.as_path\(\)\)/);
});

test("both commands are async spawn_blocking and registered", () => {
  const rust = src("src-tauri/src/companion/script_cmds.rs");
  assert.match(rust, /pub async fn list_script_commands\(\)/);
  assert.match(rust, /pub async fn run_script_command\(/);
  assert.equal((rust.match(/spawn_blocking/g) ?? []).length >= 2, true);
  assert.match(src("src-tauri/src/companion/mod.rs"), /pub\(crate\) mod script_cmds;/);
  const lib = src("src-tauri/src/lib.rs");
  assert.match(lib, /script_cmds::list_script_commands,/);
  assert.match(lib, /script_cmds::run_script_command,/);
});

test("the dock lists on focus and runs only the picked row", () => {
  const dock = src("src/lib/CompanionDock.svelte");
  assert.match(dock, /invoke<ScriptCommand\[\]>\("list_script_commands"\)/);
  assert.match(dock, /invoke\("run_script_command", \{ id: script\.id, args: arg \}\)/);
  assert.match(dock, /onfocusin=\{\(\) => void loadScripts\(\)\}/);
  assert.match(dock, /Run \$\{pending\.title\}\?/);
});

test("the script header parser is the same table as the Rust one", () => {
  const ts = src("src/lib/companion/scriptCmds.ts");
  const rust = src("src-tauri/src/companion/script_cmds.rs");
  for (const key of [
    "schemaVersion",
    "title",
    "mode",
    "icon",
    "packageName",
    "description",
    "argument1",
    "needsConfirmation",
    "currentDirectoryPath",
    "platform",
  ]) {
    assert.ok(ts.includes(key), `TS missing ${key}`);
    assert.ok(rust.includes(key), `Rust missing ${key}`);
  }
  assert.ok(ts.includes(".template."));
  assert.ok(rust.includes(".template."));
});
