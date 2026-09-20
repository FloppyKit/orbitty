<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { PersistedState } from "../persist";

  let {
    claimToast = $bindable(null),
    claimToastTimer = $bindable(undefined),
  }: {
    claimToast: {
      projectId: string;
      projectName: string;
      windowId: string;
      windowTitle?: string;
      seconds: number;
    } | null;
    claimToastTimer: ReturnType<typeof setTimeout> | undefined;
  } = $props();
</script>

{#if claimToast}
  <div class="claim-toast" role="status">
    <span>Claimed {claimToast.windowTitle || "window"} to {claimToast.projectName}</span>
    <button
      type="button"
      onclick={() => {
        if (!claimToast) return;
        void (async () => {
          try {
            const s = await invoke<PersistedState>("load_state");
            const list = s.projects ?? [];
            if (!list.length) return;
            const i = list.findIndex((p) => p.id === claimToast!.projectId);
            const next = list[(i + 1) % list.length];
            if (!next) return;
            await invoke("claim_front_change", { projectId: next.id });
            claimToast = {
              ...claimToast!,
              projectId: next.id,
              projectName: next.name,
            };
          } catch {
            /* ignore */
          }
        })();
      }}>Change</button
    >
    <button
      type="button"
      onclick={() => {
        invoke("claim_front_undo").catch(() => {});
        claimToast = null;
        if (claimToastTimer) clearTimeout(claimToastTimer);
      }}>Undo</button
    >
  </div>
{/if}

