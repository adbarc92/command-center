// Shared fleet state, extracted out of App.svelte (Lane A / SHELL §"keep store.svelte.ts
// the one source of fleet state"). A single `FleetStore` instance owns the units,
// their live WebSocket streams, the selection, and daemon health, so that every view
// (the inline Fleet cockpit + the Project Dashboard) projects the SAME state. Switching
// views never tears the store down, so units/selection/streams survive a round trip.
//
// This is a `.svelte.ts` rune module: `$state`/`$derived` fields on a class give us
// fine-grained reactivity that any component reading the instance picks up.

import { createMission, sendCommand, openStream, listUnits, health, type CreateReq } from './api';
import { fold, fromSnapshot, newUnit, isTerminal, type Unit } from './fleet';
import type { CommandName, Envelope, FleetEvent, Health, Phase, Snapshot } from './types';

/** A live `phase_changed` listener — the seam the Dashboard's `onFleetPhase` prop wires to. */
type PhaseListener = (unitId: string, to: Phase, task: string, tier: string) => void;

export class FleetStore {
  units = $state<Record<string, Unit>>({});
  order = $state<string[]>([]);
  selectedId = $state<string | null>(null);
  daemon = $state<Health | null>(null);

  // Live sockets, one per unit. Not reactive (never rendered), just owned for cleanup.
  private sockets: Record<string, WebSocket> = {};
  // Live `phase_changed` subscribers (the Dashboard wires one here so its fleet lane
  // advances off the very same stream the cockpit already consumes — no second socket).
  private phaseListeners = new Set<PhaseListener>();
  // Guards against double-connecting (e.g. a re-mounted Fleet view re-calling reconnect).
  private started = false;

  // ── derived projections ────────────────────────────────────────────────────
  readonly selected = $derived(this.selectedId ? this.units[this.selectedId] : null);
  readonly list = $derived(this.order.map((id) => this.units[id]).filter(Boolean));
  readonly activeCount = $derived(this.list.filter((u) => !isTerminal(u.phase)).length);
  readonly totalBurn = $derived(this.list.reduce((s, u) => s + u.cost, 0));

  /** All current units as `GET /units`-shaped snapshots (seeds the Dashboard's fleet lane). */
  snapshots(): Snapshot[] {
    return this.list.map((u) => ({
      unit_id: u.id,
      phase: u.phase,
      cost: u.cost,
      usd_cap: u.usdCap,
      tier: u.tier,
      task: u.task,
      last_seq: u.lastSeq,
    }));
  }

  /**
   * Repopulate the fleet from the daemon and (re)connect a stream to each unit,
   * replaying its full event log. Idempotent: safe to call from `onMount` even if a
   * previous mount already connected — existing units/sockets are left untouched.
   */
  async reconnect(): Promise<void> {
    try {
      this.daemon = await health();
    } catch {
      this.daemon = null;
    }
    let snaps: Snapshot[];
    try {
      snaps = await listUnits();
    } catch {
      return; // daemon unreachable; the header badge already reflects it
    }
    for (const s of snaps) {
      if (this.units[s.unit_id]) continue;
      this.units[s.unit_id] = fromSnapshot(s);
      this.order = [...this.order, s.unit_id];
      this.sockets[s.unit_id] = openStream(s.unit_id, 0, (e) => this.onEvt(s.unit_id, e));
    }
    if (!this.selectedId && this.order.length) this.selectedId = this.order[0];
  }

  /** First-mount entry point: connect once, return a teardown the view can ignore. */
  start(): void {
    if (this.started) return;
    this.started = true;
    void this.reconnect();
  }

  /** Fold one stream event into its unit; broadcast `phase_changed` to subscribers. */
  onEvt(id: string, e: Envelope): void {
    const prev = this.units[id];
    if (!prev) return;
    if (e.seq <= prev.lastSeq) return; // dedup replayed/overlapping frames
    const next = fold({ ...prev }, e.event);
    next.lastSeq = e.seq;
    this.units[id] = next;
    if (e.event.type === 'phase_changed') this.emitPhase(id, e.event);
  }

  private emitPhase(id: string, ev: Extract<FleetEvent, { type: 'phase_changed' }>): void {
    const u = this.units[id];
    if (!u) return;
    for (const cb of this.phaseListeners) cb(id, ev.to, u.task, u.tier);
  }

  /**
   * Subscribe to live `phase_changed` events (the Dashboard's `onFleetPhase` seam).
   * Returns an unsubscribe. The board advances its fleet cards off this — no second WS.
   */
  onPhase(cb: PhaseListener): () => void {
    this.phaseListeners.add(cb);
    return () => this.phaseListeners.delete(cb);
  }

  /** Launch a new mission, optimistically seed its tile, open its stream, select it. */
  async launch(req: CreateReq): Promise<string> {
    const id = await createMission(req);
    this.units[id] = newUnit(id, req.task, req.tier.toUpperCase());
    this.order = [id, ...this.order];
    this.selectedId = id;
    this.sockets[id] = openStream(id, 0, (e) => this.onEvt(id, e));
    return id;
  }

  /** Send a control command to a unit (defaults to the selected one). */
  async cmd(name: CommandName, unitId = this.selectedId): Promise<void> {
    if (unitId) await sendCommand(unitId, name);
  }

  select(id: string): void {
    this.selectedId = id;
  }

  /** Close every socket. Call only on real teardown (app unmount), not a view switch. */
  dispose(): void {
    Object.values(this.sockets).forEach((s) => s.close());
  }
}

/**
 * The one shared instance. Importing this module anywhere yields the same store, so
 * the Fleet view and the Project Dashboard project a single source of fleet truth and
 * switching between them preserves units, sockets, and selection.
 */
export const fleet = new FleetStore();
