/** One-shot board focus.
 *
 * A pending planet id becomes the selection. Once that id is in the
 * project list, pending clears so a later loadState cannot snap back to
 * an older board click. If the id is not in the list yet, pending stays
 * until a later resolve sees it. A null pending leaves the current
 * selection alone (manual tab pick, or a focus already consumed).
 *
 * `projectsLoaded` marks a real non-empty list (loadState / pane show).
 * A pending id missing from that list is a ghost: drop it and leave the
 * current selection alone. An empty list is "not loaded yet" and keeps
 * the pending id.
 */
export function resolveFocus(input: {
  pending: string | null;
  selected: string;
  projectIds: readonly string[];
  projectsLoaded?: boolean;
}): { selected: string; pending: string | null } {
  const pending = (input.pending ?? "").trim();
  if (!pending) {
    return { selected: input.selected, pending: null };
  }
  const present = input.projectIds.includes(pending);
  if (!present && input.projectsLoaded) {
    return { selected: input.selected, pending: null };
  }
  return {
    selected: pending,
    pending: present ? null : pending,
  };
}
