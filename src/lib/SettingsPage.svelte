<script lang="ts">
  // Gear pops this window (same pattern as STARS).
  // Load → mutate settings fields → save whole state so rail slices survive.
  // Talk source uses set_companion_talk_source (companion slice).
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { startPaneDrag } from "./dragPane";
  import { onMount, onDestroy } from "svelte";
  import { MenuRing, type RingDir } from "./menuKeys.svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import "./chrome.css";
  import MouthSettings from "./MouthSettings.svelte";
  import SettingsAppearance from "./SettingsAppearance.svelte";
  import SettingsPacks from "./SettingsPacks.svelte";
  import SettingsPlans from "./SettingsPlans.svelte";
  import type { PersistedState } from "./persist";
  import { RAIL_EDGES, stagePrefs, type RailEdge } from "./rail/habitat";
  import { maybeGraduatePeerChrome, peerChromeOn } from "./peerChrome";

  interface PackInfo {
    id: string;
    name: string;
    license: string | null;
    warnLicense: boolean;
    frameCount: number;
    path: string;
    role: string;
  }
  interface BodyPack {
    id: string;
    name: string;
  }
  interface Theme {
    id: string;
    name: string;
    block: "System" | "Custom";
  }
  // Custom holds Terminal only; the seven old
  // palette skins are gone and a dead saved id falls back to the OS pair.
  const THEMES: Theme[] = [
    { id: "systemLight", name: "Light", block: "System" },
    { id: "systemDark", name: "Dark", block: "System" },
    { id: "terminal", name: "Terminal", block: "Custom" },
  ];
  const BLOCKS = ["System", "Custom"] as const;
  const THEME_IDS = new Set(THEMES.map((t) => t.id));

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  // Seed the OS theme synchronously so --bg exists
  // before load_state lands — an empty themeId on an opaque drawer is a
  // white-on-white first paint. themeChosen stays false until a saved id
  // or an explicit pick, so the seed is never persisted as a choice.
  let {
    embedded = false,
    onClose,
    onPopOut,
    ringActive = false,
    onRingExit,
    outwardDir = "left" as RingDir,
  }: {
    embedded?: boolean;
    onClose?: () => void;
    onPopOut?: () => void;
    ringActive?: boolean;
    onRingExit?: () => void;
    outwardDir?: RingDir;
  } = $props();

  const menuRing = new MenuRing();
  let settingsRoot = $state<HTMLElement | null>(null);
  let detachRing: (() => void) | undefined;

  let themeId = $state(osThemeId());
  let themeChosen = $state(false);
  let windowOpacity = $state(1.0);
  let spriteScale = $state(1);
  let autohideRowChrome = $state(true);
  let railEdge = $state<RailEdge>("left");
  let railAutoHideOnBlur = $state(true);
  let hideTitles = $state(false);
  let hideAppIcons = $state(false);
  let peerChrome = $state(false);
  let marbleDrift = $state(false);
  let planetIdle = $state("4h");
  let talkSource = $state<"system" | "hotkey">("system");
  let cheatSheetHint = $state(true);
  let section = $state<"appearance" | "packs" | "plans">("appearance");
  let plansDir = $state("");
  let plansRunner = $state("");
  let plansModel = $state("");
  let plansHarness = $state("");
  let plansNote = $state("");
  // Companion body picker (Appearance): Spirit + role:companion packs.
  // Same persist as the old companion-row select — set_companion_pack
  // emits state:changed, and the companion window hot-swaps the art.
  let bodyPacks = $state<BodyPack[]>([]);
  let bodyId = $state("spirit");
  let bodyBusy = $state(false);
  let spendCapUsd = $state(20);
  let spendCapEnabled = $state(true);
  let runnerCap = $state(2);
  let treeGrowDays = $state(14);
  let signedLocally = $state(true);
  let privateRelay = $state(false);
  let privateRelayUrl = $state("");
  let replicaUrlNote = $state("");
  interface LinkApprovalRow {
    target: string;
    kind: string;
    approvedAt: string;
  }
  let linkApprovals = $state<LinkApprovalRow[]>([]);

  let packs = $state<PackInfo[]>([]);
  let packsDir = $state("");
  let packNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let packBusy = $state(false);


  function clampSpriteScale(n: number) {
    if (!Number.isFinite(n)) return 1;
    return Math.min(1.25, Math.max(0.5, n));
  }

  function normalizeTalk(raw: string | undefined): "system" | "hotkey" {
    const s = (raw ?? "").trim().toLowerCase();
    return s === "hotkey" || s === "handy" ? "hotkey" : "system";
  }

  async function loadPlans() {
    try {
      const s = await invoke<{
        plansDir: string;
        runner: string;
        harness: string;
        model: string;
      }>("plans_settings");
      plansDir = s.plansDir ?? "";
      plansRunner = s.runner ?? "";
      plansHarness = s.harness ?? "";
      plansModel = s.model ?? "";
    } catch (e) {
      plansNote = String(e);
    }
  }

  async function revealPlans() {
    plansNote = "";
    try {
      await invoke("plans_reveal");
    } catch (e) {
      plansNote = String(e);
    }
  }

  async function saveRunner() {
    plansNote = "";
    try {
      await invoke("plans_set_runner", {
        runner: plansRunner.trim(),
        harness: plansHarness.trim(),
        model: plansModel.trim(),
        permissions: "workspace-write",
      });
      plansNote = "Saved.";
    } catch (e) {
      plansNote = String(e);
    }
  }

  async function savePlansDir() {
    plansNote = "";
    try {
      const s = await invoke<{ plansDir: string }>("plans_set_dir", { path: plansDir });
      plansDir = s.plansDir ?? plansDir;
    } catch (e) {
      plansNote = String(e);
    }
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      const migrated = THEME_IDS.has(s.themeId) ? s.themeId : "";
      if (migrated) {
        themeId = migrated;
        themeChosen = true;
      } else {
        themeId = osThemeId();
        themeChosen = false;
      }
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      spriteScale = clampSpriteScale(s.spriteScale ?? 1);
      autohideRowChrome = s.autohideRowChrome ?? true;
      {
        const prefs = stagePrefs(s);
        railEdge = prefs.railEdge;
        railAutoHideOnBlur = prefs.railAutoHideOnBlur;
      }
      hideTitles = s.hideTitles ?? false;
      hideAppIcons = s.hideAppIcons ?? false;
      marbleDrift = s.marbleDrift ?? false;
      if (maybeGraduatePeerChrome(s)) {
        peerChrome = true;
        void invoke("set_peer_chrome", { on: true }).catch(() => {});
      } else {
        peerChrome = peerChromeOn(s);
      }
      planetIdle = ["1h", "4h", "12h", "never"].includes(s.planetIdle ?? "")
        ? (s.planetIdle as "1h" | "4h" | "12h" | "never")
        : "4h";
      talkSource = normalizeTalk(s.companion?.talkSource);
      cheatSheetHint = s.companion?.cheatSheetHint ?? true;
      const pack = s.companion?.imagePackId ?? null;
      bodyId = pack && pack.length > 0 ? pack : "spirit";
      spendCapUsd = Number.isFinite(s.spendCapUsd) && (s.spendCapUsd ?? 0) > 0 ? Number(s.spendCapUsd) : 20;
      spendCapEnabled = s.spendCapEnabled ?? true;
      const rc = Number(s.runnerCap ?? 2);
      runnerCap = Math.min(4, Math.max(1, Number.isFinite(rc) ? rc : 2));
      const gd = Number(s.treeGrowDays ?? 14);
      treeGrowDays = Number.isFinite(gd) && gd > 0 ? gd : 14;
      signedLocally = s.signedLocally ?? true;
      privateRelay = s.privateRelay ?? false;
      privateRelayUrl = s.privateRelayUrl ?? "";
    } catch {
      themeId = osThemeId();
    }
  }

  async function onBodyChange() {
    if (bodyBusy) return;
    bodyBusy = true;
    try {
      const saved = await invoke<string | null>("set_companion_pack", {
        packId: bodyId === "spirit" ? "" : bodyId,
      });
      bodyId = saved && saved.length > 0 ? saved : "spirit";
    } catch {
      bodyId = "spirit";
    }
    bodyBusy = false;
  }

  async function persistSettings() {
    try {
      const s = await invoke<PersistedState>("load_state");
      s.themeId = themeChosen ? themeId : "";
      s.windowOpacity = windowOpacity;
      s.spriteScale = spriteScale;
      s.autohideRowChrome = autohideRowChrome;
      s.hideTitles = hideTitles;
      s.hideAppIcons = hideAppIcons;
      s.marbleDrift = marbleDrift;
      s.peerChrome = peerChrome;
      s.planetIdle = planetIdle;
      if (s.companion) s.companion.cheatSheetHint = cheatSheetHint;
      s.spendCapUsd = spendCapUsd;
      s.spendCapEnabled = spendCapEnabled;
      s.runnerCap = runnerCap;
      s.treeGrowDays = treeGrowDays;
      s.signedLocally = signedLocally;
      s.privateRelay = privateRelay;
      s.privateRelayUrl = privateRelayUrl;
      await invoke("save_state", { state: s });
    } catch {
      /* next mutation retries */
    }
  }

  function pickTheme(id: string) {
    themeChosen = true;
    themeId = id;
    persistSettings();
  }

  function onOpacityInput() {
    windowOpacity = clampWindowOpacity(windowOpacity);
    applyWindowOpacity(windowOpacity);
  }
  function onOpacityCommit() {
    onOpacityInput();
    persistSettings();
  }
  function onSpriteScaleInput() {
    spriteScale = clampSpriteScale(spriteScale);
  }
  function onSpriteScaleCommit() {
    onSpriteScaleInput();
    persistSettings();
  }

  async function saveRailStage(edge: RailEdge, autoHide: boolean) {
    const prefs = stagePrefs({
      railEdge: edge,
      railAutoHideOnBlur: autoHide,
      autohideRowChrome,
    });
    railEdge = prefs.railEdge;
    railAutoHideOnBlur = prefs.railAutoHideOnBlur;
    try {
      await invoke("set_rail_stage_prefs", {
        edge: prefs.railEdge,
        autoHideOnBlur: prefs.railAutoHideOnBlur,
      });
    } catch {
      /* the next click retries */
    }
  }

  function pickRailEdge(edge: RailEdge) {
    void saveRailStage(edge, railAutoHideOnBlur);
  }

  function toggleRailAutoHide() {
    void saveRailStage(railEdge, !railAutoHideOnBlur);
  }

  function toggleAutohide() {
    autohideRowChrome = !autohideRowChrome;
    persistSettings();
  }
  function toggleHideTitles() {
    hideTitles = !hideTitles;
    persistSettings();
  }
  function toggleHideIcons() {
    hideAppIcons = !hideAppIcons;
    persistSettings();
  }
  function togglePeerChrome() {
    peerChrome = !peerChrome;
    void persistPeerChrome();
  }

  async function persistPeerChrome() {
    try {
      await invoke("set_peer_chrome", { on: peerChrome });
    } catch {
      /* next open retries */
    }
  }

  function toggleMarbleDrift() {
    marbleDrift = !marbleDrift;
    persistSettings();
  }
  function pickPlanetIdle(id: string) {
    planetIdle = id;
    persistSettings();
  }
  function toggleCheatHint() {
    cheatSheetHint = !cheatSheetHint;
    persistSettings();
  }
  function toggleSignedLocally() {
    signedLocally = !signedLocally;
    persistSettings();
  }
  function togglePrivateRelay() {
    privateRelay = !privateRelay;
    persistSettings();
  }
  function replicaUrlLooksOk(raw: string): boolean {
    if (!raw) return true;
    try {
      const u = new URL(raw);
      if (u.protocol !== "ws:") return false;
      const host = u.hostname;
      if (host === "127.0.0.1" || host === "localhost" || host === "::1") return true;
      const m = /^(\d+)\.(\d+)\.(\d+)\.(\d+)$/.exec(host);
      if (!m) return false;
      const a = Number(m[1]);
      const b = Number(m[2]);
      return a === 100 && (b & 0xc0) === 64;
    } catch {
      return false;
    }
  }
  function onReplicaUrlCommit() {
    privateRelayUrl = privateRelayUrl.trim();
    if (!replicaUrlLooksOk(privateRelayUrl)) {
      replicaUrlNote =
        "ws://100.x:13120 or ws://127.0.0.1:port for SSH -L. Empty = local only.";
      return;
    }
    replicaUrlNote = "";
    persistSettings();
  }

  async function pickTalk(src: "system" | "hotkey") {
    talkSource = src;
    try {
      const saved = await invoke<string>("set_companion_talk_source", {
        source: src,
      });
      talkSource = normalizeTalk(saved);
    } catch {
      /* state:changed restores */
    }
  }

  async function refreshPacks() {
    try {
      packs = await invoke<PackInfo[]>("list_packs");
      packsDir = await invoke<string>("packs_dir");
    } catch {
      /* decorative */
    }
    try {
      bodyPacks = await invoke<BodyPack[]>("list_companion_packs");
    } catch {
      bodyPacks = [];
    }
  }
  async function importPackFolder() {
    if (packBusy) return;
    packBusy = true;
    packNote = null;
    try {
      const p = await invoke<PackInfo | null>("import_pack_folder");
      if (p) {
        packNote = {
          kind: "ok",
          text: `${p.name}: ${p.frameCount} frame${p.frameCount === 1 ? "" : "s"}${p.warnLicense ? " — no license in manifest, confirm rights" : ""}`,
        };
        await refreshPacks();
        await persistSettings();
      }
    } catch (e) {
      packNote = { kind: "err", text: `${e}` };
    }
    packBusy = false;
  }
  async function importPackZip() {
    if (packBusy) return;
    packBusy = true;
    packNote = null;
    try {
      const p = await invoke<PackInfo | null>("import_pack_zip");
      if (p) {
        packNote = {
          kind: "ok",
          text: `${p.name}: ${p.frameCount} frame${p.frameCount === 1 ? "" : "s"}${p.warnLicense ? " — no license in manifest, confirm rights" : ""}`,
        };
        await refreshPacks();
        await persistSettings();
      }
    } catch (e) {
      packNote = { kind: "err", text: `${e}` };
    }
    packBusy = false;
  }
  async function importPackImage() {
    if (packBusy) return;
    packBusy = true;
    packNote = null;
    try {
      const p = await invoke<PackInfo | null>("import_pack_image");
      if (p) {
        packNote = {
          kind: "ok",
          text: `${p.name}: ${p.frameCount} frame${p.frameCount === 1 ? "" : "s"}${p.warnLicense ? " — no license in manifest, confirm rights" : ""}`,
        };
        await refreshPacks();
        await persistSettings();
      }
    } catch (e) {
      packNote = { kind: "err", text: `${e}` };
    }
    packBusy = false;
  }
  async function removePack(id: string) {
    try {
      await invoke("remove_pack", { id });
      await refreshPacks();
      await persistSettings();
    } catch (e) {
      packNote = { kind: "err", text: `${e}` };
    }
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    if (!embedded) invoke("ensure_glass").catch(() => {});
  });
  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  const onVisibility = () => {
    if (!document.hidden) {
      loadState();
      refreshPacks();
      loadApprovals();
    }
  };
  let unlisten: (() => void) | undefined;
  let unlistenFocus: (() => void) | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  onMount(async () => {
    if (!embedded) invoke("ensure_glass").catch(() => {});
    await loadState();
    await loadPlans();
    await refreshPacks();
    await loadApprovals();
    unlisten = await listen("state:changed", () => {
      loadState();
    });
    document.addEventListener("visibilitychange", onVisibility);
    if (embedded) return;
    const win = getCurrentWindow();
    unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        if (hideTimer) clearTimeout(hideTimer);
        hideTimer = undefined;
        return;
      }
      hideTimer = setTimeout(() => {
        invoke("hide_window", { label: "settings" }).catch(() => {});
      }, 220);
    });
    window.addEventListener("keydown", onSettingsKey);
  });
  function onSettingsKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      invoke("hide_window", { label: "settings" }).catch(() => {});
    }
  }
  $effect(() => {
    if (!embedded || !ringActive) return;
    const root = settingsRoot;
    if (!root) return;
    const outward = outwardDir;
    const els = [...root.querySelectorAll<HTMLElement>("button, a[href], input, select, textarea")].filter(
      (el) => !el.closest(".s-banner") && el.tabIndex !== -1 && !(el as HTMLButtonElement).disabled,
    );
    els.forEach((el, i) => {
      el.dataset.mkey = `set-stop-${i}`;
    });
    menuRing.setStops(
      els.map((el, i) => ({
        id: `set-stop-${i}`,
        confirm: () => el.click(),
      })),
    );
    menuRing.onEdge = (dir) => {
      if (dir !== outward) return false;
      onRingExit?.();
      return true;
    };
    if (els[0]) menuRing.select("set-stop-0");
    detachRing?.();
    detachRing = menuRing.attach(() => {
      onRingExit?.();
      return true;
    });
    return () => {
      detachRing?.();
      detachRing = undefined;
      menuRing.onEdge = null;
    };
  });

  $effect(() => {
    if (!embedded || !ringActive) return;
    const id = menuRing.activeId;
    const root = settingsRoot;
    if (!id || !root) return;
    const el = root.querySelector(`[data-mkey="${CSS.escape(id)}"]`);
    if (el instanceof HTMLElement && document.activeElement !== el) el.focus();
  });

  onDestroy(() => {
    if (hideTimer) clearTimeout(hideTimer);
    unlisten?.();
    unlistenFocus?.();
    window.removeEventListener("keydown", onSettingsKey);
    document.removeEventListener("visibilitychange", onVisibility);
  });

  // Quiet Appearance-footer button — the same
  // confirm ⌘⇧Y opens, in its own small window (works with Settings
  // closed).
  async function loadApprovals() {
    try {
      linkApprovals = await invoke<LinkApprovalRow[]>("list_link_approvals");
    } catch {
      linkApprovals = [];
    }
  }

  async function removeApproval(target: string) {
    try {
      await invoke("remove_link_approval", { target });
    } catch {
      /* the list reloads either way */
    }
    await loadApprovals();
  }

  async function clearApprovals() {
    try {
      await invoke("clear_link_approvals");
    } catch {
      /* the list reloads either way */
    }
    await loadApprovals();
  }

  function openYardWarn() {
    invoke("open_yard_warn_window").catch(() => {});
  }

  function onBannerMouseDown(e: MouseEvent) {
    startPaneDrag(e);
  }
