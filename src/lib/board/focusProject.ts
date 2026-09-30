/** One-shot board focus.
 *
 * A pending planet id becomes the selection. Once that id is in the
 * project list, pending clears so a later loadState cannot snap back to
 * an older board click. If the id is not in the list yet, pending stays
 * until a later resolve sees it. A null pending leaves the current
 * selection alone (manual tab pick, or a focus already consumed).
 */
export function resolveFocus(input: {
  pending: string | null;
  selected: string;
  projectIds: readonly string[];
}): { selected: string; pending: string | null } {
  const pending = (input.pending ?? "").trim();
  if (!pending) {
    return { selected: input.selected, pending: null };
  }
  const present = input.projectIds.includes(pending);
  return {
    selected: pending,
    pending: present ? null : pending,
  };
}
