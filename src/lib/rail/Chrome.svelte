<script lang="ts">
  import RailStack from "../RailStack.svelte";
  import type { RailApi } from "./api";

  let { rail }: { rail: RailApi } = $props();
</script>

  <div class="chrome">
    <header
      class="banner"
      onmousedown={rail.onBannerMouseDown}
    >
      <h1>
        {#if rail.embedded}
          Planets
        {:else}
        <button
          class="wordmark"
          onclick={rail.onWordmarkClick}
          title="Home — click to bring planets front, drag to move the rail"
          aria-label="Home — bring all planet panels to the front"
          ><span class="mark"><span class="o">O</span><span class="rest">rb</span><span class="idot">i</span><span class="pop">tty</span></span></button
        >
        {/if}
      </h1>
      {#if rail.embedded}
        <button
          class="popout"
          onmousedown={(e) => e.stopPropagation()}
          onclick={() => rail.onPopout?.()}
          title="Pop out to the rail window — the well closes, the companion stays"
          aria-label="Pop out Planets"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M15 3h6v6" />
            <path d="M10 14 21 3" />
            <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
          </svg>
        </button>
      {/if}
      {#if rail.status?.trusted && rail.refreshedAt}
        <span class="clock" title="last refresh">{rail.refreshedAt}</span>
      {/if}
      <button
        class="winx"
        onmousedown={(e) => e.stopPropagation()}
        onclick={() =>
          rail.embedded
            ? rail.onClose?.()
            : rail.invoke("hide_window", { label: "main" }).catch(() => {})}
        title={rail.embedded
          ? "Dismiss the Planets well — the companion stays"
          : "Hide Orbitty — quit from the tray icon"}
        aria-label={rail.embedded ? "Dismiss Planets" : "Hide Orbitty"}>×</button
      >
    </header>

    {#if !rail.embedded}
      <!-- The window stack (STARS / Sky / Board /
           Companion / ⚙) — toggle_labeled, show+focus then hide. Own
           file (module law); the embedded Planets well keeps the dock's
           dest chips instead. -->
      {#if rail.peerChrome}
        <RailStack />
      {/if}
    {/if}

    {#if rail.status?.trusted && rail.stateLoaded && !rail.embedded}
      <div class="list-head">
        <span class="lane-tabs" role="group" aria-label="Rail lane">
          <button
            class="mini lane-tab"
            class:active={rail.lane === "projects"}
            role="tab"
            aria-selected={rail.lane === "projects"}
            onclick={() => (rail.lane = "projects")}>Projects</button
          >
          <button
            class="mini lane-tab"
            class:active={rail.lane === "agents"}
            role="tab"
            aria-selected={rail.lane === "agents"}
            onclick={() => (rail.lane = "agents")}>Agents</button
          >
        </span>
        {#if rail.lane === "projects"}
          <span class="count">{rail.windows.length} window{rail.windows.length === 1 ? "" : "s"}</span>
          {#if rail.creatingProject}
            <span class="newproj">
              <input
                bind:value={rail.newProjectName}
                placeholder="Project name"
                onkeydown={(e) => e.key === "Enter" && rail.createProjectOnly()}
              />
              <button class="mini" onclick={rail.createProjectOnly}>Add</button>
              <button class="mini ghost" onclick={() => (rail.creatingProject = false)}>×</button>
            </span>
          {:else}
            <button class="mini" onclick={() => (rail.creatingProject = true)}>+ Project</button>
          {/if}
        {/if}
      </div>
      {#if !rail.embedded && rail.lane === "projects" && rail.tags.length > 0}
        <div class="tag-filters">
          <button
            class="mini"
            class:active={rail.tagFilter === null}
            onclick={() => (rail.tagFilter = null)}>All</button
          >
          {#each rail.tags as t (t.id)}
            <button
              class="mini"
              class:active={rail.tagFilter === t.id}
              onclick={() => (rail.tagFilter = rail.tagFilter === t.id ? null : t.id)}
              >{t.name}</button
            >
          {/each}
        </div>
      {/if}
    {/if}
  </div>

