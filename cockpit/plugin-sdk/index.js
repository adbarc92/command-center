// @ts-nocheck
// Cockpit view-plugin SDK (Lane V) — bundled convenience for UNTRUSTED plugins that run
// in a sandboxed iframe. It speaks the MessagePort protocol the host bridge expects:
//
//   connect() → posts `plugin-hello` to the parent, awaits the host's `init` (which
//   transfers a private MessagePort), replies `ready`, then exposes typed callbacks and
//   promise-returning command verbs. All traffic after the handshake is on the port.
//
// The plugin never sees the daemon URL, host DOM, storage, or network — only this port.
// `connect()` accepts an injectable `scope`/`parent` purely so it is testable off a real
// window; in the iframe it defaults to `window` / `window.parent`.

const PROTOCOL_VERSION = 1;

/**
 * Perform the plugin-announces-ready handshake and resolve to a connected client.
 * @param {{ scope?: any, parent?: any, timeoutMs?: number }} [opts]
 * @returns {Promise<PluginClient>}
 */
export function connect(opts = {}) {
  const scope = opts.scope ?? (typeof window !== 'undefined' ? window : undefined);
  const parent = opts.parent ?? (scope ? scope.parent : undefined);
  if (!scope || !parent) {
    return Promise.reject(new Error('cockpit-sdk: no window/parent to connect through'));
  }
  return new Promise((resolve, reject) => {
    let settled = false;
    let timer = null;

    function onMessage(e) {
      const d = e && e.data;
      if (!d || d.v !== PROTOCOL_VERSION || d.type !== 'init') return;
      const port = e.ports && e.ports[0];
      if (!port) return;
      settled = true;
      if (timer !== null) clearTimeout(timer);
      scope.removeEventListener('message', onMessage);
      resolve(attach(port, d));
    }

    scope.addEventListener('message', onMessage);
    // The plugin announces readiness FIRST (avoids a host `load`-post race). `"*"` is safe:
    // the hello is non-sensitive and the sandbox frame's real origin is the unusable "null".
    parent.postMessage({ v: PROTOCOL_VERSION, type: 'plugin-hello' }, '*');

    if (opts.timeoutMs) {
      timer = setTimeout(() => {
        if (settled) return;
        scope.removeEventListener('message', onMessage);
        reject(new Error('cockpit-sdk: handshake timed out'));
      }, opts.timeoutMs);
    }
  });
}

/**
 * Wrap an already-transferred MessagePort as a client. Exposed for tests that drive the
 * port directly (the normal path is `connect()`).
 * @param {MessagePort} port
 * @param {{ apiVersion?: number, capabilities?: string[] }} init
 * @returns {PluginClient}
 */
export function attach(port, init = {}) {
  const stateCbs = [];
  const logCbs = [];
  const resetCbs = [];
  const ackCbs = [];
  const pending = new Map(); // reqId → resolve
  let reqSeq = 0;

  port.onmessage = (e) => {
    const m = e && e.data;
    if (!m || m.v !== PROTOCOL_VERSION) return;
    switch (m.type) {
      case 'state':
        for (const cb of stateCbs) cb(m);
        break;
      case 'log-append':
        for (const cb of logCbs) cb(m.unitId, m.lines);
        break;
      case 'log-reset':
        for (const cb of resetCbs) cb();
        break;
      case 'command-ack': {
        for (const cb of ackCbs) cb(m);
        const resolve = pending.get(m.reqId);
        if (resolve) {
          pending.delete(m.reqId);
          resolve(m);
        }
        break;
      }
      default:
        // forward-compatible: ignore unknown host messages
        break;
    }
  };
  if (port.start) port.start();

  // Reply `ready` — the host answers with a full `state` snapshot.
  port.postMessage({ v: PROTOCOL_VERSION, type: 'ready' });

  function send(payload) {
    const reqId = `p${reqSeq++}`;
    return new Promise((resolve) => {
      pending.set(reqId, resolve);
      port.postMessage({ v: PROTOCOL_VERSION, type: 'command', reqId, ...payload });
    });
  }

  return {
    apiVersion: init.apiVersion,
    capabilities: init.capabilities ?? [],
    /** Dirty-delta (or full) state pushes: `{ full, changed, removed, order, degraded }`. */
    onState(cb) {
      stateCbs.push(cb);
      return () => {
        const i = stateCbs.indexOf(cb);
        if (i >= 0) stateCbs.splice(i, 1);
      };
    },
    /** Append-only log deltas: `(unitId, lines)` where each line is `{ seq, stream, line }`. */
    onLog(cb) {
      logCbs.push(cb);
      return () => {
        const i = logCbs.indexOf(cb);
        if (i >= 0) logCbs.splice(i, 1);
      };
    },
    /** Fired on a daemon-stream reconnect: discard per-unit log cursors (a full state follows). */
    onReset(cb) {
      resetCbs.push(cb);
      return () => {
        const i = resetCbs.indexOf(cb);
        if (i >= 0) resetCbs.splice(i, 1);
      };
    },
    /** Every `command-ack`, correlated by `reqId` (also resolves the originating promise). */
    onAck(cb) {
      ackCbs.push(cb);
      return () => {
        const i = ackCbs.indexOf(cb);
        if (i >= 0) ackCbs.splice(i, 1);
      };
    },
    /** Request a launch. Resolves to the `command-ack` (ok, or rejected with reasonClass). */
    launch(req) {
      return send({ launch: req });
    },
    /** Request a unit command (halt/resume/abandon/ship). Resolves to its `command-ack`. */
    command(unitId, action) {
      return send({ unit: { id: unitId, action } });
    },
  };
}
