export type Surface = 'ios' | 'android' | 'web' | 'desktop'

export interface FeedbackEvent {
  schema_version: 1
  title: string
  body: string
  release?: { version: string; surface: Surface }
  context?: { platform?: string; os_version?: string; locale?: string }
  reporter?: { anon_id?: string }
  occurred_at?: string
}

export interface RegistryEntry {
  /** "owner/name" */
  repo: string
  /** Which PAT to use, keyed by account. */
  account: 'primary' | 'secondary'
  labels: string[]
}

export interface Env {
  TELLTALE_KV: KVNamespace
  /** JSON: { "<project>": "<hmac secret>" } */
  TELLTALE_SENDER_SECRETS: string
  /** Fine-grained PAT, Issues: read+write, primary account. */
  GITHUB_TOKEN_PRIMARY: string
  /** Fine-grained PAT, Issues: read+write, secondary account. */
  GITHUB_TOKEN_SECONDARY: string
  /** Bearer token the cockpit presents to GET /v1/issues and /v1/stats. */
  OPERATOR_READ_TOKEN: string
  /** Salt for hashing client IPs. */
  IP_HASH_SALT: string
}
