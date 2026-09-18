<script lang="ts">
  import type { RailApi } from "./api";

  let { rail }: { rail: RailApi } = $props();
</script>

  {#if !rail.embedded && rail.beingEditorFor && rail.beingClaim()}
    {@const bw = rail.beingWindow()}
    {@const bc = rail.beingClaim()!}
    <div class="being-editor" role="dialog" aria-label="Being editor">
      <div class="being-head">
        <span class="being-title" title={bw ? `${bw.appName} — ${bw.title}` : bc.windowId}>
          {bw ? bw.appName : "Being"}
        </span>
        <button class="mini ghost" onclick={rail.closeBeingEditor} aria-label="Close being editor">×</button>
      </div>

      {#if rail.editorPreview}
        <img class="being-preview" src={rail.editorPreview} alt="Being preview" />
      {/if}

      <div class="being-actions">
        <button class="mini" onclick={rail.rerollBeing} title="New random being">Reroll</button>
      </div>

      <div class="being-row">
        <div class="being-slot">
          <button class="mini ghost" onclick={() => rail.cycleKind(-1)} aria-label="Previous kind">‹</button>
          <span class="slot-name">Kind</span>
          <span class="slot-val">{rail.kindLabel()}</span>
          <button class="mini ghost" onclick={() => rail.cycleKind(1)} aria-label="Next kind">›</button>
        </div>
      </div>
      <div class="being-row">
        <div class="being-slot">
          <button class="mini ghost" onclick={() => rail.cyclePalette(-1)} aria-label="Previous palette">‹</button>
          <span class="slot-name">Palette</span>
          <span class="slot-val">{rail.paletteLabel()}</span>
          <button class="mini ghost" onclick={() => rail.cyclePalette(1)} aria-label="Next palette">›</button>
        </div>
      </div>

      <label class="being-scale">
        Size
        <input
          type="range"
          min="0.4"
          max="2.5"
          step="0.05"
          value={rail.beingScale()}
          oninput={rail.onBeingScaleInput}
          onchange={rail.onBeingScaleCommit}
        />
        <span>{rail.beingScale().toFixed(2)}×</span>
        <button
          class="mini ghost"
          onclick={rail.resetBeingScale}
          title="Follow the gear's global sprite size"
          disabled={bc.spriteScale == null}
        >global</button>
      </label>

      <span class="menu-label">Tags</span>
      <div class="being-actions">
        {#each rail.tags as t (t.id)}
          <button
            class="mini"
            class:active={rail.hasTag(bc.windowId, t.id)}
            aria-pressed={rail.hasTag(bc.windowId, t.id)}
            onclick={() => rail.toggleTag(bc.windowId, t.id)}>{t.name}</button
          >
        {/each}
        {#if rail.tags.length === 0}
          <span class="slot-name">No tags yet.</span>
        {/if}
      </div>

      <span class="menu-label">Home</span>
      <div class="being-actions">
        {#each rail.projects as p (p.id)}
          <button
            class="mini"
            class:active={bc.projectId === p.id}
            aria-pressed={bc.projectId === p.id}
            onclick={() => rail.beingMoveHome(p.id)}>{p.name}</button
          >
        {/each}
        <button
          class="mini ghost"
          onclick={rail.beingUnclaim}
          title="Remove the claim — the walker leaves its planet"
        >Unclaim</button>
      </div>
    </div>
  {/if}


  {#if !rail.embedded && rail.ctxMenu}
    {@const cw = rail.ctxWindow()}
    {@const cp = rail.ctxProject()}
    <button
      class="ctx-backdrop"
      aria-label="Close menu"
      onclick={() => (rail.ctxMenu = null)}
      oncontextmenu={(e) => {
        e.preventDefault();
        rail.ctxMenu = null;
      }}
    ></button>
    <div
      class="ctx-menu"
      role="menu"
      style="left: {rail.ctxMenu.x}px; top: {rail.ctxMenu.y}px"
    >
      <button
        class="ctx-item"
        role="menuitem"
        aria-expanded={rail.ctxAssignOpen}
        onclick={() => (rail.ctxAssignOpen = !rail.ctxAssignOpen)}
        >Assign to…</button
      >
      {#if rail.ctxAssignOpen}
        <div class="ctx-sub" role="group" aria-label="Assign to project">
          {#each rail.projects as p (p.id)}
            <button
              class="ctx-item"
              class:active={cp?.id === p.id}
              role="menuitem"
              onclick={() => rail.ctxAssign(p.id)}>{p.name}</button
            >
          {/each}
          <span class="ctx-newproj">
            <input
              bind:value={rail.newProjectName}
              placeholder="New project…"
              onkeydown={(e) => e.key === "Enter" && rail.ctxCreateAndClaim()}
            />
            <button class="mini" onclick={rail.ctxCreateAndClaim}>Claim</button>
          </span>
        </div>
      {/if}
      <button class="ctx-item" role="menuitem" onclick={rail.ctxRename}>Rename</button>
      {#if cw && !cw.sleeping}
        {#if cw.isMinimized}
          <button class="ctx-item" role="menuitem" onclick={rail.ctxRestore}
            >Restore</button
          >
        {:else}
          <button class="ctx-item" role="menuitem" onclick={rail.ctxMinimize}
            >Minimize</button
          >
        {/if}
        <button class="ctx-item" role="menuitem" onclick={rail.ctxCloseWindow}
          >Close window</button
        >
      {/if}
      {#if cp}
        <div class="ctx-sep" role="separator"></div>
        <button class="ctx-item danger" role="menuitem" onclick={rail.ctxUnclaim}
          >× Unclaim</button
        >
      {/if}
    </div>
  {/if}

