<script lang="ts">
  import { onMount } from 'svelte';
  import { FLEET_BASE } from './lib/api';
  import { phaseClass, progress, isTerminal } from './lib/fleet';
  import type { CommandName } from './lib/types';
  import { fleet } from './lib/store.svelte';
  import Switcher, { type ViewEntry } from './lib/Switcher.svelte';
  import Dashboard from './views/Dashboard.svelte';
  import ApprovalOverlay, { type ApprovalRequest } from './lib/ApprovalOverlay.svelte';
  import { tauriHalyardReader, tauriAudienceReader } from './lib/dashboard/api';

  // ── top-level view switcher (Lane A) ─────────────────────────────
  // Fleet = the inline cockpit below; Projects = the read-only Project Dashboard.
  // Both project the SAME shared `fleet` store, so switching never loses state.
  type ViewId = 'fleet' | 'projects';
  let view = $state<ViewId>('fleet');

  const views: ViewEntry[] = [
    { id: 'fleet', label: 'FLEET' },
    { id: 'projects', label: 'PROJECTS' },
  ];

  // The fleet store is the single source of fleet truth, shared across views. It owns
  // its own WebSocket streams; we connect once on app mount and tear down on app
  // unmount (NOT on a view switch — switching keeps units/sockets/selection alive).
  onMount(() => {
    fleet.start();
    return () => fleet.dispose();
  });

  // Convenience aliases so the Fleet markup below reads like before, but every read
  // goes through the shared store.
  const list = $derived(fleet.list);
  const selected = $derived(fleet.selected);
  const activeCount = $derived(fleet.activeCount);
  const totalBurn = $derived(fleet.totalBurn);
  const daemon = $derived(fleet.daemon);
  const selectedId = $derived(fleet.selectedId);

  // ── new-mission form ─────────────────────────────────────────────
  let task = $state('Add a function sum(a, b) in src/index.js (module.exports.sum) returning a + b.');
  let tier = $state<'t1' | 't2' | 't3'>('t1');
  let mode = $state<'demo' | 'real'>('demo');
  let rounds = $state(2);
  let launching = $state(false);
  let launchErr = $state<string | null>(null);

  async function launch() {
    launchErr = null;
    // REAL launches are human-authority: stage them for the host overlay's confirm modal
    // instead of firing immediately. DEMO launches go straight through.
    if (mode === 'real') {
      fleet.requestRealLaunch({ task, tier, mode, min_review_rounds: rounds });
      return;
    }
    launching = true;
    try {
      await fleet.launch({ task, tier, mode, min_review_rounds: rounds });
    } catch (err) {
      launchErr = String(err);
    } finally {
      launching = false;
    }
  }

  async function cmd(name: CommandName) {
    await fleet.cmd(name);
  }

  // ── host overlay (Lane O) — human-authority modal layer ──────────────────────
  // Driven purely by host-owned fleet state, independent of the active view/plugin.
  // Oracle approval takes priority; otherwise a staged REAL launch shows its confirm.
  const awaitingApproval = $derived(fleet.awaitingApproval);
  const pendingLaunch = $derived(fleet.pendingLaunch);
  const approvalRequest = $derived<ApprovalRequest | null>(
    awaitingApproval
      ? { kind: 'oracle', unit: awaitingApproval }
      : pendingLaunch
        ? { kind: 'launch', req: pendingLaunch }
        : null,
  );
  // While a modal is open the rest of the app is `inert`: removed from focus and
  // hit-testing (honored by WebView2/Chromium), so a future mounted plugin/webview
  // subtree cannot receive input. A backdrop + z-index alone does NOT do this.
  const overlayOpen = $derived(approvalRequest !== null);

  function onApprove() {
    if (!approvalRequest) return;
    if (approvalRequest.kind === 'oracle') {
      void fleet.cmd('approve_oracle', approvalRequest.unit.id);
    } else {
      // Confirm the staged REAL launch; surface any failure on the form.
      launching = true;
      launchErr = null;
      void fleet
        .confirmLaunch()
        .catch((err) => (launchErr = String(err)))
        .finally(() => (launching = false));
    }
  }

  function onReject() {
    if (!approvalRequest) return;
    if (approvalRequest.kind === 'oracle') {
      void fleet.cmd('reject_oracle', approvalRequest.unit.id);
    } else {
      fleet.cancelLaunch();
    }
  }


  const PHASE_LABEL: Record<string, string> = {
    queued: 'QUEUED', provisioning: 'PROVISION', spec: 'ORACLE',
    awaiting_oracle_approval: 'APPROVE?', building: 'BUILD', checking: 'CHECK',
    reviewing: 'REVIEW', merge_check: 'MERGE', pr_open: 'PR OPEN',
    done: 'DONE', no_change: 'NO-OP', failed: 'FAILED',
    needs_human: 'NEEDS YOU', halted: 'HALTED',
  };

  const canApprove = $derived(selected?.phase === 'awaiting_oracle_approval');
  const canResume = $derived(selected?.phase === 'needs_human' || selected?.phase === 'halted');
  const canShip = $derived(selected?.phase === 'needs_human');
  const canHalt = $derived(
    !!selected &&
      !isTerminal(selected.phase) &&
      !['needs_human', 'halted', 'awaiting_oracle_approval'].includes(selected.phase),
  );

  let logEl: HTMLDivElement | null = $state(null);
  $effect(() => {
    if (selected && logEl) {
      void selected.log.length;
      logEl.scrollTop = logEl.scrollHeight;
    }
  });
