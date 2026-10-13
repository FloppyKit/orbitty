<script lang="ts">
  // Settings → History: drift status and review, the history list, rewind,
  // bring back, undo. Every write goes through relay/rewind.rs.
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import {
    MISSED_WRITE,
    alsoRemove,
    doneWords,
    driftEmpty,
    driftHeadline,
    eventWords,
    label,
    rewindConfirm,
    rowWords,
    shown,
    skippedWords,
    when,
    who,
    type Drift,
    type HistoryEvent,
    type HistoryPage,
    type HistoryRow,
    type Plan,
  } from "./history/words";

  const STEP = 50;

  let drift = $state<Drift | null>(null);
  let reviewing = $state(false);
  let page = $state<HistoryPage>({ rows: [], more: false, canUndo: false });
  let limit = $state(STEP);
  let open = $state<Record<string, boolean>>({});
  let busy = $state(false);
  let note = $state("");
  let err = $state("");

  interface Ask {
    key: string;
    to: string;
    at: number;
    card: string | null;
    title: string;
    removeAdded: boolean;
    plan: Plan;
  }
  let ask = $state<Ask | null>(null);

  async function check() {
    err = "";
    try {
      drift = await invoke<Drift>("history_drift");
    } catch (e) {
      err = String(e);
    }
  }

  async function load() {
    try {
      page = await invoke<HistoryPage>("history_list", { limit });
    } catch (e) {
      err = String(e);
    }
  }

  async function refresh() {
    await Promise.all([check(), load()]);
  }

  async function more() {
    limit += STEP;
    await load();
  }

  async function plan(to: string, card: string | null, removeAdded: boolean): Promise<Plan | null> {
    try {
      return await invoke<Plan>("history_plan", { to, card, removeAdded });
    } catch (e) {
      err = String(e);
      return null;
    }
  }

  async function askRewind(row: HistoryRow) {
    err = "";
    note = "";
    const p = await plan(row.id, null, false);
    if (p) ask = { key: `r:${row.id}`, to: row.id, at: row.at, card: null, title: "", removeAdded: false, plan: p };
  }

  async function askBack(row: HistoryRow, e: HistoryEvent) {
    err = "";
    note = "";
    const p = await plan(e.id, e.target, false);
    if (p) ask = { key: `b:${e.id}`, to: e.id, at: row.at, card: e.target, title: e.title, removeAdded: false, plan: p };
  }

  async function toggleRemove() {
    if (!ask) return;
    const next = !ask.removeAdded;
    const p = await plan(ask.to, ask.card, next);
    if (p && ask) ask = { ...ask, removeAdded: next, plan: p };
  }

  async function run(cmd: string, args: Record<string, unknown>, verb: string) {
    busy = true;
    err = "";
    try {
      const p = await invoke<Plan>(cmd, args);
      note = doneWords(p, verb);
      ask = null;
    } catch (e) {
      err = String(e);
    }
    busy = false;
    await refresh();
  }

  function confirmAsk() {
    if (!ask) return;
    const verb = ask.card ? "Brought back" : "Rewound";
    void run("history_apply", { to: ask.to, card: ask.card, removeAdded: ask.removeAdded }, verb);
  }

  function repair() {
    reviewing = false;
    void run("history_repair", {}, "Repaired from history");
  }

  async function keep() {
    busy = true;
    err = "";
    try {
      await invoke("history_keep");
      note = "Kept the board as it is.";
      reviewing = false;
    } catch (e) {
      err = String(e);
    }
    busy = false;
    await refresh();
  }

  function undo() {
    void run("history_undo", {}, "Undid the last rewind");
  }

  onMount(() => {
    void refresh();
  });
</script>

