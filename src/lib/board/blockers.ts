// Pure blocker edges. The sidecar is `{ cardId: [blockerIds] }`.
// An open blocker exists and is not done. Missing and done are clear.

export type EdgeMap = Record<string, string[]>;

export type BlockerCard = {
  id: string;
  title: string;
  status: string;
  kind?: string | null;
};

let edges: EdgeMap = {};

export function getEdges(): EdgeMap {
  return edges;
}

export function setEdges(next: EdgeMap): void {
  edges = next ?? {};
}

export function openBlockerIds(
  map: EdgeMap,
  cards: readonly BlockerCard[] | undefined,
  id: string,
): string[] {
  const listed = map[id] ?? [];
  const rows = cards ?? [];
  const open: string[] = [];
  for (const bid of listed) {
    const hit = rows.find((c) => c.id === bid);
    if (hit && hit.status !== "done") open.push(bid);
  }
  return open;
}

export function isBlocked(
  map: EdgeMap,
  cards: readonly BlockerCard[] | undefined,
  card: { id: string },
): boolean {
  return openBlockerIds(map, cards, card.id).length > 0;
}

export function blockedLabel(n: number): string {
  return `Blocked · ${n}`;
}

export function waitingTitle(
  map: EdgeMap,
  cards: readonly BlockerCard[] | undefined,
  id: string,
): string {
  const rows = cards ?? [];
  const titles = openBlockerIds(map, cards, id).map((bid) => {
    const title = (rows.find((c) => c.id === bid)?.title ?? "").trim();
    return title || bid;
  });
  return `Waiting on: ${titles.join(", ")}`;
}
