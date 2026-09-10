<script lang="ts">
  // GO-ORBITTY-NEXT-CARD leftover: own small window (not dock chrome).
  // ⌘⇧N or the dock pip toggles it. Drag the banner. Click-off (blur),
  // Esc, ×, or the chord again hides. Quiet ochre — no red badge.
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy, tick } from "svelte";
  import {
    applyHostTokens,
    applyWindowOpacity,
    clampWindowOpacity,
  } from "../themes/apply";
  import type {
    AverageWindow,
    Claim,
    KanbanCard,
    PersistedState,
    Project,
    StarsCheckin,
    StarsSlice,
    WindowInfo,
  } from "./persist";

  const THEME_IDS = new Set(["systemLight", "systemDark", "terminal"]);
  function osThemeId(): string {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "systemDark"
      : "systemLight";
  }
  let themeId = $state(osThemeId());
  let windowOpacity = $state(1.0);

  const DAILY_CAP = 12;
  const STARS_SPOKES = [
    { key: "sleep", label: "Sleep" },
    { key: "terroir", label: "Terroir" },
    { key: "athleticism", label: "Athleticism" },
    { key: "relationships", label: "Relationships" },
    { key: "self", label: "Self" },
  ] as const;
  type SpokeKey = (typeof STARS_SPOKES)[number]["key"];

  let cards = $state<KanbanCard[]>([]);
  let projects = $state<Project[]>([]);
  let claims = $state<Claim[]>([]);
  let stars = $state<StarsSlice>({ promptsOptIn: false, checkins: {} });
  let windows = $state<WindowInfo[]>([]);
  let lpTab = $state<"needed" | "todo">("needed");
  let answerDraft = $state("");
  let answerEl = $state<HTMLInputElement | null>(null);
  let newPlanet = $state("");
  let starsWalk = $state<number | null>(null);

  type Pile = "launchpad" | "stars" | "inspector";
  let pile = $state<Pile>("launchpad");

  function todayKey(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function nowSecs() {
    return Date.now() / 1000;
  }
  function neededOpen(c: KanbanCard): boolean {
    if (c.kind !== "needed") return false;
    if (c.neededState === "open") return true;
    if (c.neededState === "later") {
      const due = c.dueAt ?? 0;
      return due <= nowSecs();
    }
    return false;
  }

  let neededPile = $derived(
    cards.filter(neededOpen).sort((a, b) => a.createdAt - b.createdAt),
  );
  let todoPile = $derived(
    cards
      .filter((c) => c.kind !== "needed" && c.kind !== "window" && c.status !== "done")
      .sort((a, b) => a.createdAt - b.createdAt),
  );
  let neededCard = $derived(neededPile[0]);
  let todoCard = $derived(todoPile[0]);

  $effect(() => {
    if (lpTab === "needed" && !neededCard && todoCard) lpTab = "todo";
    if (lpTab === "todo" && !todoCard && neededCard) lpTab = "needed";
  });

  $effect(() => {
    const id = neededCard?.id;
    void id;
    answerDraft = "";
    if (pile !== "launchpad" || lpTab !== "needed" || !neededCard) return;
    tick().then(() => answerEl?.focus());
  });

  let todayCheck = $derived(stars.checkins[todayKey()]);
  let nextSpoke = $derived.by((): { key: SpokeKey; label: string } | null => {
    if (starsWalk == null) return todayCheck ? null : STARS_SPOKES[0];
    return STARS_SPOKES[starsWalk] ?? null;
  });
  let avgWindow = $derived<AverageWindow>(stars.averageWindow ?? "week");
  let avgMean = $derived.by(() => {
    if (!todayCheck) return null;
    const days = avgWindow === "week" ? 7 : avgWindow === "14" ? 14 : 30;
    const end = new Date();
    const keys: string[] = [];
    for (let i = 0; i < days; i++) {
      const d = new Date(end);
      d.setDate(end.getDate() - i);
      keys.push(
        `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`,
      );
    }
    const saved = keys
      .map((k) => stars.checkins[k])
      .filter((c): c is StarsCheckin => !!c);
    if (saved.length === 0) return null;
    const sum = saved.reduce(
      (a, c) =>
        a +
        (c.sleep + c.terroir + c.athleticism + c.relationships + c.self) / 5,
      0,
    );
    return sum / saved.length;
  });

  let liveClaims = $derived(
    claims.filter((c) => projects.some((p) => p.id === c.projectId)),
  );
  let unclaimed = $derived(
    windows.filter((w) => !liveClaims.some((c) => c.windowId === w.id)),
  );
  let claimWin = $derived(unclaimed[0]);

  function hide() {
    invoke("hide_window", { label: "next-card" }).catch(() => {});
  }

  function onBannerMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("button, input, a, select, textarea, label")) return;
    e.preventDefault();
    getCurrentWindow().startDragging().catch(() => {});
  }

  async function load() {
    try {
      const s = await invoke<PersistedState>("load_state");
      themeId = THEME_IDS.has(s.themeId) ? s.themeId : osThemeId();
      windowOpacity = clampWindowOpacity(s.windowOpacity ?? 1);
      cards = s.kanbanCards ?? [];
      projects = s.projects ?? [];
      claims = s.claims ?? [];
      stars = s.stars ?? { promptsOptIn: false, checkins: {} };
    } catch {
      /* next state:changed */
    }
    try {
      windows = await invoke<WindowInfo[]>("list_windows");
    } catch {
      windows = [];
    }
  }

  async function mutate(fn: (s: PersistedState) => void) {
    try {
      const s = await invoke<PersistedState>("load_state");
      s.kanbanCards ??= [];
      s.pointsLedger ??= [];
      s.claims ??= [];
      s.projects ??= [];
      s.stars ??= { promptsOptIn: false, checkins: {} };
      fn(s);
      await invoke("save_state", { state: s });
      await load();
    } catch {
      /* retry on next event */
    }
  }

  function logEvent(
    eType: string,
    content: Record<string, unknown>,
    projectId?: string,
  ) {
    invoke("log_event", {
      eType,
      content,
      projectId: projectId || null,
      actor: "user",
    }).catch(() => {});
  }

  function answerNeeded(card: KanbanCard) {
    const answer = answerDraft.trim();
    if (!answer) return;
    const now = nowSecs();
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              neededAnswer: answer,
              neededState: "answered" as const,
              lastTouchedAt: now,
            }
          : c,
      );
    });
    logEvent("needed-answer", { cardId: card.id }, card.projectId);
    answerDraft = "";
  }

  function skipNeeded(card: KanbanCard) {
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "skipped" as const, lastTouchedAt: nowSecs() }
          : c,
      );
    });
    logEvent("needed-skip", { cardId: card.id }, card.projectId);
  }

  function laterNeeded(card: KanbanCard) {
    const now = nowSecs();
    mutate((s) => {
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? { ...c, neededState: "later" as const, dueAt: now + 86400, lastTouchedAt: now }
          : c,
      );
    });
    logEvent("needed-later", { cardId: card.id }, card.projectId);
  }

  function completeTodo(card: KanbanCard) {
    const now = nowSecs();
    const day = todayKey();
    mutate((s) => {
      const todayPts = (s.pointsLedger ?? [])
        .filter((e) => e.day === day)
        .reduce((a, e) => a + e.amount, 0);
      const already = (s.pointsLedger ?? []).some((e) => e.cardId === card.id);
      const base = card.clinicalOrMaster ? 2 : 1;
      const award = !already && todayPts + base <= DAILY_CAP ? base : 0;
      s.kanbanCards = s.kanbanCards.map((c) =>
        c.id === card.id
          ? {
              ...c,
              status: "done" as const,
              completedAt: now,
              lastTouchedAt: now,
              pointsAwarded: award,
            }
          : c,
      );
      if (award > 0) {
        s.pointsLedger = [
          ...(s.pointsLedger ?? []),
          {
            id: `pt-${Date.now().toString(36)}`,
            cardId: card.id,
            amount: award,
            day,
            at: now,
            actor: "user" as const,
          },
        ];
      }
    });
    logEvent("board-move", { from: card.status, to: "done", cardId: card.id }, card.projectId);
  }

  function saveSpoke(key: SpokeKey, v: number) {
    const day = todayKey();
    mutate((s) => {
      const st = s.stars ?? { promptsOptIn: false, checkins: {} };
      const cur: StarsCheckin = st.checkins[day] ?? {
        sleep: 0,
        terroir: 0,
        athleticism: 0,
        relationships: 0,
        self: 0,
      };
      cur[key] = v;
      st.checkins = { ...st.checkins, [day]: cur };
      s.stars = st;
    });
    const i = STARS_SPOKES.findIndex((x) => x.key === key);
    starsWalk = i >= 0 && i + 1 < STARS_SPOKES.length ? i + 1 : 5;
  }

  function setAvg(w: AverageWindow) {
    mutate((s) => {
      const st = s.stars ?? { promptsOptIn: false, checkins: {} };
      st.averageWindow = w;
      s.stars = st;
    });
  }

  function newId(prefix: string) {
    return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  }

  function claimWindow(windowId: string, projectId: string) {
    const now = nowSecs();
    const w = windows.find((x) => x.id === windowId);
    mutate((s) => {
      const prev = (s.claims ?? []).find((c) => c.windowId === windowId);
      s.claims = [
        ...(s.claims ?? []).filter((c) => c.windowId !== windowId),
        {
          windowId,
          projectId,
          lastTouchedAt: now,
          spritePackId: prev?.spritePackId ?? null,
          charSeed: prev?.charSeed ?? (Math.floor(Math.random() * 0xffffffff) >>> 0),
          walkerHome: null,
          ...(w
            ? {
                bundleId: w.bundleId ?? null,
                appName: w.appName,
                titleAtClaim: w.title,
              }
            : {}),
        },
      ];
      const title = w
        ? w.title
          ? `${w.appName} — ${w.title}`
          : w.appName
        : "Window";
      const open = s.kanbanCards.find(
        (c) => c.windowId === windowId && c.status !== "done",
      );
      if (open) {
        s.kanbanCards = s.kanbanCards.map((c) =>
          c.id === open.id
            ? { ...c, projectId, windowId, title: c.title || title, kind: c.kind ?? "window", status: "todo" as const, lastTouchedAt: now }
            : c,
        );
      } else {
        s.kanbanCards = [
          ...s.kanbanCards,
          {
            id: newId("c"),
            title,
            projectId,
            windowId,
            kind: "window" as const,
            status: "todo" as const,
            createdAt: now,
            lastTouchedAt: now,
            completedAt: null,
            goalAligned: true,
            clinicalOrMaster: false,
            pointsAwarded: 0,
            starred: false,
          },
        ];
      }
    });
  }

  function addPlanetAndClaim(windowId: string) {
    const name = newPlanet.trim();
    if (!name) return;
    const id = newId("p");
    mutate((s) => {
      s.projects = [...(s.projects ?? []), { id, name }];
    }).then(() => claimWindow(windowId, id));
    newPlanet = "";
  }

  $effect(() => {
    if (!themeId) return;
    document.documentElement.dataset.theme = themeId;
    applyHostTokens(themeId);
    applyWindowOpacity(windowOpacity);
  });

  let unlisten: (() => void) | null = null;
  let unfocus: (() => void) | null = null;
  onMount(() => {
    void load();
    listen("state:changed", load).then((u) => {
      unlisten = u;
    });
    getCurrentWindow()
      .onFocusChanged((ev) => {
        if (!ev.payload) hide();
      })
      .then((u) => {
        unfocus = u;
      });
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      hide();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });
  onDestroy(() => {
    unlisten?.();
    unfocus?.();
  });
