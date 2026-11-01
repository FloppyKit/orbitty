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
let blockersOn = false;

export function getEdges(): EdgeMap {
  return edges;
}

export function setEdges(next: EdgeMap): void {
  edges = next ?? {};
}

/** Page-lifetime bit. Off clears stored edges so a drag cannot see them. */
export function blockersEnabled(): boolean {
  return blockersOn;
}

export function setBlockersOn(on: boolean): void {
  blockersOn = on;
  if (!on) edges = {};
}

/** Flag off is an empty sidecar. The chip and the drag both read this. */
export function visibleEdges(flagOn: boolean, map: EdgeMap | null | undefined): EdgeMap {
  if (!flagOn || map == null || typeof map !== "object" || Array.isArray(map)) return {};
  return map;
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
