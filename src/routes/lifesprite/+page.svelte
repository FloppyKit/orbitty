<script lang="ts">
  import { onMount } from "svelte";
  import BoardColumns from "$lib/lifesprite/BoardColumns.svelte";
  import { signAuth } from "$lib/lifesprite/auth";
  import { foldBoard, type BoardProof, type FoldedProject } from "$lib/lifesprite/fold";
  import { sampleProofs } from "$lib/lifesprite/fixtures";
  import { DEFAULT_RELAY, boardReq, readRelayFrame } from "$lib/lifesprite/relay";

  // The nsec lives in this component's memory for the sign-in field.
  // Nothing in this page writes it.

  const sample: FoldedProject[] = foldBoard(sampleProofs());

  let nsec = $state("");
  let relayUrl = $state(DEFAULT_RELAY);
  let relayStatus = $state("Checking the relay…");
  let relayDown = $state(false);
  let live = $state<FoldedProject[] | null>(null);
  let busy = $state(false);

  let socket: WebSocket | null = null;
  let probe: WebSocket | null = null;

  function closeSocket(ws: WebSocket | null) {
    if (!ws) return;
    ws.onopen = null;
    ws.onerror = null;
    ws.onclose = null;
    ws.onmessage = null;
    if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
      ws.close();
    }
  }

  function markDown(url: string) {
    relayDown = true;
    live = null;
    relayStatus = `The relay is down at ${url}.`;
  }

  function probeRelay(url: string) {
    closeSocket(probe);
    relayDown = false;
    relayStatus = "Checking the relay…";
    let opened = false;
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch {
      markDown(url);
      return;
    }
    probe = ws;
    const timer = window.setTimeout(() => {
      if (probe === ws && !opened) {
        closeSocket(ws);
        if (probe === ws) probe = null;
        markDown(url);
      }
    }, 2500);
    ws.onopen = () => {
      opened = true;
      window.clearTimeout(timer);
      if (probe !== ws) return;
      relayDown = false;
      relayStatus = "Relay is up. Sign in to read Todo, Doing, and Done.";
      closeSocket(ws);
      probe = null;
    };
    const fail = () => {
      window.clearTimeout(timer);
      if (probe !== ws || opened) return;
      closeSocket(ws);
      probe = null;
      markDown(url);
    };
    ws.onerror = fail;
    ws.onclose = fail;
  }

  function takeLive(url: string, secret: string) {
    closeSocket(probe);
    probe = null;
    closeSocket(socket);
    busy = true;
    relayDown = false;
    live = null;
    relayStatus = "Connecting…";
    const collected: BoardProof[] = [];
    let opened = false;
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch {
      busy = false;
      markDown(url);
      return;
    }
    socket = ws;
    const timer = window.setTimeout(() => {
      if (socket === ws && !opened) {
        closeSocket(ws);
        socket = null;
        busy = false;
        markDown(url);
      }
    }, 4000);
    ws.onopen = () => {
      opened = true;
    };
    ws.onerror = () => {
      if (socket !== ws) return;
      window.clearTimeout(timer);
      closeSocket(ws);
      socket = null;
      busy = false;
      if (!opened || live === null) markDown(url);
    };
    ws.onclose = () => {
      if (socket !== ws) return;
      window.clearTimeout(timer);
      socket = null;
      busy = false;
      if (!opened) markDown(url);
    };
    ws.onmessage = (msg) => {
      if (socket !== ws || typeof msg.data !== "string") return;
      let frame: unknown;
      try {
        frame = JSON.parse(msg.data);
      } catch {
        return;
      }
      const inbound = readRelayFrame(frame);
      if (inbound.type === "challenge") {
        let signed;
        try {
          signed = signAuth(secret, url, inbound.challenge);
        } catch {
          window.clearTimeout(timer);
          closeSocket(ws);
          socket = null;
          busy = false;
          relayDown = false;
          relayStatus = "That nsec could not be read.";
          return;
        }
        ws.send(JSON.stringify(["AUTH", signed]));
        return;
      }
      if (inbound.type === "auth-ok") {
        relayStatus = "Signed in. Asking for the board…";
        ws.send(JSON.stringify(boardReq()));
        return;
      }
      if (inbound.type === "auth-rejected" || inbound.type === "closed") {
        window.clearTimeout(timer);
        closeSocket(ws);
        socket = null;
        busy = false;
        relayDown = false;
        relayStatus = inbound.detail
          ? `The relay refused the sign-in (${inbound.detail}).`
          : "The relay refused the sign-in.";
        return;
      }
      if (inbound.type === "event") {
        collected.push(inbound.event);
        return;
      }
      if (inbound.type === "eose") {
        window.clearTimeout(timer);
        live = foldBoard(collected);
        busy = false;
        relayDown = false;
        relayStatus =
          collected.length === 0
            ? "Signed in. The relay has no 420, 421, or 422 lines."
            : "Signed in. This list is the relay's column proofs.";
      }
    };
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const url = relayUrl.trim() || DEFAULT_RELAY;
    relayUrl = url;
    if (!url.startsWith("ws://") && !url.startsWith("wss://")) {
      relayDown = false;
      relayStatus = "The relay URL needs to start with ws:// or wss://.";
      return;
    }
    if (!nsec.trim()) {
      relayStatus = "Enter an nsec to sign in.";
      return;
    }
    takeLive(url, nsec);
  }

  onMount(() => {
    probeRelay(DEFAULT_RELAY);
    return () => {
      closeSocket(probe);
      closeSocket(socket);
    };
  });
