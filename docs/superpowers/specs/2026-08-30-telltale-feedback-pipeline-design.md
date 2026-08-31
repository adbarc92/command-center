# Telltale — User Bug/Crash → Issue Pipeline — Design Spec

> A **project-generalizable** pipeline that turns user-reported bugs and runtime crashes, from any
> app or game in the portfolio, into issues in that project's own tracker — and surfaces them as a
> **seventh source** in the Command Center Project Dashboard.
>
> Status: **design**. Author: brainstorming session 2026-08-30. Single operator, single machine.
> Standalone by construction: every phase before P3 works with no Command Center running.

A *telltale* is the ribbon on a sail that shows which way the wind is actually blowing; in
engineering it is a warning indicator. It sits beside **Halyard** (release coordination) and does
not overlap with it (§7).

**This spec was cut roughly in half by three rounds of adversarial critique.** Read §11 before
proposing anything already removed, and §0 for the shape of the reduction.

---

## 0. The shape of the system, after cutting

The first draft built a distributed counter store, a threshold gate, a dedup decision table, and a
fleet-dispatch extension. Critique established that **Sentry already does the crash half** and that
**fleet dispatch should not be part of this system at all**. What remains:

| Half | Mechanism | Telltale code |
|---|---|---|
| **Crashes** | Sentry SDK → alert rule → **Sentry's native GitHub integration** opens and links the issue | **None.** Per-repo configuration (§3 — four steps, not trivial). |
| **Bug reports** | In-app form → one Cloudflare Worker → GitHub issue | ~350 lines + 4 senders |
| **Visibility** | Worker read endpoint; one dashboard adapter | ~1 adapter (§6.7 records the case against even this) |
| **Dispatch** | **Out of scope.** The operator works the issue by hand. | **None.** |

---

## 1. Goal & scope

**Goal:** an automated `user bug/crash → issue tracking` pipeline that works for every shipped app
and game and terminates in something the operator can act on.

**In scope:** **T1** the `FeedbackEvent` contract; **T2** per-platform senders; **T3** the ingest
worker; **T4** the GitHub Issues sink; **T5** a `feedback` source adapter for the Project Dashboard.

**Covered projects (operator decision 2026-08-30): apps + games.**

| Project | Platform | Sender | Notes |
|---|---|---|---|
| `tenzy`, `giftkeeper`, `purposefull` | Expo / React Native | RN | |
| `ironsoul` | Flutter | Dart | |
| `audience`, `lineage`, `robo.learn` | Web | Web | |
| `prima-tactica` | Godot 4.6 (verified in `project.godot`) | GDScript | outside `CURRENT/` |
| `hexy` | Vanilla JS | Web | outside `CURRENT/` |

`perennial-blade` and `heart-of-the-gods` are **cut**: jam-scale games, plausibly zero users, no
`docs/` directory.

**Out of scope (YAGNI, deliberate):** an in-cockpit triage UI (GitHub Issues *is* the triage
surface); screenshots/attachments; a user-facing reply thread; analytics; multi-tenant; CLI and
library projects; **`kind: "idea"`**; and **fleet dispatch** (§6.5).

### Locked decisions (do not relitigate)

1. **The deliverable is a contract, not an SDK** (§2). Senders are copied reference files.
2. **Telltale does not handle crashes.** Sentry's native GitHub integration owns that path end to
   end (§3). Telltale is a **bug-report** pipeline that *reads* the issues Sentry creates.
3. **Ingest runs on a Cloudflare Worker**, stateless apart from small KV counters.
4. **The ingest is authenticated, and the header is the sole project authority** (§4.1).
5. **Issues land in the project's own repo by default** (operator decision 2026-08-30; accepted
   trade-offs in §4.3 and §10). The registry can redirect **bug reports** with no code change;
   **redirecting crashes means hand-editing Sentry alert rules** (§4.3).
6. **The cockpit holds no *GitHub* credential.** It reads through the Worker with a Worker-scoped
   read token that is never shipped in an app binary (§5.4).
7. **Dispatch is out of scope** (§6.5).
8. **Standalone before integrated.** P1–P2 ship and are useful with no Command Center running.

---

## 2. The contract (T1)

The covered set spans Expo/React Native, Flutter, web, Godot 4, and vanilla JS. **No single client
SDK spans those**; building one means five runtimes of maintenance for a payload ten fields wide.
Every one of them can POST JSON.

Senders are ~60-line reference files, **copied** rather than depended upon: a project that copies one
takes on zero upgrade obligation, and an archived project keeps working forever.

### 2.1 `FeedbackEvent`

```jsonc
{
  "schema_version": 1,
  "title":       "Save button does nothing on the guide screen",
  "body":        "…",              // user-typed free text
  "release":     { "version": "1.4.2", "surface": "android" },  // optional
  "context":     { "platform": "android", "os_version": "14", "locale": "en-US" },
  "reporter":    { "anon_id": "…" }, // opaque, install-scoped, NOT trusted (§4.2)
  "occurred_at": "2026-08-30T18:04:11Z"
}
```

Three fields an earlier draft carried are gone:

- **`kind`** — every event on this endpoint is a bug report; crashes never traverse it (§3).
- **`fingerprint`** — the Worker computes it (§4.4); requiring senders to produce a value the server
  discards was ceremony.
- **`project`** — **it now lives only in the `X-Telltale-Project` header** (§4.1). Carrying it in
  both places, with nothing requiring them to match, meant an extracted secret from the weakest
  client could publish into any repo in the registry. One authority, one field fewer in five senders.

| Field | Rule |
|---|---|
| `schema_version` | MUST be `1`. Unknown → `400`, never best-effort parsed. |
| `title` | 1–120 chars after trim. **User-typed, therefore scrubbed** (§4.3). |
| `body` | 0–8000 chars. Truncated with a visible marker, never rejected — a user's report is not worth losing to a length rule. |
| `release.surface` | `ios\|android\|web\|desktop`. |
| `context` | Bounded key set (`platform`, `os_version`, `locale`). Unknown keys dropped, not stored — this prevents `context` becoming an unaudited PII channel. |
| `reporter.anon_id` | Opaque ≤64 chars, install-scoped. |
| `occurred_at` | RFC 3339, informational. **Not** security-load-bearing — the replay window uses a signed header (§4.1). |

**Deliberately absent: any contact field.** No `email`, `name`, or `account_id` — a schema-level
guarantee that a sender *cannot* transmit contact details in a structured field. (Users can still
type them into `title`/`body`; §4.3.)

### 2.2 The release key

`release` serializes to `{project}-{surface}@{version}`, the shape Halyard's `sentry-client.ts`
builds. **A SHOULD, not a MUST** — `INFRASTRUCTURE/halyard/apps/` contains one app (`aurora`) and no
covered project is Halyard-onboarded. Named so the key exists if correlation is ever wanted (§7).

---

## 3. Crashes: Sentry's native GitHub integration, and no Telltale code

