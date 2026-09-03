<script lang="ts">
  // GO-ORBITTY-SETTINGS-WIN: gear pops this window (same pattern as STARS).
  // Load → mutate settings fields → save whole state so rail slices survive.
  // Talk source uses set_companion_talk_source (companion slice).
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { onMount, onDestroy } from "svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import type { DrawerSlice, PersistedState } from "./persist";

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
  // GO-ORBITTY-COMPANION-BRAND: Custom holds Terminal only; the seven old
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

  // GO-ORBITTY-PANE-PAINT: seed the OS theme synchronously so --bg exists
  // before load_state lands — an empty themeId on an opaque drawer is a
  // white-on-white first paint. themeChosen stays false until a saved id
  // or an explicit pick, so the seed is never persisted as a choice.
  let themeId = $state(osThemeId());
  let themeChosen = $state(false);
  let windowOpacity = $state(1.0);
  let spriteScale = $state(1);
  let autohideRowChrome = $state(true);
  let hideTitles = $state(false);
  let hideAppIcons = $state(false);
  let marbleDrift = $state(false);
  let talkSource = $state<"system" | "hotkey">("system");
  // Drawers row: leash status mirror (the native side is SoT).
  let drawers = $state<Record<string, DrawerSlice>>({});
  let companionHasBounds = $state(false);
  let section = $state<"appearance" | "talk" | "packs">("appearance");
  // Companion body picker (Appearance): Spirit + role:companion packs.
  // Same persist as the old companion-row select — set_companion_pack
  // emits state:changed, and the companion window hot-swaps the art.
  let bodyPacks = $state<BodyPack[]>([]);
  let bodyId = $state("spirit");
  let bodyBusy = $state(false);

  let packs = $state<PackInfo[]>([]);
  let packsDir = $state("");
  let packNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let packBusy = $state(false);

  function clampSpriteScale(n: number) {
    if (!Number.isFinite(n)) return 1;
    return Math.min(2.5, Math.max(0.4, n));
  }

  function normalizeTalk(raw: string | undefined): "system" | "hotkey" {
    const s = (raw ?? "").trim().toLowerCase();
    return s === "hotkey" || s === "handy" ? "hotkey" : "system";
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
      hideTitles = s.hideTitles ?? false;
      hideAppIcons = s.hideAppIcons ?? false;
      marbleDrift = s.marbleDrift ?? false;
      talkSource = normalizeTalk(s.companion?.talkSource);
      const pack = s.companion?.imagePackId ?? null;
      bodyId = pack && pack.length > 0 ? pack : "spirit";
      drawers = s.drawers ?? {};
      companionHasBounds = !!s.companion?.bounds;
    } catch {
      themeId = osThemeId();
    }
  }

  // The old-save law, mirrored for the status line: an explicit entry wins;
  // a missing companion entry means plucked iff legacy bounds exist; every
  // other missing label means docked.
  function drawerDocked(label: string): boolean {
    const d = drawers[label];
    if (d && typeof d.docked === "boolean") return d.docked;
    if (label === "companion") return !companionHasBounds;
    return true;
  }

  async function retether(label: string) {
    try {
      await invoke("retether_drawer", { label });
    } catch {
      /* state:changed restores */
    }
  }
  async function retetherAll() {
    try {
      await invoke("retether_all_drawers");
    } catch {
      /* state:changed restores */
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
  function toggleMarbleDrift() {
    marbleDrift = !marbleDrift;
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
    invoke("ensure_glass").catch(() => {});
  });
  $effect(() => {
    if (!themeId) return;
    applyWindowOpacity(windowOpacity);
  });

  const onVisibility = () => {
    if (!document.hidden) {
      loadState();
      refreshPacks();
    }
  };
  let unlisten: (() => void) | undefined;
  onMount(async () => {
    invoke("ensure_glass").catch(() => {});
    await loadState();
    await refreshPacks();
    unlisten = await listen("state:changed", loadState);
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });

  function onBannerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("button, input, a, select, textarea")) return;
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  }
</script>