</script>

<svelte:head>
  <title>LifeSprite</title>
</svelte:head>

<main>
  <div class="phone">
    <div class="ear"></div>
    <header class="top">
      <p class="mark">LifeSprite</p>
      <h1>Todo, Doing, Done</h1>
      <p class="lede">Column proofs from the private relay. A mint keeps its title. A move does not bring a new one.</p>
    </header>

    <form onsubmit={submit} autocomplete="off">
      <label>
        nsec
        <input
          type="password"
          name="lifesprite-nsec"
          autocomplete="off"
          spellcheck="false"
          autocapitalize="off"
          bind:value={nsec}
          placeholder="nsec1… or 64-hex"
        />
      </label>
      <label>
        Relay
        <input
          type="text"
          name="lifesprite-relay"
          autocomplete="off"
          spellcheck="false"
          bind:value={relayUrl}
        />
      </label>
      <button type="submit" disabled={busy}>{busy ? "Opening…" : "Open board"}</button>
    </form>

    <p class="status" data-relay-down={relayDown ? "yes" : "no"}>{relayStatus}</p>

    {#if live}
      <section class="live" aria-label="Relay board">
        <h2>This key</h2>
        <BoardColumns projects={live} />
      </section>
    {/if}

    <section class="sample" aria-label="Sample events">
      <h2>Sample events</h2>
      <p class="note">Preview. These lines are fixtures, not the relay. Same reducer, no key.</p>
      <BoardColumns projects={sample} />
    </section>
  </div>
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: #d9d3c7;
  }

  main {
    min-height: 100vh;
    display: flex;
    justify-content: center;
    padding: 28px 16px 48px;
    box-sizing: border-box;
    font-family: "SF Pro Text", "Segoe UI", system-ui, sans-serif;
    color: #1c1b19;
  }

  .phone {
    width: min(390px, 100%);
    background: #f6f3ec;
    border: 10px solid #2a2826;
    border-radius: 36px;
    box-shadow: 0 18px 50px rgba(40, 32, 20, 0.18);
    padding: 8px 16px 22px;
    box-sizing: border-box;
  }

  .ear {
    width: 96px;
    height: 18px;
    margin: 4px auto 12px;
    border-radius: 999px;
    background: #2a2826;
  }

  .top h1 {
    margin: 0;
    font-size: 28px;
    letter-spacing: -0.03em;
  }

  .mark {
    margin: 0 0 4px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #8a4b2f;
  }

  .lede,
  .note,
  .status {
    color: #5c564c;
    font-size: 13px;
    line-height: 1.4;
  }

  .lede {
    margin: 8px 0 0;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    font-weight: 650;
  }

  input {
    font: inherit;
    font-size: 15px;
    font-weight: 450;
    padding: 10px 12px;
    border-radius: 12px;
    border: 1px solid #ddd6ca;
    background: #fff;
    color: inherit;
  }

  button {
    font: inherit;
    font-weight: 650;
    border: 0;
    border-radius: 12px;
    padding: 12px;
    background: #1c1b19;
    color: #f6f3ec;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .status {
    margin: 12px 0 0;
    min-height: 1.4em;
  }

  .status[data-relay-down="yes"] {
    color: #8a2e1b;
    font-weight: 650;
  }

  h2 {
    margin: 22px 0 6px;
    font-size: 15px;
  }

  .note {
    margin: 0 0 12px;
  }

  .sample {
    margin-top: 6px;
    padding-top: 4px;
    border-top: 1px solid #e4ddd2;
  }
</style>
