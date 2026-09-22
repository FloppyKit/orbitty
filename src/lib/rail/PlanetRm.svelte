<script lang="ts">
  import type { Project } from "../persist";
  import type { RailApi } from "./api";

  let {
    rail,
    project,
  }: {
    rail: RailApi;
    project: Project;
  } = $props();
</script>

<!-- the × arms an
     in-sheet confirm chip (no window.confirm — JS dialogs don't surface on
     the companion overlay). stopPropagation on mousedown AND click so the
     companion pane-drag and the section-head collapse toggle can't steal
     the press. Shared by the Inspector On planets rows and the rail list. -->
  {#if rail.rmArmedFor === project.id}
    <span class="planet-rm-confirm" aria-label="Remove planet {project.name}?">
      <span class="rm-q">Remove "{project.name}"?</span>
      <button
        class="mini rm-yes"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          rail.removeProject(project);
        }}>Yes</button
      >
      <button
        class="mini ghost"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          rail.rmArmedFor = null;
        }}>No</button
      >
    </span>
  {:else}
    <button
      class="mini ghost planet-rm"
      onmousedown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        rail.armRemove(project.id);
      }}
      title="Remove planet — cards keep their project, walkers vanish with the disc"
      aria-label="Remove planet {project.name}">×</button
    >
  {/if}