{#snippet confirmBox(a: Ask)}
  <div class="h-confirm">
    {#if a.card}
      <p class="help">Bring back "{a.title}" as it was before it was deleted?</p>
    {:else}
      <p class="help">{rewindConfirm(when(a.at), a.plan)}</p>
      {#if a.plan.added.length}
        <label class="help h-check">
          <input type="checkbox" checked={a.removeAdded} onchange={toggleRemove} />
          {alsoRemove(a.plan.added.length)}
        </label>
      {/if}
    {/if}
    {#each a.plan.skipped as s (s.id)}
      <p class="help">{skippedWords(s)}</p>
    {/each}
    <div class="setting-row">
      <button class="mini" disabled={busy} onclick={confirmAsk}>{a.card ? "Bring back" : "Rewind"}</button>
      <button class="mini ghost" onclick={() => (ask = null)}>Cancel</button>
    </div>
  </div>
{/snippet}

<section class="panel">
  <p class="block-label">Board</p>
  {#if drift && !driftEmpty(drift)}
    <div class="setting-row">
      <span class="h-status err">{driftHeadline(drift)}.</span>
      <button class="mini" onclick={() => (reviewing = !reviewing)}>Review</button>
      <button class="mini ghost" onclick={check}>Check now</button>
    </div>
    {#if reviewing}
      {#if drift.missedWrite}
        <p class="help err">{MISSED_WRITE}</p>
      {/if}
      <ul class="action-list">
        {#each drift.cards as c (c.id)}
          {#each c.fields as f (f.field)}
            <li class="action-row">
              <span class="a-name">{c.title}</span>
              <span class="a-cmd">{label(f.field)} · now: {shown(f.field, f.now)} · history: {shown(f.field, f.history)}</span>
            </li>
          {/each}
        {/each}
        {#each drift.missing as n (n.id)}
          <li class="action-row">
            <span class="a-name">{n.title}</span>
            <span class="a-cmd">not on the board · repair brings it back</span>
          </li>
        {/each}
        {#each drift.unknown as n (n.id)}
          <li class="action-row">
            <span class="a-name">{n.title}</span>
            <span class="a-cmd">not in the history · kept either way</span>
          </li>
        {/each}
        {#each drift.planets as p (p.id)}
          {#each p.fields as f (f.field)}
            <li class="action-row">
              <span class="a-name">Planet {p.title}</span>
              <span class="a-cmd">{label(f.field)} · now: {shown(f.field, f.now)} · history: {shown(f.field, f.history)}</span>
            </li>
          {/each}
        {/each}
        {#each drift.planetsMissing as n (n.id)}
          <li class="action-row">
            <span class="a-name">Planet {n.title}</span>
            <span class="a-cmd">not on the board · repair brings it back</span>
          </li>
        {/each}
        {#each drift.planetsUnknown as n (n.id)}
          <li class="action-row">
            <span class="a-name">Planet {n.title}</span>
            <span class="a-cmd">not in the history · kept either way</span>
          </li>
        {/each}
      </ul>
      <div class="setting-row">
        <button class="mini" disabled={busy} onclick={repair}>Repair from history</button>
        <button class="mini" disabled={busy} onclick={keep}>Keep the board as it is</button>
      </div>
    {/if}
  {:else if drift}
    <div class="setting-row">
      <span class="h-status">Board matches its history.</span>
      <button class="mini ghost" onclick={check}>Check now</button>
    </div>
  {/if}
  {#if page.canUndo}
    <div class="setting-row">
      <button class="mini" disabled={busy} onclick={undo}>Undo last rewind</button>
    </div>
  {/if}
  {#if note}
    <p class="help">{note}</p>
  {/if}
  {#if err}
    <p class="help err">{err}</p>
  {/if}
</section>

<section class="panel">
  <p class="block-label">History</p>
  {#if page.rows.length === 0}
    <p class="help">No history yet.</p>
  {/if}
  <ul class="action-list">
    {#each page.rows as row (row.id)}
      {@const single = row.events.length === 1 ? row.events[0] : null}
      <li>
        <div class="action-row">
          <span class="h-when">{when(row.at)}</span>
          <span class="h-who">{who(row.actor)}</span>
          {#if single}
            <span class="a-cmd">{rowWords(row)}</span>
          {:else}
            <button class="a-name h-fold" onclick={() => (open[row.id] = !open[row.id])}>{rowWords(row)}</button>
          {/if}
          <button class="mini" onclick={() => askRewind(row)}>Rewind board to here</button>
          {#if single?.what === "deleted"}
            <button class="mini" onclick={() => askBack(row, single)}>Bring back</button>
          {/if}
        </div>
        {#if !single && open[row.id]}
          <ul class="action-list h-sub">
            {#each row.events as e (e.id)}
              <li class="action-row">
                <span class="a-cmd">{eventWords(e)}</span>
                {#if e.what === "deleted"}
                  <button class="mini" onclick={() => askBack(row, e)}>Bring back</button>
                {/if}
              </li>
              {#if ask?.key === `b:${e.id}`}
                {@render confirmBox(ask)}
              {/if}
            {/each}
          </ul>
        {/if}
        {#if ask?.key === `r:${row.id}` || (single && ask?.key === `b:${single.id}`)}
          {@render confirmBox(ask)}
        {/if}
      </li>
    {/each}
  </ul>
  {#if page.more}
    <div class="setting-row">
      <button class="mini" onclick={more}>Show more</button>
    </div>
  {/if}
</section>

<style>
.panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
    margin-bottom: 10px;
  }
.block-label {
    margin: 8px 0 4px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
.setting-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
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
.mini.ghost {
    background: transparent;
  }
.mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
.help {
    margin: 8px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
  }
.help.err,
.h-status.err {
    color: var(--accent);
  }
.h-status {
    font-size: 0.72rem;
    color: var(--ink);
    margin-right: 4px;
  }
.action-list {
    list-style: none;
    margin: 2px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
.action-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-height: 22px;
  }
.a-name {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: inherit;
    font-size: 0.74rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 5px;
    padding: 2px 6px;
  }
.h-fold {
    flex: 1;
    max-width: none;
    text-align: left;
    cursor: pointer;
  }
.h-fold:hover {
    color: var(--accent);
    background: var(--bg-fill, var(--bg));
  }
.a-cmd {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--ink);
  }
.h-when,
.h-who {
    flex-shrink: 0;
    font-size: 0.64rem;
    color: var(--dim);
  }
.h-when {
    min-width: 5.5rem;
  }
.h-who {
    min-width: 4.5rem;
  }
.h-sub {
    margin-left: 10.5rem;
  }
.h-confirm {
    margin: 2px 0 8px 10.5rem;
  }
.h-check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