**Telltale does not touch the crash path.** Sentry already provides, as configuration: grouping
(better than anything computed here), per-issue event and distinct-user counts, an alert-rule
condition of exactly the form *"an issue is seen by more than 3 users in 24h"*, and a **native GitHub
integration** that opens one issue per Sentry issue, links them bidirectionally, and does not reopen
on every event.

An earlier draft routed Sentry through the Worker to recompute all of this. That was wrong twice:
it duplicated the vendor, and it could not work — the only server-observed identity on a webhook is
*Sentry's own egress IP*, so a `distinct_server_identities ≥ 3` gate had cardinality 1 and could
never fire (§11.6).

**Configuration required per project — four steps, and this is more work than "configuration only"
suggests:**

1. **Sentry project + SDK.** DSN wired; releases tagged `{project}-{surface}@{version}`;
   symbolication / source-map upload.
2. **Data scrubbing — do not skip.** Set `sendDefaultPii: false`, enable Sentry's server-side data
   scrubbers, and add a `beforeSend` that strips request URLs and local variables. **Crash issues are
   authored by Sentry directly into the same public repos and never pass through §4.3's scrub.** A
   Sentry payload is a *richer* PII channel than a bug form: exception messages, breadcrumb values,
   URLs with tokens, `user.email` when default PII is on, and locals in stack frames. This step is
   the crash half's entire PII control.
3. **Pre-create the labels in the target repo** — `telltale`, `telltale:crash`. Sentry's alert-action
   label field is a picker over labels that already exist in that repo, so a rule cannot reference a
   label that has never been created there. Per-repo bootstrap, easy to forget, silent when missed.
