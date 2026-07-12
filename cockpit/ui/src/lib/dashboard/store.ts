// The board's composition layer: one card map fed by four adapters, hybrid refresh.
//
// Spec §8: push where free (fleet `phase_changed`, app-plugin `plugin://state`),
// poll otherwise (Halyard + Audience). The store is framework-agnostic (plain
// functions over a `BoardState`) so it's unit-testable; `Dashboard.svelte` wraps it
// in Svelte 5 runes. Cards are keyed by `projectId`; a poll replaces that source's
// cards wholesale, a push event updates one card in place.

import type { ProjectCard, Source, Stage, StageOverride } from './model';
import { STAGE_ORDINAL, isPipelineStage } from './model';
import type { Phase, Snapshot } from '../types';
import { halyardCards, type HalyardReader } from './adapters/halyard';
import { audienceCards, type AudienceReader } from './adapters/audience';
import { localCards, type LocalReader } from './adapters/local';
import { fleetCard, fleetCardsFromSnapshots, type FleetUnitState } from './adapters/fleet';
import { appPluginCard, type PluginUnit, type PluginState } from './adapters/appPlugin';

export interface BoardState {
  /** All cards by projectId. */
  cards: Record<string, ProjectCard>;
  /** projectId insertion order, for stable display. */
  order: string[];
  /** Per-source last error (null = ok). */
  errors: Partial<Record<Source, string>>;
}

export function newBoard(): BoardState {
  return { cards: {}, order: [], errors: {} };
}

function upsert(state: BoardState, card: ProjectCard): void {
  if (!state.cards[card.projectId]) state.order.push(card.projectId);
  state.cards[card.projectId] = card;
}

/** Replace all cards from one source (a poll result), preserving others. */
export function replaceSource(state: BoardState, source: Source, cards: ProjectCard[]): BoardState {
  // Drop the source's existing cards, then re-insert. Keeps cross-source cards intact.
  const keep = state.order.filter((id) => state.cards[id]?.source !== source);
  const next: BoardState = { cards: {}, order: [...keep], errors: { ...state.errors } };
  for (const id of keep) next.cards[id] = state.cards[id];
  for (const c of cards) upsert(next, c);
  return next;
}

/** Update or insert a single card (a push event). */
export function applyCard(state: BoardState, card: ProjectCard): BoardState {
  const next: BoardState = {
    cards: { ...state.cards },
    order: [...state.order],
    errors: { ...state.errors },
  };
  upsert(next, card);
  return next;
}

// ── pull sources (poll) ─────────────────────────────────────────────────────

export async function pollHalyard(
  state: BoardState,
  reader: HalyardReader,
  overrides: Record<string, StageOverride> = {},
  now: () => Date = () => new Date(),
): Promise<BoardState> {
  const cards = await halyardCards(reader, { overrides, now });
  return replaceSource(state, 'halyard', cards);
}

export async function pollAudience(
  state: BoardState,
  reader: AudienceReader,
  overrides: Record<string, StageOverride> = {},
  now: () => Date = () => new Date(),
): Promise<BoardState> {
  const cards = await audienceCards(reader, { overrides, now });
  return replaceSource(state, 'audience', cards);
}

export async function pollLocal(
  state: BoardState,
  reader: LocalReader,
  now: () => Date = () => new Date(),
): Promise<BoardState> {
  const cards = await localCards(reader, { now });
  return replaceSource(state, 'local', cards);
}

// ── push sources (events) ────────────────────────────────────────────────────

/** Seed fleet cards from `GET /units` snapshots on load. */
export function seedFleet(
  state: BoardState,
  snaps: Snapshot[],
  overrides: Record<string, StageOverride> = {},
  now: () => Date = () => new Date(),
): BoardState {
  return replaceSource(state, 'fleet', fleetCardsFromSnapshots(snaps, { overrides, now }));
}

/**
 * Advance one mission's card on a `phase_changed` event (live, §6.3). The unit's
 * task/tier come from the existing card so an event alone advances the stage.
 */
export function applyFleetPhase(
  state: BoardState,
  unitId: string,
  to: Phase,
  fallback: { task: string; tier: string },
  overrides: Record<string, StageOverride> = {},
  now: () => Date = () => new Date(),
): BoardState {
  const existing = state.cards[`fleet:${unitId}`];
  const unit: FleetUnitState = {
    id: unitId,
    phase: to,
    task: fallback.task,
    tier: fallback.tier,
  };
  // Prefer the already-known name if we have one (the card's name was task-derived).
  void existing;
  return applyCard(state, fleetCard(unit, { overrides, now }));
}

/** Update one hosted-app card on a `plugin://state` event (live, §6.4). */
export function applyPluginState(
  state: BoardState,
  unit: PluginUnit,
  overrides: Record<string, StageOverride> = {},
  now: () => Date = () => new Date(),
): BoardState {
  return applyCard(state, appPluginCard(unit, { overrides, now }));
}

// ── derived selectors ────────────────────────────────────────────────────────

export function cardList(state: BoardState): ProjectCard[] {
  return state.order.map((id) => state.cards[id]).filter(Boolean);
}

/** A card is stale when its source poll/push is older than its freshness budget (§8). */
export function isStale(card: ProjectCard, nowMs: number): boolean {
  const updated = Date.parse(card.updatedIso);
  if (Number.isNaN(updated)) return true;
  return nowMs - updated > card.staleAfterSec * 1000;
}

/** The board's single most valuable number: how many projects need a human (§5.1). */
export function blockedCount(state: BoardState): number {
  return cardList(state).filter((c) => c.stage === 'Blocked').length;
}

/** Sort: Blocked/Failed first (urgency), then pipeline by ordinal, then Idle/Archived. */
export function sortedCards(state: BoardState): ProjectCard[] {
  const rank = (s: Stage): number => {
    if (s === 'Failed') return 0;
    if (s === 'Blocked') return 1;
    if (isPipelineStage(s)) return 2 + STAGE_ORDINAL[s];
    return 99; // Idle
  };
  return [...cardList(state)].sort((a, b) => rank(a.stage) - rank(b.stage));
}

export type { PluginState };
