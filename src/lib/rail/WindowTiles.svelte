<script lang="ts">
  import type { Project, WindowInfo } from "../persist";
  import type { RailApi } from "./api";
  import { TILE_COLS, tiles } from "./habitat";

  let {
    rail,
    project,
    rows,
  }: { rail: RailApi; project: Project; rows: WindowInfo[] } = $props();

  const view = $derived(tiles(rows));
  const byId = $derived(new Map(rows.map((w) => [w.id, w])));

  function takeFocus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

{#if view.shown.length > 0 || view.more > 0}
  <div
    class="tile-grid"
    style={`grid-template-columns: repeat(${TILE_COLS}, minmax(0, 1fr))`}
  >
    {#each view.shown as tile (tile.id)}
      {@const w = byId.get(tile.id)}
      {#if w}
        {@const age = rail.ageClass(w.id)}
        {@const sleeping = !!w.sleeping}
        <div
          class="tile-wrap"
          role="group"
          class:dragging={rail.dragWindowId === w.id}
          class:zfade={age === "fade"}
          class:zpark={age === "park"}
          class:zgone={age === "gone"}
          draggable="true"
          ondragstart={(e) => rail.onDragStart(e, w)}
          ondragend={rail.onDragEnd}
          title={sleeping
            ? "Asleep — the window is closed; click to wake it. Drag still re-homes."
            : "Drag onto a project to claim"}
        >
          <button
            class="circle-btn tile-circle"
            class:filling={rail.completingIds.includes(w.id)}
            onclick={() => rail.circleComplete(w.id, () => rail.completeWindow(w, project))}
            title="Complete — marks the card done, retires the sprite, closes the window"
            aria-label="Complete"
            >{#if rail.completingIds.includes(w.id)}✓{/if}</button
          >
          {#if rail.renamingId === w.id}
            <input
              class="rename-input tile-rename"
              bind:value={rail.renameValue}
              placeholder={w.title || w.appName}
              aria-label="Rename window title (empty restores the live title)"
              use:takeFocus
              onkeydown={(e) => {
                if (e.key === "Enter") rail.commitRename(w);
                if (e.key === "Escape") rail.renamingId = null;
              }}
              onblur={() => rail.commitRename(w)}
            />
          {:else}
            <button
              type="button"
              class="tile"
              class:stowed={tile.state === "stowed"}
              class:asleep={tile.state === "asleep"}
              class:front={w.isFrontmost}
              class:mkey-sel={rail.menuRing.activeId === `rtile-${w.id}`}
              class:locate-flash={rail.locateFlashIds.includes(w.id)}
              data-mkey={`rtile-${w.id}`}
              onclick={() => (sleeping ? rail.wakeSleeper(w) : rail.focusRow(w))}
              ondblclick={() =>
                sleeping ? rail.wakeSleeper(w) : w.isMinimized ? rail.restore(w) : rail.stow(w)}
              oncontextmenu={(e) => rail.openCtxMenu(e, w)}
              title={sleeping
                ? `${w.appName} — ${w.title} · asleep — click wakes`
                : w.isMinimized
                  ? `${w.appName} — ${w.title} · click brings forward, double-click restores`
                  : `${w.appName} — ${w.title} · click brings forward, double-click minimizes`}
            >
              {#if w.iconBase64}
                <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
              {:else}
                <span class="app-icon fallback" aria-hidden="true"
                  >{w.appName.charAt(0) || "?"}</span
                >
              {/if}
              <span class="wtitle tile-title">{rail.titleFor(w)}</span>
              {#if tile.state === "stowed"}<span class="tile-mark">stowed</span>{/if}
            </button>
          {/if}
        </div>
      {/if}
    {/each}
    {#if view.more > 0}
      <span class="tile-more">+{view.more}</span>
    {/if}
  </div>
{/if}
