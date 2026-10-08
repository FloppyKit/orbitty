// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function toggleExpanded(scope: any) {
  scope.expanded = !scope.expanded;
  // Expanding with bubbles muted would be an empty slab — turn captions
  // back on with the expand.
  if (scope.expanded && !scope.captions) {
    scope.captions = true;
    scope.invoke("set_companion_captions", { enabled: true }).catch(() => {});
  }
  scope.invoke("set_companion_expanded", { expanded: scope.expanded }).catch(() => {});
  // HOST-CC (overrides HOST-SPLIT): expanding while hosted KEEPS the
  // well — the transcript overlays it dimmed. Never dismissHost here;
  // the native host frame (1080×620) stays up.
}

export async function enterHost(scope: any, mode: "launchpad" | "stars" | "inspector") {
  const gen = ++scope.tileGen; // a fresh enter/swap supersedes any in-flight leave
  if (scope.hosted !== "off") {
    // Swap dests: re-assert host (no second grow). Same dest does not turn.
    scope.tileReset();
    const prev = scope.hosted;
    // Different dest turns the prism; same dest (tray raise) does not.
    if (prev !== mode) {
      const from = scope.DEST_RING.indexOf(prev);
      scope.swapDir = scope.DEST_RING.indexOf(mode) === (from + 1) % scope.DEST_RING.length ? 1 : -1;
      // Prism apothem from the pane width: r = w / (2·tan(π/3)). The
      // pane is already mounted and keeps its width across a swap.
      const w = scope.wellpaneEl?.clientWidth ?? 0;
      scope.prismR = w > 16 ? Math.round(w / (2 * Math.tan(Math.PI / 3))) : 0;
      // r=0 + rotateY(120) + backface-hidden is a black card. Instant
      // swap instead of a 3D turn that cannot land.
      scope.swapPrev = scope.prismR > 0 ? prev : null;
    }
    scope.hosted = mode;
    try {
      await scope.invoke("set_companion_host", { mode });
    } catch (e) {
      scope.hosted = prev;
      scope.swapPrev = null;
      scope.sendNote = `${e}`;
      return;
    }
    // Both faces are mounted now (outgoing under swapPrev, incoming
    // under hosted) — drop the outgoing face once the CSS turn has
    // finished (timeout matches the 400ms keyframes; 140 reduced). A
    // fresh enter/swap/leave bumps tileGen and owns it.
    if (scope.swapPrev !== null) {
      const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      setTimeout(
        () => {
          if (gen === scope.tileGen) scope.swapPrev = null;
        },
        (reduce ? 140 : scope.TILE_YAW_MS) + 40,
      );
    }
    return;
  }
  // Measure the idle spirit box before native grow; hosted spirit
  // translates onto that box. Never scale about the feet.
  const idleSpirit =
    !scope.expanded && scope.sprite
      ? (scope.mainEl
          ?.querySelector(".stage .floatwrap")
          ?.getBoundingClientRect() ?? null)
      : null;
  if (idleSpirit && scope.mainEl && idleSpirit.width >= 2) {
    const win = scope.mainEl.getBoundingClientRect();
    scope.mainEl.style.setProperty(
      "--hold-x",
      `${(idleSpirit.left - win.left).toFixed(1)}px`,
    );
    scope.mainEl.style.setProperty(
      "--hold-b",
      `${(win.bottom - idleSpirit.bottom).toFixed(1)}px`,
    );
    scope.mainEl.style.setProperty("--hold-w", `${idleSpirit.width.toFixed(1)}px`);
    scope.mainEl.style.setProperty("--hold-h", `${idleSpirit.height.toFixed(1)}px`);
    scope.spiritHold = true;
  }
  scope.tilePre = true;
  // Hold class + native grow in parallel — sequential tick-then-invoke
  // was the empty pause before the sprite moved.
  const grow = scope.invoke("set_companion_host", { mode });
  try {
    await Promise.all([scope.tick(), grow]);
  } catch (e) {
    scope.tileReset();
    scope.spiritHold = false;
    scope.sendNote = `${e}`;
    return;
  }
  scope.hosted = mode;
  await scope.tick();
  let shifted = false;
  if (idleSpirit && scope.mainEl && idleSpirit.width >= 2) {
    const art =
      scope.mainEl.querySelector(".wellhead .floatwrap") ??
      scope.mainEl.querySelector(".wellhead .spirshift");
    const rest = art?.getBoundingClientRect();
    if (rest && rest.width >= 2 && rest.height >= 2) {
      const dx =
        idleSpirit.left + idleSpirit.width / 2 - (rest.left + rest.width / 2);
      const dy = idleSpirit.bottom - rest.bottom;
      const s = idleSpirit.width / rest.width;
      scope.mainEl.style.setProperty("--spir-dx", `${dx.toFixed(1)}px`);
      scope.mainEl.style.setProperty("--spir-dy", `${dy.toFixed(1)}px`);
      scope.mainEl.style.setProperty("--spir-s", Number.isFinite(s) ? s.toFixed(3) : "1");
      shifted = true;
    }
  }
  if (!shifted && scope.mainEl) {
    scope.mainEl.style.removeProperty("--spir-dx");
    scope.mainEl.style.removeProperty("--spir-dy");
    scope.mainEl.style.removeProperty("--spir-s");
  }
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  scope.tilePre = false;
  scope.tileEnter = true;
  scope.spiritHold = false;
  setTimeout(
    () => {
      if (gen !== scope.tileGen) return;
      scope.tileEnter = false;
    },
    (reduce ? 140 : scope.TILE_ENTER_MS) + 80,
  );
  // Force collapsed on enter (the existing persisted expanded flag).
  if (scope.expanded) {
    scope.expanded = false;
    scope.invoke("set_companion_expanded", { expanded: false }).catch(() => {});
  }
}

