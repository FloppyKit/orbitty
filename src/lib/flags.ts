export type FlagReply = { on: boolean; source: string };

/** Pure parse of a `flags_get` reply. One page load memoizes `flagOn`. */
export function parseFlagReply(raw: unknown): FlagReply {
  if (raw === null || typeof raw !== "object") {
    throw new Error("flags_get: reply");
  }
  const rec = raw as { on?: unknown; source?: unknown };
  if (typeof rec.on !== "boolean") {
    throw new Error("flags_get: on");
  }
  if (typeof rec.source !== "string" || rec.source.length === 0) {
    throw new Error("flags_get: source");
  }
  return { on: rec.on, source: rec.source };
}

const memo = new Map<string, Promise<boolean>>();

export async function flagOn(id: string): Promise<boolean> {
  let hit = memo.get(id);
  if (!hit) {
    hit = (async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const raw = await invoke("flags_get", { id });
      return parseFlagReply(raw).on;
    })();
    memo.set(id, hit);
  }
  return hit;
}