4. **The issue-alert rule:** *seen by > 3 users in 24h* → create a GitHub issue in **that project's
   own repo** (per locked decision #5), with the labels from step 3. Pin the rule frequency
   deliberately; the default throttles to one notification per issue per 30 min, which is fine here
   because Sentry counts, not Telltale.

Telltale's only relationship to crashes is that the **dashboard adapter reads those issues** by
label, exactly as it reads bug issues (§6).

### 3.1 Instrumentation is the real blocker, and P0 is scoped down because of it

**Verified 2026-08-30.** Nine project trees grepped for `sentry|bugsnag|crashlytics|rollbar`; seven
are in the covered set:

| Checked, in scope | Crash reporting |
|---|---|
| `tenzy` | **Yes** — `client/lib/sentry.ts`, `@sentry/*` in `client/package.json` |
| `giftkeeper`, `audience`, `purposefull`, `lineage`, `robo.learn` | None found |
| `ironsoul` | None in source (only a Dart build artifact matched) |

`appforge` and `reqdrive` were checked but are **out of scope**. **No game was checked.** So: *of
seven in-scope projects checked, one has crash reporting; the games are unmeasured.* An earlier
draft's "eleven of twelve" extrapolated beyond its sample and is withdrawn.

Rolling this out portfolio-wide means all four steps above across Expo/RN, Flutter, **Godot (no
first-party SDK — a community GDExtension)**, and vanilla JS. For a single operator that is plausibly
larger than everything else here combined. **P0 is therefore a two-project feasibility probe** —
`tenzy` (already instrumented) plus one game — not a rollout (§9). The bug half has no such
dependency and ships first.

---

## 4. The ingest worker (T3)

One Cloudflare Worker, three routes:

| Route | Auth | Purpose |
|---|---|---|
| `POST /v1/events` | Per-project HMAC (§4.1) | Bug-report intake |
| `GET /v1/issues` | **Operator read token** (§5.4) | The cockpit's read path |
| `GET /v1/stats` | **Operator read token** | Operability (§4.6) |

It holds the only GitHub credential in the system.

### 4.1 Authentication

The threat is concrete: this endpoint turns an HTTP request into a **public GitHub issue in the
operator's repo, authored by the operator's token** — a remote "publish arbitrary text under Alex's
identity" primitive. Slugs are guessable by design.

- Headers: `X-Telltale-Project`, `X-Telltale-Timestamp`,
  `X-Telltale-Signature: HMAC-SHA256(secret, timestamp + "." + rawBody)`.
- **`X-Telltale-Project` is the sole authority** for both secret selection and repo resolution. The
  body carries no project field (§2.1), so the two cannot disagree.
- **Signed over raw request bytes**, never a canonicalized JSON re-serialization. Five independent
  canonicalizers (GDScript, Dart, RN, browser JS, Worker) agreeing byte-for-byte on key order and
  number formatting is a silent-`401` generator.
- The timestamp is a **header**, not `occurred_at`, so senders never reason about canonicalization.
- Replay window ±10 min against server time. **No nonce: a captured request is replayable inside
  that window.** Accepted — the payoff is a duplicate report, which dedup collapses anyway.
- **Clock skew** (common on Android) is handled by the response, not by client state: a
  skew rejection returns `401` with the server time in a `X-Telltale-Server-Time` header, and the
  sender's *already-mandated* single retry (§8.3) re-signs with it. An earlier draft had each of five
  runtimes persist a clock offset across sessions — heavier than the problem, and it failed on a
  device's first report, which for many users is the only one they ever send.

**A secret shipped inside a distributed client binary is extractable.** HMAC raises the bar from a
one-line `curl` to reverse-engineering a specific app and — the real benefit — makes abuse
**attributable to one project**, so the response is rotating one secret. Because the header is now
the sole authority, that rotation actually protects the victim. It is a managed risk (§10.1), not a
solved one.

### 4.2 Rate limiting, and the honest role of `anon_id`

`anon_id` is client-controlled and regenerable, so it is **never a security boundary**. But per-IP
limiting alone is wrong here: carrier-grade NAT puts thousands of mobile users behind one IPv4
address, and four of five sender rows are mobile. A tight per-IP cap would silently destroy the 21st
genuine reporter on a carrier.

| Key | Limit |
|---|---|
| IP + `anon_id` pair | 10 events/hour |
| IP (distinct `anon_id`s) | 200 events/hour |
| Project | 1000 events/hour |

Stored in Worker **KV** with 1h TTLs. KV is correct *here* — approximate abuse counters where a lost
increment is harmless. (An earlier draft used KV for a correctness-critical dedup gate, where it was
not; that gate no longer exists.) Exceeded → `429` + `Retry-After`; dropped, not queued.

### 4.3 PII scrub — accepted-risk mitigation

**Recorded operator decision (2026-08-30): issues land in the project's own repo, always, including
public repos. The exposure below was raised and knowingly accepted.**

Residual hazard: `title` and `body` are user-typed. A user who types their email into a bug report on
a public repo has it published permanently — editing does not scrub issue history.

1. **Ingest-side scrub of BOTH `title` and `body`**, before the sink. An earlier draft scrubbed only
   `body`, leaving the issue *title* — the most visible, most indexed, most notification-carrying
   field — unredacted. Email and E.164/NANP phone patterns → a visible `[redacted:email]` marker, so
   triage knows something was removed rather than silently mangled. The **long-digit-run** rule is
   `body`-only (on a title it is all false positives). Redaction is lossy and one-way: the original
   is never stored, because storing it recreates the hazard.
2. **Submit-time notice** adjacent to submit, stating the report will be posted publicly.

**This covers the bug half only.** The crash half never passes through the Worker, so its PII
control is Sentry-side scrubbing (§3 step 2) — a different mechanism in a different console, and the
one most likely to be skipped. Similarly, the escape hatch differs: redirecting **bug reports** to a
private intake repo is a `registry.yml` edit, but redirecting **crashes** means hand-editing each
Sentry alert rule. Locked decision #5's "no code change" claim is scoped accordingly.

Neither scrub is a guarantee — patterns miss obfuscated forms ("alex at example dot com"), and a
notice is not consent. **Accepted as tolerable at this portfolio's scale.**

**Request-level data:** Cloudflare and the Worker observe the client IP. Used transiently for rate
limiting, stored only as a salted hash with a 1h TTL, **never written to an issue**.

### 4.4 Fingerprinting and dedup

`fingerprint = sha256(normalize(scrubbed_title))[0:16]`, where `normalize` lowercases, collapses
whitespace, and strips punctuation. **Computed over the scrubbed title** so identity is stable
regardless of what redaction removed.

This is *deliberately weak* grouping over the title alone. It catches verbatim repeats and misses
paraphrases. Semantic grouping is a model call, and a model call in the dedup path makes issue
identity non-deterministic and untestable. **Not doing it is the decision.**

**Dedup is a label lookup.** The fingerprint is carried as the label `tt:{fingerprint}`. Before
creating, the Worker calls `GET /repos/{o}/{r}/issues?labels=tt:{fingerprint}&state=all`:

| Result | Action |
|---|---|
| No match | **Create** with the `tt:` label. |
| Open match | **Comment** (throttled: one per fingerprint per hour, KV-tracked). |
| Closed, `state_reason: completed` | **Comment only.** Never reopen. |
| Closed, `state_reason: not_planned`, or labeled `telltale:muted` | **Ignore.** Silent by operator intent. |
| Closed, `state_reason: null` (legacy closures) | Treat as `completed` — comment only. |
| Multiple matches | Use the lowest-numbered open one; log `duplicate_fingerprint` to `/v1/stats`. |
| Any match that is a **pull request** | Skip it. `GET /issues` returns PRs as issues; a fix PR carrying the `telltale` label would otherwise read as an open bug report. Filter on the `pull_request` key. |

**Reopen is never automatic.** An earlier draft's "reopen always" was a trap: mobile users run old
builds for months, so a crash fixed in 1.4.3 keeps arriving from 1.4.1 clients and would perpetually
reopen the issue its PR closed.

**The label is the idempotency key — for retries, not for concurrency.** `POST /issues` has no
idempotency key, so a create that succeeds with a lost response would double-open; the pre-create
lookup finds that orphan on retry. But the lookup is a **check-then-act with no mutual exclusion**:
two simultaneous reports of the same title can both query before either create returns, and both
create. That is why the "multiple matches" row exists. A narrowing mitigation — a `tt:` KV marker
with a 30s TTL, reusing the throttle infrastructure — collapses the window to sub-second without
reintroducing a Durable Object. **The residual duplicate is accepted and listed in §10.5.**

**Label-creation failure is silent and must be detected.** GitHub **silently drops** `labels` on
`POST /issues` when the token lacks push/triage access on that repo. Since `tt:` is simultaneously
the idempotency key, the dedup key, and the read-path key, a dropped label means every subsequent
report opens a fresh duplicate forever — while the Worker reports success. **The Worker MUST verify
`labels` in the create response and emit a `labels_dropped` rejection to `/v1/stats`.**

**Over-merge has no mechanism, by decision.** Title-only grouping will occasionally merge two
unrelated defects. An earlier draft's `telltale:split` re-salted with the retired issue number but
still keyed on `normalize(title)`, so both defects produced the same new fingerprint and re-merged
immediately — a "start over" button, not a split. The remedy is the operator closing the issue and
opening two by hand. **Zero mechanism beats a mechanism that does not work** (§11.3).

**Label growth is unbounded and deliberately unmanaged.** Every distinct report title mints a
permanent `tt:` label in a shipped product's repo; after a year the label picker holds hundreds. This
is still the right trade — labels are the only queryable, indexed key, and the REST list endpoint's
`labels=` filter is exact, AND-semantic, and not subject to the search API's 30 req/min limit or its
eventual consistency. The janitor job it implies (delete `tt:` labels whose issue closed >90d ago) is
**named and deliberately not built**.

### 4.5 Ordering and failure

Scrub → fingerprint → label lookup → create-or-comment → verify labels. A GitHub failure returns
`503`; the sender retries once (§8.3) and the label lookup makes that retry safe. **No queue** — a
durable hosted queue is a service to operate, and the failure it guards costs one report.

### 4.6 A read path, so silence is diagnosable

`GET /v1/stats` returns accepted/rejected counts by reason over 24h — no bodies, no PII. Reasons
include `bad_signature`, `clock_skew`, `rate_limited`, `unregistered_project`, `labels_dropped`,
`duplicate_fingerprint`. Roughly 30 lines.

Without it, the operator's observable universe is "issues appear, or nothing happens," and a `401`
storm from signature drift, a rate-limit drop, a dropped-label cascade, and *"nobody filed a bug this
week"* are **the same observation**. §9.1's grader is a synthetic probe; it cannot explain why
production is quiet. This is the difference between an operable system and one abandoned the first
time it goes silent.

---

## 5. The GitHub Issues sink (T4)

### 5.1 The registry

A file **checked into this repo** at `telltale/registry.yml`:

```yaml
version: 1
projects:
  tenzy:
    repo: <org>/tenzy
    labels: [telltale]
  prima-tactica:
    repo: <org>/prima-tactica
    labels: [telltale, game]
  pawsport:
    repo: <org>/telltale-intake   # crash-only; no sender ships for this project
    labels: [telltale]
  __probe__:
    repo: <org>/telltale-probe    # §9.1's grader target — never a real product repo
    labels: [telltale]
```

- The **Worker** bundles it at build time and is its only authority. The cockpit does not read it —
  it calls `GET /v1/issues` and the Worker resolves repos. An earlier draft gave the cockpit its own
  copy; drift there means writing a user's bug report into the wrong repository.
- An entry **without a shipping sender** (e.g. `pawsport`) is crash-only: it receives Sentry-created
  issues and appears on the board, but no app POSTs to it.
- `base_branch`/`test_cmd` are gone with dispatch (§6.5).

### 5.2 Issue shape

- **Title:** the scrubbed `title`, prefixed `[bug]`. (Sentry sets its own for crashes.)
- **Labels:** `telltale`, `telltale:bug`, `tt:{fingerprint}`, plus the registry's per-project labels.
- **Body:** the scrubbed body, plus context and release, and a human-facing footer.

An earlier draft also wrote `origin:{project}`. **Cut:** under locked decision #5 the issue is in the
project's own repo, so *the repo is the project* — the Worker knows which repo it queried and maps it
back through the registry. `origin:` carried zero information and cost a hand-configured field in
every Sentry alert rule.

Also cut: `telltale:count-*` labels (§11.4) — they needed a lifetime counter the store cannot
provide, and churned a label on every event.

**Note the body is free.** `GET /repos/{o}/{r}/issues` returns each issue's full `body` in the list
response. An earlier draft justified the label scheme by "avoiding body parsing"; that was a strawman
(the real cost was the *comments* fetch it had chosen). Labels are still right for dedup and
filtering — indexed and queryable, which a body is not — but prose is available for free.

### 5.3 Repos that cannot receive issues

`pawsport` and `elevation-broker` are believed archived on GitHub and to reject writes; some repos
are private with a known Actions billing failure. **Asserted, not verified this session** — the
GitHub MCP server failed to connect (`400: Authorization header is badly formatted`). **Verify before
P1.** The registry redirect exists either way, configured explicitly, never inferred.

### 5.4 Credentials

| Holder | Credential | Scope |
|---|---|---|
| Worker | GitHub App installation token, minted per-request from the App private key in Worker secrets | `issues: write`, `metadata: read` |
| Worker | Per-project HMAC secrets | one per sender |
| Cockpit | **A Worker-scoped read token** — `TELLTALE_TOKEN`, read in `dashboard.rs` alongside the existing `HALYARD_BIN` / `AUDIENCE_API_URL` env vars | Calls `/v1/issues` and `/v1/stats` only |

**The cockpit holds no *GitHub* credential** — that is the claim, and it is narrower than an earlier
draft's "no credential." `/v1/issues` cannot be anonymous: the Worker can read **private** registry
repos, so an open read endpoint would serve every bug-report body in every private repo to anyone who
guesses the hostname. The read token is distinct from the per-project sender secrets and is **never
shipped in an app binary**, which is what makes it meaningfully different from the extractable HMAC
secrets of §4.1.

Two earlier drafts got the GitHub side wrong: `public_repo` is a *write* scope and cannot read
private repos; a second App installation in the cockpit was worse, since installation tokens expire
hourly and are minted by signing a JWT with the **App private key** — the desktop would hold a
credential minting tokens for every installed repo.

The registry spans two accounts (two separate GitHub accounts), so the App needs an installation on each
— two installations, one private key, all Worker-side.

---

## 6. Command Center integration (T5)

### 6.1 What actually changes

The dashboard spec's locked decision #6 says a new source is "one new adapter, no board change."
**That is not true here**, and the local-tracker spec already retracted the same overclaim for itself.
Verified change surface:

| File | Change |
|---|---|
| `cockpit/ui/src/lib/dashboard/model.ts` | `Source` union `+ 'feedback'`; **amend the file-header comment**, which still asserts "adding a fifth source later is one new adapter, zero board change" — the exact overclaim this table retracts |
| `cockpit/ui/src/lib/dashboard/adapters/feedback.ts` | New adapter + `FeedbackReader` seam |
| `cockpit/ui/src/lib/dashboard/api.ts` | `tauriFeedbackReader`, beside `tauriHalyardReader` / `tauriAudienceReader` |
| `cockpit/ui/src/lib/dashboard/store.ts` | New `pollFeedback` |
| `cockpit/ui/src/views/Dashboard.svelte` | `SOURCE_LABEL` entry (without it the badge renders raw lowercase `feedback` via the `?? c.source` fallback); a `pollFeedback` call |
| `cockpit/ui/src-tauri/src/dashboard.rs` | `feedback_issues` command; `TELLTALE_BASE_URL` + `TELLTALE_TOKEN` env vars beside the existing `HALYARD_BIN` / `AUDIENCE_API_URL` |
| `cockpit/ui/src-tauri/src/lib.rs` | Register `dashboard::feedback_issues` in `tauri::generate_handler![…]` — without it the command does not exist at runtime |

`model.ts`'s `dispatch` field and its "local source only" comment are **untouched** — a consequence
of cutting dispatch (§6.5).

### 6.2 The adapter and its read seam

```ts
export interface TelltaleIssue {
  repo: string;
  number: number;
  title: string;
  body: string;
  kind: 'bug' | 'crash' | 'unknown';  // explicit whitelist over telltale:* — see below
  project: string;                    // resolved by the Worker from the registry, not from a label
  isOpen: boolean;
  hasAssignee: boolean;               // the triage signal (§6.4)
  createdIso: string;                 // `n new this week` is a created_at question
  updatedIso: string;
  labels: string[];
  url: string;
}

/** The swappable read seam — mirrors `HalyardReader` / `AudienceReader`. */
export interface FeedbackReader {
  issues(): Promise<TelltaleIssue[]>;
}
```

`kind` is derived from an **explicit whitelist** (`telltale:bug` → `bug`, `telltale:crash` → `crash`,
anything else → `unknown`), not a `telltale:*` prefix parse — `telltale:muted` also matches that
prefix, and an issue hand-labeled plain `telltale` matches neither.

Written against `FeedbackReader` rather than `fetch`, for the same reason `halyard.ts` is written
against `HalyardReader`: environment-agnostic and **unit-testable with fakes**.

### 6.3 Transport, cadence, and partial failure

`dashboard.rs` gains `feedback_issues`, which calls `GET /v1/issues` and — following
`halyard_status`'s existing pattern of returning an untyped `Value` and letting TypeScript map it —
does no parsing in Rust.

**Cadence.** The cockpit polls the *Worker*, not GitHub, so the GitHub-side arithmetic an earlier
draft worried about no longer applies. The Worker caches the issues list for 60s and uses conditional
requests (`ETag` / `If-None-Match`) upstream, where 304s do not count against the REST limit. GitHub
therefore sees at most one request per repo per minute regardless of poll rate. The earlier draft's
second mitigation — a per-source 300s interval requiring a structural change to `Dashboard.svelte`'s
single-timer `refresh()` — is **redundant and dropped**; the existing 15s shared timer is fine.
`staleAfterSec: 600`.

**Partial repo failure is required behaviour, not an edge case.** §5.3 states outright that some
registry repos are archived and some are private with broken billing. If `/v1/issues` fans out over
~10 repos and one 403s, a naive implementation fails the whole call — and because a failed poll
yields a single synthetic `__source__` card while `replaceSource` drops that source's prior cards,
**one bad repo would blank the entire feedback lane.** So `/v1/issues` returns the repos that
answered plus a per-repo error list, and the adapter degrades only the affected projects.

### 6.4 Cards

**One card per registry project *with at least one open issue*.** An earlier draft emitted a card per
registry project including empty ones; that is wrong for a verified reason: `sortedCards` ranks
`Idle` at **99**, below `Archived`, so ~10 permanently-grey "no open reports" cards would pile at the
bottom of the grid, inflate the header's project `total`, and duplicate projects already on the board
via the `local` source — carrying no information to justify the duplication.

| Condition | Stage | Detail |
|---|---|---|
| Any open `telltale:crash` issue with **no assignee** | `Blocked` — `BlockedInfo { gate: 'manual', action: 'triage crash report', deepLink: <issue url> }` | `n open · untriaged crash` |
| Any other open issues | `Idle` | `n open · m new this week` |

Precedence is explicit, top row wins.

**Why `Idle` and not `Build`.** `model.ts` documents `PIPELINE` as the *"cross-tool happy-path
pipeline"* — where a project sits in its lifecycle. Mapping "has ≥1 open bug report" to `Build` would
make every shipped project permanently display **BUILD**, and `sortedCards` ranks `Build` above
`Review`, `Ship`, and `Live` — so "one old bug exists" would sort above "this project is live in
production." `Idle` is the honest reading: *known to a source, not currently advancing.*

**Why `no assignee` and not a `telltale:triaged` label.** `blockedCount` is the board's headline
**"NEEDS YOU"** number, so a `Blocked` condition that never clears is a permanent false positive on
the board's most valuable signal. An earlier draft gated it on a `telltale:triaged` label — but
nothing created that label, nothing applied it, and no test covered it; it would have required the
operator to type an exact magic string on every crash issue forever, with no affordance and no
default. **Assignee is native, free in the list response, self-clearing, and one click on
github.com.** Self-assigning is already what "I am looking at this" means.

**`family` is set to the project slug but is inert.** Verified: `family` is written by `halyard.ts`,
`audience.ts`, and `appPlugin.ts` and **read by nothing** — `sortedCards` sorts by stage rank and
`Dashboard.svelte` renders a flat grid; the tracker spec lists *"real `family` clustering"* under
"named, not built." A feedback card renders as an unrelated card elsewhere on the board today.

**`health` on a failed poll:** the copied pattern returns a *single synthetic `__source__` card*, and
`replaceSource` drops that source's prior cards — so the per-project feedback cards **do disappear**,
replaced by one placeholder. Stated plainly rather than described as "degrading gracefully."
Last-known-good retention is a `store.ts` change and is **not** in scope.

### 6.5 Dispatch is out of scope

An earlier draft specified fleet dispatch from a feedback card: a `DispatchTarget` type, a `CreateReq`
change in `crates/fleetd/src/server.rs`, a registry-fed repo allowlist, source-conditional write-back,
a modified boot reconcile sweep, and new dispatch UI. **All cut.**

A second draft then justified the cut by claiming the operator could paste a `cc-item` into a
project's `ROADMAP.md` and "the existing local-tracker Phase 2 path dispatches it with zero changes."
**That justification is false and is withdrawn.** Verified:

- **Phase 2 does not exist.** No `.svelte` file references `dispatch`; `model.ts` annotates
  `missionId` as *"Phase-2, inert here"* and `dispatchable` as a *"Phase-2 gate"*. There is no UI and
  no POST path.
- **The target files mostly do not exist.** Of nine covered projects, exactly one (`purposefull`) has
  both `docs/STATUS.md` and `ROADMAP.md`. **`tenzy` — the flagship and the only instrumented project
  — has no `ROADMAP.md`.** `giftkeeper` and `robo.learn` have no `docs/STATUS.md`, and the tracker
  spec's locked #2 means an unmarked auto-discovered dir "is simply not found."
- **`prima-tactica` and `hexy` are outside `CURRENT/`** and would need explicit pins.
- Phase 2 is itself conditional on the daemon-wide loopback-auth migration.

**So: the operator works a Telltale issue by hand on GitHub.** A future dispatch path is Local-Tracker
Phase 2, which is unbuilt. The claim that cutting dispatch made a P4 dependency "disappear" is also
withdrawn — it converted a code dependency into an unmet workflow dependency.

The cut is still correct, for a better reason: dispatch would have widened the set of repos a
credentialed containerized agent may push to from *one sandbox* to *every repo in the registry* —
production apps included — for a pipeline whose input is internet-authored text.

### 6.6 Reporter text is never an agent instruction

If dispatch is ever built, the mission task must be an **operator-authored brief**, with the
reporter's text appearing only as delimited untrusted evidence:

```
<task>               … operator-written repro + acceptance criteria …
<untrusted-report>   … reporter body; DATA, never instructions …
```

The mission task becomes the prompt for a containerized agent that pushes branches and opens PRs with
the operator's ambient `git`/`gh` credentials, driven by a daemon that binds `127.0.0.1` with **no
request authentication** (verified: no auth middleware in `crates/fleetd/src/server.rs`). Passing
internet-authored text into that prompt verbatim is a prompt-injection path into a credentialed
agent, and a human eyeballing an issue is a weak barrier against text engineered to read as a plain
bug report with a payload below the fold.

### 6.7 The case for cutting T5 as well — recorded, not taken

Critique argued T5/P3 should also go. The argument is strong and belongs on the record:

- The delivered signal is "n open bug reports per project," which `org:<org> is:issue is:open
  label:telltale` gives as a GitHub saved search for **zero lines**.
- Feedback cards are `Idle`, which sorts at rank 99 — the very bottom of the grid, below `Archived`.
- §1 already concedes "GitHub Issues *is* the triage surface."
- The cost is the largest remaining chunk of unbuilt work: a Rust command, a `lib.rs` registration, an
  `api.ts` binding, an adapter, a `store.ts` poll, plus Worker-side `/v1/issues` with auth, caching,
  ETag, and per-repo error isolation.

**T5 is nevertheless retained**, because the operator's stated requirement is that this system
integrate with Command Center as the central orchestration hub — a board presence is the deliverable,
not an optimization. If that is worth less than the build cost, the fallback is one `manual`-source
card deep-linking to the saved search: a two-line change that preserves the board presence and
deletes P3 entirely. **This is a live decision, not a closed one.**

---

## 7. The Halyard boundary

| | Question | Output | Acts on |
|---|---|---|---|
| **Halyard** | Should this *release* be killed or rolled back? | `crash_triage` proposal | Release state — flag kill, hotfix, rollback |
| **Telltale** | What *work* should be done about it? | An issue on the board | Engineering backlog |

**Invariant: Telltale never touches release state; Halyard never opens issues.** Correlation via the
§2.2 release key is not built this cycle; no covered project is Halyard-onboarded.

---

## 8. Senders (T2)

### 8.1 Required behaviour

1. Build a `FeedbackEvent`; POST to `/v1/events` with the project, timestamp, and HMAC headers (§4.1).
2. Persist an `anon_id` (generate once, store locally).
3. **Never block the UI.** Fire-and-forget.
4. **Never transmit contact details** in a structured field — the schema has none.
5. Display the §4.3 public-posting notice adjacent to submit.
6. On a `401` carrying `X-Telltale-Server-Time`, re-sign once with that time (§4.1).

**Size:** ~60 lines for RN/web/Dart. **Godot 4 is the outlier** — `HMACContext` exists in Godot 4 but
not Godot 3; `prima-tactica` is verified Godot 4.6. Note §4.1's HMAC means every sender implements
SHA-256 regardless; an earlier draft justified removing the client fingerprint by "sparing senders
from SHA-256," which the auth requirement had already made false.

### 8.2 Reference form

`tenzy`'s shipped feedback screen (`client/app/feedback.tsx`) is the UX model: title + description,
disabled submit until both non-empty, double-submit guard, deep-link-safe back navigation. **Its UX
is the reference; its Convex transport is replaced by the ingest POST.**

### 8.3 Retry

One retry after 2s, then drop. No persistent outbox — that means durable on-device storage of
user-typed text, and one recovered report does not justify it.

---

## 9. Build phasing & testing

| Phase | Content | Depends on |
|---|---|---|
| **P0** | Crash **feasibility probe**: all four §3 steps on `tenzy` + one game. Configuration, no code. | — |
| **P1** | Contract (T1) + Worker: auth, scrub, label dedup, label-drop detection, `/v1/issues`, `/v1/stats` (T3, T4). Standalone. | — |
| **P2** | Senders (T2): RN, web, Dart, GDScript. | P1 |
| **P3** | Command Center `feedback` adapter (T5) — see §6.7. | P1 |

**There is no P4.** P0 and P1 are independent and run concurrently.

### 9.1 Testing

- **Pure logic** — scrub, `normalize`, fingerprint, and the §4.4 decision table are pure functions,
  unit-tested directly. Named cases: scrub applies to **title as well as body**; the long-digit rule
  does **not** apply to titles; the fingerprint is computed over the **scrubbed** title;
  `not_planned` and `telltale:muted` are silent; `state_reason: null` behaves as `completed`; a
  completed-closed match comments and does **not** reopen; **a matching pull request is skipped**.
- **Auth** — unsigned, wrong-secret, and stale-timestamp requests each return `401` and reach no
  sink; a stale-timestamp `401` carries `X-Telltale-Server-Time`.
- **Label-drop detection** — a create whose response omits `labels` emits `labels_dropped` and does
  not report success.
- **Idempotency** — a create whose response is lost, followed by a sender retry, yields **one**
  issue. (The *concurrent* case is a known residual, §10.5 — asserted as a risk, not as a passing test.)
- **Adapter** — mapping tests against a `FeedbackReader` fake, mirroring `adapters.test.ts`:
  the §6.4 precedence order, the `kind` whitelist fallback to `unknown`, per-repo partial failure,
  and the source-down path.
- **Independent grader** — POST N identical synthetic reports **to the `__probe__` registry entry**
  (§5.1), then assert **by reading GitHub back** that exactly one issue exists, then close what it
  created. It must never target a product repo: an earlier draft's grader would have published
  synthetic issues into a shipped product's public tracker, and would have tested the dedup path
  rather than the create path on every run after the first.

---

## 10. Known risks, stated plainly

1. **Abuse of the ingest.** It converts a request into a public issue under the operator's identity.
   HMAC (§4.1) makes abuse attributable and rotatable; rate limiting (§4.2) bounds it; the registry
   redirect (§5.1) is the escalation for the bug half. **A secret in a distributed binary is
   extractable — managed, not solved.** *This risk was never put to the operator as a decision (the
   PII question was); it should be.*
2. **PII in public issues.** Bug reports are scrubbed (§4.3); **crash issues are not** — they are
   authored by Sentry directly and depend entirely on §3 step 2's Sentry-side scrubbing, a different
   control in a different console. The private-repo escape hatch is a config edit for bugs but N
   hand-edited alert rules for crashes.
3. **Prompt injection into a credentialed agent** (§6.6) — currently moot, since dispatch is out of
   scope and the agent's push target stays at the sandbox.
4. **The crash half is inert wherever instrumentation is absent** (§3.1) — six of seven checked
   in-scope projects; the games are unmeasured. §3's four configuration steps are the real cost.
5. **Concurrent duplicate issues** (§4.4) — simultaneous reports of the same title can both pass the
   pre-create lookup. Narrowed by a 30s KV marker, not eliminated. The "multiple matches" row is the
   containment.
6. **Weak grouping** (§4.4) — paraphrased duplicates open separate issues; over-merges have no
   automated remedy by decision; `tt:` labels accumulate without a janitor.
7. **Unverified external facts**, to check before P1: the archive state of `pawsport` /
   `elevation-broker` (§5.3 — GitHub MCP was down this session); Cloudflare free-tier limits for
   Workers + KV; and whether Sentry's GitHub alert action can set the §3 step-3 labels as assumed.

---

## 11. Cut from earlier drafts (recorded so they are not re-proposed)

1. **The whole crash-side pipeline** — Durable Object, threshold gate, dedup decision table, count
   labels. Sentry's alert rules and native GitHub integration do all of it as configuration (§3).
2. **Fleet dispatch** — six verified blockers and a widened agent blast radius (§6.5).
3. **`telltale:split`** — re-salted with the retired issue number but still keyed on
   `normalize(title)`, so both defects re-merged immediately.
4. **`telltale:count-*` labels** — needed a lifetime counter the store could not provide.
5. **A cockpit GitHub credential** — `public_repo` is a write scope; a second App installation means
   the desktop holds the App private key. The cockpit reads through the Worker (§5.4).
6. **`distinct_server_identities ≥ 3` for crashes** — the only server identity on a Sentry webhook is
   Sentry, so the branch had cardinality 1 and could never fire.
7. **`telltale:triaged`** — a magic string nothing created or applied; replaced by assignee (§6.4).
8. **`origin:{project}` labels** — zero information when the issue is in the project's own repo.
9. **`project` in the request body** — a second, unvalidated copy of the header's authority (§2.1).
10. **The per-source 300s poll interval** — redundant once the cockpit polls the Worker (§6.3).
11. **`kind: "idea"`**, the HyperLogLog sketch, `telltale:tier2`, the 30-day regression rule, the
    unsatisfiable Halyard-slug MUST, and `perennial-blade` / `heart-of-the-gods`.
12. **"Zero new dispatch code" / "no board change" / "`family` clusters the cards" / "Phase 2
    dispatches it with zero changes"** — all four verifiably false; replaced by §6.1, §6.4, §6.5.

---

## 12. References

- `docs/superpowers/specs/2026-06-09-project-dashboard-design.md` — `ProjectCard`, `SourceAdapter`,
  the stage model, locked decision #6 (which §6.1 declines to rely on).
- `docs/superpowers/specs/2026-07-06-local-project-tracker-design.md` — the `cc-item` convention and
  the unbuilt Phase 2 path §6.5 declines to depend on.
- `cockpit/ui/src/lib/dashboard/model.ts` · `adapters/halyard.ts` · `adapters/local.ts` · `api.ts` ·
  `store.ts` · `stage.ts` · `views/Dashboard.svelte` · `src-tauri/src/dashboard.rs` ·
  `src-tauri/src/lib.rs` — the change surface in §6.1.
- `crates/fleetd/src/server.rs` — the absent auth middleware behind §6.6.
- `INFRASTRUCTURE/halyard/docs/OBSERVABILITY.md` — the boundary §7 draws.
- `CURRENT/tenzy/client/app/feedback.tsx` — the in-app form UX reference (§8.2).

---

## Design Critique Log

Three independent adversarial rounds, each on the revision the prior round produced. The design lost
roughly half its mass across them; the crash pipeline and fleet dispatch were both deleted outright.

### Critique Round 1

Verified every integration claim against `model.ts`, `halyard.ts`, `local.ts`, `api.ts`, `store.ts`,
`Dashboard.svelte`, `App.svelte`, `roadmap.ts`, `crates/fleetd/src/server.rs`, and Halyard. Fourteen
findings; the severe ones all sustained.

| # | Finding | Resolution |
|---|---|---|
| 1 | **Unauthenticated public endpoint** writing to public repos under the operator's token; one `curl` published arbitrary text. Abuse absent from the risk list. | §4.1 HMAC; §4.2 rate limiting; abuse promoted to risk #1. |
| 2 | **Prompt injection into a credentialed agent** — the issue body became the mission prompt. | §6.6; later moot once dispatch was cut. |
| 3 | **"Zero new dispatch code" false in six ways** — hardcoded sandbox repo/`base_branch`/`test_cmd`; allowlist and `test_cmd` derived from a `STATUS.md` a feedback project lacks; write-back needs `projectDir`+`roadmapHash`; boot reconcile orphans; no dispatch UI. | Retracted; specified honestly in R2, **cut entirely** in R3, and its replacement justification withdrawn in R4 (§6.5). |
| 4 | **"No board change" false** — `SOURCE_LABEL`, `App.svelte`, `store.ts`, and a `model.ts` comment all change. | Retracted; §6.1 is a change-surface table (completed in R4). |
| 5 | **KV cannot implement the gate** — last-write-wins loses increments during the storm it exists for. | Durable Object in R2; gate **deleted** in R3. |
| 6 | **`public_repo` read-only is not a real scope.** | Two App installations in R2; **cockpit GitHub credential deleted** in R3 (§5.4). |
| 7 | **Limits and gate keyed on client-chosen `anon_id`.** | §4.2, refined in R3 for CGNAT. |
| 8 | **`count`/`project` had no workable read path.** | §5.2 labels; `origin:` later cut as redundant. |
| 9 | **Fingerprint contract inconsistent** — required-then-discarded; a hex rule would `400` Sentry's decimal ids. | §2.1/§4.4; simplified again once crashes left the Worker. |
| 10 | **P0 numbers didn't hold** — out-of-scope projects counted, games unchecked, extrapolation beyond the sample. | §3.1 rewritten to what was verified; P0 scoped to a two-project probe. |
| 11 | Scope to cut: `kind:"idea"`, HLL sketch, 30-day regression, `telltale:tier2`, unsatisfiable Halyard MUST. | All cut (§11.11). |
| 12 | **`health: 'unknown'` unachievable** — `replaceSource` drops the source's cards. | §6.4 states the real behaviour. |
| 13 | **"Does not increment counters" discarded the storm the counters existed for.** | Ordering fixed; moot (no counters). |
| 14 | Client IP observability unaddressed; digit-run redaction would destroy crash stack addresses. | §4.3. |

### Critique Round 2

A fresh critic on revision 2, tasked with what round 1 missed and what its fixes broke. Eighteen
findings; two were structural and forced a rewrite.

| # | Finding | Resolution |
|---|---|---|
| **2** | **The system reimplemented Sentry.** The DO, gate, dedup table, and count labels recomputed grouping and distinct-user counts Sentry supplies — and Sentry's *native GitHub integration* already opens and links one issue per group with zero code. | **The entire crash-side pipeline deleted** (§3, §11.1). Roughly halved the spec. |
| **18** | **Dispatch should not exist here.** It would widen the credentialed agent's push target from one sandbox to every registry repo, for a pipeline whose input is internet-authored text — a blast-radius risk never listed. | **Cut** (§6.5). |
| 1 | **The crash gate was unimplementable** — a Sentry webhook's only server identity is Sentry, so the `N` branch had cardinality 1. | Moot: gate deleted (§11.6). |
| 3 | **The DO serialized on a GitHub fetch** inside the critical section, on the one fingerprint a storm shares. | Moot: DO deleted. (Its removal reintroduced a concurrency window — see R3 #6.) |
| 4 | **Reopen-always was a trap** — `not_planned` closures reopen forever; mobile users on old builds perpetually reopen fixed issues. No rows for deletion or transfer. | §4.4: never auto-reopen; explicit rows. |
| 5 | **`telltale:split` didn't split** — re-salting still keyed on `normalize(title)`. | **Cut**; remedy is manual. |
| 6 | **HMAC destroyed §4.5's own rationale**, and `canonical_body` was undefined across five runtimes. | §4.1 signs **raw bytes** with a timestamp **header**. |
| 7 | **CGNAT breaks per-IP limiting** for the four mobile rows. | §4.2 IP+`anon_id` pair keying. |
| 8 | **No read path** — a `401` storm and "nobody filed a bug" were the same observation. | §4.6 `/v1/stats`. |
| 9 | **The dispatch brief had no transport**; §5.2's "no body parsing" justification was a strawman. | `body` added (§6.2); §5.2 corrected. |
| 10 | **`family` clustering claim false** — written by three adapters, read by nothing. | §6.4 states it is inert. |
| 11 | **Stage mapping abused the vocabulary** — `Build` outranked `Live`; `Blocked` never cleared; precedence unstated. | §6.4 rewritten. |
| 12 | **`staleAfterSec` is not a poll interval** — the real cadence is a shared 15s timer. | §6.3 (arithmetic corrected again in R4). |
| 13 | **The registry had three consumers and one deployment location.** | §5.1: checked in, Worker-bundled, cockpit reads none of it. |
| 14 | **The cockpit's installation token had no minting story** — it would need the App private key. | §5.4 (completed in R4 — see R3 #1). |
| 16 | **`POST /issues` has no idempotency key** — a create with a lost response double-opens, violating the *at most one open issue per fingerprint* invariant. (An earlier log entry mis-cited this as "locked decision #6"; that numbering was stale.) | §4.4: the `tt:` label is the idempotency key. |
| 17 | **`title` was never scrubbed** — the most visible field in the system. | §4.3 scrubs both; §4.4 fingerprints the scrubbed title. |
| 15 | DO state table missing lifetime count, throttle timestamp, retirement, repo, GC, cost model. | Moot: DO deleted. |
| — | `perennial-blade`/`heart-of-the-gods` are jam games, invisible to discovery, plausibly zero users. | Cut from §1. |

### Critique Round 3

A fresh critic on revision 3, tasked specifically with what the aggressive simplification **broke** —
deletion often severs load-bearing things. Fifteen findings; four were severe, and three of those were
defects the *simplification itself* introduced.

| # | Finding | Resolution |
|---|---|---|
| **1** | **`GET /v1/issues` had no authentication**, and could not have one without falsifying "the cockpit holds no credential." The Worker can read private registry repos, so an open read endpoint served every private bug-report body to anyone guessing the hostname. R2's deletion of the cockpit's GitHub credential had *relocated* the problem, and the log recorded it as a clean win. | §5.4: the cockpit holds a **Worker-scoped read token** (`TELLTALE_TOKEN`, beside the existing `HALYARD_BIN`/`AUDIENCE_API_URL` env vars), never shipped in a binary. The claim is narrowed to "no *GitHub* credential" in the header, locked #6, and §5.4. |
| **2** | **Project identity was asserted twice and never reconciled.** The HMAC header selected the secret; the body's `project` resolved the repo. Extract the weakest client's secret (`hexy`), sign a body naming `tenzy`, publish into the flagship's public tracker — and the stated response, rotating the attributable project's secret, protects the wrong project. A defect *introduced* by R2's header fix. | **`project` deleted from the body schema** (§2.1); the header is the sole authority (§4.1, locked #4). One fewer field in five senders. |
| **4** | **The simplification routed half the pipeline around every PII control.** §4.3's scrub applies only to the Worker; crash issues are authored by Sentry straight into the same public repos with zero scrubbing — and a Sentry payload is a *richer* PII channel (exception messages, breadcrumbs, URLs with tokens, `user.email`, stack locals). The private-repo escape hatch also doesn't reach them: that's N hand-edited alert rules, not a `registry.yml` edit. | §3 gains **step 2 (Sentry data scrubbing)** as a required configuration step; locked #5 and §4.3 scope the redirect claim to the bug half; §10.2 restated. |
| **3** | **"Phase 2 dispatches it with zero changes" was false in three ways** — Phase 2 has no UI and no POST path; of nine covered projects exactly one has both `docs/STATUS.md` and `ROADMAP.md`, and **`tenzy` has no `ROADMAP.md` at all**; `prima-tactica` and `hexy` are outside the scan root. The cut was right, the reason was not. | §6.5 rewritten: dispatch is **out of scope**, worked by hand. The "P4 dependency disappears" claim is withdrawn — it became an unmet *workflow* dependency. Added to §11.12. |
| **5** | **`telltale:triaged` was a magic string nothing created, applied, or tested** — yet the board's headline `blockedCount` depended on it clearing. Its own stated purpose was defeated by its mechanism. | §6.4: `Blocked` iff an open crash issue has **no assignee** — native, free in the list response, self-clearing, one click. Label cut (§11.7). |
| **6** | **"Solves create idempotency" overreached.** Deleting the DO removed the only serialization; the pre-create lookup is check-then-act, so simultaneous reports of the same title both create. The "multiple matches" row was framed as an anomaly rather than the design's own motivating scenario. The named test covers only the sequential retry. | §4.4 restated as **retry** idempotency, with a 30s KV marker as a narrowing and the residual listed as **risk §10.5**. §9.1 says plainly that the concurrent case is a risk, not a passing test. |
| **7** | **T5 should arguably be cut too** — `Idle` sorts at rank 99 (below `Archived`), so cards pile at the grid's bottom; ~10 cards inflate the header total and duplicate `local` cards; the whole delivered signal is a GitHub saved search. | **T5 retained** — a board presence is the operator's stated requirement, not an optimization. But the argument is recorded verbatim in **§6.7** with a two-line fallback, and §6.4 now emits a card **only for projects with open issues**, fixing the count inflation and grey-card clutter. |
| **8** | **Two silent GitHub behaviours the label scheme depends on:** `GET /issues` returns **pull requests** as issues (a labeled fix PR reads as an open bug); and `POST /issues` **silently drops `labels`** without push access — which would break the idempotency key, the dedup key, and the read key at once while reporting success. | §4.4: a `pull_request` filter row, and mandatory **label verification** in the create response emitting `labels_dropped` to `/v1/stats`. Both in §9.1. |
| **9** | **`TelltaleIssue` could not produce §6.4's strings** — no `createdIso` (so "new this week" was an `updated_at` question), no `state`, and a `telltale:*` prefix parse would yield `'muted'`. | §6.2: `createdIso`, `isOpen`, `hasAssignee`, and an explicit `kind` whitelist with an `'unknown'` fallback. |
| **10** | **The "verified change surface" table was itself incomplete** — missing `lib.rs`'s `generate_handler!` registration (without which the command doesn't exist at runtime), the `api.ts` binding (which §12 already named, contradicting §6.1 two sections apart), and the Worker base-URL/token config. `model.ts`'s file header still asserts the very overclaim §6.1 retracts. | §6.1 completed; the header-comment amendment added. |
| 11 | **§6.3's arithmetic was inherited from a design that no longer exists** — the cockpit polls the Worker, not GitHub, so the per-source interval was redundant while costing a structural `refresh()` change. And **partial repo failure** was unaddressed: one 403 on an archived repo would blank the entire feedback lane. | §6.3: interval dropped (§11.10); per-repo error isolation specified as required behaviour. |
| 12 | **`origin:{project}` was dead weight** (the repo *is* the project) and cost a hand-configured field in every Sentry alert rule; `tt:` label growth was unmentioned. | Label cut (§11.8); growth and the unbuilt janitor named in §4.4. |
| 13 | **The clock-skew mitigation was heavier than the problem** — a persisted offset in five runtimes that still failed on a device's first report. | §4.1: `401` + `X-Telltale-Server-Time`, re-signed by the retry that §8.3 already mandates. |
| 14 | **The grader wrote real issues into production trackers**, and tested the dedup path rather than the create path on every run after the first. | §5.1 adds a `__probe__` registry entry; §9.1's grader targets it and cleans up. |
| 15 | Smaller: no row for `state_reason: null`; §3 said "the registry repo," which does not exist; Sentry's label picker requires labels **pre-created per repo**; `pawsport` appeared in the registry but not the covered set. | All fixed in §4.4, §3 (steps 3–4), and §5.1. |

**Explicitly confirmed sound in round 3, do not re-litigate:** the `family`-is-inert claim; the
health-on-failed-poll description; the `Idle`-not-`Build` reasoning and its `sortedCards` citation;
the never-auto-reopen rule; the choice of the REST list endpoint with `labels=` over the search API
(not eventually consistent, not subject to the 30 req/min limit, AND-semantic, and `tt:` + 16 hex is
well inside the 50-char label limit); and §1's project verification, including `prima-tactica` as
Godot 4.6.
