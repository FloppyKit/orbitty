<script lang="ts">
  import RailPlanetRm from "./PlanetRm.svelte";
  import type { RailApi } from "./api";

  let { rail }: { rail: RailApi } = $props();
</script>

<!-- Hosted well — top orb strip, one body.
     Extra rows scroll inside the selected tab, not the tile. -->
  <div class="sheet">
  <div class="planet-strip" role="tablist" aria-label="Planets">
    <button
      type="button"
      class="planet-tab"
      class:sel={rail.sheetSlot === "unclaimed"}
      class:mkey-sel={rail.menuRing.activeId === "ptab-unclaimed"}
      data-mkey="ptab-unclaimed"
      role="tab"
      aria-selected={rail.sheetSlot === "unclaimed"}
      onclick={() => (rail.sheetSlot = "unclaimed")}
    >
      <span class="planet-tab-name">Unclaimed</span>
      <span class="section-count">{rail.unmapped.length}</span>
    </button>
    {#each rail.projects as p (p.id)}
      <button
        type="button"
        class="planet-tab"
        class:sel={rail.sheetSlot === p.id}
        class:mkey-sel={rail.menuRing.activeId === `ptab-${p.id}`}
        data-mkey={`ptab-${p.id}`}
        role="tab"
        aria-selected={rail.sheetSlot === p.id}
        title={p.name}
        onclick={() => rail.onSheetPlanetClick(p.id)}
        ondblclick={() => rail.onSheetPlanetDblClick(p.id)}
      >
        <span
          class="planet-chip"
          class:live={rail.hackProjectId === p.id}
          style="background: {rail.planetCss(p.id)}; --pglow: {rail.planetCss(p.id)}"
          aria-hidden="true"
        ></span>
        <span class="planet-tab-name">{p.name}</span>
        <span class="section-count">{rail.sheetRowsFor(p.id).length}</span>
      </button>
    {/each}
    {#if rail.creatingProject}
      <span class="newproj strip-add">
        <input
          bind:this={rail.sheetNameEl}
          bind:value={rail.newProjectName}
          placeholder="Planet name"
          onkeydown={(e) => {
            if (e.key === "Enter") rail.createProjectOnly();
            if (e.key === "Escape") rail.creatingProject = false;
          }}
        />
        <button class="mini" onclick={rail.createProjectOnly}>Add</button>
        <button class="mini ghost" onclick={() => (rail.creatingProject = false)}>×</button>
      </span>
    {:else}
      <button
        type="button"
        class="planet-tab add"
        class:mkey-sel={rail.menuRing.activeId === "ptab-add"}
        data-mkey="ptab-add"
        onclick={() => (rail.creatingProject = true)}>+ Planet</button
      >
    {/if}
  </div>

  <div class="sheet-body">
  {#if rail.sheetSlot === "unclaimed"}
    <section class="sheet-sec" aria-label="Unclaimed windows">
      {#if rail.unmapped.length === 0}
        <p class="sheet-empty">Every window has a planet.</p>
      {:else}
        <ul class="wins">
          {#each rail.unmapped as w (w.id)}
            <li
              class="sheet-row"
              class:mkey-sel={rail.menuRing.activeId === `prow-${w.id}`}
              data-mkey={`prow-${w.id}`}
            >
              <span class="app-icon-slot">
                {#if w.iconBase64}
                  <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
                {:else}
                  <span class="app-icon fallback" aria-hidden="true"
                    >{w.appName.charAt(0) || "?"}</span
                  >
                {/if}
              </span>
              <span class="wtitle" title="{w.appName} — {w.title}">{rail.titleFor(w)}</span>
              <button
                class="mini claim-btn"
                class:active={rail.claimPickerFor === w.id}
                aria-expanded={rail.claimPickerFor === w.id}
                onclick={() =>
                  (rail.claimPickerFor = rail.claimPickerFor === w.id ? null : w.id)}
                title="Claim to a planet">claim ▸</button
              >
            </li>
            {#if rail.claimPickerFor === w.id}
              <li class="claim-picker">
                {#if rail.projects.length === 0}
                  <button class="mini" onclick={() => (rail.creatingProject = true)}
                    >No planets yet — add one above</button
                  >
                {:else}
                  {#each rail.projects as p (p.id)}
                    <button
                      class="mini claim-pick"
                      class:mkey-sel={rail.menuRing.activeId === `pclaim-${w.id}-${p.id}`}
                      data-mkey={`pclaim-${w.id}-${p.id}`}
                      onclick={() => rail.sheetClaim(w.id, p.id)}
                      title="Claim to {p.name}"
                      ><span
                        class="planet-chip"
                        style="background: {rail.planetCss(p.id)}"
                        aria-hidden="true"
                      ></span
                      >{p.name}</button
                    >
                  {/each}
                {/if}
              </li>
            {/if}
          {/each}
        </ul>
      {/if}
    </section>
  {:else}
    {@const p = rail.projects.find((x) => x.id === rail.sheetSlot)}
    {@const rows = p ? rail.sheetRowsFor(p.id) : []}
    <section class="sheet-sec" aria-label={p ? `Windows on ${p.name}` : "Planet"}>
      {#if p}
        <div class="head-row">
          <h2 class="sheet-h">{p.name}</h2>
          <RailPlanetRm {rail} project={p} />
        </div>
        {#if rail.rmErrorFor?.id === p.id}
          <p class="rm-err" role="alert">{rail.rmErrorFor.msg}</p>
        {/if}
      {/if}
      {#if rows.length === 0}
        <p class="sheet-empty">No windows on this planet.</p>
      {:else}
        <ul class="wins">
          {#each rows as w (w.id)}
            <li>
              <button
                class="row"
                class:front={w.isFrontmost}
                class:min={w.isMinimized}
                class:mkey-sel={rail.menuRing.activeId === `prow-${w.id}`}
                data-mkey={`prow-${w.id}`}
                onclick={() => rail.sheetFocus(w)}
                title="{w.appName} — {w.title} · click to focus"
              >
                {#if w.iconBase64}
                  <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
                {:else}
                  <span class="app-icon fallback" aria-hidden="true"
                    >{w.appName.charAt(0) || "?"}</span
                  >
                {/if}
                <span class="wtitle">{rail.titleFor(w)}</span>
                {#if w.isFrontmost}<span class="tag front-tag">front</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
  </div>
  </div>

