<script lang="ts">
  // GO-ORBITTY-NEEDED-RULES-ANSWER — "if this again": attach a standing
  // rule while answering a Needed. Lock: ~/.buzz/PLANS/ORBITTY_NEEDED_RULES_LOCK_v0.md
  //
  // Default OFF: the row is closed, and never opening it writes nothing
  // (today's behavior). Open → the lock's small param list, no model:
  // flavor + go check/filled from THIS card's title (same classify table
  // as needed_rules.rs / orbitty_needed_rules.py), optional prompt_kind /
  // project / title_prefix / title_exact, optional issued. The standing
  // answer defaults to the answer he just typed (RULE: stanza stripped —
  // that hand-typed door stays the watch's job to record).
  //
  // Save is a thin invoke → `orbitty-needed rule add` (the python CLI
  // owns rules.json: keys, 48 cap, atomic save, log). Save NEVER answers
  // or dispatches this card — the normal Answer/Send path is untouched,
  // and the answer text is never stuffed with a RULE: stanza.
  // Cancel = no rule. Rules are user-minted only — nothing here fires
  // without Matt pressing Save.
  import { invoke } from "@tauri-apps/api/core";
  import type { KanbanCard } from "./persist";

  let {
    card,
    answer,
  }: {
    card: KanbanCard;
    answer: string; // the answer draft in the host's real answer box
  } = $props();

  const FLAVORS = ["dogfood", "writer", "writer-leftover", "fact"] as const;
  const PROMPT_KINDS = ["first", "leftover"] as const;
  const ISSUED_VALUES = ["shipped", "running", "absent"] as const;

  // The bus's GO token: \b(GO-[A-Z0-9][-A-Z0-9]*)\b on the uppercased,
  // de-questioned text.
  const GO_RE = /\b(GO-[A-Z0-9][-A-Z0-9]*)\b/;
  function extractGo(text: string): string | null {
    const m = GO_RE.exec((text || "").toUpperCase().replace(/\?/g, " "));
    return m ? m[1] : null;
  }
  // A fact title with no token (doing-fire) matches go against the why's
  // go= — same fallback as the matcher.
  function whyGo(why: string): string {
    for (const tok of (why || "").split(/\s+/)) {
      if (tok.startsWith("go=")) {
        const v = tok.slice(3).trim();
        if (v) return v.toUpperCase();
      }
    }
    return "";
  }
  // Lock table, no model: Dogfood GO-…? / Writer for GO-… leftover? /
  // Writer for GO-…? / anything else = fact.
  function classify(title: string, why: string) {
    const t = (title || "").trim();
    const low = t.toLowerCase();
    const go = extractGo(t) || whyGo(why) || null;
    let flavor: string = "fact";
    let promptKind: string | null = null;
    if (t.startsWith("Dogfood ") && go) {
      flavor = "dogfood";
      if (low.includes("leftover")) promptKind = "leftover";
    } else if (t.startsWith("Writer for ") && t.endsWith("?")) {
      if (low.includes("leftover")) {
        flavor = "writer-leftover";
        promptKind = "leftover";
      } else {
        flavor = "writer";
        promptKind = "first";
      }
    }
    return { go, flavor, promptKind };
  }
  // Trailing RULE: lines are teaching for the watch's from-answer door —
  // never let them leak into the standing answer this row saves.
  function stripRuleLines(text: string): string {
    const lines = (text || "").split("\n");
    let i = lines.length;
    while (i > 0 && /^RULE:\s*/.test(lines[i - 1].trim())) i--;
    return lines.slice(0, i).join("\n").trim();
  }

  const auto = $derived(classify(card.title, card.neededWhy ?? ""));

  let open = $state(false);
  let savedId = $state<string | null>(null);
  let note = $state<string | null>(null);
  let saving = $state(false);

  let ruleAnswer = $state("");
  let answerTouched = $state(false);

  let useFlavor = $state(true);
  let flavor = $state<string>("fact");
  let useGo = $state(false);
  let go = $state("");
  let usePromptKind = $state(false);
  let promptKind = $state<string>("first");
  let useProject = $state(false);
  let project = $state("");
  let useTitlePrefix = $state(false);
  let titlePrefix = $state("");
  let useTitleExact = $state(false);
  let titleExact = $state("");
  let useIssued = $state(false);
  let issued = $state<string>("shipped");

  function openRow() {
    // Seed from THIS card + the answer he just typed on every open —
    // a Cancel threw the edits away, a saved rule starts fresh.
    ruleAnswer = stripRuleLines(answer);
    answerTouched = false;
    useFlavor = true;
    flavor = auto.flavor;
    useGo = !!auto.go;
    go = auto.go ?? "";
    usePromptKind = false;
    promptKind = auto.promptKind ?? "first";
    useProject = false;
    project = card.projectId ?? "";
    useTitlePrefix = false;
    titlePrefix = card.title;
    useTitleExact = false;
    titleExact = card.title;
    useIssued = false;
    issued = "shipped";
    note = null;
    open = true;
  }

  function cancel() {
    open = false; // Cancel = no rule. Nothing was ever written.
    note = null;
  }

  // While the row's answer field is untouched, it tracks what he is
  // typing in the real answer box.
  $effect(() => {
    if (open && !answerTouched) ruleAnswer = stripRuleLines(answer);
  });

  // The lock refuses when.issued without params.go — the row's courtesy
  // is auto-checking go; the hint below (never a silent save) does the
  // rest.
  function onIssuedToggle() {
    if (useIssued && !useGo && auto.go) useGo = true;
  }

  let anyParam = $derived(
    useFlavor ||
      (useGo && go.trim().length > 0) ||
      usePromptKind ||
      (useProject && project.trim().length > 0) ||
      (useTitlePrefix && titlePrefix.trim().length > 0) ||
      (useTitleExact && titleExact.trim().length > 0),
  );
  let issuedNeedsGo = $derived(useIssued && !(useGo && go.trim().length > 0));
  let canSave = $derived(
    !saving && ruleAnswer.trim().length > 0 && anyParam && !issuedNeedsGo,
  );

  async function save() {
    if (!canSave) return;
    const params: [string, string][] = [];
    if (useFlavor) params.push(["flavor", flavor]);
    if (useGo && go.trim()) params.push(["go", go.trim()]);
    if (usePromptKind) params.push(["prompt_kind", promptKind]);
    if (useProject && project.trim()) params.push(["project", project.trim()]);
    if (useTitlePrefix && titlePrefix.trim())
      params.push(["title_prefix", titlePrefix.trim()]);
    if (useTitleExact && titleExact.trim())
      params.push(["title_exact", titleExact.trim()]);
    if (useIssued) params.push(["issued", issued]); // the CLI sorts it into `when`
    saving = true;
    note = null;
    try {
      const rule = await invoke<{ id?: string }>("needed_rule_add", {
        params,
        answer: ruleAnswer.trim(),
        fromNeeded: card.id,
      });
      savedId = rule?.id ?? null;
      open = false;
    } catch (e) {
      note = `${e}`; // the CLI's refusal, verbatim
    } finally {
      saving = false;
    }
  }
