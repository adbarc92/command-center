<script lang="ts">
  // Top-level view switcher (Lane A / SHELL). A thin segmented control that toggles
  // the active top-level view. Generic over a list of {id,label} entries so SHELL can
  // add more entries (Audience, skin) later without touching this component.
  export interface ViewEntry {
    id: string;
    label: string;
    /** Optional attention count rendered as a badge (e.g. "needs you"). */
    badge?: number;
  }

  interface Props {
    views: ViewEntry[];
    active: string;
    onselect: (id: string) => void;
  }

  let { views, active, onselect }: Props = $props();
</script>

<nav class="switcher" data-testid="switcher" aria-label="Top-level view">
  {#each views as v (v.id)}
    <button
      class="seg disp"
      class:on={active === v.id}
      data-testid="switch-{v.id}"
      aria-pressed={active === v.id}
      onclick={() => onselect(v.id)}
    >
      {v.label}
      {#if v.badge}<span class="badge mono" data-testid="switch-badge-{v.id}">{v.badge}</span>{/if}
    </button>
  {/each}
</nav>

<style>
  .switcher {
    display: flex;
    gap: 0;
    border: 1px solid var(--line-2);
    border-radius: 2px;
    overflow: hidden;
    background: var(--panel);
  }
  .seg {
    background: transparent;
    color: var(--text-dim);
    border: none;
    border-right: 1px solid var(--line-1);
    padding: 7px 16px;
    font-size: 12px;
    letter-spacing: 1.5px;
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .seg:last-child { border-right: none; }
  .seg.on { background: var(--cyan); color: #02201f; }
  .seg:not(.on):hover { background: var(--elev); color: var(--text); }
  .badge {
    font-size: 10px;
    font-weight: 700;
    min-width: 16px;
    height: 16px;
    padding: 0 5px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--amber);
    color: #2a1c00;
  }
  .seg.on .badge { background: #02201f; color: var(--cyan); }
</style>