<main class="settings">
  <!-- One drag source: JS startDragging on this banner. Drawers are
       borderless — no native Overlay strip competing with the click. -->
  <header class="s-banner" onmousedown={onBannerMouseDown}>
    <button
      class="winx"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => invoke("hide_window", { label: "settings" }).catch(() => {})}
      title="Hide Settings — reopen from the gear"
      aria-label="Hide Settings">×</button
    >
    <h1><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">ty</span></span> Settings</h1>
  </header>

  <nav class="tabs" aria-label="Settings sections">
    <button
      class="tab"
      class:on={section === "appearance"}
      onclick={() => (section = "appearance")}>Appearance</button
    >
    <button
      class="tab"
      class:on={section === "talk"}
      onclick={() => (section = "talk")}>Talk</button
    >
    <button
      class="tab"
      class:on={section === "packs"}
      onclick={() => (section = "packs")}>Packs</button
    >
  </nav>

  <div class="s-body">
    {#if section === "appearance"}
      <section class="panel">
        {#each BLOCKS as block (block)}
          <p class="block-label">{block}</p>
          <div class="theme-grid">
            {#each THEMES.filter((t) => t.block === block) as t (t.id)}
              <button
                class="swatch"
                class:active={themeId === t.id}
                onclick={() => pickTheme(t.id)}
              >
                <span class="chips" data-theme={t.id}
                  ><i class="c-bg"></i><i class="c-fg"></i><i class="c-ac"></i></span
                >
                {t.name}
              </button>
            {/each}
          </div>
        {/each}
        <div class="opacity-row">
          <label for="opacity-slider">Background opacity</label>
          <input
            id="opacity-slider"
            type="range"
            min="0.2"
            max="1"
            step="0.05"
            bind:value={windowOpacity}
            oninput={onOpacityInput}
            onchange={onOpacityCommit}
          />
          <span class="opacity-val">{Math.round(windowOpacity * 100)}%</span>
        </div>
        <div class="opacity-row">
          <label for="sprite-scale">Sprite size</label>
          <input
            id="sprite-scale"
            type="range"
            min="0.4"
            max="2.5"
            step="0.1"
            bind:value={spriteScale}
            oninput={onSpriteScaleInput}
            onchange={onSpriteScaleCommit}
          />
          <span class="opacity-val">{Math.round(spriteScale * 100)}%</span>
        </div>
        <p class="block-label">Rows</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={autohideRowChrome}
            aria-pressed={autohideRowChrome}
            onclick={toggleAutohide}
            title="Hide the complete-circle, stow arrows and folder+ until a row is hovered"
          >Auto-hide row chrome</button>
          <button
            class="mini"
            class:active={hideTitles}
            aria-pressed={hideTitles}
            onclick={toggleHideTitles}
            title="Hide window titles — rows collapse to icon + chrome"
          >Hide titles</button>
          <button
            class="mini"
            class:active={hideAppIcons}
            aria-pressed={hideAppIcons}
            onclick={toggleHideIcons}
            title="Hide app icons — rows collapse to title only"
          >Hide icons</button>
        </div>
        <p class="block-label">Marbles</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={marbleDrift}
            aria-pressed={marbleDrift}
            onclick={toggleMarbleDrift}
            title="Nearby marbles ring an open planet about two marble-widths off its current size and go around the disc instead of through it. If none is open, they take slow rings around a quiet midpoint. They do not chase each other. Expanded planets never move. Reduce Motion stills everything"
          >Marbles drift</button>
        </div>
        <p class="block-label">Companion body</p>
        <div class="setting-row">
          <select
            class="bodypick"
            bind:value={bodyId}
            onchange={onBodyChange}
            disabled={bodyBusy}
            aria-label="Companion body"
            title="Which body the companion wears — the blessed Spirit, a bundled 3D body, or an imported companion pack"
          >
            <option value="spirit">Spirit</option>
            <!-- GO-ORBITTY-COMPANION-3D: reserved bundled-GLB ids, never
                 pack folders. Spirit stays the default. -->
            <option value="tang">Tang · 3D</option>
            <option value="female">Female · 3D</option>
            {#each bodyPacks as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        {#if bodyId === "tang"}
          <p class="help">
            Tang is derived from Jacob by jupifox (CC BY 4.0) — credit
            jupifox / Jacob.
          </p>
        {/if}
        <p class="block-label">Drawers</p>
        <div class="setting-row">
          <button
            class="mini"
            onclick={() => retether("companion")}
            title="Dock Companion back onto the rail"
          >Re-tether Companion</button>
          <button
            class="mini"
            onclick={() => retether("kanban")}
            title="Dock Board/STARS back onto the rail"
          >Re-tether Board</button>
          <button
            class="mini"
            onclick={() => retether("settings")}
            title="Dock Settings back onto the rail"
          >Re-tether Settings</button>
          <button
            class="mini"
            onclick={retetherAll}
            title="Dock every pane back onto the rail"
          >Re-tether all</button>
        </div>
        <p class="help">
          Docked panes ride the rail and slide out of it on first open; drag
          one ~48px off the rail to pluck it free. Now: Companion
          {drawerDocked("companion") ? "docked" : "plucked"} · Board
          {drawerDocked("kanban") ? "docked" : "plucked"} · Settings
          {drawerDocked("settings") ? "docked" : "plucked"}.
        </p>
      </section>
    {:else if section === "talk"}
      <section class="panel">
        <p class="block-label">Source</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={talkSource === "system"}
            aria-pressed={talkSource === "system"}
            onclick={() => pickTalk("system")}
          >System speech</button>
          <button
            class="mini"
            class:active={talkSource === "hotkey"}
            aria-pressed={talkSource === "hotkey"}
            onclick={() => pickTalk("hotkey")}
          >Hotkey (Handy)</button>
        </div>
        <p class="help">
          System speech is the default — macOS dictation, nothing extra to
          download. Hotkey needs Handy running and holds Ctrl+Space while
          Listening. Speak / Prompts stay on the companion. Send is still
          your tap.
        </p>
      </section>
    {:else}
      <section class="panel">
        <div class="setting-row">
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackFolder}
            title="Pick a folder of PNG frames (optional manifest.json)"
          >Import folder…</button>
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackZip}
            title="Pick a .zip you downloaded"
          >Import zip…</button>
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackImage}
            title="Pick a PNG/GIF/WebP/JPEG"
          >Import image…</button>
          {#if packsDir}
            <button
              class="mini ghost"
              onclick={() => shellOpen(packsDir).catch(() => {})}
              title={packsDir}
            >Reveal</button>
          {/if}
        </div>
        {#if packNote}
          <p class="help" class:err={packNote.kind === "err"}>{packNote.text}</p>
        {/if}
        {#if packs.length > 0}
          <ul class="pack-list">
            {#each packs.filter((p) => p.role !== "companion") as p (p.id)}
              <li class="pack-row">
                <span class="pack-name" title={p.path}>{p.name}</span>
                <span class="pack-meta">{p.frameCount} frame{p.frameCount === 1 ? "" : "s"}</span>
                {#if p.license}
                  <span class="pack-license">{p.license}</span>
                {:else}
                  <span class="pack-license warn">no license ⚠</span>
                {/if}
                <button
                  class="mini ghost"
                  onclick={() => removePack(p.id)}
                  title="Remove this pack — deletes only its own subfolder"
                >×</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="help">A single PNG/JPEG/GIF/WebP is enough (it stands still).</p>
        {/if}
        <p class="help">
          Walkers are the built-in Season 1 bitty crew or imported from your
          files — Orbitty never downloads art. Missing license warns, never
          blocks.
        </p>
      </section>
    {/if}
  </div>
</main>

<style>
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
  .winx {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    font: inherit;
    font-size: 0.82rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .winx:hover {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }
  .s-banner h1 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  /* GO-ORBITTY-WORDMARK: ochre i-dot / ink O / accent ty. */
  .mark {
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.28),
      -1px -1px 0 rgba(255, 255, 255, 0.09);
  }

  .o {
    color: var(--ink);
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .idot {
    color: #e59e3d;
    text-shadow:
      1px 1px 0 rgba(0, 0, 0, 0.42),
      -1px -1px 0 rgba(255, 255, 255, 0.14);
  }

  .rest {
    color: var(--ink);
  }

  .pop {
    color: var(--accent);
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
  .panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
  }
  .block-label {
    margin: 8px 0 4px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
  .theme-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .setting-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
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
  .mini.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg);
    font-weight: 700;
  }
  .mini.ghost {
    background: transparent;
  }
  .mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .swatch {
    display: flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 4px 8px;
    cursor: pointer;
  }
  .swatch.active {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 700;
  }
  .bodypick {
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
    outline: none;
  }
  .bodypick:focus {
    border-color: var(--accent);
  }
  .bodypick:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chips {
    display: inline-flex;
    gap: 2px;
  }
  .chips i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    display: inline-block;
  }
  .chips[data-theme="systemLight"] .c-bg { background: #ececec; }
  .chips[data-theme="systemLight"] .c-fg { background: rgba(0, 0, 0, 0.85); }
  .chips[data-theme="systemLight"] .c-ac { background: #3d8f8a; }
  .chips[data-theme="systemDark"] .c-bg { background: #1e1e1e; }
  .chips[data-theme="systemDark"] .c-fg { background: rgba(255, 255, 255, 0.85); }
  .chips[data-theme="systemDark"] .c-ac { background: #3d8f8a; }
  .chips[data-theme="terminal"] .c-bg { background: #0a120e; }
  .chips[data-theme="terminal"] .c-fg { background: #c8f0d0; }
  .chips[data-theme="terminal"] .c-ac { background: #3ddc84; }
  .opacity-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    font-size: 0.68rem;
    color: var(--ink);
  }
  .opacity-row label {
    flex-shrink: 0;
    color: var(--dim);
  }
  .opacity-row input[type="range"] {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }
  .opacity-val {
    flex-shrink: 0;
    width: 34px;
    text-align: right;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
  .help {
    margin: 8px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
  }
  .help.err {
    color: var(--accent);
  }
  .pack-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pack-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 22px;
  }
  .pack-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--ink);
  }
  .pack-meta {
    flex-shrink: 0;
    font-size: 0.64rem;
    color: var(--dim);
  }
  .pack-license {
    flex-shrink: 0;
    font-size: 0.62rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.35rem;
  }
  .pack-license.warn {
    color: var(--accent);
    border-color: var(--accent);
  }
</style>
