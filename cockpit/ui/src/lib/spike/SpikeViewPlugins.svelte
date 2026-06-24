<script lang="ts">
  // THROWAWAY P4 SPIKE — view-plugin handshake harness (host side). Do NOT merge to main.
  //
  // Drives the gated go/no-go from the view-plugins design spec §"Spike #1", inside the
  // Tauri WebView2 window (where the `ccplugin://` scheme handler exists — a normal browser
  // does NOT have it, so this must run in the cockpit window, dev or packaged):
  //   (a) renders        — the sandboxed iframe paints its marker.
  //   (b) handshake       — plugin-hello → init(MessagePort) → ready across 100 reloads,
  //                         ZERO drops (the headline number; catches the timing race).
  //   (c-CORS) self-load  — opaque-origin module import of a 2nd same-scheme module.
  //   (c-CSP) no network  — connect-src 'none' blocks fetch.
  //   (d) host CSP        — authored without breaking Vite HMR (SEPARATE manual step; see
  //                         the spike runbook — left out here so the harness runs as-is).
  import { onMount } from 'svelte';

  let { onclose }: { onclose: () => void } = $props();

  // Windows/WebView2 (primary) custom-scheme origin form. macOS/Linux would be
  // `ccplugin://localhost/index.html` — flip PLUGIN_ORIGIN if you run the spike there.
  const PLUGIN_ORIGIN = 'http://ccplugin.localhost';
  const PLUGIN_URL = `${PLUGIN_ORIGIN}/index.html`;

  const TARGET = 100;
  const HANDSHAKE_TIMEOUT_MS = 3000;

  let iframe: HTMLIFrameElement | null = $state(null);
  let running = $state(false);
  let runs = $state(0);
  let ok = $state(0);
  let dropped = $state(0);
  let rendered = $state(false);
  let corsOk = $state<boolean | null>(null);
  let corsErr = $state<string | null>(null);
  let networkBlocked = $state<boolean | null>(null);
  let logLines = $state<string[]>([]);

  // Per-round handshake latch. The persistent window 'message' listener resolves the
  // active round when the plugin's `ready` arrives over the freshly transferred port.
  let settleRound: (() => void) | null = null;
  let activePort: MessagePort | null = null;

  function log(s: string) {
    logLines = [...logLines.slice(-300), s];
  }

  // Host replies to the plugin's announcement by transferring a fresh MessagePort. The
  // host's identity for the plugin is *holding port1*, never event.origin ("null" here).
  function onWindowMessage(e: MessageEvent) {
    // Stray-message guard: only accept the announce from OUR iframe's content window.
    if (!iframe || e.source !== iframe.contentWindow) return;
    const msg = e.data;
    if (!msg || msg.type !== 'plugin-hello') return;

    activePort?.close();
    const channel = new MessageChannel();
    activePort = channel.port1;
    channel.port1.onmessage = onPortMessage;
    const init = { v: 1, type: 'init', apiVersion: 1, capabilities: ['log-append'] };
    // e.source is the iframe's contentWindow; "*" is safe — the payload is non-sensitive.
    (e.source as Window).postMessage(init, '*', [channel.port2]);
  }

  function onPortMessage(e: MessageEvent) {
    const msg = e.data;
    if (!msg) return;
    if (msg.type === 'ready') {
      rendered = true;
      settleRound?.();
    } else if (msg.type === 'probe') {
      if (typeof msg.corsOk === 'boolean') corsOk = msg.corsOk;
      if (typeof msg.corsErr === 'string') corsErr = msg.corsErr;
      if (typeof msg.networkBlocked === 'boolean') networkBlocked = msg.networkBlocked;
    }
  }

  // One reload round: bump the iframe src (cache-bust ⇒ fresh document ⇒ fresh
  // plugin-hello) and resolve true on `ready`, false on timeout (a dropped handshake).
  function loadOnce(n: number): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      let done = false;
      const finish = (v: boolean) => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        settleRound = null;
        resolve(v);
      };
      const timer = setTimeout(() => finish(false), HANDSHAKE_TIMEOUT_MS);
      settleRound = () => finish(true);
      if (iframe) iframe.src = `${PLUGIN_URL}?n=${n}`;
    });
  }

  async function run() {
    if (running) return;
    running = true;
    runs = 0;
    ok = 0;
    dropped = 0;
    corsOk = null;
    corsErr = null;
    networkBlocked = null;
    logLines = [];
    log(`▶ ${TARGET} reloads against ${PLUGIN_URL} (timeout ${HANDSHAKE_TIMEOUT_MS}ms/round)`);
    for (let n = 1; n <= TARGET; n++) {
      const success = await loadOnce(n);
      runs = n;
      if (success) ok++;
      else {
        dropped++;
        log(`round ${n}: ✕ DROPPED (no handshake in ${HANDSHAKE_TIMEOUT_MS}ms)`);
      }
      if (n % 10 === 0) log(`… ${n}/${TARGET} · ok=${ok} dropped=${dropped}`);
    }
    log(
      `■ DONE — handshake ${ok}/${TARGET}, dropped ${dropped} · ` +
        `corsOk=${corsOk} networkBlocked=${networkBlocked}` +
        (dropped === 0 ? '  → gate (b) PASS' : '  → gate (b) FAIL'),
    );
    running = false;
  }

  onMount(() => {
    window.addEventListener('message', onWindowMessage);
    return () => {
      window.removeEventListener('message', onWindowMessage);
      activePort?.close();
    };
  });

  const pct = $derived(runs === 0 ? 0 : Math.round((runs / TARGET) * 100));
  function verdict(v: boolean | null): string {
    return v === null ? '…' : v ? 'PASS' : 'FAIL';
  }
