// @ts-nocheck
// Reference view-plugin — the small UI shell that dogfoods the FULL message surface:
//   • dirty `state` deltas       → renders the unit list + coarse `degraded` badge
//   • `log-append`               → per-unit operation ticker (last line)
//   • `log-reset`                → clears tickers on a daemon-stream reconnect
//   • a policed demo `launch`    → button; ack shown
//   • a `command-ack` REJECTION  → a REAL launch (blocked pending host confirm) + a halt
//                                  on a non-existent id both surface their rejection ack
//   • presence during `awaiting_oracle_approval` → a NON-INTERACTIVE indicator only
//     (approval is a host-authority action rendered by the host overlay — never a plugin
//     verb; this plugin has no approve/reject button and cannot dismiss the modal).
//
// The SDK is served alongside this plugin as ./sdk.js (the loader/packaging step copies
// cockpit/plugin-sdk there — see Lane S integration note). "Trivial" here means a small
// UI, NOT narrow coverage: the contract is exercised end-to-end, not merely compiled.

import { connect } from './sdk.js';
import {
  createModel,
  applyState,
  applyLog,
  applyReset,
  recordAck,
  list,
  awaitingApproval,
  ticker,
} from './model.js';

const model = createModel();
const el = (id) => document.getElementById(id);

// Log/unit fields are untrusted daemon text relayed by the host — escape before it ever
// touches innerHTML (defense-in-depth even inside the sandboxed, network-less frame).
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

function render() {
  const banner = el('banner');
  const await_ = awaitingApproval(model);
  if (await_) {
    // NON-INTERACTIVE: no verb. The host overlay owns approval.
    banner.textContent = `◆ AWAITING ORACLE APPROVAL · ${await_.id} · respond in host overlay`;
    banner.hidden = false;
  } else {
    banner.hidden = true;
  }

  el('degraded').textContent = model.degraded ? 'DAEMON: DEGRADED' : 'DAEMON: OK';

  const rows = list(model)
    .map((u) => {
      const t = ticker(model, u.id);
      const line = t ? esc(`${t.stream}: ${t.line}`) : '—';
      return `<li data-id="${esc(u.id)}"><b>${esc(u.id)}</b> <span>${esc(u.phase)}</span> · $${u.cost.toFixed(3)} · <i>${line}</i></li>`;
    })
    .join('');
  el('units').innerHTML = rows || '<li class="empty">no units</li>';

  const last = model.acks[model.acks.length - 1];
  el('acks').textContent = last
    ? `ack ${last.reqId}: ${last.ok ? 'OK' : `REJECTED (${last.reasonClass})`}`
    : 'no commands yet';
}

connect({ timeoutMs: 3000 })
  .then((fleet) => {
    el('status').textContent = `connected · caps: ${fleet.capabilities.join(', ') || 'none'}`;

    fleet.onState((s) => {
      applyState(model, s);
      render();
    });
    fleet.onLog((unitId, lines) => {
      applyLog(model, unitId, lines);
      render();
    });
    fleet.onReset(() => {
      applyReset(model);
      render();
    });
    fleet.onAck((ack) => {
      recordAck(model, ack);
      render();
    });

    // Policed DEMO launch.
    el('launch-demo').onclick = async () => {
      await fleet.launch({ task: 'reference demo launch', tier: 't1', mode: 'demo' });
    };
    // REAL launch → blocked pending host confirm → REJECTION ack surfaces here.
    el('launch-real').onclick = async () => {
      await fleet.launch({ task: 'reference real launch', tier: 't1', mode: 'real' });
    };
    // Halt the first unit (or a bogus id, to exercise the unknown-id rejection ack).
    el('halt').onclick = async () => {
      const first = list(model)[0];
      await fleet.command(first ? first.id : 'no-such-unit', 'halt');
    };

    render();
  })
  .catch((err) => {
    el('status').textContent = `failed to connect: ${err.message}`;
  });
