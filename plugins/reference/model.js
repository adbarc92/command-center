// @ts-nocheck
// Reference view-plugin — pure state model (no DOM). Factored out of app.js so the full
// message surface it consumes (dirty `state` deltas, `log-append`, `log-reset`, and
// `command-ack`) is unit-testable without a browser. app.js renders off this model.

/** A fresh, empty model. */
export function createModel() {
  return {
    /** unitId → UnitLite (latest). */
    units: {},
    /** fleet order (from the host). */
    order: [],
    /** coarse daemon health. */
    degraded: false,
    /** unitId → accumulated log lines [{seq,stream,line}]. */
    logs: {},
    /** every command-ack seen (newest last). */
    acks: [],
  };
}

/**
 * Apply a `state` message. A FULL snapshot resets the unit baseline (matching the host's
 * `lastEmitted` reset); a delta merges only its changed units. `removed` is reserved-empty
 * this cycle but honored if present.
 */
export function applyState(model, msg) {
  if (msg.full) model.units = {};
  for (const u of msg.changed) model.units[u.id] = u;
  for (const id of msg.removed ?? []) delete model.units[id];
  model.order = msg.order ?? model.order;
  model.degraded = !!msg.degraded;
  return model;
}

/** Append `log-append` lines to a unit's buffer (append-only, seq-ordered). */
export function applyLog(model, unitId, lines) {
  const buf = (model.logs[unitId] ??= []);
  for (const l of lines) buf.push(l);
  return model;
}

/** On `log-reset` (daemon-stream reconnect): discard all accumulated log lines. */
export function applyReset(model) {
  model.logs = {};
  return model;
}

/** Record a `command-ack` (correlated by reqId at the SDK layer). */
export function recordAck(model, ack) {
  model.acks.push(ack);
  return model;
}

/** Units in fleet order (skips any id in `order` without a materialized unit). */
export function list(model) {
  return model.order.map((id) => model.units[id]).filter(Boolean);
}

/**
 * The first unit parked in `awaiting_oracle_approval`, or null. The plugin renders only a
 * NON-INTERACTIVE indicator from this — approval is a host-authority action (the overlay),
 * never a plugin verb.
 */
export function awaitingApproval(model) {
  return list(model).find((u) => u.phase === 'awaiting_oracle_approval') ?? null;
}

/** The operation-ticker for a unit = its last log line, or null. */
export function ticker(model, unitId) {
  const buf = model.logs[unitId];
  return buf && buf.length ? buf[buf.length - 1] : null;
}