</script>

<div class="vp-spike">
  <header>
    <span class="title">P4 · VIEW-PLUGIN HANDSHAKE SPIKE</span>
    <span class="env">sandboxed iframe + MessagePort · {PLUGIN_ORIGIN}</span>
    <button class="x" onclick={onclose} aria-label="close spike">✕</button>
  </header>

  <div class="body">
    <!-- left: the sandboxed plugin under test. NO allow-same-origin ⇒ opaque origin. -->
    <section class="stage">
      <div class="lbl">SANDBOXED PLUGIN (untrusted)</div>
      <iframe
        bind:this={iframe}
        title="cc view-plugin (sandboxed)"
        sandbox="allow-scripts"
        src="about:blank"
      ></iframe>
    </section>

    <!-- right: gate scoreboard + driver -->
    <section class="panel">
      <div class="gates">
        <div class="gate">
          <span class="g">a · renders</span>
          <span class="v" class:pass={rendered} class:wait={!rendered}>{rendered ? 'PASS' : '…'}</span>
        </div>
        <div class="gate">
          <span class="g">b · handshake ×{TARGET}</span>
          <span
            class="v"
            class:pass={runs === TARGET && dropped === 0}
            class:fail={dropped > 0}
            class:wait={runs < TARGET && dropped === 0}
          >
            {ok}/{TARGET} ok · {dropped} dropped
          </span>
        </div>
        <div class="gate">
          <span class="g">c · self-code-load (CORS)</span>
          <span class="v" class:pass={corsOk === true} class:fail={corsOk === false} class:wait={corsOk === null}>
            {verdict(corsOk)}
          </span>
        </div>
        <div class="gate">
          <span class="g">c · network blocked (CSP)</span>
          <span class="v" class:pass={networkBlocked === true} class:fail={networkBlocked === false} class:wait={networkBlocked === null}>
            {verdict(networkBlocked)}
          </span>
        </div>
        <div class="gate muted">
          <span class="g">d · host CSP / HMR</span>
          <span class="v wait">manual — see runbook</span>
        </div>
      </div>

      {#if corsErr}<div class="note">CORS import error: {corsErr}</div>{/if}

      <div class="bar"><i style="width:{pct}%"></i></div>

      <button class="run" onclick={run} disabled={running}>
        {running ? `RUNNING… ${runs}/${TARGET}` : `▶ RUN ${TARGET}-RELOAD HANDSHAKE`}
      </button>

      <div class="log">
        {#each logLines as l}<div class="ln">{l}</div>{/each}
      </div>
    </section>
  </div>
</div>

<style>
  /* Throwaway/utilitarian on purpose — never merges. */
  .vp-spike {
    position: fixed; inset: 0; z-index: 9000;
    display: flex; flex-direction: column;
    background: #060c18; color: #cfe6ff;
    font: 13px/1.5 ui-monospace, "Cascadia Mono", monospace;
  }
  header {
    display: flex; align-items: center; gap: 16px;
    padding: 10px 14px; border-bottom: 1px solid #21406a; background: #0a1426;
  }
  .title { font-weight: 700; letter-spacing: 1px; color: #c08bff; }
  .env { color: #5f86b8; font-size: 11px; }
  .x { margin-left: auto; background: transparent; color: #88a; border: 1px solid #2a4a78; border-radius: 3px; cursor: pointer; padding: 4px 9px; }
  .body { flex: 1; display: grid; grid-template-columns: 1fr 380px; min-height: 0; }
  .stage { display: flex; flex-direction: column; padding: 14px; gap: 8px; min-height: 0; }
  .lbl { font-size: 10px; letter-spacing: 2px; color: #5f86b8; }
  iframe { flex: 1; width: 100%; border: 1px solid #21406a; border-radius: 4px; background: #0c1830; }
  .panel { border-left: 1px solid #21406a; padding: 14px; display: flex; flex-direction: column; gap: 12px; min-height: 0; }
  .gates { display: flex; flex-direction: column; gap: 6px; }
  .gate { display: flex; justify-content: space-between; align-items: center; padding: 7px 10px; border: 1px solid #21406a; border-radius: 3px; background: #0a1426; }
  .gate.muted { opacity: 0.65; }
  .g { color: #9fc0e6; font-size: 12px; }
  .v { font-weight: 700; font-size: 12px; }
  .v.pass { color: #35d07f; }
  .v.fail { color: #ff5d6c; }
  .v.wait { color: #f0b429; }
  .note { font-size: 11px; color: #ff9aa4; border: 1px solid #5a2030; border-radius: 3px; padding: 6px 8px; }
  .bar { height: 5px; background: #0c1830; border: 1px solid #21406a; border-radius: 3px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: linear-gradient(90deg, #2bd6ff, #c08bff); transition: width 0.15s linear; }
  .run {
    padding: 11px; font: 700 13px/1 ui-monospace, monospace; letter-spacing: 1px;
    background: linear-gradient(180deg, #2bd6ff, #1aa6cf); color: #021a1f; border: none; border-radius: 3px; cursor: pointer;
  }
  .run:disabled { opacity: 0.5; cursor: not-allowed; }
  .log { flex: 1; overflow-y: auto; background: #0a1220; border: 1px solid #21406a; border-radius: 3px; padding: 8px; font-size: 11.5px; min-height: 0; }
  .ln { color: #9fc0e6; white-space: pre-wrap; word-break: break-word; }
</style>
