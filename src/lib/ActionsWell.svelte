<script lang="ts">
  // GO-ORBITTY-CLEAN-KANBAN-WELLS: Launchpad Actions macros. Same
  // actionShortcuts array as the rail. Agents well stays on KanbanPage.
  import { invoke } from "@tauri-apps/api/core";
  import type { ActionShortcut, KanbanCard, PersistedState } from "./persist";

  let {
    actionShortcuts,
    cards,
    mutate,
    onFocusCard,
  }: {
    actionShortcuts: ActionShortcut[];
    cards: KanbanCard[];
    mutate: (fn: (s: PersistedState) => void) => void;
    onFocusCard: (cardId: string) => void;
  } = $props();

  interface ActionResult {
    ok: boolean;
    code: number | null;
    stdout: string;
    stderr: string;
  }

  let actionNote = $state<{ kind: "ok" | "err"; text: string } | null>(null);
  let addingAction = $state(false);
  let newActionName = $state("");
  let newActionCommand = $state("");
  let newActionCwd = $state("");
  let runningActionId = $state<string | null>(null);

  let nextTodo = $derived.by(() => {
    const card = cards.find(
      (c) => c.kind !== "needed" && c.status === "doing",
    );
    if (!card) return null;
    const sub = card.subtasks?.find((s) => !s.done);
    return { cardId: card.id, text: sub?.title || card.title };
  });

  function addAction() {
    const name = newActionName.trim();
    const command = newActionCommand.trim();
    if (!name || !command) return;
    const cwd = newActionCwd.trim() || null;
    mutate((s) => {
      s.actionShortcuts = [
        ...(s.actionShortcuts ?? []),
        {
          id: `a-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
          name,
          command,
          cwd,
        },
      ];
    });
    newActionName = "";
    newActionCommand = "";
    newActionCwd = "";
    addingAction = false;
  }

  function deleteAction(id: string) {
    mutate((s) => {
      s.actionShortcuts = (s.actionShortcuts ?? []).filter((a) => a.id !== id);
    });
  }

  async function runAction(a: ActionShortcut) {
    if (runningActionId) return;
    runningActionId = a.id;
    actionNote = null;
    try {
      const r = await invoke<ActionResult>("run_action", {
        command: a.command,
        cwd: a.cwd ?? null,
      });
      const out = [r.stdout, r.stderr].filter(Boolean).join("\n");
      const head = r.ok ? `${a.name}: done` : `${a.name}: exit ${r.code ?? "?"}`;
      actionNote = {
        kind: r.ok ? "ok" : "err",
        text: out ? `${head}\n${out}` : head,
      };
    } catch (e) {
      actionNote = { kind: "err", text: `${a.name}: ${e}` };
    }
    runningActionId = null;
  }
</script>

<section class="k-actions" aria-label="Actions">
  <header class="ka-head">
    <span class="ka-title">Actions</span>
    <button
      class="ka-plus"
      onclick={() => (addingAction = !addingAction)}
      title={addingAction ? "Cancel" : "Add an action"}
      aria-label={addingAction ? "Cancel add action" : "Add action"}
      aria-expanded={addingAction}>{addingAction ? "×" : "+"}</button
    >
  </header>

  {#if nextTodo}
    {@const t = nextTodo}
    <button
      class="ka-next"
      onclick={() => onFocusCard(t.cardId)}
      title="Focus the smallest open to-do — {t.text}"
    >
      <span class="ka-next-label">Next</span>
      <span class="ka-next-text">{t.text}</span>
    </button>
  {/if}

  {#each actionShortcuts as a (a.id)}
    <div class="ka-row">
      <button
        class="ka-run"
        class:running={runningActionId === a.id}
        onclick={() => runAction(a)}
        title="Run: {a.command}">{a.name}</button
      >
      <button
        class="ka-del"
        onclick={() => deleteAction(a.id)}
        title="Delete {a.name}"
        aria-label="Delete {a.name}">×</button
      >
    </div>
  {:else}
    {#if !addingAction}
      <p class="ka-hint">No actions yet.</p>
    {/if}
  {/each}

  {#if addingAction}
    <div class="ka-form">
      <input
        bind:value={newActionName}
        placeholder="Name"
        aria-label="Action name"
      />
      <input
        bind:value={newActionCommand}
        placeholder="Command"
        aria-label="Shell command"
        onkeydown={(e) => e.key === "Enter" && addAction()}
      />
      <input
        bind:value={newActionCwd}
        placeholder="Working directory (optional)"
        aria-label="Working directory (optional)"
        onkeydown={(e) => e.key === "Enter" && addAction()}
      />
      <button class="mini" onclick={addAction}>Save</button>
    </div>
  {/if}

  {#if actionNote}
    <div class="ka-note" class:err={actionNote.kind === "err"}>
      <span class="ka-notetext">{actionNote.text}</span>
      <button class="mini ghost" onclick={() => (actionNote = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}
</section>

<style>
  .k-actions {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .ka-head {
    display: flex;
    align-items: center;
    padding: 0 4px 0 10px;
  }

  .ka-title {
    flex: 1;
    font-size: 0.66rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--dim);
  }

  .ka-plus {
    font: inherit;
    font-size: 0.82rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 1px 6px;
    cursor: pointer;
  }

  .ka-plus:hover {
    color: var(--accent);
    background: var(--bg-fill, var(--bg));
  }

  .ka-next {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    font: inherit;
    font-size: 0.74rem;
    text-align: left;
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .ka-next:hover {
    border-color: var(--focus, #6e4db0);
  }

  .ka-next-label {
    flex-shrink: 0;
    font-size: 0.62rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }

  .ka-next-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }

  .ka-row {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ka-run {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: inherit;
    font-size: 0.78rem;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 7px;
    padding: 4px 10px;
    cursor: pointer;
  }

  .ka-run:hover {
    background: var(--bg-fill, var(--bg));
    color: var(--accent);
  }

  .ka-run.running {
    opacity: 0.55;
  }

  .ka-del {
    flex-shrink: 0;
    font: inherit;
    font-size: 0.74rem;
    line-height: 1;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 4px;
    padding: 1px 5px;
    cursor: pointer;
  }

  .ka-del:hover {
    color: #ff453a;
  }

  .ka-hint {
    margin: 0;
    padding: 0 10px;
    font-size: 0.66rem;
    color: var(--dim);
  }

  .ka-form {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 2px 0;
  }

  .ka-form input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.76rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 5px 8px;
  }

  .ka-form input:focus {
    outline: none;
    border-color: var(--focus, #6e4db0);
  }

  .ka-note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 5px 8px;
    font-size: 0.68rem;
    line-height: 1.4;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    word-break: break-all;
  }

  .ka-note.err {
    border-color: var(--accent);
  }

  .ka-notetext {
    flex: 1;
    min-width: 0;
    white-space: pre-wrap;
  }

  .mini {
    font: inherit;
    font-size: 0.64rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 3px 8px;
    cursor: pointer;
  }

  .mini.ghost {
    color: var(--dim);
  }
</style>