</script>

<main class="settings" class:embedded bind:this={settingsRoot}>
  <!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click.
       Embedded in the rail, this banner does not drag the stage window. -->
  <header class="s-banner" onmousedown={(e) => { if (!embedded) onBannerMouseDown(e); }}>
    <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">tty</span></span> Settings</h1>
    {#if embedded}
      <span class="embed-actions">
        <button type="button" class="mini" onclick={() => onPopOut?.()}>Pop out</button>
        <button type="button" class="mini ghost" aria-label="Close settings" onclick={() => onClose?.()}>×</button>
      </span>
    {/if}
  </header>

  <nav class="tabs" aria-label="Settings sections">
    <button
      class="tab"
      class:on={section === "appearance"}
      onclick={() => (section = "appearance")}>Appearance</button
    >
    <button
      class="tab"
      class:on={section === "packs"}
      onclick={() => (section = "packs")}>Packs</button
    >
    <button
      class="tab"
      class:on={section === "plans"}
      onclick={() => (section = "plans")}>Plans</button
    >
  </nav>

  <div class="s-body">
    {#if section === "appearance"}
      <SettingsAppearance {BLOCKS} {THEMES} {themeId} {pickTheme} bind:windowOpacity {onOpacityInput} {onOpacityCommit} bind:spriteScale {onSpriteScaleInput} {onSpriteScaleCommit} {autohideRowChrome} {toggleAutohide} {hideTitles} {toggleHideTitles} {hideAppIcons} {toggleHideIcons} {RAIL_EDGES} {railEdge} {pickRailEdge} {railAutoHideOnBlur} {toggleRailAutoHide} {peerChrome} {togglePeerChrome} {marbleDrift} {toggleMarbleDrift} {planetIdle} {pickPlanetIdle} bind:bodyId {onBodyChange} {bodyBusy} {bodyPacks} {talkSource} {pickTalk} bind:spendCapUsd bind:spendCapEnabled {persistSettings} bind:runnerCap bind:treeGrowDays {signedLocally} {toggleSignedLocally} {privateRelay} {togglePrivateRelay} bind:privateRelayUrl {onReplicaUrlCommit} {replicaUrlNote} {cheatSheetHint} {toggleCheatHint} {linkApprovals} {removeApproval} {clearApprovals} {openYardWarn} />
    {:else if section === "packs"}
      <SettingsPacks {packBusy} {importPackFolder} {importPackZip} {importPackImage} {packsDir} {packNote} {packs} {removePack} />
    {:else if section === "plans"}
      <SettingsPlans bind:plansDir {savePlansDir} {revealPlans} bind:plansRunner bind:plansHarness bind:plansModel {saveRunner} {plansNote} />
    {/if}
  </div>
</main>

<style>
.settings.embedded {
    height: 100%;
    min-height: 0;
  }
.settings.embedded .s-banner {
    cursor: default;
  }
.embed-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }
.settings {
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border-radius: 12px;
    font-family:
      "SF Pro Text", "SF Pro Display", system-ui, -apple-system,
      BlinkMacSystemFont, "Helvetica Neue", sans-serif;
  }
.s-banner {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 14px 0 10px;
    border-bottom: 1px solid var(--edge);
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }
.s-banner:active {
    cursor: grabbing;
  }
.s-banner h1 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
.tabs {
    flex-shrink: 0;
    display: flex;
    gap: 4px;
    padding: 8px 12px 0;
  }
.tab {
    font: inherit;
    font-size: 0.72rem;
    color: var(--dim);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }
.tab.on {
    color: var(--accent);
    border-color: var(--edge);
    background: var(--panel-fill, var(--panel));
    font-weight: 700;
  }
.s-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 12px 16px;
  }
.mini {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
  }
.mini:hover {
    border-color: var(--accent);
  }
.mini.ghost {
    background: transparent;
  }
.mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
/* The actions face — one monospace list. */
  .action-list {
    list-style: none;
    margin: 2px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
.action-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-height: 22px;
  }
.a-name {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: inherit;
    font-size: 0.74rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 2px 6px;
    cursor: pointer;
  }
.a-name:hover {
    color: var(--accent);
    background: var(--bg-fill, var(--bg));
  }
.a-name.running {
    opacity: 0.55;
  }
.a-combo {
    flex-shrink: 0;
    min-width: 44px;
    text-align: center;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 5px;
    padding: 1px 6px;
    cursor: pointer;
  }
.a-combo:hover {
    border-color: var(--accent);
  }
.a-combo.rec {
    color: var(--accent);
    border-color: var(--accent);
  }
.a-conflict {
    flex-shrink: 0;
    font-size: 0.62rem;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0 0.35rem;
  }
.a-cmd {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--dim);
  }
.help code {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.62rem;
  }
</style>