</script>

<main class="nextwin">
  <header class="nc-banner" onmousedown={onBannerMouseDown}>
    <span class="nc-tabs" role="tablist">
      <button class="nc-tab" class:on={pile === "launchpad"} onclick={() => (pile = "launchpad")}
        >Launchpad</button
      >
      <button class="nc-tab" class:on={pile === "stars"} onclick={() => (pile = "stars")}>STARS</button>
      <button class="nc-tab" class:on={pile === "inspector"} onclick={() => (pile = "inspector")}
        >Planets</button
      >
    </span>
  </header>
  <div class="nc-body">
    {#if pile === "launchpad"}
      <header class="nc-head">
        <span class="nc-kicker">Launchpad</span>
        <span class="nc-tabs" role="tablist">
          {#if neededCard}
            <button
              class="nc-tab"
              class:on={lpTab === "needed"}
              onclick={() => (lpTab = "needed")}>Needed</button
            >
          {/if}
          {#if todoCard}
            <button
              class="nc-tab"
              class:on={lpTab === "todo"}
              onclick={() => (lpTab = "todo")}>Todo</button
            >
          {/if}
        </span>
      </header>
      {#if lpTab === "needed" && neededCard}
        <p class="nc-title">{neededCard.title}</p>
        {#if neededCard.neededWhy}
          <p class="nc-why">{neededCard.neededWhy}</p>
        {/if}
        <div class="nc-answer">
          <input
            bind:this={answerEl}
            bind:value={answerDraft}
            placeholder="Your answer…"
            aria-label="Your answer"
            onkeydown={(e) => {
              if (e.key === "Enter") answerNeeded(neededCard);
            }}
          />
        </div>
        <div class="nc-acts">
          <button class="mini ghost" onclick={() => skipNeeded(neededCard)}>Skip</button>
          <button class="mini ghost" onclick={() => laterNeeded(neededCard)}>Later</button>
        </div>
      {:else if todoCard}
        <p class="nc-title">{todoCard.title}</p>
        <div class="nc-acts">
          <button class="mini" onclick={() => completeTodo(todoCard)}>Done</button>
        </div>
      {:else}
        <p class="nc-why">No Needed or todos.</p>
      {/if}
    {:else if pile === "stars"}
      <header class="nc-head">
        <span class="nc-kicker">STARS</span>
      </header>
      {#if nextSpoke}
        <p class="nc-title">{nextSpoke.label}</p>
        <div class="nc-acts" role="group" aria-label="{nextSpoke.label} 0 to 4">
          {#each [0, 1, 2, 3, 4] as v}
            <button class="mini" onclick={() => saveSpoke(nextSpoke.key, v)}>{v}</button>
          {/each}
        </div>
      {:else}
        <p class="nc-title">
          Today saved{#if avgMean != null}
            · mean {avgMean.toFixed(1)}{/if}
        </p>
        <div class="nc-acts">
          <button class="mini ghost" class:on={avgWindow === "week"} onclick={() => setAvg("week")}
            >week</button
          >
          <button class="mini ghost" class:on={avgWindow === "14"} onclick={() => setAvg("14")}
            >14</button
          >
          <button class="mini ghost" class:on={avgWindow === "month"} onclick={() => setAvg("month")}
            >month</button
          >
        </div>
      {/if}
    {:else if claimWin}
      <header class="nc-head">
        <span class="nc-kicker">Planets</span>
      </header>
      <p class="nc-title">{claimWin.appName}{claimWin.title ? ` — ${claimWin.title}` : ""}</p>
      <div class="nc-acts">
        {#each projects.slice(0, 4) as p (p.id)}
          <button class="mini" onclick={() => claimWindow(claimWin.id, p.id)}>{p.name}</button>
        {/each}
      </div>
      <div class="nc-answer">
        <input
          bind:value={newPlanet}
          placeholder="+ Planet"
          aria-label="New planet name"
          onkeydown={(e) => {
            if (e.key === "Enter") addPlanetAndClaim(claimWin.id);
          }}
        />
        <button class="mini" onclick={() => addPlanetAndClaim(claimWin.id)}>+</button>
      </div>
    {:else}
      <p class="nc-why">Nothing waiting on this pile.</p>
    {/if}
  </div>
</main>

<style>
  .nextwin {
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    color: var(--ink);
    background: var(--bg-fill, var(--panel-fill, var(--panel)));
    border: 1px solid var(--edge);
    border-radius: 12px;
    overflow: hidden;
  }
  .nc-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 6px;
    cursor: grab;
  }
  .nc-body {
    flex: 1;
    min-height: 0;
    padding: 4px 12px 12px;
  }
  .nc-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 4px;
  }
  .nc-kicker {
    font-size: 0.68rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--dim);
  }
  .nc-tabs {
    display: flex;
    gap: 4px;
  }
  .nc-tab,
  .mini {
    font: inherit;
    font-size: 0.78rem;
    padding: 2px 8px;
    border-radius: 8px;
    border: 1px solid var(--edge);
    background: transparent;
    color: var(--ink);
    cursor: pointer;
  }
  .nc-tab.on,
  .mini.on {
    background: var(--panel-fill, var(--panel));
  }
  .mini.ghost {
    color: var(--dim);
  }
  .nc-title {
    margin: 0 0 4px;
    font-size: 0.92rem;
    line-height: 1.3;
  }
  .nc-why {
    margin: 0 0 6px;
    font-size: 0.78rem;
    color: var(--dim);
  }
  .nc-acts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }
  .nc-answer {
    display: flex;
    gap: 6px;
    margin: 6px 0 4px;
  }
  .nc-answer input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.82rem;
    padding: 4px 8px;
    border-radius: 8px;
    border: 1px solid var(--edge);
    background: var(--panel-fill, var(--panel));
    color: var(--ink);
  }
</style>
