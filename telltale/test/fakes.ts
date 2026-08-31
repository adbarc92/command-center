/** Minimal in-memory KVNamespace stand-in. Enough for counters and TTL-less reads. */
export class FakeKV {
  store = new Map<string, string>()
  async get(k: string): Promise<string | null> { return this.store.get(k) ?? null }
  async put(k: string, v: string, _o?: { expirationTtl?: number }): Promise<void> { this.store.set(k, v) }
  async delete(k: string): Promise<void> { this.store.delete(k) }
  async list({ prefix }: { prefix: string }) {
    return { keys: [...this.store.keys()].filter((k) => k.startsWith(prefix)).map((name) => ({ name })) }
  }
}
