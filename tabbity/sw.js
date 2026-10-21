import {
  HOST,
  bindReopen,
  handleCommand,
  mintId,
  onAttached,
  onDetached,
  onMoved,
  onRemoved,
  onReplaced,
  onUpdated,
  reconcile,
  rematch,
  windowOrdinal,
  favicon,
} from "./claims.js";

const DEBOUNCE_MS = 2000;
let port = null;
let claims = [];
let backoff = 1000;
let pending = new Map();
let timers = new Map();

function browserName() {
  return /Brave/i.test(navigator.userAgent || "") ? "brave" : "chrome";
}

async function load() {
  const got = await chrome.storage.local.get("tabbityClaims");
  claims = Array.isArray(got.tabbityClaims) ? got.tabbityClaims : [];
}

async function save() {
  await chrome.storage.local.set({ tabbityClaims: claims });
}

function send(tool, args) {
  return new Promise((resolve, reject) => {
    if (!port) {
      reject(new Error(`${browserName() === "brave" ? "Brave" : "Chrome"} isn't connected`));
      return;
    }
    const id = mintId();
    pending.set(id, { resolve, reject });
    port.postMessage({ id, origin: chrome.runtime.id, tool, args });
  });
}

function scheduleUpdate(id) {
  const prev = timers.get(id);
  if (prev) clearTimeout(prev);
  timers.set(
    id,
    setTimeout(() => {
      timers.delete(id);
      const c = claims.find((x) => x.id === id);
      if (!c) return;
      send("tabbity.update", {
        claims: [
          {
            id: c.id,
            browser: c.browser,
            url: c.url,
            title: c.title,
            favIconUrl: c.favIconUrl,
            live: c.tabId == null ? null : { tabId: c.tabId, windowId: c.windowId, windowOrdinal: c.windowOrdinal, index: c.index },
          },
        ],
      }).catch(() => {});
    }, DEBOUNCE_MS),
  );
}

async function applyAction(action) {
  if (!action) return;
  if (action.type === "focus") {
    if (action.windowId != null) await chrome.windows.update(action.windowId, { focused: true });
    await chrome.tabs.update(action.tabId, { active: true });
  } else if (action.type === "reopen" && action.url) {
    const tab = await chrome.tabs.create({ url: action.url, active: true });
    claims = bindReopen(claims, action.claimId, tab);
    await save();
  } else if (action.type === "close") {
    await chrome.tabs.remove(action.tabId);
  } else if (action.type === "reconcile") {
    const orbitty = action.claimIds.map((id) => claims.find((c) => c.id === id) || { id });
    claims = reconcile(claims, orbitty.filter((c) => c.url || claims.some((x) => x.id === c.id)));
    await save();
  }
}

function onPortMessage(msg) {
  if (msg && msg.id && pending.has(msg.id)) {
    const wait = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.ok === false) wait.reject(new Error(msg.error || "refused"));
    else wait.resolve(msg.data ?? msg);
    return;
  }
  const next = handleCommand(claims, msg);
  claims = next.claims;
  applyAction(next.action)
    .then(() => send("tabbity.ack", { cmdId: msg.cmdId, ok: true }))
    .catch((e) => send("tabbity.ack", { cmdId: msg.cmdId, ok: false, error: String(e) }))
    .finally(() => save());
}

function connect() {
  try {
    port = chrome.runtime.connectNative(HOST);
  } catch {
    port = null;
  }
  if (!port) {
    setTimeout(connect, backoff);
    backoff = Math.min(backoff * 2, 30000);
    return;
  }
  backoff = 1000;
  port.onMessage.addListener(onPortMessage);
  port.onDisconnect.addListener(() => {
    port = null;
    setTimeout(connect, backoff);
    backoff = Math.min(backoff * 2, 30000);
  });
  const browser = browserName();
  send("tabbity.hello", { browser })
    .then((data) => {
      claims = reconcile(claims, data.claims || []);
      return chrome.tabs.query({});
    })
    .then((tabs) => chrome.windows.getAll().then((windows) => ({ tabs, windows })))
    .then(({ tabs, windows }) => {
      claims = rematch(claims, tabs, windows);
      return save();
    })
    .catch(() => {});
}

chrome.tabs.onUpdated.addListener((tabId, change, tab) => {
  const out = onUpdated(claims, tabId, change, tab);
  claims = out.claims;
  if (out.dirty) {
    save();
    const c = claims.find((x) => x.tabId === tabId);
    if (c) scheduleUpdate(c.id);
  }
});
chrome.tabs.onRemoved.addListener((tabId) => {
  claims = onRemoved(claims, tabId);
  save();
});
chrome.tabs.onReplaced.addListener((added, removed) => {
  claims = onReplaced(claims, added, removed);
  save();
});
chrome.tabs.onAttached.addListener((tabId, info) => {
  chrome.windows.getAll().then((windows) => {
    claims = onAttached(claims, tabId, info, windows);
    save();
  });
});
chrome.tabs.onDetached.addListener((tabId) => {
  claims = onDetached(claims, tabId);
  save();
});
chrome.tabs.onMoved.addListener((tabId, info) => {
  claims = onMoved(claims, tabId, info);
  save();
});

chrome.runtime.onMessage.addListener((msg, _sender, sendReply) => {
  const browser = browserName();
  if (msg.type === "planets") {
    send("tabbity.planets", {})
      .then((rows) => sendReply({ planets: rows }))
      .catch((e) => sendReply({ error: String(e) }));
    return true;
  }
  if (msg.type === "claim" || msg.type === "claim-all") {
    chrome.tabs.query(msg.type === "claim-all" ? { currentWindow: true } : { active: true, currentWindow: true }).then(async (tabs) => {
      const windows = await chrome.windows.getAll();
      const batch = tabs.map((tab) => ({
        id: mintId(),
        browser,
        url: tab.url || "",
        title: tab.title || "",
        favIconUrl: favicon(tab.favIconUrl),
        windowOrdinal: windowOrdinal(windows, tab.windowId),
        index: tab.index ?? 0,
        tabId: tab.id,
        windowId: tab.windowId,
        lastSeenAt: Date.now() / 1000,
      }));
      try {
        await send("tabbity.claim", {
          projectId: msg.projectId,
          claims: batch.map((c) => ({
            id: c.id,
            browser: c.browser,
            url: c.url,
            title: c.title,
            favIconUrl: c.favIconUrl,
          })),
        });
        claims = [...claims, ...batch];
        await save();
        sendReply({ ok: true, n: batch.length });
      } catch (e) {
        sendReply({ error: String(e) });
      }
    });
    return true;
  }
  if (msg.type === "release") {
    chrome.tabs.query({ active: true, currentWindow: true }).then(async ([tab]) => {
      const c = claims.find((x) => x.tabId === tab?.id);
      if (!c) {
        sendReply({ error: "this tab is not claimed" });
        return;
      }
      try {
        await send("tabbity.unclaim", { browser, ids: [c.id] });
        claims = claims.filter((x) => x.id !== c.id);
        await save();
        sendReply({ ok: true });
      } catch (e) {
        sendReply({ error: String(e) });
      }
    });
    return true;
  }
  return false;
});

load().then(() => connect());