</script>

<!-- Content subtree. Marked `inert` while a host-overlay modal is open so nothing here
     (including a future mounted plugin/webview) can take focus or pointer input — the
     real input block, not the backdrop. -->
<div class="hud" inert={overlayOpen} data-testid="hud" aria-hidden={overlayOpen}>
  <header class="topbar">
    <div class="brand">
      <span class="glyph">◢◤</span>
      <span class="wordmark disp">FLEET&nbsp;COMMAND</span>
      <Switcher {views} active={view} onselect={(id) => (view = id as ViewId)} />
    </div>
    {#if view === 'fleet'}
      <div class="stats">
        <div class="stat"><b class="disp">{activeCount}</b><span>ACTIVE</span></div>
        <div class="stat"><b class="disp">{list.length}</b><span>UNITS</span></div>
        <div class="stat"><b class="disp">${totalBurn.toFixed(2)}</b><span>BURN</span></div>
        <div class="badges mono">
          <span class="hb" class:ok={daemon?.docker} class:bad={daemon && !daemon.docker} title="Docker daemon">
            ◉ DOCKER
          </span>
          <span class="hb" class:ok={daemon?.anthropic_key} class:bad={daemon && !daemon.anthropic_key} title="ANTHROPIC_API_KEY set">
            ◉ KEY
          </span>
        </div>
        <div class="conn mono" title={FLEET_BASE}>◉ {FLEET_BASE.replace(/^https?:\/\//, '')}</div>
      </div>
    {/if}
  </header>

  {#if view === 'projects'}
    <!-- Projects: the read-only Project Dashboard. Mounts with the real Tauri-backed
         readers, seeds its fleet lane from the shared store, and advances live off the
         store's `phase_changed` stream (no second socket). The store stays alive while
         this view is mounted, so switching back to Fleet preserves all state. -->
    <Dashboard
      halyardReader={tauriHalyardReader}
      audienceReader={tauriAudienceReader}
      fleetSnapshots={fleet.snapshots()}
      onFleetPhase={(cb) => fleet.onPhase(cb)}
    />
  {:else}
  <main class="grid">
    <!-- left: new mission -->
    <aside class="panel left">
      <h2 class="disp ph">NEW MISSION</h2>
      <label class="fld">
        <span>TASK</span>
        <textarea bind:value={task} rows="4" class="mono"></textarea>
      </label>

      <div class="fld">
        <span>TIER · autonomy ladder</span>
        <div class="seg">
          {#each ['t1', 't2', 't3'] as t}
            <button class="disp" class:on={tier === t} onclick={() => (tier = t as any)}>{t.toUpperCase()}</button>
          {/each}
        </div>
        <small class="hint">
          {tier === 't1' ? 'autonomous oracle · auto-PR' : tier === 't2' ? 'human approves tests · auto-PR' : 'human approves tests · human ships PR'}
        </small>
      </div>

      <div class="row">
        <label class="fld grow">
          <span>MODE</span>
          <div class="seg">
            <button class="disp" class:on={mode === 'demo'} onclick={() => (mode = 'demo')}>DEMO</button>
            <button class="disp" class:on={mode === 'real'} onclick={() => (mode = 'real')}>REAL</button>
          </div>
        </label>
        <label class="fld">
          <span>ROUNDS</span>
          <input type="number" min="1" max="6" bind:value={rounds} class="mono num" />
        </label>
      </div>

      <button class="launch disp" onclick={launch} disabled={launching || !task.trim()}>
        {launching ? 'LAUNCHING…' : '▸ LAUNCH UNIT'}
      </button>
      {#if launchErr}<div class="err mono">{launchErr}</div>{/if}
      {#if mode === 'real'}<div class="warn mono">REAL mode needs ANTHROPIC_API_KEY + Docker; spends real $.</div>{/if}
    </aside>

    <!-- center: fleet grid -->
    <section class="fleet">
      {#if list.length === 0}
        <div class="empty disp">NO UNITS DEPLOYED · launch a mission ◣</div>
      {:else}
        <div class="tiles">
          {#each list as u (u.id)}
            <button class="tile {phaseClass(u.phase)}" class:sel={selectedId === u.id} onclick={() => fleet.select(u.id)}>
              <div class="tile-top">
                <span class="uid mono">{u.id}</span>
                <span class="tier disp">{u.tier}</span>
                {#if u.awaitingSlot}<span class="slot disp" title="waiting for a concurrency slot">◷ SLOT</span>{/if}
                {#if u.rateLimited}<span class="slot disp" title="backing off through an Anthropic rate limit">⏳ RATE-LIMIT</span>{/if}
                <span class="badge disp {phaseClass(u.phase)}">{PHASE_LABEL[u.phase] ?? u.phase}</span>
              </div>
              <div class="task">{u.task}</div>
              <div class="rail"><i style="width:{progress(u.phase) * 100}%"></i></div>
              <div class="cbar" title="${u.cost.toFixed(3)} of ${u.usdCap.toFixed(2)} cap">
                <i class:hot={u.cost >= u.usdCap} style="width:{Math.min(1, u.usdCap > 0 ? u.cost / u.usdCap : 0) * 100}%"></i>
              </div>
              <div class="meters mono">
                <span>${u.cost.toFixed(3)} / ${u.usdCap.toFixed(0)}</span>
                <span>{((u.tokensIn + u.tokensOut) / 1000).toFixed(1)}k tok</span>
                <span>r{u.iters.review} · b{u.iters.build}</span>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <!-- right: detail -->
    <aside class="panel right">
      {#if !selected}
        <div class="muted disp">SELECT A UNIT</div>
      {:else}
        <div class="dhead">
          <span class="uid mono">{selected.id}</span>
          <span class="badge disp {phaseClass(selected.phase)}">{PHASE_LABEL[selected.phase] ?? selected.phase}</span>
        </div>

        <!-- Oracle approval is a host-authority action: the verb lives in the focus-stealing
             host overlay (Lane O), not here. The Fleet view shows only a non-interactive
             AWAITING APPROVAL indicator — no approve/reject button in-view. -->
        {#if canApprove}
          <div class="await disp" data-testid="awaiting-approval-indicator">◆ AWAITING APPROVAL · respond in overlay</div>
        {/if}

        <div class="cmds">
          <button class="cmd" disabled={!canResume} onclick={() => cmd('resume')}>RESUME</button>
          <button class="cmd ok" disabled={!canShip} onclick={() => cmd('ship')}>SHIP</button>
          <button class="cmd warn" disabled={!canHalt} onclick={() => cmd('halt')}>HALT</button>
          <button class="cmd bad" disabled={isTerminal(selected.phase)} onclick={() => cmd('abandon')}>ABANDON</button>
        </div>

        {#if selected.blocked}<div class="alert">⚠ {selected.blocked}</div>{/if}
        {#if selected.error}<div class="alert bad">✕ {selected.error}</div>{/if}

        {#if selected.oracleFiles.length}
          <div class="block">
            <div class="block-h disp">ORACLE · frozen tests</div>
            {#each selected.oracleFiles as f}<div class="kv mono">{f}</div>{/each}
          </div>
        {/if}

        {#if selected.findings.length}
          <div class="block">
            <div class="block-h disp">FINDINGS</div>
            {#each selected.findings as f}
              <div class="kv mono"><span class="rnd">r{f.round}</span> {f.title}</div>
            {/each}
          </div>
        {/if}

        {#if selected.pr}
          <a class="pr disp" href={selected.pr} target="_blank" rel="noreferrer">▸ OPEN PULL REQUEST</a>
        {/if}

        <div class="block grow">
          <div class="block-h disp">ACTIVITY</div>
          <div class="log mono" bind:this={logEl}>
            {#each selected.log as l}
              <div class="ln {l.stream}"><span class="tag">{l.stream[0]}</span>{l.line}</div>
            {/each}
            {#if selected.result}<div class="ln done">— {selected.result} —</div>{/if}
          </div>
        </div>
      {/if}
    </aside>
  </main>
  {/if}
</div>

<!-- The host overlay lives OUTSIDE the inert subtree so it remains interactive and
     focusable while the rest of the app is frozen. -->
<ApprovalOverlay request={approvalRequest} onapprove={onApprove} onreject={onReject} />

<style>
  .hud { display: flex; flex-direction: column; height: 100vh; }

  .topbar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0 18px; height: 52px;
    border-bottom: 1px solid var(--line-2);
    background: linear-gradient(180deg, var(--panel-2), var(--panel));
  }
  .brand { display: flex; align-items: center; gap: 14px; }
  .glyph { color: var(--cyan); letter-spacing: -2px; filter: drop-shadow(0 0 6px var(--cyan-glow)); }
  .wordmark { font-weight: 800; font-size: 19px; letter-spacing: 3px; color: #eaf6fa; }
  .sub { color: var(--text-faint); font-size: 11px; letter-spacing: 1px; }
  .stats { display: flex; align-items: center; gap: 22px; }
  .stat { display: flex; flex-direction: column; align-items: flex-end; line-height: 1; }
  .stat b { font-size: 18px; color: var(--cyan); font-weight: 700; }
  .stat span { font-size: 9px; letter-spacing: 1.5px; color: var(--text-faint); margin-top: 3px; }
  .conn { font-size: 11px; color: var(--green); letter-spacing: 0.5px; }
  .badges { display: flex; gap: 8px; }
  .hb { font-size: 10px; letter-spacing: 1px; color: var(--text-faint); border: 1px solid var(--line-2); padding: 2px 6px; border-radius: 2px; }
  .hb.ok { color: var(--green); border-color: rgba(53, 208, 127, 0.4); }
  .hb.bad { color: var(--red); border-color: rgba(255, 93, 108, 0.45); }

  .grid { flex: 1; display: grid; grid-template-columns: 300px 1fr 360px; min-height: 0; }
  .panel { background: var(--panel); display: flex; flex-direction: column; min-height: 0; }
  .left { border-right: 1px solid var(--line-1); padding: 16px; gap: 14px; overflow-y: auto; }
  .right { border-left: 1px solid var(--line-1); padding: 14px; gap: 10px; }
  .ph { margin: 0; }
  h2.disp { font-size: 13px; letter-spacing: 3px; color: var(--text-dim); font-weight: 700; }

  .fld { display: flex; flex-direction: column; gap: 6px; }
  .fld > span { font-size: 10px; letter-spacing: 1.5px; color: var(--text-faint); font-family: var(--font-display); }
  .row { display: flex; gap: 12px; }
  .grow { flex: 1; }
  textarea, input, .num {
    background: var(--bg); border: 1px solid var(--line-2); color: var(--text);
    padding: 9px 10px; font-size: 13px; border-radius: 2px; resize: vertical; outline: none;
    font-family: var(--font-mono);
  }
  textarea:focus, input:focus { border-color: var(--cyan-dim); box-shadow: 0 0 0 1px var(--cyan-glow); }
  .num { width: 64px; text-align: center; }
  .hint { color: var(--text-faint); font-size: 10px; letter-spacing: 0.3px; }

  .seg { display: flex; border: 1px solid var(--line-2); border-radius: 2px; overflow: hidden; }
  .seg button {
    flex: 1; background: transparent; color: var(--text-dim); border: none;
    padding: 8px 0; font-size: 12px; letter-spacing: 1px; cursor: pointer; font-weight: 600;
    border-right: 1px solid var(--line-1);
  }
  .seg button:last-child { border-right: none; }
  .seg button.on { background: var(--cyan); color: #02201f; }
  .seg button:not(.on):hover { background: var(--elev); color: var(--text); }

  .launch {
    margin-top: 4px; padding: 12px; font-size: 14px; letter-spacing: 2px; font-weight: 700;
    background: linear-gradient(180deg, var(--cyan), var(--cyan-dim)); color: #021a19;
    border: none; border-radius: 2px; cursor: pointer; box-shadow: 0 0 18px var(--cyan-glow);
  }
  .launch:disabled { opacity: 0.4; cursor: not-allowed; box-shadow: none; }
  .err, .warn { font-size: 11px; padding: 8px; border-radius: 2px; }
  .err { color: var(--red); border: 1px solid var(--red); }
  .warn { color: var(--amber); border: 1px solid rgba(240, 180, 41, 0.4); background: rgba(240, 180, 41, 0.06); }

  .fleet { padding: 16px; overflow-y: auto; min-height: 0; }
  .empty, .muted { color: var(--text-faint); font-size: 13px; letter-spacing: 2px; padding: 40px 8px; text-align: center; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
  .tile {
    text-align: left; background: var(--panel-2); border: 1px solid var(--line-1);
    border-left: 2px solid var(--line-2); border-radius: 2px; padding: 11px 12px;
    cursor: pointer; display: flex; flex-direction: column; gap: 8px; animation: blip 0.25s ease both;
    color: var(--text);
  }
  .tile:hover { border-color: var(--line-hot); background: var(--elev); }
  .tile.sel { border-color: var(--cyan-dim); box-shadow: inset 0 0 0 1px var(--cyan-glow); }
  .tile.active { border-left-color: var(--cyan); }
  .tile.attention { border-left-color: var(--amber); }
  .tile.good { border-left-color: var(--green); }
  .tile.bad { border-left-color: var(--red); }
  .tile-top { display: flex; align-items: center; gap: 8px; }
  .uid { color: var(--text); font-size: 12px; font-weight: 600; }
  .tier { font-size: 10px; color: var(--violet); letter-spacing: 1px; border: 1px solid var(--line-2); padding: 1px 5px; border-radius: 2px; }
  .badge { margin-left: auto; font-size: 10px; letter-spacing: 1px; padding: 2px 7px; border-radius: 2px; font-weight: 700; }
  .badge.active { color: #022; background: var(--cyan); animation: pulse 1.4s ease-in-out infinite; }
  .badge.attention { color: #2a1c00; background: var(--amber); }
  .badge.good { color: #03210f; background: var(--green); }
  .badge.bad { color: #2a0007; background: var(--red); }
  .badge.idle { color: var(--text-dim); background: var(--elev); }
  .task { font-size: 12px; color: var(--text-dim); line-height: 1.35; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .rail { height: 3px; background: var(--bg); border-radius: 2px; overflow: hidden; }
  .rail i { display: block; height: 100%; background: linear-gradient(90deg, var(--cyan-dim), var(--cyan)); transition: width 0.4s ease; }
  .cbar { height: 4px; background: var(--bg); border: 1px solid var(--line-1); border-radius: 2px; overflow: hidden; }
  .cbar i { display: block; height: 100%; background: linear-gradient(90deg, var(--green), var(--amber)); transition: width 0.4s ease; }
  .cbar i.hot { background: var(--red); }
  .slot { font-size: 9.5px; letter-spacing: 1px; color: var(--amber); border: 1px solid rgba(240, 180, 41, 0.4); padding: 1px 5px; border-radius: 2px; }
  .meters { display: flex; justify-content: space-between; font-size: 10.5px; color: var(--text-faint); }

  .dhead { display: flex; align-items: center; gap: 10px; }
  .dhead .uid { font-size: 15px; }
  .cmds { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 6px; }
  .cmd {
    background: var(--elev); color: var(--text-dim); border: 1px solid var(--line-2);
    padding: 7px 0; font-size: 11px; letter-spacing: 1px; border-radius: 2px; cursor: pointer; font-weight: 600;
    font-family: var(--font-display);
  }
  .cmd:hover:not(:disabled) { border-color: var(--line-hot); color: var(--text); }
  .cmd:disabled { opacity: 0.3; cursor: not-allowed; }
  .cmd.ok:not(:disabled) { color: var(--green); border-color: rgba(53, 208, 127, 0.4); }
  .cmd.warn:not(:disabled) { color: var(--amber); border-color: rgba(240, 180, 41, 0.4); }
  .cmd.bad:not(:disabled) { color: var(--red); border-color: rgba(255, 93, 108, 0.4); }

  .await {
    font-size: 11px; letter-spacing: 1.5px; font-weight: 700; color: var(--cyan);
    border: 1px solid var(--cyan-dim); background: rgba(74, 222, 222, 0.08);
    padding: 8px 10px; border-radius: 2px; text-align: center;
    animation: pulse 1.6s ease-in-out infinite;
  }

  .alert { font-size: 12px; color: var(--amber); border: 1px solid rgba(240, 180, 41, 0.4); background: rgba(240, 180, 41, 0.07); padding: 8px; border-radius: 2px; }
  .alert.bad { color: var(--red); border-color: rgba(255, 93, 108, 0.4); background: rgba(255, 93, 108, 0.07); }

  .block { display: flex; flex-direction: column; gap: 5px; min-height: 0; }
  .block.grow { flex: 1; }
  .block-h { font-size: 10px; letter-spacing: 2px; color: var(--text-faint); border-bottom: 1px solid var(--line-1); padding-bottom: 4px; }
  .kv { font-size: 11.5px; color: var(--text-dim); }
  .rnd { color: var(--violet); }
  .pr { display: block; text-align: center; padding: 9px; background: rgba(74, 168, 255, 0.1); border: 1px solid rgba(74, 168, 255, 0.45); color: var(--blue); text-decoration: none; border-radius: 2px; font-size: 12px; letter-spacing: 1px; }

  .log { flex: 1; overflow-y: auto; background: var(--bg); border: 1px solid var(--line-1); border-radius: 2px; padding: 8px; font-size: 11.5px; line-height: 1.5; min-height: 120px; }
  .ln { display: flex; gap: 7px; color: var(--text-dim); animation: blip 0.18s ease both; white-space: pre-wrap; word-break: break-word; }
  .ln .tag { color: var(--text-faint); flex-shrink: 0; }
  .ln.agent { color: var(--text); }
  .ln.check .tag { color: var(--amber); }
  .ln.system { color: var(--text-faint); }
  .ln.done { color: var(--green); }
</style>
