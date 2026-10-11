<script lang="ts">
  import MouthSettings from "./MouthSettings.svelte";
  let {
    BLOCKS,
    THEMES,
    themeId,
    pickTheme,
    windowOpacity = $bindable(),
    onOpacityInput,
    onOpacityCommit,
    spriteScale = $bindable(),
    onSpriteScaleInput,
    onSpriteScaleCommit,
    autohideRowChrome,
    toggleAutohide,
    hideTitles,
    toggleHideTitles,
    hideAppIcons,
    toggleHideIcons,
    RAIL_EDGES,
    railEdge,
    pickRailEdge,
    railAutoHideOnBlur,
    toggleRailAutoHide,
    peerChrome,
    togglePeerChrome,
    marbleDrift,
    toggleMarbleDrift,
    planetIdle,
    pickPlanetIdle,
    bodyId = $bindable(),
    onBodyChange,
    bodyBusy,
    bodyPacks,
    talkSource,
    pickTalk,
    spendCapUsd = $bindable(),
    spendCapEnabled = $bindable(),
    persistSettings,
    runnerCap = $bindable(),
    treeGrowDays = $bindable(),
    signedLocally,
    toggleSignedLocally,
    privateRelay,
    togglePrivateRelay,
    privateRelayUrl = $bindable(),
    onReplicaUrlCommit,
    replicaUrlNote,
    cheatSheetHint,
    toggleCheatHint,
    linkApprovals,
    removeApproval,
    clearApprovals,
    openYardWarn,
  } = $props();
