// Moved function bodies. scope fields are the lets each function already closed over.

import type { AgentRun, CardTombstone, Claim, KanbanCard, PointsEntry, Project, TitleOverride, WindowInfo, WindowTags } from "../persist";
import { nudgePick } from "./nudgeTrigger";

interface AxStatus {
  trusted: boolean;
  bundlePath: string;
  signingLine: string;
  inBundle: boolean;
  isAdhoc: boolean;
}

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_onDragStart = {
  dragWindowId: string | null;
  dragStartedAt: number;
  ctxMenu: { x: number; y: number; windowId: string } | null;
};

type Scope_onDragEnd = {
  dragWindowId: string | null;
  dropTarget: string | null;
  pendingDisk: { kanbanCards: KanbanCard[]; pointsLedger: PointsEntry[]; agentRuns: AgentRun[]; cardTombstones: CardTombstone[] } | null;
  persistDeferred: boolean;
  dragStartedAt: number;
  windowDragHot: (...args: any[]) => boolean;
  absorbPendingCards: (...args: any[]) => any;
  persist: (...args: any[]) => Promise<any>;
};

type Scope_isWindowDrag = {
  dragWindowId: string | null;
};

type Scope_onSectionDragEnter = {
  isWindowDrag: (...args: any[]) => boolean;
  dropTarget: string | null;
};

type Scope_onSectionDragOver = {
  isWindowDrag: (...args: any[]) => boolean;
  dropTarget: string | null;
};

type Scope_onSectionDragLeave = {
  dropTarget: string | null;
};

type Scope_onSectionDrop = {
  dragWindowId: string | null;
  claim: (...args: any[]) => any;
  unclaim: (...args: any[]) => any;
  dropTarget: string | null;
};

type Scope_tick = {
  windowDragHot: (...args: any[]) => boolean;
  invoke: Invoke;
  noteAx: (...args: any[]) => any;
  dragWindowId: string | null;
  windows: WindowInfo[];
  refreshedAt: string;
  windowsPolled: boolean;
  embedded: boolean;
  rematchClaims: (...args: any[]) => any;
  healOrphanWindowCards: (...args: any[]) => boolean;
  persist: (...args: any[]) => Promise<any>;
  liveClaims: Claim[];
  touchClaim: (...args: any[]) => any;
  checkNudge: (...args: any[]) => any;
  syncHabitat: (...args: any[]) => Promise<any>;
};

type Scope_rematchClaims = {
  stateLoaded: boolean;
  windows: WindowInfo[];
  liveClaims: Claim[];
  titleIndexFor: (...args: any[]) => number;
  isHermesClass: (...args: any[]) => boolean;
  claimStableKey: (...args: any[]) => string | null;
  rewriteWindowId: (...args: any[]) => any;
  persist: (...args: any[]) => Promise<any>;
};

type Scope_rewriteWindowId = {
  nowEpoch: (...args: any[]) => number;
  claims: Claim[];
  titleIndexFor: (...args: any[]) => number;
  isHermesClass: (...args: any[]) => boolean;
  kanbanCards: KanbanCard[];
  windowTags: WindowTags[];
  titleOverrides: TitleOverride[];
  nudgeSuppressed: string[];
};

type Scope_healOrphanWindowCards = {
  stateLoaded: boolean;
  kanbanCards: KanbanCard[];
  liveClaims: Claim[];
  windows: WindowInfo[];
  projects: Project[];
  claim: (...args: any[]) => any;
  windowsPolled: boolean;
  nowEpoch: (...args: any[]) => number;
};

type Scope_checkNudge = {
  stateLoaded: boolean;
  windows: WindowInfo[];
  nudgePrimed: boolean;
  prevMinimized: Set<string>;
  nudgeSuppressed: string[];
  persist: (...args: any[]) => Promise<any>;
  nudgeFor: string | null;
  liveClaims: Claim[];
  nudgeNewName: string;
};

type Scope_openSettings = {
  invoke: Invoke;
};

type Scope_retry = {
  invoke: Invoke;
  tick: (...args: any[]) => Promise<any>;
};

type Scope_copyPath = {
  status: AxStatus | null;
  copied: boolean;
};