export async function dismissHost(scope: any) {
  if (scope.hosted === "off" || scope.tileLeave) return;
  // ⌘⌥↑ leftover: remember what is being lowered (written just before
  // `hosted = "off"` below; a superseded leave never completes the
  // lower, so it never writes).
  const lowered = scope.hosted;
  const gen = ++scope.tileGen;
  scope.tilePre = false;
  scope.tileEnter = false;
  scope.spiritHold = false;
  // A mid-drop dismiss supersedes the enter timeout; the spirit is
  // already in the hosted column (no hold overlay to release).
  scope.swapPrev = null; // a mid-turn dismiss drops the outgoing face
  // Reverse the drop in the grown frame; native idle restore waits
  // for CSS. Never restore mid-animation.
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!reduce) {
    scope.tileLeave = true;
    await new Promise((r) => setTimeout(r, scope.TILE_LEAVE_MS));
    if (gen !== scope.tileGen) return; // a fresh enter/swap superseded us
    scope.tileLeave = false;
  }
  scope.lastDest = lowered;
  scope.hosted = "off";
  try {
    await scope.invoke("set_companion_host", { mode: "off" });
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function refreshDestOpen(scope: any) {
  try {
    scope.destOpen = await scope.invoke<{
      launchpad: boolean;
      stars: boolean;
      inspector: boolean;
    }>("dest_open_state");
  } catch {
    /* chip paint retries on the next pane-vis */
  }
}

export async function toggleDest(scope: any, mode: "launchpad" | "stars" | "inspector") {
  if (scope.hosted !== "off") await scope.dismissHost();
  try {
    await scope.invoke("dest_toggle", { dest: mode });
  } catch (e) {
    scope.sendNote = `${e}`;
  }
  await scope.refreshDestOpen();
}

export async function popoutLaunchpad(scope: any) {
  if (scope.hosted !== "launchpad") return;
  scope.tileGen++; // cancel any in-flight leave; pop-out stays instant
  scope.tileReset();
  scope.hosted = "off";
  try {
    await scope.invoke("set_companion_host", { mode: "off" });
    await scope.invoke("open_kanban");
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function popoutInspector(scope: any) {
  if (scope.hosted !== "inspector") return;
  scope.tileGen++; // cancel any in-flight leave; pop-out stays instant
  scope.tileReset();
  scope.hosted = "off";
  try {
    await scope.invoke("set_companion_host", { mode: "off" });
    await scope.invoke("show_rail");
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export function restartTimer(scope: any) {
  if (scope.timer) clearInterval(scope.timer);
  // Reduced motion: hold frame 0 of the active set (rest or thinking).
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  scope.timer = setInterval(() => {
    const n = scope.drawFrames.length;
    if (n > 1) scope.frameIdx = (scope.frameIdx + 1) % n;
  }, scope.frameMs);
}

export async function sliceSheet(scope: any, sheet: CompanionSheet): Promise<string[]> {
  const img = new Image();
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("sheet decode failed"));
    img.src = sheet.dataUrl;
  });
  const c = document.createElement("canvas");
  c.width = sheet.frameW;
  c.height = sheet.frameH;
  const ctx = c.getContext("2d");
  if (!ctx) return [];
  ctx.imageSmoothingEnabled = false;
  const out: string[] = [];
  for (let i = 0; i < sheet.count; i++) {
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.drawImage(
      img,
      i * sheet.frameW,
      0,
      sheet.frameW,
      sheet.frameH,
      0,
      0,
      sheet.frameW,
      sheet.frameH,
    );
    out.push(c.toDataURL("image/png"));
  }
  return out;
}

export async function loadArt(scope: any) {
  const packId =
    scope.bodyId === "spirit" || scope.bodyId === "female"
      ? null
      : scope.bodyId;
  try {
    const art = await scope.invoke<CompanionArt>("companion_frames", {
      packId,
    });
    let f = art.frames ?? [];
    if (f.length === 0 && art.sheet) f = await scope.sliceSheet(art.sheet);
    if (f.length === 0) throw new Error("no frames");
    scope.frameMs = art.frameMs > 0 ? art.frameMs : 140;
    scope.artSmooth = !!art.smooth;
    scope.frameIdx = 0;
    scope.restFrames = f;
    scope.thinkFrames = art.thinkingFrames ?? [];
    scope.restBust = art.bustFrames ?? [];
    scope.thinkBust = art.bustThinkingFrames ?? [];
    scope.missing = false;
    scope.restartTimer();
  } catch {
    if (packId) {
      scope.bodyId = "spirit";
      try {
        const art = await scope.invoke<CompanionArt>("companion_frames", {
          packId: null,
        });
        let f = art.frames ?? [];
        if (f.length === 0 && art.sheet) f = await scope.sliceSheet(art.sheet);
        if (f.length === 0) throw new Error("no frames");
        scope.frameMs = art.frameMs > 0 ? art.frameMs : 140;
        scope.artSmooth = false;
        scope.frameIdx = 0;
        scope.restFrames = f;
        scope.thinkFrames = art.thinkingFrames ?? [];
        scope.restBust = art.bustFrames ?? [];
        scope.thinkBust = art.bustThinkingFrames ?? [];
        scope.missing = false;
        scope.restartTimer();
        scope.artLoaded = true;
        return;
      } catch {
        /* fall through to missing */
      }
    }
    scope.restFrames = [];
    scope.thinkFrames = [];
    scope.restBust = [];
    scope.thinkBust = [];
    scope.missing = true;
  }
  scope.artLoaded = true;
}

export async function onGlbFail(scope: any) {
  if (scope.glbFailed) return;
  scope.glbFailed = true;
  scope.sendNote = "3D body failed; back on Spirit.";
  scope.bodyId = "spirit";
  try {
    await scope.invoke<string | null>("set_companion_pack", { packId: "" });
  } catch {
    /* the local revert stands; the next state:changed re-syncs */
  }
}

export async function loadState(scope: any) {
  try {
    const s = await scope.invoke<PersistedState>("load_state");
    scope.themeId = scope.KNOWN_THEMES.has(s.themeId) ? s.themeId : scope.osThemeId();
    scope.windowOpacity = scope.clampWindowOpacity(s.windowOpacity ?? 1);
    // sendLine already swapped the you-row to
    // the line just sent; while sending, a state:changed reload must
    // not clobber it with the STALE persisted lastUserLine (still the
    // previous prompt — the native side writes the new one only when
    // the reply lands). Persisted truth reapplies on the next load
    // once sending is false.
    if (!scope.sending) scope.userLine = s.companion?.lastUserLine ?? "";
    scope.botLine = s.companion?.lastBotLine ?? "";
    const incoming = s.companion?.turns ?? [];
    if (scope.sending) {
      const optimistic = scope.turns.at(-1);
      scope.turns = incoming;
      if (
        optimistic?.role === "you" &&
        scope.turns.at(-1)?.text !== optimistic.text
      ) {
        scope.turns = [...scope.turns, optimistic].slice(-40);
      }
    } else {
      scope.turns = incoming;
    }
    scope.expanded = s.companion?.expanded ?? false;
    scope.speak = s.companion?.speakReplies ?? false;
    scope.speakVolume = typeof s.companion?.speakVolume === "number"
      ? Math.max(0, Math.min(1, s.companion.speakVolume))
      : 1;
    scope.figureView = s.companion?.viewCollapsed === "head" ? "head" : "full";
    scope.sprite = s.companion?.sprite ?? true;
    scope.captions = s.companion?.captions ?? true;
    scope.planetsHidden = s.companion?.planetsHidden ?? false;
    scope.cheatSheetHint = s.companion?.cheatSheetHint ?? true;
    if (scope.maybeGraduatePeerChrome(s)) {
      scope.peerChrome = true;
      scope.invoke("set_peer_chrome", { on: true }).catch(() => {});
    } else {
      scope.peerChrome = scope.peerChromeOn(s);
    }
    // DOCK-HACK: missing keys (old saves) = dark LED, unknown source.
    scope.hackLive = s.companion?.hackLive ?? false;
    scope.hackWho = s.companion?.hackWho ?? "";
    const pack = s.companion?.imagePackId ?? null;
    const nextBody = pack && pack.length > 0 ? pack : "spirit";
    if (nextBody !== scope.bodyId) {
      scope.bodyId = nextBody;
      scope.glbFailed = false; // a fresh pick re-tries the 3D load
      scope.artLoaded = false;
      void scope.loadArt();
    }
    scope.stars = s.stars ?? { promptsOptIn: false, checkins: {} };
    scope.prompts = scope.stars.promptsOptIn ?? false;
    scope.cards = s.kanbanCards ?? [];
    void scope.refreshDestOpen();
  } catch {
    // keep last good state; next state:changed retries
  } finally {
    scope.captionReady = true;
  }
}