</script>

      <section class="panel">
        {#each BLOCKS as block (block)}
          <p class="block-label">{block}</p>
          <div class="theme-grid">
            {#each THEMES.filter((t: { id: string; name: string; block: string }) => t.block === block) as t (t.id)}
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
            min="0.5"
            max="1.25"
            step="0.05"
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
        <p class="block-label">Rail</p>
        <div class="setting-row" role="group" aria-label="Rail edge">
          {#each RAIL_EDGES as edge (edge)}
            <button
              class="mini"
              class:active={railEdge === edge}
              aria-pressed={railEdge === edge}
              onclick={() => pickRailEdge(edge)}
              title="Park the inspector on this screen edge. This Mac only."
            >{edge}</button>
          {/each}
        </div>
        <div class="setting-row">
          <button
            class="mini"
            class:active={railAutoHideOnBlur}
            aria-pressed={railAutoHideOnBlur}
            onclick={toggleRailAutoHide}
            title="Hide the rail strip when focus leaves it. Separate from auto-hide row chrome."
          >Auto-hide rail when focus leaves</button>
        </div>
        <p class="block-label">Peer chrome</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={peerChrome}
            aria-pressed={peerChrome}
            onclick={togglePeerChrome}
            title="Show companion Launchpad/STARS/Planets strip and the rail STARS·Sky·Board·Comp stack. Off for demo — tray/Dock stay the map. Turns on after 3 claims or first Done."
          >Peer chrome</button>
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
        <p class="block-label">Planet sleep</p>
        <div class="setting-row" role="group" aria-label="Planet sleep">
          {#each [["1h", "1 hour"], ["4h", "4 hours"], ["12h", "12 hours"], ["never", "Never"]] as [id, lab]}
            <button
              class="mini"
              class:active={planetIdle === id}
              aria-pressed={planetIdle === id}
              onclick={() => pickPlanetIdle(id)}
            >{lab}</button>
          {/each}
        </div>
        <p class="help">Unused expanded planets shrink to marbles. Never leaves them up.</p>
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
            <!-- LEGACY (to be replaced by the new companion pack, Matt 2026-09-29) -->
            <option value="spirit">Spirit</option>
            <!-- Reserved bundled-GLB ids, never
                 pack folders. Spirit stays the default. -->
            <!-- LEGACY (to be replaced by the new companion pack, Matt 2026-09-29) -->
            <option value="female">Female · 3D</option>
            {#each bodyPacks as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
        </div>
        <p class="block-label">Talk</p>
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
          System speech is the default — macOS dictation. Hotkey needs Handy
          and holds Ctrl+Space while Listening.
        </p>
        <p class="block-label">Caps</p>
        <div class="opacity-row">
          <label for="spend-cap">Spend / day (USD)</label>
          <input
            id="spend-cap"
            type="number"
            min="0.01"
            step="1"
            bind:value={spendCapUsd}
            disabled={!spendCapEnabled}
            onchange={() => {
              const n = Number(spendCapUsd);
              spendCapUsd = Number.isFinite(n) && n > 0 ? n : 20;
              persistSettings();
            }}
          />
          <button
            class="mini"
            class:active={!spendCapEnabled}
            aria-pressed={!spendCapEnabled}
            onclick={() => {
              spendCapEnabled = !spendCapEnabled;
              persistSettings();
            }}
            title="Off = Never for money"
          >Never</button>
        </div>
        <div class="opacity-row">
          <label for="runner-cap">Runners</label>
          <input
            id="runner-cap"
            type="number"
            min="1"
            max="4"
            step="1"
            bind:value={runnerCap}
            onchange={() => {
              const n = Number(runnerCap);
              runnerCap = Math.min(4, Math.max(1, Number.isFinite(n) ? n : 2));
              persistSettings();
            }}
          />
          <span class="opacity-val">{runnerCap} / 4</span>
        </div>
        <div class="opacity-row">
          <label for="tree-grow">Todo tree (days to full)</label>
          <input
            id="tree-grow"
            type="number"
            min="1"
            max="365"
            step="1"
            bind:value={treeGrowDays}
            onchange={() => {
              const n = Number(treeGrowDays);
              treeGrowDays = Number.isFinite(n) && n > 0 ? n : 14;
              persistSettings();
            }}
          />
          <span class="opacity-val">{treeGrowDays}d</span>
        </div>
        <p class="help">
          Default $20/day and 2 runners (hard 4). Needed is the gate — no toast.
        </p>
        <p class="block-label">Event log</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={signedLocally}
            aria-pressed={signedLocally}
            onclick={toggleSignedLocally}
            title="New 420/421/422 and journal lines get a NIP-01 id and a device signature. Old lines stay unsigned. Local file only — not a relay, not the wallet."
          >Signed locally</button>
          <button
            class="mini"
            class:active={privateRelay}
            aria-pressed={privateRelay}
            onclick={togglePrivateRelay}
            title="Signed 420/421/422 with t=orbitty also EVENT to the loopback sidecar. Default off. Failures never fail the board. Journal 31200 stays file-only."
          >Private relay</button>
        </div>
        <div class="opacity-row">
          <label for="relay-replica">Replica URL</label>
          <input
            id="relay-replica"
            class="keyin"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder="ws://100.x:13120 — empty = this Mac only"
            bind:value={privateRelayUrl}
            onchange={onReplicaUrlCommit}
          />
        </div>
        <p class="help">
          New log lines are signed on this Mac. Off writes unsigned again. History is never rewritten. Private relay copies signed board proofs to the sidecar on this Mac; a replica URL also copies to that one-user Tailscale/SSH box. Empty replica is local only. Failures never fail the board.
        </p>
        {#if replicaUrlNote}
          <p class="help err">{replicaUrlNote}</p>
        {/if}
        <MouthSettings />
        <p class="block-label">Cheat sheet</p>
        <div class="setting-row">
          <button
            class="mini"
            class:active={cheatSheetHint}
            aria-pressed={cheatSheetHint}
            onclick={toggleCheatHint}
            title="Show Action: ⌘?(⌘⇧/) on the dock"
          >Show Action ⌘? on dock</button>
        </div>
        <p class="block-label">Approved links</p>
        <p class="help">
          The first open of a link asks. After you click Open or Run, that exact path or URL opens directly. A program asks again if its contents change. This list stays on this Mac.
        </p>
        {#if linkApprovals.length === 0}
          <p class="help">None yet.</p>
        {:else}
          <ul class="approved">
            {#each linkApprovals as row (row.target)}
              <li>
                <span class="path" title={row.target}>{row.target}</span>
                <button
                  type="button"
                  class="mini ghost"
                  onclick={() => removeApproval(row.target)}>Remove</button>
              </li>
            {/each}
          </ul>
          <button type="button" class="mini" onclick={clearApprovals}>Clear all</button>
        {/if}
        <div class="yard-foot">
          <button
            class="mini ghost"
            onclick={openYardWarn}
            title="Open the Herding Cats confirm — ⌘⇧Y works too, even with Settings closed"
          >Herding Cats…</button>
        </div>
      </section>

<style>
.panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--glass-glow, color-mix(in srgb, var(--accent) 45%, transparent));
    border-radius: 12px;
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
/* The BYOK key field — same chrome as bodypick,
     flexes to the row. */
  .keyin {
    flex: 1;
    min-width: 140px;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    outline: none;
  }
.keyin:focus {
    border-color: var(--accent);
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
/* Quiet footer of the Appearance tab — not a
     section, not on launch. */
  .approved {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
.approved li {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
.approved .path {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: 0.72rem;
    color: var(--dim);
  }
.yard-foot {
    margin-top: 12px;
    padding-top: 8px;
    border-top: 1px solid var(--edge);
    display: flex;
    justify-content: flex-end;
  }
</style>
