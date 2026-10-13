import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function src(rel: string): string {
  return readFileSync(new URL(`../${rel}`, import.meta.url), "utf8");
}

test("the rail adopts disk planets on state:changed unless a local edit waits", () => {
  const file = src("src/lib/RailPage.svelte");
  const start = file.indexOf('listen("state:changed"');
  assert.notEqual(start, -1);
  const body = file.slice(start, file.indexOf("\n    });", start));
  const drag = body.indexOf("if (windowDragHot())");
  const before = body.indexOf("const projectsBefore = projects;");
  const load = body.indexOf('await invoke<PersistedState>("load_state")', before);
  const adopt = body.indexOf(
    "if (!persistDeferred && projects === projectsBefore) projects = s.projects ?? projects;",
  );
  assert.ok(drag !== -1 && before > drag, "not on the drag-hot path");
  assert.ok(load > before && adopt > load, "captured before the await, adopted after");
});
