// Orbitty relay frames. The nsec never enters this module.
// The socket speaks NIP-42: ["AUTH", challenge], then our AUTH event,
// then ["REQ", id, { kinds: [420, 421, 422] }], then EVENT lines and EOSE.

import { KIND_DOING, KIND_DONE, KIND_TODO, type BoardProof } from "./fold.ts";

export const DEFAULT_RELAY = "ws://127.0.0.1:13120";
export const BOARD_SUB = "lifesprite";

export type RelayFrame =
  | { type: "challenge"; challenge: string }
  | { type: "auth-ok" }
  | { type: "auth-rejected"; detail: string }
  | { type: "event"; event: BoardProof }
  | { type: "eose" }
  | { type: "closed"; detail: string }
  | { type: "notice"; detail: string }
  | { type: "ignore" };

export function boardReq(): [string, string, { kinds: number[] }] {
  return ["REQ", BOARD_SUB, { kinds: [KIND_TODO, KIND_DOING, KIND_DONE] }];
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return null;
}

function asProof(value: unknown): BoardProof | null {
  const rec = asRecord(value);
  if (!rec || typeof rec.kind !== "number") return null;
  const tags = Array.isArray(rec.tags)
    ? rec.tags.filter((tag): tag is string[] => Array.isArray(tag) && tag.every((p) => typeof p === "string"))
    : undefined;
  return {
    id: typeof rec.id === "string" ? rec.id : undefined,
    pubkey: typeof rec.pubkey === "string" ? rec.pubkey : undefined,
    created_at: typeof rec.created_at === "number" ? rec.created_at : undefined,
    kind: rec.kind,
    tags,
    content: typeof rec.content === "string" ? rec.content : undefined,
    sig: typeof rec.sig === "string" ? rec.sig : undefined,
  };
}

/** Parse one relay message. Challenge AUTH is a string; a rejected OK carries the reason. */
export function readRelayFrame(frame: unknown): RelayFrame {
  if (!Array.isArray(frame) || frame.length === 0 || typeof frame[0] !== "string") {
    return { type: "ignore" };
  }
  const cmd = frame[0];
  if (cmd === "AUTH" && typeof frame[1] === "string") {
    return { type: "challenge", challenge: frame[1] };
  }
  if (cmd === "OK") {
    if (frame[2] === true) return { type: "auth-ok" };
    const detail = typeof frame[3] === "string" ? frame[3] : "rejected";
    return { type: "auth-rejected", detail };
  }
  if (cmd === "EVENT") {
    const event = asProof(frame[2]);
    return event ? { type: "event", event } : { type: "ignore" };
  }
  if (cmd === "EOSE") return { type: "eose" };
  if (cmd === "CLOSED") {
    const detail = typeof frame[2] === "string" ? frame[2] : "";
    return { type: "closed", detail };
  }
  if (cmd === "NOTICE") {
    const detail = typeof frame[1] === "string" ? frame[1] : "";
    return { type: "notice", detail };
  }
  return { type: "ignore" };
}
