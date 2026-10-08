// @ts-nocheck
// Moved function bodies. scope fields are the lets each function already closed over.
export function measureHugHeight(scope: any): number | null {
  if (!scope.mainEl) return null;
  const dock = scope.mainEl.querySelector(".ghostdock");
  if (!dock) return null;
  const dockR = dock.getBoundingClientRect();
  let top = dockR.top;
  scope.mainEl.querySelectorAll(".ccrow, .note, .starstrip").forEach((el) => {
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

export function reportHug(scope: any) {
  if (scope.expanded || scope.hosted !== "off" || scope.sprite) return;
  const h = scope.measureHugHeight();
  if (h) scope.invoke("set_companion_sprite", { enabled: false, hugHeight: h }).catch(() => {});
}

export function reportHostHug(scope: any) {
  if (scope.hosted === "off" || scope.expanded || scope.tilePre || scope.tileEnter || scope.swapPrev) return;
  if (scope.hugTimer) clearTimeout(scope.hugTimer);
  scope.hugTimer = setTimeout(() => {
    scope.hugTimer = undefined;
    if (scope.hosted === "off" || scope.expanded || scope.tilePre || scope.tileEnter || scope.swapPrev) return;
    if (Math.abs(scope.HOST_COMFORT_H - scope.lastHostHug) < 4) return;
    scope.lastHostHug = scope.HOST_COMFORT_H;
    scope.invoke("set_companion_host_hug", { height: scope.HOST_COMFORT_H }).catch(() => {});
  }, 80);
}

export function actionRowExtra(scope: any): number {
  if (!scope.cheatSheetHint || !scope.mainEl) return 0;
  const row = scope.mainEl.querySelector(".actionrow");
  if (!row) return 0;
  const h = row.getBoundingClientRect().height;
  return h > 2 ? Math.round(h) : 0;
}

export function reportComposerGrow(scope: any) {
  scope.invoke("set_companion_composer_grow", {
    extra: scope.composerExtra + scope.overlayExtra + scope.actionRowExtra(),
  }).catch(() => {});
}

export async function toggleSprite(scope: any, e: MouseEvent) {
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

export async function toggleCaptions(scope: any) {
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

export async function togglePlanetsHidden(scope: any) {
  const next = !scope.planetsHidden;
  scope.planetsHidden = next;
  try {
    await scope.invoke("set_planets_hidden", { hidden: next });
  } catch (e) {
    scope.planetsHidden = !next;
    scope.sendNote = `${e}`;
  }
}

export async function setSpeakVolume(scope: any, v: number) {
  const vol = Math.max(0, Math.min(1, v));
  scope.speakVolume = vol;
  try {
    await scope.invoke("set_companion_speak_volume", { volume: vol });
    if (vol > 0 && !scope.speak) await scope.toggleSpeak();
  } catch (e) {
    scope.sendNote = `${e}`;
  }
}

export async function toggleSpeak(scope: any) {
  const next = !scope.speak;
  scope.speak = next;
  try {
    await scope.invoke("set_companion_speak", { enabled: next });
  } catch (e) {
    scope.speak = !next;
    scope.sendNote = `${e}`;
  }
}
