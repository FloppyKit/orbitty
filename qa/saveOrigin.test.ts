// Background window saves must tag save_state; a user mutate must not.
// Runs the real functions against an injected invoke. Not a source regex.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { test } from "node:test";
import { mutate } from "../src/lib/board/pageMutate.svelte.ts";
import { loadState } from "../src/lib/board/pageChips.svelte.ts";

const root = new URL("..", import.meta.url);

function src(rel: string): string {
  return readFileSync(new URL(rel, root), "utf8");
}

function braceBody(text: string, from: number): string {
  const open = text.indexOf("{", from);
  assert.ok(open >= 0, "brace");
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") {
      depth--;
      if (depth === 0) return text.slice(open, i + 1);
    }
  }
  throw new Error("unbalanced");
}

function decl(text: string, head: string): string {
  const at = text.indexOf(head);
  assert.ok(at >= 0, head);
  assert.equal(text.indexOf(head, at + 1), -1, `${head} once`);
  const body = braceBody(text, at);
  const open = text.indexOf(body, at);
  assert.ok(open >= at);
  return stripTypeScriptTypes(text.slice(at, open + body.length));
}

type SaveArgs = { state?: unknown; origin?: string };

function scopeFor(saves: SaveArgs[], disk: Record<string, unknown>) {
  return {
    invoke: async (cmd: string, args?: SaveArgs) => {
      if (cmd === "load_state") return structuredClone(disk);
      if (cmd === "save_state") {
        saves.push(args ?? {});
        return;
      }
      if (cmd === "list_windows") return [];
      if (cmd === "yard_status") return false;
      return undefined;
    },
    loadState: async () => {},
    errText: () => "",
    showLinkToast: () => {},
  };
}

test("mutate without origin sends no origin key", async () => {
  const saves: SaveArgs[] = [];
  const disk = { kanbanCards: [] as { id: string }[] };
  await mutate(scopeFor(saves, disk), (s) => {
    s.kanbanCards = [{ id: "user-write" }];
  });
  assert.equal(saves.length, 1);
  assert.equal("origin" in saves[0], false);
  assert.deepEqual(Object.keys(saves[0]), ["state"]);
});

test("mutate forwards a given origin and does not rewrite it", async () => {
  const tagged: SaveArgs[] = [];
  await mutate(scopeFor(tagged, { kanbanCards: [] }), (s) => {
    s.kanbanCards = [];
  }, "background");
  assert.equal(tagged[0].origin, "background");

  const other: SaveArgs[] = [];
  await mutate(scopeFor(other, { kanbanCards: [] }), (s) => {
    s.kanbanCards = [];
  }, "rail");
  assert.equal(other[0].origin, "rail");
});

test("Kanban mutate wrapper forwards origin only when given", async () => {
  const page = src("src/lib/KanbanPage.svelte");
  const script = page.slice(page.indexOf("<script"), page.indexOf("</script>"));
  const code = decl(script, "async function mutate(fn:");
  const saves: SaveArgs[] = [];
  const disk = { kanbanCards: [] as unknown[] };
  const S = {
    mutateBody: mutate,
    errText: () => "",
    invoke: scopeFor(saves, disk).invoke,
    loadState: async () => {},
    showLinkToast: () => {},
  };
  const fns = new Function(
    "S",
    `with (S) {\n${code}\nreturn { mutate };\n}`,
  )(S) as { mutate: (fn: (s: { kanbanCards: unknown[] }) => void, origin?: string) => Promise<void> };

  await fns.mutate((s) => {
    s.kanbanCards = [{ id: "click" }];
  });
  assert.equal("origin" in saves[0], false);

  await fns.mutate((s) => {
    s.kanbanCards = [{ id: "auto" }];
  }, "background");
  assert.equal(saves[1].origin, "background");
});

