// Fixture proofs shaped like the Rust log. Content is a JSON string.
// Mint: `{ cardId, title }`. Move: `{ from, to, cardId }` with no title.
// Tags: `t=orbitty`, `d` = card id, plus project / actor / trace.

import { KIND_DOING, KIND_DONE, KIND_TODO, type BoardProof } from "./fold.ts";

const PUBKEY = "ab".repeat(32);
const SIG = "cd".repeat(64);

let n = 0;

function id(): string {
  n += 1;
  return n.toString(16).padStart(64, "0");
}

export function proof(partial: {
  kind: number;
  created_at: number;
  cardId: string;
  content: Record<string, unknown>;
  project?: string;
  extraTags?: string[][];
  t?: string;
  id?: string;
}): BoardProof {
  const tags: string[][] = [
    ["t", partial.t ?? "orbitty"],
    ["d", partial.cardId],
  ];
  if (partial.extraTags) tags.push(...partial.extraTags);
  if (partial.project !== undefined) tags.push(["project", partial.project]);
  tags.push(["actor", "user"], ["trace", "tr-fixture"]);
  return {
    id: partial.id ?? id(),
    pubkey: PUBKEY,
    created_at: partial.created_at,
    kind: partial.kind,
    tags,
    content: JSON.stringify(partial.content),
    sig: SIG,
  };
}

/**
 * Arrival order is not time order. The doing-move for the bowls is listed
 * before the mint that names them. A 31200 line and a t=nch line sit in
 * the same array and must not become cards.
 */
export function sampleProofs(): BoardProof[] {
  return [
    proof({
      kind: KIND_DOING,
      created_at: 200,
      cardId: "c-bowls",
      project: "p-kitchen",
      extraTags: [["e", "0".repeat(64)]],
      content: { from: "todo", to: "doing", cardId: "c-bowls" },
    }),
    proof({
      kind: KIND_TODO,
      created_at: 100,
      cardId: "c-bowls",
      project: "p-kitchen",
      content: { cardId: "c-bowls", title: "Warm the bowls" },
    }),
    proof({
      kind: 31200,
      created_at: 400,
      cardId: "c-bowls",
      t: "local-only",
      project: "p-kitchen",
      extraTags: [["e-type", "board-notes"]],
      content: { cardId: "c-bowls", title: "Renamed bowls" },
    }),
    proof({
      kind: KIND_TODO,
      created_at: 150,
      cardId: "c-bread",
      project: "p-kitchen",
      content: { cardId: "c-bread", title: "Slice the bread", parentId: "c-bowls" },
    }),
    proof({
      kind: KIND_DOING,
      created_at: 250,
      cardId: "c-step",
      project: "p-yard",
      content: { cardId: "c-step", title: "Sweep the step" },
    }),
    proof({
      kind: KIND_DONE,
      created_at: 300,
      cardId: "c-rake",
      project: "p-yard",
      content: { from: "todo", to: "done", cardId: "c-rake" },
    }),
    proof({
      kind: KIND_TODO,
      created_at: 50,
      cardId: "c-nch",
      t: "nch",
      project: "p-yard",
      content: { cardId: "c-nch", title: "Not an Orbitty card" },
    }),
  ];
}
