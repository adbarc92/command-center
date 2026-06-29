// Faithful browser message/port semantics for view-plugin handshake tests.
//
// WHY THIS EXISTS: jsdom's built-in MessagePort does NOT enforce `start()` — a probe
// showed `addEventListener('message')` delivers even when `start()` is never called.
// Real browsers / WebView2 (the cockpit's actual runtime) DO require `start()`: a
// transferred port buffers and delivers nothing until started. The single most likely
// cause of "every handshake fails" is a host that listens on the transferred port but
// forgets to start it — and jsdom's lenient port would hide exactly that bug.
//
// These fakes model the STRICTER real semantics so the handshake suite can actually
// catch the class of deterministic, 100%-failure bugs. They are a transport model, not
// a mock of the code under test: `bridge.ts` is exercised for real; only the window/port
// substrate is modeled (and modeled to be more realistic than jsdom, not less).
//
// NOT collected as a suite (filename lacks `.test.`); imported by the handshake tests.

export interface MessageEventish {
  data: unknown;
  origin: string;
  source: FakeWindow | null;
  ports: FakePort[];
}

type MsgListener = (e: MessageEventish) => void;

/** A MessagePort that, like a real one, delivers nothing until `start()` (or `onmessage`). */
export class FakePort {
  /** The entangled peer; set by FakeMessageChannel. */
  peer!: FakePort;
  started = false;
  closed = false;
  private inbound: unknown[] = [];
  private listeners = new Set<(e: { data: unknown }) => void>();
  private _onmessage: ((e: { data: unknown }) => void) | null = null;

  /** Send to the PEER's inbound (serialized, mimicking the structured-clone boundary). */
  postMessage(data: unknown): void {
    if (this.closed) return;
    this.peer.receive(structuredClone(data));
  }

  private receive(data: unknown): void {
    if (this.closed) return;
    if (!this.started) {
      this.inbound.push(data); // buffer until started — the real behavior jsdom omits
      return;
    }
    this.dispatch(data);
  }

  start(): void {
    if (this.started) return;
    this.started = true;
    const q = this.inbound;
    this.inbound = [];
    for (const d of q) this.dispatch(d);
  }

  close(): void {
    this.closed = true;
  }

  private dispatch(data: unknown): void {
    queueMicrotask(() => {
      if (this.closed) return;
      const e = { data };
      this._onmessage?.(e);
      for (const l of this.listeners) l(e);
    });
  }

  addEventListener(type: 'message', cb: (e: { data: unknown }) => void): void {
    if (type === 'message') this.listeners.add(cb);
  }
  removeEventListener(type: 'message', cb: (e: { data: unknown }) => void): void {
    this.listeners.delete(cb);
  }
  set onmessage(cb: ((e: { data: unknown }) => void) | null) {
    this._onmessage = cb;
    if (cb) this.start(); // assigning onmessage auto-starts, exactly like the real API
  }
  get onmessage(): ((e: { data: unknown }) => void) | null {
    return this._onmessage;
  }
}

export class FakeMessageChannel {
  port1 = new FakePort();
  port2 = new FakePort();
  constructor() {
    this.port1.peer = this.port2;
    this.port2.peer = this.port1;
  }
}

/**
 * A Window that models `postMessage(data, targetOrigin, transfer)` delivery and the
 * `message` event shape. Used as the cockpit host window and the plugin iframe's
 * contentWindow. Delivery is async (a microtask), like the real event loop.
 */
export class FakeWindow {
  private listeners = new Set<MsgListener>();
  constructor(public readonly origin = 'null') {}

  addEventListener(type: 'message', cb: MsgListener): void {
    if (type === 'message') this.listeners.add(cb);
  }
  removeEventListener(type: 'message', cb: MsgListener): void {
    this.listeners.delete(cb);
  }

  /**
   * The standard 3-arg DOM signature the production bridge calls
   * (`iframe.contentWindow.postMessage(init, '*', [port2])`). `source` is left null —
   * the SDK identifies the host by the transferred port, never by event.source.
   */
  postMessage(data: unknown, targetOrigin: string, transfer: FakePort[] = []): void {
    this.deliver({ data, origin: '', source: null, ports: transfer }, targetOrigin);
  }

  /**
   * Lower-level delivery used by the test's fake plugin to post `plugin-hello` to the
   * host with a proper `source` (the plugin window) — mirroring a child posting to
   * `parent`. Honors targetOrigin so a mismatched-origin post is dropped, like the browser.
   */
  deliver(evt: MessageEventish, targetOrigin = '*'): void {
    queueMicrotask(() => {
      if (targetOrigin !== '*' && targetOrigin !== this.origin) return; // browser drops it
      for (const l of this.listeners) l(evt);
    });
  }
}

/** Flush pending microtask-delivered messages. Awaiting this lets queued posts arrive. */
export async function flush(times = 3): Promise<void> {
  for (let i = 0; i < times; i++) await Promise.resolve();
}
