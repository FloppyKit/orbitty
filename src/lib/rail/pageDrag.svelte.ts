// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function onDragStart(scope: any, e: DragEvent, w: WindowInfo) {
  scope.dragWindowId = w.id;
  scope.dragStartedAt = Date.now();
  scope.ctxMenu = null;
  e.dataTransfer?.setData("text/plain", w.id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
}

export function onDragEnd(scope: any) {
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

export function isWindowDrag(scope: any, e: DragEvent): boolean {
  return (
    !!scope.dragWindowId || !!e.dataTransfer?.types?.includes("text/plain")
  );
}

export function onSectionDragEnter(scope: any, e: DragEvent, key: string) {
  if (!scope.isWindowDrag(e)) return;
  e.preventDefault();
  scope.dropTarget = key;
}

export function onSectionDragOver(scope: any, e: DragEvent, key: string) {
  if (!scope.isWindowDrag(e)) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  if (scope.dropTarget !== key) scope.dropTarget = key;
}

export function onSectionDragLeave(scope: any, e: DragEvent, key: string) {
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

export function onSectionDrop(scope: any, e: DragEvent, projectId: string | null) {
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

export async function tick(scope: any) {
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
      if (embedded) return;
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

export function rematchClaims(scope: any) {
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

export function rewriteWindowId(scope: any, oldId: string, w: WindowInfo) {
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

export function healOrphanWindowCards(scope: any): boolean {
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

export function checkNudge(scope: any) {
  if (!scope.stateLoaded) return;
  const minimizedNow = new Set(
    scope.windows.filter((w) => w.isMinimized).map((w) => w.id),
  );
  if (!scope.nudgePrimed) {
    scope.nudgePrimed = true;
    scope.prevMinimized = minimizedNow;
    const alreadyMin = scope.windows.filter((w) => w.isMinimized);
    if (alreadyMin.length > 0) {
      const extra = alreadyMin.flatMap((w) => [w.id, w.stableKey]);
      scope.nudgeSuppressed = [...new Set([...scope.nudgeSuppressed, ...extra])];
      scope.persist();
    }
    return; // first poll never nudges
  }
  if (scope.nudgeFor && !minimizedNow.has(scope.nudgeFor)) {
    scope.nudgeFor = null; // restored or gone before acting on the banner
  }
  if (!scope.nudgeFor) {
    const candidate = scope.windows.find(
      (w) =>
        w.isMinimized &&
        !scope.prevMinimized.has(w.id) && // a real visible → minimized transition
        !scope.liveClaims.some((c) => c.windowId === w.id) &&
        !scope.nudgeSuppressed.includes(w.id) &&
        !scope.nudgeSuppressed.includes(w.stableKey),
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

export async function openSettings(scope: any) {
  try {
    await scope.invoke("open_accessibility_settings");
  } catch {}
}

export async function retry(scope: any) {
  try {
    await scope.invoke("prompt_accessibility");
  } catch {}
  await scope.tick();
}

export async function copyPath(scope: any) {
  if (!scope.status) return;
  try {
    await navigator.clipboard.writeText(scope.status.bundlePath);
    scope.copied = true;
    setTimeout(() => (scope.copied = false), 1500);
  } catch {}
}
