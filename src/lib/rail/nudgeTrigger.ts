/** A window the Claim chip can name. */
export type NudgeWindow = {
  id: string;
  stableKey: string;
  isMinimized: boolean;
};

/**
 * The chip's trigger after the first poll has been recorded.
 * A visible → minimized transition of an unclaimed window whose id and
 * stableKey are not already suppressed. The first poll itself never calls
 * this with a chip: it only stores `prevMinimized` and returns.
 */
/** A shrink we just heard, not a poll. One prompt at a time. */
export function armOnShrink(
  id: string,
  nudgeFor: string | null,
  claimedWindowIds: ReadonlySet<string>,
  suppressed: readonly string[],
  stableKey: string | null,
): string | null {
  if (!id || nudgeFor) return nudgeFor;
  if (claimedWindowIds.has(id)) return null;
  if (suppressed.includes(id)) return null;
  if (stableKey && suppressed.includes(stableKey)) return null;
  return id;
}

export function nudgePick<T extends NudgeWindow>(
  windows: readonly T[],
  prevMinimized: ReadonlySet<string>,
  claimedWindowIds: ReadonlySet<string>,
  suppressed: readonly string[],
): T | undefined {
  return windows.find(
    (w) =>
      w.isMinimized &&
      !prevMinimized.has(w.id) &&
      !claimedWindowIds.has(w.id) &&
      !suppressed.includes(w.id) &&
      !suppressed.includes(w.stableKey),
  );
}
