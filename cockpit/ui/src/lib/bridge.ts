// Host side of the view-plugin bridge: the trusted cockpit's end of the handshake with
// an untrusted, sandboxed iframe plugin. Design:
// docs/superpowers/specs/2026-06-07-view-plugins-design.md §"The plugin contract".
//
// Handshake (plugin-announces-ready, no load-race): the host listens, the plugin posts
// `plugin-hello`, the host replies `init` transferring a MessagePort, and thereafter all
// traffic flows over that private port — the host identifies the plugin by HOLDING the
// port, never by event.origin (a sandboxed iframe's origin is the unusable "null").
//
// Structural Window/Port interfaces (not the DOM lib types) so the faithful
// start()-enforcing test harness in bridge.testkit can drive this exactly as a browser would.

export interface MessagePortLike {
  postMessage(data: unknown): void;
  start?(): void;
  close(): void;
  addEventListener(type: 'message', cb: (e: { data: unknown }) => void): void;
  removeEventListener(type: 'message', cb: (e: { data: unknown }) => void): void;
}

export interface MessageEventLike {
  data: unknown;
  ports: readonly MessagePortLike[];
}

export interface WindowLike {
  addEventListener(type: 'message', cb: (e: MessageEventLike) => void): void;
  removeEventListener(type: 'message', cb: (e: MessageEventLike) => void): void;
}

interface IframeLike {
  contentWindow:
    | { postMessage(data: unknown, targetOrigin: string, transfer: MessagePortLike[]): void }
    | null;
}

export interface ConnectOptions {
  /** Window the host listens on for `plugin-hello`. Defaults to the global window. */
  window?: WindowLike;
  /** Channel factory — defaults to the real `MessageChannel`; tests inject a strict fake. */
  channelFactory?: () => { port1: MessagePortLike; port2: MessagePortLike };
  /** Capabilities echoed in `init` (the host's supported set). */
  capabilities?: string[];
  /** API version offered in `init`. Defaults to 1. */
  apiVersion?: number;
  /** Ms to wait for the plugin's `ready` before giving up. Defaults to 3000 (spec). */
  readyTimeoutMs?: number;
  /** API versions the host can speak. A plugin announcing anything else is refused. */
  supportedApiVersions?: number[];
}

/** Why a handshake failed — lets the caller (view-switcher) revert to the ops grid. */
export class HandshakeError extends Error {
  constructor(public readonly reason: 'ready-timeout' | 'unsupported-api-version') {
    super(`view-plugin handshake failed: ${reason}`);
    this.name = 'HandshakeError';
  }
}

export interface PluginHandle {
  /** The private port the host holds; all post-handshake traffic flows over it. */
  readonly port: MessagePortLike;
  /** Tear down: close the port. */
  dispose(): void;
}

interface HelloMsg {
  type?: string;
  apiVersion?: number;
}

export function connectPlugin(iframe: IframeLike, opts: ConnectOptions = {}): Promise<PluginHandle> {
  const win = opts.window ?? (globalThis as unknown as { window: WindowLike }).window;
  const makeChannel = opts.channelFactory ?? (() => new MessageChannel());
  const apiVersion = opts.apiVersion ?? 1;
  const capabilities = opts.capabilities ?? [];
  const readyTimeoutMs = opts.readyTimeoutMs ?? 3000;
  const supportedApiVersions = opts.supportedApiVersions ?? [apiVersion];

  return new Promise<PluginHandle>((resolve, reject) => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    const onHello = (e: MessageEventLike) => {
      const m = e.data as HelloMsg;
      if (m?.type !== 'plugin-hello') return;
      win.removeEventListener('message', onHello);

      // Refuse an unsupported apiVersion BEFORE minting/transferring a port — a plugin
      // the host can't speak to gets nothing.
      if (!supportedApiVersions.includes(m.apiVersion ?? -1)) {
        clearTimeout(timer);
        reject(new HandshakeError('unsupported-api-version'));
        return;
      }

      const { port1, port2 } = makeChannel();

      const onPortMessage = (pe: { data: unknown }) => {
        const rm = pe.data as { type?: string };
        if (rm?.type === 'ready') {
          clearTimeout(timer);
          port1.removeEventListener('message', onPortMessage);
          resolve({
            port: port1,
            dispose() {
              port1.close();
            },
          });
        }
      };
      port1.addEventListener('message', onPortMessage);
      // MUST start the held port or nothing the plugin posts (incl. `ready`) is ever
      // delivered — the deterministic "every handshake fails" trap.
      port1.start?.();

      iframe.contentWindow?.postMessage(
        { v: 1, type: 'init', apiVersion, capabilities },
        '*',
        [port2],
      );

      // Liveness: if `ready` never arrives, give up and let the host revert to the
      // trusted ops grid rather than hanging on a dead plugin.
      clearTimeout(timer);
      timer = setTimeout(() => {
        port1.removeEventListener('message', onPortMessage);
        port1.close();
        reject(new HandshakeError('ready-timeout'));
      }, readyTimeoutMs);
    };

    win.addEventListener('message', onHello);

    // Also bound the wait for `plugin-hello` itself: a frame that never announces is
    // just as dead as one that never readies.
    timer = setTimeout(() => {
      win.removeEventListener('message', onHello);
      reject(new HandshakeError('ready-timeout'));
    }, readyTimeoutMs);
  });
}
