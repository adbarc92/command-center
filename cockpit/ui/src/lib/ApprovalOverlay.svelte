<script lang="ts" module>
  import type { CreateReq } from './api';
  import type { Unit } from './fleet';

  /**
   * What the host overlay is currently asking the human to decide. Exactly one modal is
   * ever open at a time; `App.svelte` derives this from host-owned fleet state:
   *  - `oracle`  — a unit entered `awaiting_oracle_approval`; show the frozen test set +
   *                APPROVE/REJECT (wired to `fleet.cmd(id, 'approve_oracle'|'reject_oracle')`).
   *  - `launch`  — a REAL-mode launch was staged (`fleet.pendingLaunch`); confirm/cancel.
   */
  export type ApprovalRequest =
    | { kind: 'oracle'; unit: Unit }
    | { kind: 'launch'; req: CreateReq };
</script>

<script lang="ts">
  import { tick } from 'svelte';

  interface Props {
    /** The decision to surface, or null when no modal should show. */
    request: ApprovalRequest | null;
    /** APPROVE (oracle) / CONFIRM (launch). */
    onapprove: () => void;
    /** REJECT (oracle) / CANCEL (launch). */
    onreject: () => void;
  }

  let { request, onapprove, onreject }: Props = $props();

  // The modal's own focus anchor. When a request opens we pull focus here so keystrokes
  // land on the host overlay, not on whatever was focused in the content beneath. The
  // content subtree is additionally made `inert` by App.svelte, which removes it from
  // focus/hit-testing entirely (a backdrop + z-index alone does NOT stop a focused
  // element underneath from receiving keys).
  let dialogEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (request && dialogEl) {
      // Defer until the node is in the DOM, then steal focus to the dialog.
      void tick().then(() => dialogEl?.focus());
    }
  });

  // ESC rejects/cancels; Enter approves/confirms. Keeps the host fully in control while open.
  function onkeydown(e: KeyboardEvent) {
    if (!request) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      onreject();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      onapprove();
    }
  }
</script>

