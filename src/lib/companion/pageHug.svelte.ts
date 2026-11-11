// Moved function bodies. scope fields are the lets each function already closed over.

type HostMode = "off" | "launchpad" | "stars" | "inspector";

type Invoke = <T = unknown>(cmd: string, args?: any) => Promise<T>;

type Scope_measureHugHeight = {
  mainEl: HTMLElement | null;
  overlayWinH: number;
};

type Scope_reportHug = {
  expanded: boolean;
  hosted: HostMode;
  sprite: boolean;
  measureHugHeight: (...args: any[]) => number | null;
  invoke: Invoke;
};

type Scope_reportHostHug = {
  hosted: HostMode;
  expanded: boolean;
  tilePre: boolean;
  tileEnter: boolean;
  swapPrev: HostMode | null;
  hugTimer: ReturnType<typeof setTimeout> | undefined;
  HOST_COMFORT_H: number;
  lastHostHug: number;
  mainEl: HTMLElement | null;
  invoke: Invoke;
};

type Scope_actionRowExtra = {
  cheatSheetHint: boolean;
  mainEl: HTMLElement | null;
};

type Scope_reportComposerGrow = {
  invoke: Invoke;
  composerExtra: number;
  overlayExtra: number;
  actionRowExtra: (...args: any[]) => number;
};

type Scope_toggleSprite = {
  sprite: boolean;
  expanded: boolean;
  hosted: HostMode;
  measureHugHeight: (...args: any[]) => number | null;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_toggleCaptions = {
  captions: boolean;
  expanded: boolean;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_togglePlanetsHidden = {
  planetsHidden: boolean;
  invoke: Invoke;
  sendNote: string | null;
};

type Scope_setSpeakVolume = {
  speakVolume: number;
  invoke: Invoke;
  speak: boolean;
  toggleSpeak: (...args: any[]) => Promise<any>;
  sendNote: string | null;
};

type Scope_toggleSpeak = {
  speak: boolean;
  invoke: Invoke;
  sendNote: string | null;
};

export function measureHugHeight(scope: Scope_measureHugHeight): number | null {
  if (!scope.mainEl) return null;
  const dock = scope.mainEl.querySelector(".ghostdock");
  if (!dock) return null;
  const dockR = dock.getBoundingClientRect();
  let top = dockR.top;
  scope.mainEl.querySelectorAll(".ccrow, .note, .starstrip, .model-setup-card").forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.height >= 2 && r.top < top) top = r.top;
  });
  let h = Math.round(dockR.bottom - top + 8);
  // the open reply overlay rides the
  // hug — grow the bar to its wanted window height so the fixed popup
  // fits whole (it covers the bar), never clipped to the idle bar.
  // The native side still clamps to the tall idle rect — that clamp is
  // the absurd line where the overlay keeps its inner scroll.
  if (scope.overlayWinH > 0) h = Math.max(h, scope.overlayWinH);
  return h < 120 ? 120 : h;
}

export function reportHug(scope: Scope_reportHug) {
  if (scope.expanded || scope.hosted !== "off" || scope.sprite) return;
  const h = scope.measureHugHeight();
  if (h) scope.invoke("set_companion_sprite", { enabled: false, hugHeight: h }).catch(() => {});
}

function launchpadStackHeight(main: HTMLElement | null): number {
  const well = main?.querySelector(".wellwrap") as HTMLElement | null;
  const chat = main?.querySelector(".dockchat") as HTMLElement | null;
  const dock = main?.querySelector(".ghostdock") as HTMLElement | null;
  const wellH = Math.max(well?.offsetHeight ?? 0, 320);
  const chatH = chat?.offsetHeight ?? 0;
  const dockH = dock?.offsetHeight ?? 0;
  return Math.round(wellH + 6 + chatH + 6 + dockH + 16);
}

export function reportHostHug(scope: Scope_reportHostHug) {
  if (scope.hosted === "off" || scope.expanded || scope.tilePre || scope.tileEnter || scope.swapPrev) return;
  if (scope.hugTimer) clearTimeout(scope.hugTimer);
  scope.hugTimer = setTimeout(() => {
    scope.hugTimer = undefined;
    if (scope.hosted === "off" || scope.expanded || scope.tilePre || scope.tileEnter || scope.swapPrev) return;
    // Launchpad is the board tile, the bubbles, and the dock, with the
    // same 6px gap the bubbles already keep above the dock.
    const want =
      scope.hosted === "launchpad"
        ? launchpadStackHeight(scope.mainEl)
        : scope.HOST_COMFORT_H;
    if (Math.abs(want - scope.lastHostHug) < 4) return;
    scope.lastHostHug = want;
    scope.invoke("set_companion_host_hug", { height: want }).catch(() => {});
  }, 80);
}

export function actionRowExtra(scope: Scope_actionRowExtra): number {
  if (!scope.cheatSheetHint || !scope.mainEl) return 0;
  const row = scope.mainEl.querySelector(".actionrow");
  if (!row) return 0;
  const h = row.getBoundingClientRect().height;
  return h > 2 ? Math.round(h) : 0;
}

export function reportComposerGrow(scope: Scope_reportComposerGrow) {
  scope.invoke("set_companion_composer_grow", {
    extra: scope.composerExtra + scope.overlayExtra + scope.actionRowExtra(),
  }).catch(() => {});
}

export async function toggleSprite(scope: Scope_toggleSprite, e: MouseEvent) {
  e.preventDefault();
  const next = !scope.sprite;
  scope.sprite = next;
  try {
    // SPRITE-HUG: pass the measured bar height on the off flip so the
    // native hug lands with the flag write; on never needs a height.
    const hugHeight =
      next || scope.expanded || scope.hosted !== "off" ? null : scope.measureHugHeight();
    await scope.invoke("set_companion_sprite", { enabled: next, hugHeight });
  } catch (err) {
    scope.sprite = !next;
    scope.sendNote = `${err}`;
  }
}

export async function toggleCaptions(scope: Scope_toggleCaptions) {
  const next = !scope.captions;
  const wasExpanded = scope.expanded;
  scope.captions = next;
  // Optimistic: muted bubbles never keep the big expanded chrome.
  if (!next && scope.expanded) scope.expanded = false;
  try {
    // Native captions=false also clears expanded in one save so
    // state:changed cannot reload expanded:true and fight us.
    await scope.invoke("set_companion_captions", { enabled: next });
  } catch (err) {
    scope.captions = !next;
    if (!next && wasExpanded) scope.expanded = true;
    scope.sendNote = `${err}`;
  }
}

export async function togglePlanetsHidden(scope: Scope_togglePlanetsHidden) {
  const next = !scope.planetsHidden;
  scope.planetsHidden = next;
  try {
    await scope.invoke("set_planets_hidden", { hidden: next });
  } catch (e) {
    scope.planetsHidden = !next;
    scope.sendNote = `${e}`;
  }
}

export async function setSpeakVolume(scope: Scope_setSpeakVolume, v: number) {
  const vol = Math.max(0, Math.min(1, v));
  scope.speakVolume = vol;
  try {
    await scope.invoke("set_companion_speak_volume", { volume: vol });
    if (vol > 0 && !scope.speak) await scope.toggleSpeak();
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function toggleSpeak(scope: Scope_toggleSpeak) {
  const next = !scope.speak;
  scope.speak = next;
  try {
    await scope.invoke("set_companion_speak", { enabled: next });
  } catch (e) {
    scope.speak = !next;
    scope.sendNote = `${e}`;
  }
}
