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

/**
 * One seq-tagged log line, retained by the store so the view-plugin bridge can build
 * `log-append` deltas since a plugin's per-unit seq cursor. Kept deliberately OUTSIDE
 * the reactive `Unit` (whose `fleet.ts` `log` is capped + untagged): the bridge needs
 * the daemon `seq` to diff against a cursor, and this tail must not drive a component
 * re-render, so it lives in a plain (non-`$state`) store field.
 */
export interface LogDelta {
  seq: number;
  stream: string;
  line: string;
}

/** How many seq-tagged log lines the store retains per unit for plugin `log-append`. */
const LOG_TAIL_CAP = 500;

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

  // ── view-plugin command-sink support (Lane V) ──────────────────────────────
  // The store is the SINGLE command sink: every daemon write (built-in ops grid AND
  // policed plugin commands) flows through `launch`/`cmd`, which own `cmd_id`, the
  // optimistic insert, and socket-open. Plugin-originated commands are validated by the
  // bridge's command policy BEFORE they reach these methods; trusted built-in calls skip
  // it. These plain (non-reactive) accumulators are what the bridge drains on its tick —
  // Svelte-5 runes tell *components* what changed but hand a plain consumer nothing, so
  // the fold/command path records touched ids here explicitly.
  //
  // `dirty` = ids touched since the bridge last drained (drives per-unit `state` deltas).
  private dirty = new Set<string>();
  // `logTail` = per-unit seq-tagged log lines, for `log-append` since a plugin's cursor.
  private logTail: Record<string, LogDelta[]> = {};

  // A REAL-mode launch the host has staged but not yet confirmed. While set, the host
  // overlay (Lane O) shows a confirm modal; nothing is sent to the daemon until the human
  // confirms. The store stays the single source of fleet state, so the modal is driven
  // purely off this field — independent of which view/plugin is mounted.
  pendingLaunch = $state<CreateReq | null>(null);

  // ── derived projections ────────────────────────────────────────────────────
  readonly selected = $derived(this.selectedId ? this.units[this.selectedId] : null);
  readonly list = $derived(this.order.map((id) => this.units[id]).filter(Boolean));
  readonly activeCount = $derived(this.list.filter((u) => !isTerminal(u.phase)).length);
  readonly totalBurn = $derived(this.list.reduce((s, u) => s + u.cost, 0));

  /**
   * The first unit (in fleet order) parked in `awaiting_oracle_approval`, or null.
   * This is the host overlay's trigger (Lane O): when non-null, the host pops a
   * focus-stealing Oracle Approval modal regardless of the active view/plugin. Driven
   * purely by host-owned fleet state, so a plugin cannot suppress or satisfy it.
   */
  readonly awaitingApproval = $derived(
    this.list.find((u) => u.phase === 'awaiting_oracle_approval') ?? null,
  );

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
      this.markDirty(s.unit_id);
      this.ensureStream(s.unit_id);
    }
    if (!this.selectedId && this.order.length) this.selectedId = this.order[0];
  }

  /**
   * Open the WS stream for a unit exactly once. Both `reconnect` (snapshot repopulate)
   * and `launch` (optimistic new unit) route through here so a bridge `launch` racing a
   * concurrent `reconnect()` can never open a second socket for the same unit — the
   * single-socket half of the single-sink guarantee.
   */
  private ensureStream(id: string): void {
    if (this.sockets[id]) return;
    this.sockets[id] = openStream(id, 0, (e) => this.onEvt(id, e));
  }

  /** Record that a unit changed so the plugin bridge picks it up on its next drain. */
  private markDirty(id: string): void {
    this.dirty.add(id);
  }

  /**
   * Drain the set of unit ids touched since the last call (the bridge invokes this on its
   * ~60ms tick to build per-unit `state` deltas). Returns the ids and clears the set.
   */
  drainDirty(): string[] {
    const ids = [...this.dirty];
    this.dirty.clear();
    return ids;
  }

  /**
   * Seq-tagged log lines for a unit with `seq > sinceSeq` — the bridge's `log-append`
   * source, diffed against the plugin's per-unit cursor. Bounded by `LOG_TAIL_CAP`.
   */
  logsSince(id: string, sinceSeq: number): LogDelta[] {
    const tail = this.logTail[id];
    if (!tail) return [];
    return sinceSeq <= 0 ? tail.slice() : tail.filter((l) => l.seq > sinceSeq);
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
    this.markDirty(id);
    if (e.event.type === 'log') {
      const tail = (this.logTail[id] ??= []);
      tail.push({ seq: e.seq, stream: e.event.stream, line: e.event.line });
      if (tail.length > LOG_TAIL_CAP) tail.splice(0, tail.length - LOG_TAIL_CAP);
    }
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

  /**
   * Launch a new mission, optimistically seed its tile, open its stream, select it. The
   * single command sink for new units: the ops grid calls it directly (trusted); the
   * plugin bridge calls it only AFTER its command policy passes. Guards the optimistic
   * insert against a concurrent `reconnect()` that already materialised this id from a
   * snapshot, so a bridge `launch` racing a reconnect yields exactly one unit + one
   * socket (`ensureStream` enforces the socket half).
   */
  async launch(req: CreateReq): Promise<string> {
    const id = await createMission(req);
    if (!this.units[id]) {
      this.units[id] = newUnit(id, req.task, req.tier.toUpperCase());
      this.order = [id, ...this.order];
      this.markDirty(id);
    }
    this.selectedId = id;
    this.ensureStream(id);
    return id;
  }

  /**
   * Stage a REAL-mode launch for human confirmation instead of firing it immediately.
   * Sets `pendingLaunch`, which the host overlay (Lane O) renders as a confirm modal.
   * DEMO launches don't need confirmation — call `launch()` directly for those.
   */
  requestRealLaunch(req: CreateReq): void {
    this.pendingLaunch = req;
  }

  /**
   * Confirm the staged REAL launch: actually create the mission and clear the pending
   * state. Returns the new unit id, or null if nothing was staged. Clears
   * `pendingLaunch` even if the launch throws, so a failed confirm closes the modal and
   * surfaces the error through the normal launch path.
   */
  async confirmLaunch(): Promise<string | null> {
    const req = this.pendingLaunch;
    if (!req) return null;
    this.pendingLaunch = null;
    return this.launch(req);
  }

  /** Dismiss a staged REAL launch without sending anything to the daemon. */
  cancelLaunch(): void {
    this.pendingLaunch = null;
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