{#if request}
  <!-- z-index above all content. The backdrop is the visual layer; the real input block
       is App.svelte marking the content subtree `inert`. -->
  <div
    class="overlay"
    data-testid="approval-overlay"
    role="presentation"
    onkeydown={onkeydown}
  >
    <div class="backdrop" data-testid="approval-backdrop"></div>

    <div
      class="modal"
      bind:this={dialogEl}
      tabindex="-1"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="approval-title"
      data-testid="approval-modal"
    >
      {#if request.kind === 'oracle'}
        <div class="head">
          <span class="glyph">◆</span>
          <h2 id="approval-title" class="disp">ORACLE APPROVAL REQUIRED</h2>
        </div>
        <div class="meta mono">
          <span class="uid">{request.unit.id}</span>
          <span class="tier">{request.unit.tier}</span>
        </div>
        <p class="task">{request.unit.task}</p>

        <div class="block">
          <div class="block-h disp">FROZEN TEST SET · {request.unit.oracleFiles.length} file{request.unit.oracleFiles.length === 1 ? '' : 's'}</div>
          {#if request.unit.oracleFiles.length}
            <ul class="files mono" data-testid="approval-files">
              {#each request.unit.oracleFiles as f}<li>{f}</li>{/each}
            </ul>
          {:else}
            <div class="files mono empty">no test files reported</div>
          {/if}
        </div>

        <p class="note">
          Approving freezes these tests as the spec the unit must satisfy. This is a
          human-authority action — it cannot be issued by a plugin.
        </p>

        <div class="acts">
          <button class="act reject disp" data-testid="approval-reject" onclick={onreject}>✕ REJECT</button>
          <button class="act approve disp" data-testid="approval-approve" onclick={onapprove}>✓ APPROVE</button>
        </div>
      {:else}
        <div class="head">
          <span class="glyph warn">⚠</span>
          <h2 id="approval-title" class="disp">CONFIRM REAL LAUNCH</h2>
        </div>
        <p class="task">{request.req.task}</p>

        <div class="block">
          <div class="block-h disp">LAUNCH PARAMETERS</div>
          <div class="kv mono"><span>MODE</span><b class="hot">REAL</b></div>
          <div class="kv mono"><span>TIER</span><b>{request.req.tier.toUpperCase()}</b></div>
          <div class="kv mono"><span>REVIEW ROUNDS</span><b>{request.req.min_review_rounds}</b></div>
        </div>

        <p class="note warn">
          REAL mode spends real money and needs ANTHROPIC_API_KEY + Docker. This launch
          will create a live unit.
        </p>

        <div class="acts">
          <button class="act reject disp" data-testid="approval-reject" onclick={onreject}>✕ CANCEL</button>
          <button class="act approve disp" data-testid="approval-approve" onclick={onapprove}>▸ LAUNCH FOR REAL</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 1000; /* above all content */
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(2, 8, 12, 0.78);
    backdrop-filter: blur(2px);
    animation: fade 0.16s ease both;
  }
  .modal {
    position: relative;
    width: min(520px, 100%);
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 22px;
    background: linear-gradient(180deg, var(--panel-2), var(--panel));
    border: 1px solid var(--cyan-dim);
    border-radius: 3px;
    box-shadow: 0 0 0 1px var(--cyan-glow), 0 20px 60px rgba(0, 0, 0, 0.6);
    outline: none;
    animation: rise 0.18s ease both;
  }

  .head { display: flex; align-items: center; gap: 10px; }
  .glyph { color: var(--cyan); font-size: 18px; filter: drop-shadow(0 0 6px var(--cyan-glow)); }
  .glyph.warn { color: var(--amber); filter: drop-shadow(0 0 6px rgba(240, 180, 41, 0.5)); }
  h2.disp { margin: 0; font-size: 15px; letter-spacing: 2px; font-weight: 800; color: #eaf6fa; }

  .meta { display: flex; align-items: center; gap: 10px; }
  .meta .uid { color: var(--text); font-size: 12px; font-weight: 600; }
  .meta .tier { font-size: 10px; color: var(--violet); letter-spacing: 1px; border: 1px solid var(--line-2); padding: 1px 5px; border-radius: 2px; }

  .task { margin: 0; font-size: 13px; line-height: 1.4; color: var(--text-dim); }

  .block { display: flex; flex-direction: column; gap: 5px; }
  .block-h { font-size: 10px; letter-spacing: 2px; color: var(--text-faint); border-bottom: 1px solid var(--line-1); padding-bottom: 4px; }
  .files { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 3px; max-height: 180px; overflow-y: auto; }
  .files li { font-size: 11.5px; color: var(--text); padding-left: 12px; position: relative; }
  .files li::before { content: '›'; position: absolute; left: 0; color: var(--cyan-dim); }
  .files.empty { color: var(--text-faint); font-size: 11.5px; }

  .kv { display: flex; justify-content: space-between; font-size: 11.5px; color: var(--text-dim); }
  .kv b { color: var(--text); }
  .kv b.hot { color: var(--amber); }

  .note { margin: 0; font-size: 11px; line-height: 1.4; color: var(--text-faint); }
  .note.warn { color: var(--amber); }

  .acts { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 4px; }
  .act {
    padding: 11px 0; font-size: 13px; letter-spacing: 1.5px; font-weight: 700;
    border-radius: 2px; cursor: pointer; border: 1px solid var(--line-2);
    font-family: var(--font-display);
  }
  .act.reject { background: var(--elev); color: var(--text-dim); }
  .act.reject:hover { border-color: var(--red); color: var(--red); }
  .act.approve {
    background: linear-gradient(180deg, var(--cyan), var(--cyan-dim)); color: #021a19;
    border-color: transparent; box-shadow: 0 0 16px var(--cyan-glow);
  }
  .act.approve:hover { filter: brightness(1.08); }

  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
</style>
