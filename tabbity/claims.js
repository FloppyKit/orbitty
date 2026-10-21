// Pure tab-claim logic. The worker wires chrome.*; tests pass a fake.

export const HOST = "space.orbitty.tabbity";
export const EXTENSION_ID = "pbdnjlgnnkgbmnikjnfakajdkndgdicn";

export function mintId(bytes) {
  const b = bytes ? Uint8Array.from(bytes) : crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
  return `tab:${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

export function windowOrdinal(windows, windowId) {
  const sorted = [...(windows ?? [])].sort((a, b) => a.id - b.id);
  const i = sorted.findIndex((w) => w.id === windowId);
  return i < 0 ? 0 : i;
}

export function favicon(url) {
  if (!url || url.length > 1024) return null;
  if (url.startsWith("https://") || url.startsWith("http://")) return url;
  return null;
}

function dist(claim, tab, windows) {
  const ord = windowOrdinal(windows, tab.windowId);
  return Math.abs(ord - (claim.windowOrdinal ?? 0)) * 1000 + Math.abs((tab.index ?? 0) - (claim.index ?? 0));
}

/** Exact URL only. Ties by closest saved (windowOrdinal, index). One tab per claim. */
export function rematch(claims, tabs, windows) {
  const used = new Set();
  return claims.map((c) => {
    if (c.tabId != null && tabs.some((t) => t.id === c.tabId)) {
      used.add(c.tabId);
      return { ...c };
    }
    const candidates = tabs.filter((t) => !used.has(t.id) && (t.url || "") === (c.url || ""));
    if (candidates.length === 0) {
      return { ...c, tabId: null, windowId: null };
    }
    candidates.sort((a, b) => dist(c, a, windows) - dist(c, b, windows));
    const pick = candidates[0];
    used.add(pick.id);
    return {
      ...c,
      tabId: pick.id,
      windowId: pick.windowId,
      index: pick.index ?? 0,
      windowOrdinal: windowOrdinal(windows, pick.windowId),
      title: pick.title || c.title,
      url: pick.url || c.url,
    };
  });
}

export function reconcile(local, orbittyClaims) {
  const ids = new Set((orbittyClaims ?? []).map((c) => c.id));
  const kept = local.filter((c) => ids.has(c.id));
  for (const o of orbittyClaims ?? []) {
    if (!kept.some((c) => c.id === o.id)) {
      kept.push({
        id: o.id,
        browser: o.browser,
        url: o.url || "",
        title: o.title || "",
        favIconUrl: o.favIconUrl ?? null,
        windowOrdinal: 0,
        index: 0,
        tabId: null,
        windowId: null,
        lastSeenAt: o.lastSeenAt ?? 0,
      });
    }
  }
  return kept;
}

export function onUpdated(claims, tabId, change, tab) {
  const c = claims.find((x) => x.tabId === tabId);
  if (!c) return { claims, dirty: false };
  const url = change.url ?? tab?.url ?? c.url;
  const title = change.title ?? tab?.title ?? c.title;
  const dirty = url !== c.url || title !== c.title;
  const next = claims.map((x) =>
    x.tabId === tabId
      ? { ...x, url, title, favIconUrl: favicon(tab?.favIconUrl) ?? x.favIconUrl, lastSeenAt: Date.now() / 1000 }
      : x,
  );
  return { claims: next, dirty };
}

export function onRemoved(claims, tabId) {
  return claims.map((c) => (c.tabId === tabId ? { ...c, tabId: null, windowId: null } : c));
}

export function onReplaced(claims, addedId, removedId) {
  return claims.map((c) => (c.tabId === removedId ? { ...c, tabId: addedId } : c));
}

export function onAttached(claims, tabId, info, windows) {
  return claims.map((c) =>
    c.tabId === tabId
      ? {
          ...c,
          windowId: info.newWindowId,
          index: info.newPosition ?? c.index,
          windowOrdinal: windowOrdinal(windows, info.newWindowId),
        }
      : c,
  );
}

export function onDetached(claims, tabId) {
  return claims.map((c) => (c.tabId === tabId ? { ...c, windowId: null } : c));
}

export function onMoved(claims, tabId, info) {
  return claims.map((c) =>
    c.tabId === tabId ? { ...c, index: info.toIndex, windowId: info.windowId ?? c.windowId } : c,
  );
}

export function handleCommand(claims, cmd) {
  if (!cmd) return { claims, action: null };
  if (cmd.cmd === "unclaimed" || cmd.cmd === "reconcile") {
    const ids = new Set(cmd.ids ?? cmd.claimIds ?? []);
    if (cmd.cmd === "unclaimed") {
      return { claims: claims.filter((c) => !ids.has(c.id)), action: null };
    }
    return { claims, action: { type: "reconcile", claimIds: [...ids] } };
  }
  const c = claims.find((x) => x.id === cmd.claimId);
  if (cmd.cmd === "focus" && c?.tabId != null) {
    return { claims, action: { type: "focus", tabId: c.tabId, windowId: c.windowId } };
  }
  if (cmd.cmd === "reopen") {
    return { claims, action: { type: "reopen", claimId: cmd.claimId, url: cmd.url || c?.url } };
  }
  if (cmd.cmd === "close" && c?.tabId != null) {
    return { claims, action: { type: "close", tabId: c.tabId } };
  }
  return { claims, action: null };
}

export function bindReopen(claims, claimId, tab) {
  return claims.map((c) =>
    c.id === claimId
      ? { ...c, tabId: tab.id, windowId: tab.windowId, index: tab.index ?? 0, url: tab.url || c.url }
      : c,
  );
}
