// Tab claim rows and rematch guards. A tab claim is a walker row, never a
// macOS window, so window rematch must not adopt it and it must not adopt
// a window.

import type { Claim, TabRef, WindowInfo } from "../persist";

export function isTabId(id: string | null | undefined): boolean {
  return (id ?? "").startsWith("tab:");
}

export function isTabClaim(c: { windowId?: string; tab?: TabRef | null }): boolean {
  return !!c.tab || isTabId(c.windowId);
}

const BUNDLE: Record<string, { bundleId: string; appName: string }> = {
  brave: { bundleId: "com.brave.Browser", appName: "Brave" },
  chrome: { bundleId: "com.google.Chrome", appName: "Google Chrome" },
};

export function browserFace(browser: string | undefined): { bundleId: string; appName: string } {
  return BUNDLE[browser ?? ""] ?? { bundleId: "", appName: browser || "Browser" };
}

/** Walker Assign rebuilds the claim. `tab` has to come from the previous row. */
export function keptTab(prev?: { tab?: TabRef | null } | null): TabRef | null {
  return prev?.tab ?? null;
}

/**
 * Window rematch pick. Tab claims never adopt a window. A pick whose id is
 * `tab:` is never a window, so a window claim cannot land on a tab row.
 */
export function rematchPick(
  c: Claim,
  windows: WindowInfo[],
  used: Set<string>,
  stableKey: string | null,
): WindowInfo | undefined {
  if (isTabClaim(c)) return undefined;
  const usable = (w: WindowInfo) => !used.has(w.id) && !isTabId(w.id);
  let pick = stableKey
    ? windows.find((w) => usable(w) && w.stableKey === stableKey)
    : undefined;
  if (!pick && (c.bundleId || c.appName) && c.titleAtClaim != null) {
    const bundle = c.bundleId || c.appName;
    pick = windows.find(
      (w) => usable(w) && (w.bundleId || w.appName) === bundle && w.title === c.titleAtClaim,
    );
  }
  if (!pick && c.filePath) {
    pick = windows.find((w) => usable(w) && w.documentPath === c.filePath);
  }
  if (!pick) {
    const bundle = c.bundleId || c.appName;
    if (bundle) {
      const same = windows.filter((w) => usable(w) && (w.bundleId || w.appName) === bundle);
      if (same.length === 1) pick = same[0];
    }
  }
  return pick;
}

export type TabLive = { id: string; title?: string; live: boolean; browser?: string };

let liveIds = new Set<string>();

export function setTabLive(rows: TabLive[] | null | undefined) {
  liveIds = new Set((rows ?? []).filter((r) => r.live).map((r) => r.id));
}

export function tabIsLive(id: string): boolean {
  return liveIds.has(id);
}

export function tabRow(c: Claim, windows: WindowInfo[] = []): WindowInfo {
  const face = browserFace(c.tab?.browser);
  const icon =
    windows.find((w) => w.bundleId === face.bundleId && w.iconBase64)?.iconBase64 ?? null;
  const live = tabIsLive(c.windowId);
  return {
    id: c.windowId,
    pid: 0,
    appName: face.appName,
    title: c.tab?.title || c.titleAtClaim || face.appName,
    bundleId: face.bundleId || null,
    stableKey: "",
    iconBase64: icon,
    isFrontmost: false,
    isMinimized: !live,
    documentPath: null,
    sleeping: !live,
  };
}
