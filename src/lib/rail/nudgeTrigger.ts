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
