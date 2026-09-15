<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // The rail window stack. Five dests, one native
  // helper — `toggle_labeled` owns the visible?-hide-else-show branch:
  // first click shows + focuses, a click while visible hides via
  // `hide_window`. Hide never quits (tray Quit stays the only quit) and
  // never destroys a webview; companion hide keeps its talk-stop +
  // persist-hidden bookkeeping inside hide_window. Board and STARS
  // are separate windows.
  // Words, not marks: RAIL-MARKS art (eight PNGs) is still blocked, so
  // the buttons keep their text labels — a missing mark is a word, never
  // tofu. Chrome still says Sky (Cloud waits on that art). Active-state
  // polish stays out (lock law 7).
  const DESTS = [
    { label: "stars", word: "STARS", name: "STARS" },
    { label: "sky", word: "Sky", name: "Sky" },
    { label: "kanban", word: "Board", name: "Board" },
    { label: "companion", word: "Companion", name: "Companion" },
    { label: "settings", word: "⚙", name: "Settings" },
  ] as const;

  function toggle(label: string) {
    invoke("toggle_labeled", { label }).catch(() => {});
  }
</script>

<nav class="railstack" aria-label="Windows">
  {#each DESTS as d (d.label)}
    <button
      type="button"
      class="stackdest"
      onmousedown={(e) => e.stopPropagation()}
      onclick={() => toggle(d.label)}
      title="{d.name} — click again to hide"
      aria-label="Toggle {d.name}"
      >{d.word}</button
    >
  {/each}
</nav>

<style>
  /* Grab law: fat hit-rects — one row when the rail is wide, wrapping to
     two when it is narrow; never a 3px chip you have to hunt for. The
     row mounts in .chrome below the banner, so the banner's startDragging
     mousedown can never steal a press; the stopPropagation above is the
     belt-and-braces half of that house pattern. */
  .railstack {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .stackdest {
    flex: 1 1 60px;
    min-height: 28px;
    padding: 4px 6px;
    font: inherit;
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .stackdest:hover {
    border-color: var(--focus, #6e4db0);
  }
</style>
