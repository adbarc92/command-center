<script lang="ts">
  // The Project Dashboard board (spec §1, §5, §8): a read-only projector that shows,
  // at a glance, what stage every project is in. Four adapters → one ProjectCard
  // model → this board. Read-only: every "act here" affordance deep-links OUT to the
  // owning tool (§8 #2); the board never writes a source.
  import { onMount, untrack } from 'svelte';
  import type { BoardState } from '../lib/dashboard/store';
  import {
    newBoard,
    pollHalyard,
    pollAudience,
    pollLocal,
    seedFleet,
    applyFleetPhase,
    applyPluginState,
    cardList,
    sortedCards,
    blockedCount,
    isStale,
  } from '../lib/dashboard/store';
  import type { ProjectCard, Stage } from '../lib/dashboard/model';
  import type { HalyardReader } from '../lib/dashboard/adapters/halyard';
  import type { AudienceReader } from '../lib/dashboard/adapters/audience';
  import type { LocalReader } from '../lib/dashboard/adapters/local';
  import type { Snapshot, Phase } from '../lib/types';
  import type { PluginUnit } from '../lib/dashboard/adapters/appPlugin';

  // Injectable seams so the board renders/tests in isolation (no Tauri required).
  // App.svelte / SHELL wires the real Tauri-backed readers + event subscriptions.
  interface Props {
    halyardReader?: HalyardReader;
    audienceReader?: AudienceReader;
    localReader?: LocalReader;
    fleetSnapshots?: Snapshot[];
    /** Subscribe to fleet `phase_changed`; returns an unsubscribe. */
    onFleetPhase?: (cb: (unitId: string, to: Phase, task: string, tier: string) => void) => () => void;
    /** Subscribe to `plugin://state`; returns an unsubscribe. */
    onPluginState?: (cb: (unit: PluginUnit) => void) => () => void;
    pollIntervalMs?: number;
    /** Inject board state directly (tests/standalone); skips live wiring. */
    initialBoard?: BoardState;
  }

  let {
    halyardReader,
    audienceReader,
    localReader,
    fleetSnapshots = [],
    onFleetPhase,
    onPluginState,
    pollIntervalMs = 15000,
    initialBoard,
  }: Props = $props();

  // Snapshot the (one-shot) initial board prop into local state; `board` is then
  // owned by this component and mutated by polls/events. `untrack` makes the one-time
  // read explicit so it isn't flagged as a missed reactive dependency.
  let board = $state<BoardState>(untrack(() => initialBoard) ?? newBoard());
  let nowMs = $state(Date.now());
  let pulling = $state(false);

  const cards = $derived(sortedCards(board));
  const needsMe = $derived(blockedCount(board));
  const total = $derived(cardList(board).length);
  const liveCount = $derived(cardList(board).filter((c) => c.stage === 'Live').length);
  const downSources = $derived(
    [...new Set(cardList(board).filter((c) => c.health === 'unknown').map((c) => c.source))],
  );

  async function refresh() {
    pulling = true;
    try {
      if (halyardReader) board = await pollHalyard(board, halyardReader, {}, () => new Date());
      if (audienceReader) board = await pollAudience(board, audienceReader, {}, () => new Date());
      if (localReader) board = await pollLocal(board, localReader);
    } finally {
      pulling = false;
      nowMs = Date.now();
    }
  }

  onMount(() => {
    if (initialBoard) return; // standalone/test: render what was injected

    // Seed fleet from snapshots (push source; events advance it live afterwards).
    if (fleetSnapshots.length) board = seedFleet(board, fleetSnapshots);

    void refresh();
    const poll = setInterval(refresh, pollIntervalMs);
    const tick = setInterval(() => (nowMs = Date.now()), 5000); // re-evaluate staleness

    const unsubs: Array<() => void> = [];
    if (onFleetPhase) {
      unsubs.push(
        onFleetPhase((unitId, to, task, tier) => {
          board = applyFleetPhase(board, unitId, to, { task, tier });
        }),
      );
    }
    if (onPluginState) {
      unsubs.push(onPluginState((unit) => (board = applyPluginState(board, unit))));
    }

    return () => {
      clearInterval(poll);
      clearInterval(tick);
      unsubs.forEach((u) => u());
    };
  });

  const STAGE_TONE: Record<Stage, 'good' | 'active' | 'attention' | 'bad' | 'idle'> = {
    Idea: 'idle',
    Spec: 'active',
    Plan: 'active',
    Build: 'active',
    Review: 'active',
    Ship: 'active',
    Live: 'good',
    Archived: 'idle',
    Blocked: 'attention',
    Failed: 'bad',
    Idle: 'idle',
  };

  const SOURCE_LABEL: Record<string, string> = {
    halyard: 'HALYARD',
    audience: 'AUDIENCE',
    fleet: 'FLEET',
    'app-plugin': 'APP',
    manual: 'MANUAL',
    local: 'LOCAL',
  };

  // §8 #2: actions deep-link OUT. Tauri's opener handles custom + http schemes; in a
  // plain browser standalone we no-op gracefully.
  async function openLink(href: string) {
    try {
      const mod = await import('@tauri-apps/api/core');
      await mod.invoke('plugin:opener|open_url', { url: href }).catch(() => window.open(href, '_blank'));
    } catch {
      window.open(href, '_blank');
    }
  }