export function onDragStart(scope: Scope_onDragStart, e: DragEvent, w: WindowInfo) {
  scope.dragWindowId = w.id;
  scope.dragStartedAt = Date.now();
  scope.ctxMenu = null;
  e.dataTransfer?.setData("text/plain", w.id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
}

export function onDragEnd(scope: Scope_onDragEnd) {
  scope.dragWindowId = null;
  scope.dropTarget = null;
  if (!scope.pendingDisk && !scope.persistDeferred) return;
  const wait = Math.max(0, 420 - (Date.now() - scope.dragStartedAt));
  setTimeout(() => {
    if (scope.windowDragHot()) return;
    if (scope.pendingDisk) scope.absorbPendingCards();
    if (scope.persistDeferred) scope.persist();
  }, wait);
}

export function isWindowDrag(scope: Scope_isWindowDrag, e: DragEvent): boolean {
  return (
    !!scope.dragWindowId || !!e.dataTransfer?.types?.includes("text/plain")
  );
}

export function onSectionDragEnter(scope: Scope_onSectionDragEnter, e: DragEvent, key: string) {
  if (!scope.isWindowDrag(e)) return;
  e.preventDefault();
  scope.dropTarget = key;
}

export function onSectionDragOver(scope: Scope_onSectionDragOver, e: DragEvent, key: string) {
  if (!scope.isWindowDrag(e)) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  if (scope.dropTarget !== key) scope.dropTarget = key;
}

export function onSectionDragLeave(scope: Scope_onSectionDragLeave, e: DragEvent, key: string) {
  // dragleave fires when moving between children; only clear when the
  // pointer actually leaves the section.
  const next = e.relatedTarget as Node | null;
  if (
    next &&
    e.currentTarget instanceof HTMLElement &&
    e.currentTarget.contains(next)
  )
    return;
  if (scope.dropTarget === key) scope.dropTarget = null;
}

export function onSectionDrop(scope: Scope_onSectionDrop, e: DragEvent, projectId: string | null) {
  e.preventDefault();
  // dataTransfer is authoritative inside drop; Svelte state is the fallback
  // in case the 800ms poll tore down and rebuilt the source row mid-gesture.
  const id = e.dataTransfer?.getData("text/plain") || scope.dragWindowId;
  if (id) {
    if (projectId) {
      scope.claim(id, projectId);
    } else {
      scope.unclaim(id);
    }
  }
  scope.dragWindowId = null;
  scope.dropTarget = null;
}

export async function tick(scope: Scope_tick) {
  // Frozen while a window drag is hot: no AX status, no list_windows, no
  // syncHabitat — mutating the dragged row's DOM cancels the HTML5 drag
  // session in WKWebView, and the sync is what beach-balled the drop.
  if (scope.windowDragHot()) return;
  try {
    const s = await scope.invoke<AxStatus>("accessibility_status");
    scope.noteAx(s);
    // Don't swap the rows array mid-drag: mutating the dragged row's DOM
    // can cancel the HTML5 drag session in WKWebView.
    if (s.trusted && !scope.dragWindowId) {
      scope.windows = await scope.invoke<WindowInfo[]>("list_windows");
      scope.refreshedAt = new Date().toLocaleTimeString();
      scope.windowsPolled = true;
      // INSPECTOR-HOST: habitat sync / rematch stay on the main rail
      // instance so two mounts don't double-write. Embedded still lists
      // windows so the well stays live.
      if (scope.embedded) return;
      scope.rematchClaims();
      // ORPHAN-CARDS: heal right after rematch — windows is fresh, so
      // live orphans re-claim and dead ones demote on this same pass.
      if (scope.healOrphanWindowCards()) scope.persist();
      // Frontmost tick: a claimed window the human is actively in counts as
      // touched (throttled persist inside), so zombies self-heal by use.
      const front = scope.windows.find((w) => w.isFrontmost);
      if (front && scope.liveClaims.some((c) => c.windowId === front.id)) {
        scope.touchClaim(front.id);
      }
      scope.checkNudge();
      scope.syncHabitat();
    }
  } catch {
    // keep last good state; next poll retries
  }
}

export function rematchClaims(scope: Scope_rematchClaims) {
  if (!scope.stateLoaded) return;
  const liveIds = new Set(scope.windows.map((w) => w.id));
  const used = new Set<string>();
  let changed = false;
  // Live claims keep their ids; backfill the fingerprint on pre-GO claims
  // so they survive the NEXT reboot too. While a claim is live, refresh
  // its sleeper wake trail (file path / Hermes session) from the window.
  for (const c of scope.liveClaims) {
    if (!liveIds.has(c.windowId)) continue;
    used.add(c.windowId);
    const w = scope.windows.find((x) => x.id === c.windowId);
    if (!w) continue;
    if (c.titleAtClaim == null) {
      c.bundleId = w.bundleId ?? null;
      c.appName = w.appName;
      c.titleAtClaim = w.title;
      c.titleIndex = scope.titleIndexFor(w);
      changed = true;
    }
    if (w.documentPath && c.filePath !== w.documentPath) {
      c.filePath = w.documentPath;
      changed = true;
    }
    if (scope.isHermesClass(w) && w.title && c.hermesSession !== w.title) {
      c.hermesSession = w.title;
      changed = true;
    }
  }
  for (const c of scope.liveClaims) {
    if (liveIds.has(c.windowId)) continue;
    const key = scope.claimStableKey(c);
    let pick: WindowInfo | undefined;
    if (key) pick = scope.windows.find((w) => !used.has(w.id) && w.stableKey === key);
    if (!pick && (c.bundleId || c.appName) && c.titleAtClaim != null) {
      const bundle = c.bundleId || c.appName;
      pick = scope.windows.find(
        (w) =>
          !used.has(w.id) &&
          (w.bundleId || w.appName) === bundle &&
          w.title === c.titleAtClaim,
      );
    }
    // Document path (Preview / editors): title may drift; the stamped
    // file is stronger than the AX title.
    if (!pick && c.filePath) {
      pick = scope.windows.find(
        (w) => !used.has(w.id) && w.documentPath === c.filePath,
      );
    }
    // Soft rematch: title often drifts (browser nav, wallet rename,
    // Finder folder). If exactly one unused live window shares this
    // claim's bundle, adopt it — otherwise leave the orphan (ambiguous).
    if (!pick) {
      const bundle = c.bundleId || c.appName;
      if (bundle) {
        const same = scope.windows.filter(
          (w) => !used.has(w.id) && (w.bundleId || w.appName) === bundle,
        );
        if (same.length === 1) pick = same[0];
      }
    }
    if (!pick) continue; // orphan — waits for its window to return
    used.add(pick.id);
    scope.rewriteWindowId(c.windowId, pick);
    changed = true;
  }
  if (changed) scope.persist();
}

export function rewriteWindowId(scope: Scope_rewriteWindowId, oldId: string, w: WindowInfo) {
  // Rematch / sleeper wake is claim activity — refresh lastTouched so
  // zombie fade dim clears when the walker comes back live.
  const touched = scope.nowEpoch();
  scope.claims = scope.claims.map((c) =>
    c.windowId === oldId
      ? {
          ...c,
          windowId: w.id,
          bundleId: w.bundleId ?? null,
          appName: w.appName,
          titleAtClaim: w.title,
          titleIndex: scope.titleIndexFor(w),
          filePath: w.documentPath ?? c.filePath ?? null,
          hermesSession:
            scope.isHermesClass(w) && w.title ? w.title : (c.hermesSession ?? null),
          lastTouchedAt: touched,
        }
      : c,
  );
  scope.kanbanCards = scope.kanbanCards.map((c) =>
    c.windowId === oldId ? { ...c, windowId: w.id } : c,
  );
  scope.windowTags = scope.windowTags.map((wt) =>
    wt.windowId === oldId ? { ...wt, windowId: w.id } : wt,
  );
  scope.titleOverrides = scope.titleOverrides.map((o) =>
    o.windowId === oldId ? { ...o, windowId: w.id } : o,
  );
  scope.nudgeSuppressed = scope.nudgeSuppressed.map((id) => (id === oldId ? w.id : id));
}

export function healOrphanWindowCards(scope: Scope_healOrphanWindowCards): boolean {
  if (!scope.stateLoaded) return false;
  let changed = false;
  const orphans = scope.kanbanCards.filter(
    (k) =>
      k.kind === "window" &&
      k.status !== "done" &&
      !!k.windowId &&
      !scope.liveClaims.some((c) => c.windowId === k.windowId),
  );
  for (const card of orphans) {
    const w = scope.windows.find((x) => x.id === card.windowId);
    if (w && scope.projects.some((p) => p.id === card.projectId)) {
      scope.claim(w.id, card.projectId); // the one door; persists itself
      changed = true;
    } else if (!w && scope.windowsPolled) {
      const now = scope.nowEpoch();
      scope.kanbanCards = scope.kanbanCards.map((k) =>
        k.id === card.id
          ? { ...k, kind: "todo" as const, windowId: null, lastTouchedAt: now }
          : k,
      );
      changed = true;
    }
  }
  return changed;
}

export function checkNudge(scope: Scope_checkNudge) {
  if (!scope.stateLoaded) return;
  const minimizedNow = new Set(
    scope.windows.filter((w) => w.isMinimized).map((w) => w.id),
  );
  if (!scope.nudgePrimed) {
    scope.nudgePrimed = true;
    scope.prevMinimized = minimizedNow;
    return; // first poll never nudges; a later restore → minimize still can
  }
  if (scope.nudgeFor && !minimizedNow.has(scope.nudgeFor)) {
    scope.nudgeFor = null; // restored or gone before acting on the banner
  }
  if (!scope.nudgeFor) {
    const claimed = new Set(scope.liveClaims.map((c) => c.windowId));
    const candidate = nudgePick(
      scope.windows,
      scope.prevMinimized,
      claimed,
      scope.nudgeSuppressed,
    );
    if (candidate) {
      scope.nudgeSuppressed = [
        ...new Set([...scope.nudgeSuppressed, candidate.id, candidate.stableKey]),
      ];
      scope.nudgeFor = candidate.id;
      scope.nudgeNewName = "";
      scope.persist();
    }
  }
  scope.prevMinimized = minimizedNow;
}

export async function openSettings(scope: Scope_openSettings) {
  try {
    await scope.invoke("open_accessibility_settings");
  } catch {}
}

export async function retry(scope: Scope_retry) {
  try {
    await scope.invoke("prompt_accessibility");
  } catch {}
  await scope.tick();
}

export async function copyPath(scope: Scope_copyPath) {
  if (!scope.status) return;
  try {
    await navigator.clipboard.writeText(scope.status.bundlePath);
    scope.copied = true;
    setTimeout(() => (scope.copied = false), 1500);
  } catch {}
}
