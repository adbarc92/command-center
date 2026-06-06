// Thin client for the fleetd HTTP/WS server.

import type { CommandName, Envelope } from './types';

const BASE: string =
  (import.meta as any).env?.VITE_FLEET_URL ?? 'http://127.0.0.1:8787';

export interface CreateReq {
  task: string;
  tier: 't1' | 't2' | 't3';
  mode: 'demo' | 'real';
  min_review_rounds: number;
}

export async function createMission(req: CreateReq): Promise<string> {
  const res = await fetch(`${BASE}/missions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(req),
  });
  if (!res.ok) throw new Error(`create mission failed: ${res.status} ${await res.text()}`);
  const { unit_id } = (await res.json()) as { unit_id: string };
  return unit_id;
}

let cmdSeq = 0;
export async function sendCommand(
  unitId: string,
  command: CommandName,
  extra: Record<string, unknown> = {},
): Promise<number> {
  const body = { command, cmd_id: `c${Date.now()}-${cmdSeq++}`, ...extra };
  const res = await fetch(`${BASE}/units/${unitId}/commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  return res.status;
}

/** Open the event stream for a unit. Returns the socket so callers can close it. */
export function openStream(unitId: string, onEvent: (e: Envelope) => void): WebSocket {
  const wsBase = BASE.replace(/^http/, 'ws');
  const ws = new WebSocket(`${wsBase}/units/${unitId}/stream`);
  ws.onmessage = (msg) => {
    try {
      onEvent(JSON.parse(msg.data) as Envelope);
    } catch {
      /* ignore malformed frames */
    }
  };
  return ws;
}

export const FLEET_BASE = BASE;
