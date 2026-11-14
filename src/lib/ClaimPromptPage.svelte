<script lang="ts">
  // Claim prompt. Own window (label claim-prompt), beside the rail.
  // One line, one planet menu, a new-planet field, Claim, Later.
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { startPaneDrag } from "./dragPane";
  import type { PersistedState, Project } from "./persist";
  import { nudgeClaim, nudgeCreate } from "./rail/pageClaim.svelte";
  import { applyHostTokens, applyWindowOpacity } from "../themes/apply";

  const THEME_IDS = new Set(["systemLight", "systemDark", "terminal"]);

  type Arm = { id: string; appName: string; title: string };

  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }

  let themeId = $state(osThemeId());
  let windowOpacity = $state(1);
  let arm = $state<Arm | null>(null);
  let projects = $state<Project[]>([]);
  let planetId = $state("");
  let newName = $state("");
  let pendingName = "";
  let actSent = false;

  const line = $derived(
    arm
      ? `Put ${arm.appName} — ${arm.title || "window"} on a planet.`
      : "Put this window on a planet.",
  );

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

  async function refresh() {
    try {
      const s = await invoke<PersistedState>("load_state");
      if (THEME_IDS.has(s.themeId)) themeId = s.themeId;
      windowOpacity = s.windowOpacity ?? 1;
      projects = s.projects ?? [];
    } catch {
      projects = [];
    }
    try {
      arm = await invoke<Arm | null>("claim_prompt_peek");
    } catch {
      arm = null;
    }
    if (!projects.some((p) => p.id === planetId)) {
      planetId = projects[0]?.id ?? "";
    }
  }

  function promptScope() {
    return {
      get nudgeFor() {
        return arm?.id ?? null;
      },
      set nudgeFor(v: string | null) {
        if (v == null && !actSent) void emitTo("main", "claim-prompt:later");
        actSent = false;
      },
      get nudgeNewName() {
        return newName;
      },
      set nudgeNewName(v: string) {
        newName = v;
      },
      createProject(name: string): Project {
        pendingName = name.trim();
        return { id: "__new__", name: pendingName };
      },
      claim(windowId: string, projectId: string) {
        actSent = true;
        if (projectId === "__new__" && pendingName) {
          void emitTo("main", "claim-prompt:act", {
            kind: "create",
            windowId,
            name: pendingName,
          });
        } else {
          void emitTo("main", "claim-prompt:act", { kind: "claim", windowId, projectId });
        }
        pendingName = "";
      },
    };
  }

  function onClaim() {
    const id = arm?.id;
    if (!id) return;
    if (newName.trim()) {
      void invoke("claim_prompt_act", {
        kind: "create",
        windowId: id,
        name: newName.trim(),
      });
    } else if (planetId) {
      void invoke("claim_prompt_act", {
        kind: "claim",
        windowId: id,
        projectId: planetId,
      });
    }
  }

  function onLater() {
    void invoke("claim_prompt_later");
  }

  onMount(() => {
    const offs: Array<() => void> = [];
    void listen<{ label?: string; visible?: boolean }>("orbitty:pane-vis", (e) => {
      if (e.payload.label === "claim-prompt" && e.payload.visible) void refresh();
    }).then((un) => offs.push(un));
    void refresh();
    return () => {
      for (const un of offs) un();
    };
  });
</script>

<main class="claim-prompt">
  <p class="line" onpointerdown={startPaneDrag}>{line}</p>
  <label class="field">
    Planet
    <select bind:value={planetId} aria-label="Planet">
      {#each projects as p (p.id)}
        <option value={p.id}>{p.name}</option>
      {/each}
    </select>
  </label>
  <input
    bind:value={newName}
    placeholder="New planet"
    aria-label="New planet"
    onkeydown={(e) => e.key === "Enter" && onClaim()}
  />
  <div class="actions">
    <button type="button" onclick={onClaim}>Claim</button>
    <button type="button" class="later" onclick={onLater}>Later</button>
  </div>
</main>

<style>
  .claim-prompt {
    box-sizing: border-box;
    height: 100vh;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    color: var(--ink);
    background: var(--bg-fill, var(--bg, #1e1e1e));
    border-radius: 12px;
    font: 13px/1.4 "SF Pro Text", system-ui, sans-serif;
  }

  .line {
    margin: 0;
    cursor: grab;
    user-select: none;
    white-space: normal;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--dim, #888);
    font-size: 11px;
  }

  select,
  input,
  button {
    font: inherit;
    color: var(--ink);
    background: var(--panel-fill, var(--panel, #2a2a2a));
    border: 1px solid var(--edge, #444);
    border-radius: 6px;
    padding: 4px 8px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  button {
    cursor: pointer;
  }

  .later {
    background: transparent;
  }
</style>
