// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function startGrab(scope: any, card: KanbanCard) {
  if (scope.grabbedId) return; // a repeated Shift+Enter is not a re-grab
  scope.grabbedId = card.id;
  scope.grabCol = card.status;
  // Start where the card really sits so the grab never jumps it.
  scope.grabIdx = Math.max(
    0,
    scope.boardCols[card.status].findIndex((c) => c.id === card.id),
  );
}

export function grabArrow(scope: any, dir: RingDir) {
  if (!scope.grabbedId || !scope.grabCol) return;
  if (dir === "left" || dir === "right") {
    // Column hop, clamped at the ends. Empty columns are legal targets
    // here — you are placing the card, not selecting one.
    const order = scope.COLUMNS.map((c) => c.id);
    const next = order.indexOf(scope.grabCol) + (dir === "left" ? -1 : 1);
    if (next < 0 || next >= order.length) return;
    scope.grabCol = order[next];
  } else {
    scope.grabIdx += dir === "up" ? -1 : 1;
  }
  // Same row, clamped into the column (its length excludes the held card).
  const len = scope.boardCols[scope.grabCol].filter((c) => c.id !== scope.grabbedId).length;
  scope.grabIdx = Math.min(Math.max(scope.grabIdx, 0), len);
  // The card moved under a pinned highlight — keep it scrolled in view.
  scope.menuRing.select(`card-${scope.grabbedId}`);
}

export function dropGrab(scope: any) {
  const id = scope.grabbedId;
  const target = scope.grabCol;
  const idx = scope.grabIdx;
  scope.grabbedId = null;
  scope.grabCol = null;
  scope.grabIdx = 0;
  scope.grabGhost = null;
  const card = id ? scope.kanbanCards.find((c) => c.id === id) : undefined;
  if (!card || !target) return;
  if (target !== card.status) {
    scope.moveCard(card, target);
    return;
  }
  scope.persistColumnOrder(card, target, idx);
}

export function persistColumnOrder(scope: any, card: KanbanCard, col: Status, toIdx: number) {
  scope.mutate((s) => {
    const cards = s.kanbanCards;
    const match = (c: KanbanCard) =>
      c.status === col &&
      c.kind !== "needed" &&
      c.kind !== "window" &&
      !c.parentId &&
      scope.inScope(c);
    const group = cards.filter(match);
    const from = group.findIndex((c) => c.id === card.id);
    if (from < 0) return;
    const [held] = group.splice(from, 1);
    group.splice(Math.min(Math.max(toIdx, 0), group.length), 0, held);
    let gi = 0;
    s.kanbanCards = cards.map((c) => (match(c) ? group[gi++] : c));
  });
}

export function ungrab(scope: any) {
  // Esc (pad B): no mutation ever happened — the card is still in its
  // pre-grab column; the held paint just stops.
  scope.grabbedId = null;
  scope.grabCol = null;
  scope.grabIdx = 0;
  scope.grabGhost = null;
}
