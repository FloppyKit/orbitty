/** Shared planet order. Board and Stars both call this, then save `projects`. */

export function reorderProjects<T extends { id: string }>(
  list: readonly T[],
  fromId: string,
  beforeId: string | null,
): T[] {
  if (!fromId || fromId === beforeId) return list as T[];
  const next = [...list];
  const from = next.findIndex((p) => p.id === fromId);
  if (from < 0) return list as T[];
  const [moved] = next.splice(from, 1);
  const to = beforeId ? next.findIndex((p) => p.id === beforeId) : next.length;
  next.splice(to < 0 ? next.length : to, 0, moved);
  const same =
    next.length === list.length && next.every((p, i) => p === list[i]);
  return same ? (list as T[]) : next;
}