test("Needed re-sort passes origin background", async () => {
  const page = src("src/lib/KanbanPage.svelte");
  const at = page.indexOf("let neededDoingSortReady = false;");
  assert.ok(at >= 0);
  const effectAt = page.indexOf("$effect(() =>", at);
  assert.ok(effectAt > at);
  const body = braceBody(page, effectAt);
  const calls: unknown[][] = [];
  const cards = [
    { id: "plain", status: "doing" },
    { id: "need", status: "doing", neededId: "n1" },
  ];
  const S = {
    grabbedId: null as string | null,
    kanbanCards: cards,
    isDoingRow: (c: { status: string }) => c.status === "doing",
    neededForTodo: (c: { neededId?: string }) => Boolean(c.neededId),
    seenNeededDoing: new Set<string>(),
    neededDoingSortReady: false,
    mutate: (...args: unknown[]) => {
      calls.push(args);
    },
  };
  new Function("S", `with (S) ${body}`)(S);
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1], "background");
  assert.equal(calls[0].length, 2);
});

test("tracker migration passes origin background through mutate", async () => {
  const saves: SaveArgs[] = [];
  const disk = {
    themeId: "systemDark",
    windowOpacity: 1,
    projects: [{ id: "p1", trackers: [] as unknown[] }],
    kanbanCards: [{ id: "c1", projectId: "p1", trackers: [{ name: "old" }] }],
    companion: {},
  };
  const invoke = scopeFor(saves, disk).invoke;
  const scope = {
    invoke,
    themeId: "",
    KNOWN_THEMES: new Set(["systemDark"]),
    osThemeId: () => "systemDark",
    windowOpacity: 1,
    clampWindowOpacity: (v: number) => v,
    projects: [] as unknown[],
    applyPendingFocus: () => {},
    tags: [] as unknown[],
    kanbanCards: [] as unknown[],
    pointsLedger: [] as unknown[],
    titleOverrides: [] as unknown[],
    collapsedSectionIds: [] as string[],
    agentShortcuts: [] as unknown[],
    agentRuns: [] as unknown[],
    neededSplit: 0.28,
    clampSharedSplit: (v: number) => v,
    previewWidth: 280,
    openSubtaskIds: [] as string[],
    hackProjectId: null as string | null,
    migratingCardTrackers: false,
    mutate: (
      fn: (s: typeof disk) => void,
      origin?: string,
    ) => mutate({ invoke, loadState: async () => {}, errText: () => "", showLinkToast: () => {} }, fn, origin),
  };
  await loadState(scope);
  assert.equal(saves.length, 1);
  assert.equal(saves[0].origin, "background");
  assert.equal(scope.migratingCardTrackers, false);
});

function yardFns() {
  const page = src("src/lib/YardWarnPage.svelte");
  const script = page.slice(page.indexOf("<script"), page.indexOf("</script>"));
  const code = [
    decl(script, "async function countUnclaimed("),
    decl(script, "function persistSources("),
  ].join("\n");
  return (S: Record<string, unknown>) =>
    new Function(
      "S",
      `with (S) {\n${code}\nreturn { countUnclaimed, persistSources };\n}`,
    )(S) as {
      countUnclaimed: () => Promise<void>;
      persistSources: () => void;
    };
}

const loadYard = yardFns();

test("Yard heal save tags background; checkbox persist stays a user save", async () => {
  const healSaves: SaveArgs[] = [];
  const heal = loadYard({
    THEME_IDS: new Set(["systemDark"]),
    themeId: "systemDark",
    windowOpacity: 1,
    yardWindows: true,
    yardTabs: false,
    unclaimed: null,
    listErr: false,
    yardOn: false,
    dropping: false,
    invoke: scopeFor(healSaves, { themeId: "systemDark", windowOpacity: 1 }).invoke,
  });
  await heal.countUnclaimed();
  assert.equal(healSaves.length, 1);
  assert.equal(healSaves[0].origin, "background");

  const userSaves: SaveArgs[] = [];
  const click = loadYard({
    yardWindows: true,
    yardTabs: false,
    invoke: scopeFor(userSaves, {
      themeId: "systemDark",
      windowOpacity: 1,
      yardWindows: true,
      yardTabs: false,
    }).invoke,
  });
  click.persistSources();
  await new Promise((r) => setImmediate(r));
  await new Promise((r) => setImmediate(r));
  assert.equal(userSaves.length, 1);
  assert.equal("origin" in userSaves[0], false);
  assert.equal(userSaves[0].state && (userSaves[0].state as { yardWindows: boolean }).yardWindows, true);
});