</script>

<div class="board" data-testid="dashboard">
  <header class="bar">
    <div class="title">
      <span class="glyph">▣</span>
      <span class="wordmark disp">PROJECT&nbsp;BOARD</span>
      <span class="sub mono">read-only · all projects, one stage each</span>
    </div>
    <div class="stats">
      <div class="stat needs" class:hot={needsMe > 0} data-testid="needs-me">
        <b class="disp">{needsMe}</b><span>NEEDS YOU</span>
      </div>
      <div class="stat"><b class="disp">{liveCount}</b><span>LIVE</span></div>
      <div class="stat"><b class="disp">{total}</b><span>PROJECTS</span></div>
      <button class="refresh disp" onclick={refresh} disabled={pulling} data-testid="refresh">
        {pulling ? 'POLLING…' : '↻ REFRESH'}
      </button>
    </div>
  </header>

  {#if downSources.length}
    <div class="degraded mono" data-testid="degraded">
      ⚠ source unreachable: {downSources.map((s) => SOURCE_LABEL[s] ?? s).join(', ')} — those lanes are greyed; other sources unaffected.
    </div>
  {/if}

  {#if cards.length === 0}
    <div class="empty disp" data-testid="empty">NO PROJECTS · every source is idle or unreachable</div>
  {:else}
    <div class="grid">
      {#each cards as c (c.projectId)}
        {@const stale = isStale(c, nowMs)}
        <article
          class="card {STAGE_TONE[c.stage]}"
          class:stale
          class:unknown={c.health === 'unknown'}
          data-testid="card"
          data-project-id={c.projectId}
          data-stage={c.stage}
          data-health={c.health}
          data-stale={stale}
        >
          <div class="ctop">
            <span class="src disp">{SOURCE_LABEL[c.source] ?? c.source}</span>
            {#if c.stageSource === 'declared'}
              <span class="chip declared" title={c.override?.reason}>DECLARED</span>
            {/if}
            {#if c.conflict}
              <span class="chip conflict" title="declared {c.conflict.declared} vs inferred {c.conflict.inferred}">
                ⚑ DRIFT
              </span>
            {/if}
            <span class="badge disp {STAGE_TONE[c.stage]}" data-testid="stage">{c.stage.toUpperCase()}</span>
          </div>

          <div class="name" title={c.name}>{c.name}</div>
          <div class="detail mono">{c.detail}</div>

          {#if c.blocked}
            <button class="act disp" onclick={() => openLink(c.blocked!.deepLink)} data-testid="act">
              ▸ {c.blocked.action}
            </button>
          {:else if c.url}
            <button class="open mono" onclick={() => openLink(c.url!)}>↗ {c.url}</button>
          {/if}

          <div class="cfoot mono">
            {#if c.health === 'unknown'}
              <span class="hflag bad">SOURCE UNREACHABLE</span>
            {:else if stale}
              <span class="hflag warn">STALE · poll overdue</span>
            {:else if c.health === 'degraded'}
              <span class="hflag warn">DEGRADED</span>
            {:else}
              <span class="hflag ok">◉ live</span>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {/if}
</div>

<style>
  .board { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--bg); }

  .bar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0 18px; height: 52px; border-bottom: 1px solid var(--line-2);
    background: linear-gradient(180deg, var(--panel-2), var(--panel));
  }
  .title { display: flex; align-items: baseline; gap: 12px; }
  .glyph { color: var(--cyan); filter: drop-shadow(0 0 6px var(--cyan-glow)); }
  .wordmark { font-weight: 800; font-size: 18px; letter-spacing: 3px; color: #eaf6fa; }
  .sub { color: var(--text-faint); font-size: 11px; letter-spacing: 1px; }
  .stats { display: flex; align-items: center; gap: 20px; }
  .stat { display: flex; flex-direction: column; align-items: flex-end; line-height: 1; }
  .stat b { font-size: 18px; color: var(--cyan); font-weight: 700; }
  .stat span { font-size: 9px; letter-spacing: 1.5px; color: var(--text-faint); margin-top: 3px; }
  .stat.needs.hot b { color: var(--amber); }
  .refresh {
    background: var(--elev); color: var(--text-dim); border: 1px solid var(--line-2);
    padding: 7px 12px; font-size: 11px; letter-spacing: 1px; border-radius: 2px; cursor: pointer;
    font-weight: 600;
  }
  .refresh:hover:not(:disabled) { border-color: var(--line-hot); color: var(--text); }
  .refresh:disabled { opacity: 0.5; cursor: progress; }

  .degraded {
    margin: 10px 16px 0; padding: 8px 10px; font-size: 11.5px; color: var(--amber);
    border: 1px solid rgba(240, 180, 41, 0.4); background: rgba(240, 180, 41, 0.07); border-radius: 2px;
  }

  .empty { color: var(--text-faint); font-size: 13px; letter-spacing: 2px; padding: 60px 8px; text-align: center; }

  .grid {
    padding: 16px; overflow-y: auto; min-height: 0;
    display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px;
  }
  .card {
    background: var(--panel-2); border: 1px solid var(--line-1); border-left: 2px solid var(--line-2);
    border-radius: 2px; padding: 11px 12px; display: flex; flex-direction: column; gap: 8px;
    color: var(--text);
  }
  .card.active { border-left-color: var(--cyan); }
  .card.attention { border-left-color: var(--amber); }
  .card.good { border-left-color: var(--green); }
  .card.bad { border-left-color: var(--red); }
  .card.idle { border-left-color: var(--line-2); }
  /* Stale or unreachable cards grey out (§8) — never hidden, never asserting a stage. */
  .card.stale, .card.unknown { opacity: 0.45; filter: grayscale(0.6); }

  .ctop { display: flex; align-items: center; gap: 7px; }
  .src { font-size: 10px; letter-spacing: 1px; color: var(--violet); border: 1px solid var(--line-2); padding: 1px 5px; border-radius: 2px; }
  .chip { font-size: 9px; letter-spacing: 0.5px; padding: 1px 5px; border-radius: 2px; }
  .chip.declared { color: var(--blue); border: 1px solid rgba(74, 168, 255, 0.4); }
  .chip.conflict { color: var(--amber); border: 1px solid rgba(240, 180, 41, 0.5); }
  .badge { margin-left: auto; font-size: 10px; letter-spacing: 1px; padding: 2px 7px; border-radius: 2px; font-weight: 700; }
  .badge.active { color: #022; background: var(--cyan); }
  .badge.attention { color: #2a1c00; background: var(--amber); }
  .badge.good { color: #03210f; background: var(--green); }
  .badge.bad { color: #2a0007; background: var(--red); }
  .badge.idle { color: var(--text-dim); background: var(--elev); }

  .name { font-size: 13px; color: var(--text); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail { font-size: 11px; color: var(--text-faint); }

  .act {
    text-align: left; background: rgba(240, 180, 41, 0.1); border: 1px solid rgba(240, 180, 41, 0.45);
    color: var(--amber); padding: 7px 9px; font-size: 11.5px; letter-spacing: 0.5px; border-radius: 2px;
    cursor: pointer; font-weight: 600;
  }
  .act:hover { background: rgba(240, 180, 41, 0.18); }
  .open {
    text-align: left; background: transparent; border: 1px solid var(--line-2); color: var(--blue);
    padding: 6px 9px; font-size: 11px; border-radius: 2px; cursor: pointer; overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap;
  }
  .open:hover { border-color: var(--line-hot); }

  .cfoot { display: flex; justify-content: flex-end; font-size: 10px; }
  .hflag.ok { color: var(--green); }
  .hflag.warn { color: var(--amber); }
  .hflag.bad { color: var(--red); }
</style>