</script>

<div class="nrr">
  {#if !open}
    <button
      type="button"
      class="nrr-toggle"
      onclick={openRow}
      aria-expanded="false"
      title={savedId
        ? "Saved — future matches answer themselves. Click to teach another."
        : "Teach a standing rule for cards like this one — off unless you open it"}
    >
      <span class="nrr-chev" aria-hidden="true">▸</span>
      <span class="nrr-label"
        >{savedId ? `rule saved (${savedId})` : "if this again"}</span
      >
    </button>
  {:else}
    <div class="nrr-form">
      <div class="nrr-head">
        <button
          type="button"
          class="nrr-toggle"
          onclick={cancel}
          aria-expanded="true"
          title="Close — Cancel writes no rule"
        >
          <span class="nrr-chev open" aria-hidden="true">▸</span>
          <span class="nrr-label">if this again</span>
        </button>
        <span class="nrr-spacer"></span>
        <button
          type="button"
          class="nrr-btn"
          disabled={!canSave}
          onclick={save}
          title="Save the standing rule — this card still answers/dispatches as normal"
          >{saving ? "Saving…" : "Save rule"}</button
        >
        <button type="button" class="nrr-btn ghost" onclick={cancel}>Cancel</button>
      </div>
      <label class="nrr-field">
        <span class="nrr-key">standing answer</span>
        <input
          bind:value={ruleAnswer}
          oninput={() => (answerTouched = true)}
          placeholder="the answer, every time this matches"
          aria-label="Standing answer"
        />
      </label>
      <div class="nrr-param">
        <label class="nrr-check">
          <input type="checkbox" bind:checked={useFlavor} />
          <span class="nrr-key">flavor</span>
        </label>
        <select bind:value={flavor} disabled={!useFlavor} aria-label="flavor">
          {#each FLAVORS as f}<option value={f}>{f}</option>{/each}
        </select>
      </div>
      <div class="nrr-param">
        <label class="nrr-check">
          <input type="checkbox" bind:checked={useGo} disabled={!auto.go} />
          <span class="nrr-key">go</span>
        </label>
        <input
          bind:value={go}
          disabled={!useGo}
          placeholder={auto.go ? "" : "no GO token on this card"}
          spellcheck="false"
          aria-label="go"
        />
      </div>
      <div class="nrr-param">
        <label class="nrr-check">
          <input type="checkbox" bind:checked={usePromptKind} />
          <span class="nrr-key">prompt_kind</span>
        </label>
        <select
          bind:value={promptKind}
          disabled={!usePromptKind}
          aria-label="prompt_kind"
        >
          {#each PROMPT_KINDS as k}<option value={k}>{k}</option>{/each}
        </select>
      </div>
      <div class="nrr-param">
        <label class="nrr-check">
          <input
            type="checkbox"
            bind:checked={useProject}
            disabled={!(card.projectId ?? "")}
          />
          <span class="nrr-key">project</span>
        </label>
        <input
          bind:value={project}
          disabled={!useProject}
          placeholder="planet id or exact name"
          spellcheck="false"
          aria-label="project"
        />
      </div>
      <div class="nrr-param">
        <label class="nrr-check">
          <input type="checkbox" bind:checked={useTitlePrefix} />
          <span class="nrr-key">title_prefix</span>
        </label>
        <input
          bind:value={titlePrefix}
          disabled={!useTitlePrefix}
          spellcheck="false"
          aria-label="title_prefix"
        />
      </div>
      <div class="nrr-param">
        <label class="nrr-check">
          <input type="checkbox" bind:checked={useTitleExact} />
          <span class="nrr-key">title_exact</span>
        </label>
        <input
          bind:value={titleExact}
          disabled={!useTitleExact}
          spellcheck="false"
          aria-label="title_exact"
        />
      </div>
      <div class="nrr-param">
        <label
          class="nrr-check"
          title={auto.go
            ? "only when the writer lock for this go is…"
            : "issued needs a go param — no GO token on this card"}
        >
          <input
            type="checkbox"
            bind:checked={useIssued}
            disabled={!auto.go}
            onchange={onIssuedToggle}
          />
          <span class="nrr-key">issued</span>
        </label>
        <select bind:value={issued} disabled={!useIssued} aria-label="issued">
          {#each ISSUED_VALUES as v}<option value={v}>{v}</option>{/each}
        </select>
      </div>
      {#if !anyParam}
        <p class="nrr-note">pick at least one param — “this” has to pin something</p>
      {:else if issuedNeedsGo}
        <p class="nrr-note">issued needs the go param (the lock refuses it otherwise)</p>
      {/if}
      {#if note}<p class="nrr-note">{note}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .nrr {
    width: 100%;
    min-width: 0;
  }

  .nrr-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    margin: 0;
    padding: 2px 4px;
    font: inherit;
    font-size: 0.68rem;
    color: var(--dim);
    background: transparent;
    border: none;
    border-radius: 7px;
    cursor: pointer;
    text-align: left;
  }

  .nrr-toggle:hover {
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
  }

  .nrr-chev {
    flex: none;
    font-size: 0.58rem;
    transition: transform 0.12s;
  }

  .nrr-chev.open {
    transform: rotate(90deg);
  }

  .nrr-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nrr-form {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 6px 8px 8px;
    border: 1px solid var(--edge);
    border-radius: 9px;
  }

  .nrr-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .nrr-head .nrr-toggle {
    width: auto;
    padding: 0 2px;
  }

  .nrr-spacer {
    flex: 1;
  }

  .nrr-btn {
    font: inherit;
    font-size: 0.68rem;
    font-weight: 600;
    padding: 2px 9px;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    cursor: pointer;
  }

  .nrr-btn:hover {
    border-color: var(--focus, #6e4db0);
  }

  .nrr-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .nrr-btn.ghost {
    color: var(--dim);
    font-weight: 400;
  }

  .nrr-field,
  .nrr-param {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .nrr-check {
    flex: none;
    width: 96px;
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }

  .nrr-check input {
    margin: 0;
    accent-color: var(--focus, #6e4db0);
  }

  .nrr-field .nrr-key {
    flex: none;
    width: 96px;
  }

  .nrr-key {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 0.64rem;
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nrr-field input,
  .nrr-param input,
  .nrr-param select {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.72rem;
    padding: 3px 7px;
    color: var(--ink);
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 7px;
    outline: none;
  }

  .nrr-param select {
    flex: 0 1 auto;
  }

  .nrr-field input:focus,
  .nrr-param input:focus,
  .nrr-param select:focus {
    border-color: var(--focus, #6e4db0);
  }

  .nrr-param input:disabled,
  .nrr-param select:disabled {
    opacity: 0.45;
  }

  .nrr-note {
    margin: 0;
    font-size: 0.66rem;
    line-height: 1.4;
    color: var(--mark, #e59e3d);
    overflow-wrap: anywhere;
  }
</style>
