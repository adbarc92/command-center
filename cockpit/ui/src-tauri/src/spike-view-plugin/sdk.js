// THROWAWAY P4 SPIKE — minimal plugin SDK. Served by the `ccplugin://` scheme handler
// under the plugin-doc CSP (`script-src 'self'; connect-src 'none'`). Do NOT merge to main.
//
// Implements the plugin side of the handshake the spike proves:
//   gate b      — plugin announces `plugin-hello`, host replies `init` (+ transferred
//                 MessagePort), plugin acks `ready` over the private port.
//   gate c-CORS — can an opaque-origin module `import()` a 2nd same-scheme module?
//   gate c-CSP  — `connect-src 'none'` must block ALL network (fetch rejects ⇒ pass).

const hsEl = document.getElementById('hs');
const orgEl = document.getElementById('org');
// For a sandbox without `allow-same-origin` this prints the literal "null" — the whole
// point: identity is the held port, never the (unusable) origin.
if (orgEl) orgEl.textContent = String(location.origin);

// Listen for the host's `init` BEFORE announcing readiness so the transferred port is
// never missed (plugin-announces-ready handshake — eliminates the classic load-race).
window.addEventListener('message', (e) => {
  const msg = e.data;
  if (!msg || msg.type !== 'init') return;
  const port = e.ports && e.ports[0];
  if (!port) return;

  // gate b — acknowledge over the private port. This is what the host counts.
  port.postMessage({ v: 1, type: 'ready' });
  if (hsEl) hsEl.textContent = 'READY';

  // gate c-CORS — opaque-origin module graph importing a same-scheme sibling module.
  // If this throws, the real runtime falls back to a single inlined bundle (spec fallback #1)
  // — or simply have the scheme handler add `Access-Control-Allow-Origin: *` (in-place remedy).
  import('./probe.js')
    .then((m) => port.postMessage({ v: 1, type: 'probe', corsOk: m.PROBE === 'probe-ok' }))
    .catch((err) => port.postMessage({ v: 1, type: 'probe', corsOk: false, corsErr: String(err) }));

  // gate c-CSP — connect-src 'none' must block the network. A reject is the PASS case.
  fetch('https://example.com/cc-spike-probe', { mode: 'no-cors' }).then(
    () => port.postMessage({ v: 1, type: 'probe', networkBlocked: false }),
    () => port.postMessage({ v: 1, type: 'probe', networkBlocked: true }),
  );
});

// Plugin announces readiness first. A null-origin child may postMessage to parent with "*".
parent.postMessage({ v: 1, type: 'plugin-hello', apiVersion: 1 }, '*');
