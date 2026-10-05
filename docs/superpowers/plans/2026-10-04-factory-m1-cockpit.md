# Factory M1: the cockpit — Implementation Plan

> **For agentic workers:** steps use checkbox (`- [ ]`) syntax for tracking. This plan has three
> parts. **Part 1 ("wave 0")** is executed first, by one agent alone: it generates the API types,
> rewrites the API client over them, retires the hand-written types, and puts one mount point
> for each lane into the page. Its code is printed here in full. **Part 2** has one section per
> lane; each lane is executed by one agent, in its own git worktree, after Part 1 has merged,
> and ends with a pull request that the agent does not merge. A lane is given its tests in full
> and the code that is easy to get wrong; it writes the rest. **Part 3** is the order the lanes
> merge in and the operator's checklist.

> **Read this before Part 1.** The schema this plan generates from,
> `crates/fleet-api/contract/fleet-api.schema.json`, did not exist when the plan was written: it
> is committed by lane CC-API of the control-plane plan. The types and the client printed here
> were generated and tested against a schema produced in a scratch crate from that plan's
> `dto.rs`, copied verbatim, with `schemars` 1.2.2. **The first step of Part 1 is therefore to
> regenerate from the real schema and compare** (Task 1). If the real schema names a type this
> plan uses differently, or drops one, stop and report: do not edit a test to fit.

**Goal:** Make the cockpit work against the milestone-1 daemon: every request carries the
daemon's token, every API type is generated from the daemon's schema, the header shows which
harnesses exist and what each guarantees, a unit shows its seven stages, and a person can sign
a spec and dispatch it from the page.

**Architecture:** The Tauri shell mints one token per run, hands it to `fleetd` in its
environment and to the page through one command; the page keeps it in memory and sends it as a
bearer header and as a WebSocket subprotocol. One typed client (`src/lib/api.ts`), written
over types generated from `fleet-api`'s committed JSON Schema, is the only code that talks to
the daemon, and an in-memory fake of the same interface stands in for it in every test. The
shared store folds the daemon's event stream into per-unit state as it does today, and three
new pieces read that state or the client: the harness panel, the stage rail, and the sign and
dispatch panel.

**Tech Stack:** Svelte 5 (runes) and TypeScript 6 on Vite 8; vitest 3 with jsdom and
`@testing-library/svelte`; `json-schema-to-typescript` 16.0.0 (development only); Tauri
2.11.2 (Rust 2021) with `tauri-plugin-shell`, `reqwest` 0.12 and `getrandom` 0.3; Node 20 in
CI.

**Spec:** The ReqDrive factory design v0.4, in the private nexus repository: https://github.com/adbarc92/nexus/blob/main/docs/specs/2026-10-04-reqdrive-factory-design.md

The API this plan builds against is defined in
`docs/superpowers/plans/2026-10-04-factory-m1-control-plane.md` ("the control-plane plan"
below): Part 1, Task 8 for the types and routes, lane CC-API for their behaviour, lane CC-WIRE
for how the daemon reads its token. Where this plan and that one disagree about the API, that
one is right.

**Words used here.** The *page* is the Svelte app in the main webview. The *shell* is the
Tauri process that hosts it. The *daemon* is `fleetd`, started by the shell as a sidecar. A
*unit* is one piece of work run for one signed spec. A *harness* is the program the daemon
starts to run a unit's agents. The *store* is `FleetStore` in `src/lib/store.svelte.ts`. A
*stub* is a file wave 0 creates with its final props or signatures and a placeholder body,
which one lane then owns and fills.

## What is in milestone 1, and what is not

In: the token, end to end (lane UI-TOKEN, with lane CC-WIRE of the control-plane plan; closes
issue #91). Generated types, the drift check and the typed client (lane UI-TYPES, which is
wave 0). The harness panel in place of the Docker and API-key badges (lane UI-HARNESS). The
seven-stage rail, a minimal Sign surface, a minimal Dispatch form and the `reverify` command
(lane UI-RAIL).

Not in, and not to be built by any lane here: the gate panel, the evidence panel, the diff and
log views, the board's per-issue cards, queued-unit reasons, spec sets, the merge queue, the
metrics page, scope grants, restart, and drafting or editing a spec. Issue #92 (per-issue
cards with a Dispatch action) stays open: its Dispatch moves onto the board in a later
milestone. A lane that finds it needs one of these reports it and does not build it.

## Where the API differs from what a reader might assume

Read from the control-plane plan. Each of these shaped a decision below.

| The API | What this plan does about it |
|---|---|
| `UnitEventDto`'s `metric` carries `tokens_in`, `tokens_out`, `cost_usd` and `elapsed_ms`, cumulative for the unit, and nothing else. It names no stage, no adapter and no model | The rail attributes the difference between two metrics to the stage that was running. Adapter and model are shown when a metric carries them and are a dash otherwise, which in milestone 1 is always (lane UI-RAIL) |
| `EventEnvelopeDto` is `unit_id`, `seq`, `event`. No record carries a time | A stage's time is the metered agent time its metrics add up to, not wall-clock time. The rail says so in its caption |
| `UnitSummary.phase`, `UnitSummary.tier` and `Snapshot.phase` are strings; `UnitDetail.phase` is `PhaseDto` | `asPhase` in `src/lib/fleet.ts` narrows the string, with a compile-time check that it lists every phase |
| `HarnessView.state` is a string: `unprobed`, `healthy` or `unhealthy` | Compared as text; an unknown value is shown as it is |
| A refusal is an `ApiError` (`code`, `message`, `problems`) on the routes milestone 1 adds. The deprecated mission shape answers in plain text, a body that fits no shape is answered 422 by the framework, and the command route answers with a status and an empty body | `ApiFailure` carries the API's code when there is one and `http_<status>` with the body's text when there is not. `sendCommand` returns one of four outcomes |
| `/health` and `/swarms` stay in `fleetd` and are not in the schema | The page stops calling `/health`: there is no generated type for it and nothing left that shows it. The shell's own health check stays, with the token |
| No route lists repositories, or the specs in one | The Sign surface takes a repository and a spec id typed by a person |
| The token is any 32 or more printable ASCII characters, and is also sent as `fleetd.token.<token>` in `Sec-WebSocket-Protocol` | A subprotocol name allows fewer characters than that. The shell mints lowercase hex, and the page refuses a development token it could not send on a stream |
| The schema file is committed by lane CC-API, not by the control plane's wave 0 | This plan's wave 0 needs lane CC-API merged |

## Global Constraints

These are decisions already made. A lane follows them and does not reopen them.

**Repository and conduct**

- This repository is public. The text of the private design and of the doctrine stays out of
  it. Cite doctrine principles by id only (for example C1, C4, H2).
- Work only in the worktree your part or lane names. The checkout at
  `D:\MajorProjects\INFRASTRUCTURE\command-center` holds the owner's uncommitted work: make no
  edit, commit, stash or branch switch there. `git -C <that path> fetch`, pushing a new branch
  and `worktree add` are allowed, because they do not touch its working tree.
- Write only the files under your **Owns**. If you need a change anywhere else, ask the
  coordinator for it in your final report. No lane edits `cockpit/ui/package.json`,
  `cockpit/ui/package-lock.json`, `cockpit/ui/src-tauri/Cargo.toml`, its `Cargo.lock`,
  `cockpit/ui/src-tauri/tauri.conf.json`, `cockpit/ui/src-tauri/capabilities/**`,
  `.github/**`, `xtask/**`, a roadmap file, `CLAUDE.md` or `docs/STATUS.md`.
- After wave 0 has merged, no lane edits `src/App.svelte`, `src/lib/api.ts`,
  `src/lib/store.svelte.ts`, `src/lib/fleet.ts`, `src/lib/testkit/**` or
  `src/lib/generated/**`. A lane that cannot build a behaviour on those files as they are
  stops and reports; it does not change a signature or a prop.
- No commit message and no pull-request body has a `Co-Authored-By` line or a "Generated with"
  footer. Never force-push. Never push to `main`. Never merge your own pull request.
- Spend nothing and reach nothing live: no model call, no deploy, no real API key. The only
  network use is `npm ci` and `cargo` fetching what the lock files already name, pushing your
  branch, and opening your pull request.
- Test first (C1): each behaviour has a failing test, run and seen to fail for the reason this
  plan states, before the code that passes it.
- Before claiming a lane is done, run every command under its **Verify** and report the real
  output, failures included. Before parsing a tool's output, check that it is not empty.
- The build host is Windows 11 with PowerShell 7 and Git-Bash. A command block marked
  *Git-Bash* uses POSIX syntax. Unmarked `git`, `npm`, `npx`, `cargo` and `gh` commands work in
  either shell. Create and edit files with your file tool, never by typing a here-document
  into a command line: quoting differs between the shells and mangles backslashes.

**Dependencies**

- No new runtime dependency without a one-line justification in **Requests to the
  coordinator**. This plan adds none to the page: `json-schema-to-typescript` is a development
  dependency and nothing in `src/` imports it. It adds one to the shell, `getrandom`, which is
  already in that crate's lock file.
- No external CDN, script, stylesheet or font. Nothing this plan adds fetches anything from
  outside the daemon's loopback address.

**The token**

- The token exists in three places and no others: the shell's memory, the daemon's
  environment, and the page's memory. It is never written to a log, a file, `localStorage`,
  `sessionStorage`, a cookie, a URL, the DOM, the console or an error message.
- The page sends it as `Authorization: Bearer <token>` on HTTP requests and as the WebSocket
  subprotocol `fleetd.token.<token>`, offered beside `fleetd.v1`. Never as a query parameter.
- Without a token the page makes no request to the daemon.

**The page**

- Every wire type is imported from `src/lib/generated/fleet-api.ts`. No file under `src/`
  declares an interface or a union for something the daemon sends or receives. A type built
  from generated ones (`Pick`, `Required`, an index into a union) is fine.
- Every request to the daemon goes through a `FleetApi`. A component takes the client as a
  prop or uses the store; it does not call `fetch` or construct a `WebSocket`.
- Components are Svelte 5: `$props()`, `$state`, `$derived`, `$effect`, and `onclick`
  attributes. No `export let`, no `$:`, no `on:click`, no stores from `svelte/store`.
- A component's `<style>` is scoped. A class defined in `App.svelte` does not reach a child
  component. New components state the few rules they need, using the custom properties
  `src/app.css` defines (`--panel`, `--text`, `--cyan`, `--amber`, `--red`, `--green`,
  `--line-2` and the rest) and its two global classes, `mono` and `disp`.
- The layout stays dense and built for situational awareness, like the existing Fleet view
  (H2): small type, uppercase labels, no decorative space, nothing that needs a scroll to see
  whether a person is needed.
- Baseline accessibility for every new control: it is a real `button`, `input` or `select`; it
  is reachable and operable from the keyboard; it has a visible label or an `aria-label`; its
  state is never carried by colour alone; and its text meets a contrast of 4.5 to 1 against
  its background. Measured against `--panel`, `--panel-2` and `--bg`: `--text`, `--text-dim`,
  `--cyan`, `--amber`, `--green` and `--red` all meet it. **`--text-faint` does not** (about
  2.7 to 1 on `--panel`), so new text does not use it; and `--text-dim` on `--elev` falls just
  short (4.47 to 1), so text on an `--elev` background uses `--text`.
- A daemon's refusal is shown in the daemon's words: `ApiFailure.message`, unedited, with its
  `problems` when it has any. The page does not paraphrase a refusal or guess at its cause.

**Tests**

- The idiom is the repository's: vitest with `describe`/`it`/`expect`, `render`, `fireEvent`
  and `waitFor` from `@testing-library/svelte`, elements found by `data-testid`. A test of a
  `.svelte.ts` rune module is named `*.svelte.test.ts`.
- A test that needs the daemon uses `createFakeApi` from `src/lib/testkit/fakeApi.ts`, never a
  `vi.mock` of `./api`. A test of the store builds its own:
  `new FleetStore(fake, async () => TEST_TOKEN)`.
- A test listed in this plan is the contract for its behaviour. It may be added to; it is not
  weakened. If a lane finds a listed test that contradicts the control-plane plan, that plan
  wins: stop and report, do not edit the test to pass.
- The shell's tests need no window, no daemon and no network: they are unit tests of pure
  functions and scans of the crate's own source.

## Requests to the coordinator

The coordinating lane owns the manifests, the lock files, CI and `tauri.conf.json`. Part 1
assumes requests C1, C2 and C4 are already on `factory/m1`; its Task 1 checks them and stops if
one is missing. Lane UI-TOKEN assumes C3.

| # | What | Why |
|---|---|---|
| C1 | `cockpit/ui/package.json` `devDependencies` gains `"json-schema-to-typescript": "16.0.0"`, pinned exactly, with `package-lock.json` updated by `npm install --save-dev --save-exact json-schema-to-typescript@16.0.0`. The lock file gains ten development packages: that one, `@apidevtools/json-schema-ref-parser` 11.9.3, `@jsdevtools/ono` 7.1.3, `@types/json-schema` 7.0.15, `@types/lodash` 4.17.25, `lodash` 4.18.1, `minimist` 1.2.8, `prettier` 3.9.9, and two nested `js-yaml` | The type generator. It reads the draft 2020-12 `$defs` that `schemars` writes as they are and turns its `oneOf`-with-`const` enums into discriminated unions, where `openapi-typescript` 7 would need the schema wrapped in an OpenAPI document that `fleet-api` does not export. Pinned exactly, with `prettier` held by the lock file, because the drift check compares the generator's output byte for byte |
| C2 | `cockpit/ui/package.json` `scripts`: add `"gen:types": "node scripts/gen-types.mjs"` and `"check:types": "node scripts/gen-types.mjs --check"`, and change `check` to `"npm run check:types && svelte-check --tsconfig ./tsconfig.app.json && tsc -p tsconfig.node.json"` | The drift check runs first in `npm run check`, which is what the CI job `svelte-check + tsc` runs. So the existing type-check job fails when the generated file and the schema disagree, and the workflow file does not change |
| C3 | `cockpit/ui/src-tauri/Cargo.toml` `[dependencies]` gains `getrandom = "0.3"`, with the crate's `Cargo.lock` updated | The operating system's random source, for minting the token. `getrandom` 0.3.4 is already in that lock file through other crates, so the lock gains one line (an edge from `app`) and no package |
| C4 | Lane CC-API of the control-plane plan is merged into `factory/m1` before this plan's Part 1 starts | It commits `crates/fleet-api/contract/fleet-api.schema.json`, which Part 1 generates from |
| C5 | This plan's lane UI-TOKEN and lane CC-WIRE of the control-plane plan land together (step 4 of that plan's merge order) | The page and the daemon work together only when both are in. Once CC-WIRE is in, the daemon refuses every request without the token. Before it is in, the daemon's cross-origin rules do not list `Authorization`, so the webview will not send a request that carries one |

**Nothing is asked of CI.** The job names of the milestone-0 scaffold plan stay as they are:
`svelte-check + tsc` runs `npm run check` (types, and now drift), `vitest (cockpit/ui)` runs
the suite, `cargo test (cockpit)` runs the shell's tests, and `cargo xtask test static` lints
the shell's crate. If the coordinator also wants the drift check under `cargo xtask test
static`, that is one more command in `xtask` (`npm run check:types` in `cockpit/ui`), and is
the coordinator's to add or not.

**Nothing is asked of `tauri.conf.json`.** The content-security policy already allows the
page to reach `http://127.0.0.1:8787` and `ws://127.0.0.1:8787`, and offering subprotocols
does not change what a stream connects to. A test in lane UI-TOKEN pins the policy to the
client's default address, so the two cannot drift apart unnoticed.

## Review Focus

The five failure modes most likely to reach a person, each of which the tests a plan like this
would otherwise list would not exercise. Each has a test in the lane that owns it; the
reviewing session reads those tests first.

| # | What goes wrong | Why ordinary tests miss it | The test, and its lane |
|---|---|---|---|
| 1 | The token contains a character that is legal in an HTTP header and illegal in a WebSocket subprotocol name (the `/` and `=` of base64, or a `:`). Every HTTP request works and every stream throws, so the fleet lists and never updates | A fake WebSocket accepts any string | `every token it accepts can be offered as a subprotocol` in `src/lib/auth.test.ts`, which offers the token to jsdom's real `WebSocket`; and `a_minted_token_is_64_lowercase_hex_characters` in `token.rs`. Lane UI-TOKEN |
| 2 | A development token is compiled into a release bundle, or the token is written somewhere that outlives the page | The value only exists at build time; a unit test of `loadToken` sees whatever it stubs | `is ignored in a production build, even when the variable is set`, `is in memory only…` and `the module names no storage, cookie or console API at all` in `src/lib/auth.test.ts`; and the canary build under lane UI-TOKEN's **Verify**, which greps the built bundle. Lane UI-TOKEN |
| 3 | The daemon's address in the client and the address the content-security policy allows drift apart. In the packaged app every request and stream then fails, with nothing in a log | jsdom enforces no policy | `src/lib/csp.test.ts`, all four tests. Lane UI-TOKEN |
| 4 | A stream closes (the sidecar restarted, or the daemon closed it after the replay because no run holds the unit) and is never reopened, so the page shows a fleet that stopped changing; or it is reopened in a tight loop | Every existing test mocked the client away, so no test ever saw a socket close (`docs/testing/PLAN.md` GAP-015, GAP-078, GAP-079) | `reopens a dropped stream from the last record folded, not from the start`, `waits longer each time a reopened stream brings nothing, up to a cap`, `leaves a finished unit alone once its log has been read to the end` and `dispose closes every stream and cancels every retry, and start connects again` in `src/lib/store.connection.svelte.test.ts`. Wave 0 |
| 5 | A person signs bytes they did not see: the page shows text that is not what the daemon hashed (the file is not UTF-8, or something between the daemon and the screen changed it), and sends back the hash the daemon supplied | A fixture's text and hash always agree unless a test makes them disagree on purpose | `will not sign when the text shown is not the bytes the daemon hashed` and `sends the commit that was read and the SHA-256 of the bytes shown` in `src/lib/unit/FactoryPanel.test.ts`. Lane UI-RAIL |

Three more that are listed as ordinary behaviours, and deserve the same eye: the token being
readable from a webview that is not the page (`only_the_main_webview_may_read_the_token`, lane
UI-TOKEN); a stage's cost being counted twice because metrics are cumulative (`the stages
together account for exactly the unit total`, lane UI-RAIL); and a second unit being started by
a retry after an unclear failure (`shows a refusal's reason verbatim and keeps the same key for
the next press`, lane UI-RAIL).

## How to read this plan

- A path with no directory prefix is under `cockpit/ui/`. So `src/lib/api.ts` is
  `cockpit/ui/src/lib/api.ts`, and `src-tauri/src/token.rs` is
  `cockpit/ui/src-tauri/src/token.rs`.
- A code block headed by a path is that file in full. A block marked `diff` is the change to an
  existing file, against `main` as it was when this plan was written; apply it by hand with
  your file tool and check the result, do not pipe it to `patch`.
- `npm` and `npx` commands run in `cockpit/ui`. `cargo` commands for the shell run in
  `cockpit/ui/src-tauri`.
- The shell's crate does not compile until the sidecar binary exists. Before the first
  `cargo` command in `cockpit/ui/src-tauri`, run `npm ci` and `npm run sidecar` in
  `cockpit/ui`, as CI does.

---

# Part 1 — Wave 0: generated types, the typed client, and the seams

One agent executes this part alone, before any lane starts. It is lane **UI-TYPES** in full,
plus what every other lane compiles against.

**Owns:** `scripts/gen-types.mjs`; `src/lib/generated/**`; `src/lib/api.ts` and
`src/lib/api.test.ts`; `src/lib/testkit/**`; `src/lib/fleet.ts`; `src/lib/store.svelte.ts` and
the four `src/lib/store.*.test.ts` files; `src/App.svelte` and `src/App.connection.test.ts`;
the deletion of `src/lib/types.ts`; and, for this part only, the one-line import changes in
`src/lib/bridge.ts`, `src/lib/bridge.test.ts`, `src/lib/dashboard/store.ts`,
`src/lib/dashboard/store.test.ts`, `src/lib/dashboard/adapters/fleet.ts`,
`src/lib/dashboard/adapters/adapters.test.ts`, `src/views/Dashboard.svelte`,
`src/App.overlay.test.ts` and `src/App.appPlugin.test.ts`. It creates seven stub files that
lanes then own: `src/lib/auth.ts`, `src/lib/NotConnected.svelte`,
`src/lib/harness/HarnessBadge.svelte`, `src/lib/unit/rail.ts`, `src/lib/unit/StageRail.svelte`,
`src/lib/unit/ReverifyButton.svelte` and `src/lib/unit/FactoryPanel.svelte`. No manifest, no
lock file, nothing under `src-tauri/`.

**Reads:** this plan; the control-plane plan, Part 1 Task 8 (`dto.rs`, the route table) and
lane CC-API (the status codes); `docs/testing/PLAN.md` entries GAP-015, GAP-078 and GAP-079;
the files it changes.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-ui-types`, branch `feat/m1-ui-types`,
cut from `origin/factory/m1`. Pull request: `feat/m1-ui-types` into `factory/m1`.

**Needs:** requests C1, C2 and C4 on `factory/m1`.

**Blocks:** every lane of Part 2.

**What wave 0 is.** Four things, all complete:

1. *The generated types and the drift check.* A script turns the committed schema into
   `src/lib/generated/fleet-api.ts`; the same script, with `--check`, fails when the two
   disagree.
2. *The client.* `src/lib/api.ts`: the `FleetApi` interface, the real client over `fetch` and
   `WebSocket`, and `ApiFailure`. `src/lib/testkit/fakeApi.ts`: an in-memory `FleetApi`.
3. *The store on the client.* `FleetStore` takes a `FleetApi` and a token loader, knows whether
   it is connected, makes no request without a token, reopens a stream that closes, and can
   dispatch a signed spec. `src/lib/types.ts` is gone.
4. *The seams.* `App.svelte` mounts one component per lane, and each of those exists as a stub
   with its final props. After this part, no lane touches `App.svelte`.

**What a person sees after wave 0 merges and before lane UI-TOKEN does.** The desktop app
shows the plain "not connected" line, because the stub `auth.ts` has no token to give. That is
expected on the integration branch, and it stays that way until lane UI-TOKEN and the daemon's
lane CC-WIRE are both in (request C5).

**Verify** (run in `cockpit/ui` in the worktree after Task 7):

| Command | Expected |
|---|---|
| `npm ci` | exit 0 |
| `npm run check:types` | `generated types are up to date`, exit 0 |
| `npm run check` | the line above, then `svelte-check` reporting 0 errors and 0 warnings, then `tsc` with no output; exit 0 |
| `npx vitest run` | `Test Files  27 passed (27)`, `Tests  236 passed (236)`: the 166 tests `main` had, and 70 more in `api.test.ts` (39), `testkit/fakeApi.test.ts` (11), `store.connection.svelte.test.ts` (16) and `App.connection.test.ts` (4) |
| `npx vite build` | exit 0 |
| *Git-Bash:* `grep -rn "from '.*/types'" src --include=*.ts --include=*.svelte \| grep -v "dashboard/model"` | no output: nothing imports the deleted file |
| *Git-Bash:* `grep -rnE "fetch\(\|new WebSocket\(\|new Socket\(" src --include=*.ts --include=*.svelte \| grep -v "\.test\.ts" \| grep -v "src/lib/api.ts"` | no output: only the client talks to the daemon |
| *Git-Bash:* `test ! -e src/lib/types.ts && echo gone` | `gone` |
| `npm run sidecar`, then `cargo test` in `cockpit/ui/src-tauri` | passes, unchanged by this part: 35 library tests, 3 and 2 in the two guard suites |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

The shell's crate is not touched by this part; its tests are run to show that.

### Task 1: The worktree, the coordinator's requests, and the real schema

**Files:** none yet.

- [ ] **Step 1: Create the worktree**

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b feat/m1-ui-types D:/MajorProjects/.swarm-wt/m1-ui-types origin/factory/m1
```

- [ ] **Step 2: Check the requests are in**

In `D:\MajorProjects\.swarm-wt\m1-ui-types`:

*Git-Bash:*

```
test -f crates/fleet-api/contract/fleet-api.schema.json && echo "C4 schema: present"
grep -c '"json-schema-to-typescript": "16.0.0"' cockpit/ui/package.json
grep -c '"gen:types": "node scripts/gen-types.mjs"' cockpit/ui/package.json
grep -c '"check": "npm run check:types && svelte-check' cockpit/ui/package.json
```

Expected: `C4 schema: present`, then `1`, `1`, `1`. If any is missing, stop and report which:
this part does not edit `package.json` and does not write the schema.

- [ ] **Step 3: Record the starting point**

In `cockpit/ui`:

```
npm ci
npx vitest run
```

Expected: `Tests  166 passed (166)` in 23 files. Write the two numbers down; the Verify table
is stated relative to them. (`npm run check` fails at this point, by design: `check:types`
has no script file to run yet. That is the red state of Task 2.)

- [ ] **Step 4: Look at the real schema before generating from it**

*Git-Bash, in the worktree root:*

```
node -e "const s=require('./crates/fleet-api/contract/fleet-api.schema.json'); console.log(s['\$schema']); console.log(Object.keys(s['\$defs']).sort().join(' '))"
```

Expected: `https://json-schema.org/draft/2020-12/schema`, then exactly these 32 names:

```
ApiError Capabilities CommandDto ControlKind Delivery EventEnvelopeDto GateKind HarnessInfo HarnessView Isolation LegacyMissionRequest Metering MissionBody MissionRequest MissionResponse Network PhaseDto PresetInfo ProblemView ProfileInfo SignRequest SignResponse SignatureView Snapshot SpecView Stage StageStatus TierDto UnitDetail UnitEventDto UnitKind UnitSummary
```

A name missing from the real schema, or spelt differently, is a finding: stop and report it
with the list you got. An extra name is fine. Everything below was built against the list
above.

### Task 2: The generator and the drift check

**Files:**
- Create: `scripts/gen-types.mjs`
- Create (generated): `src/lib/generated/fleet-api.ts`

**Produces:** `npm run gen:types`, `npm run check:types`, and the 33 exported types every
other file imports.

What to know:

- The schema is draft 2020-12 with every type under `$defs`. `schemars` writes a plain enum as
  `{"enum": [...]}`, an internally tagged enum as `oneOf` whose members each have a `const`
  discriminator, an `Option<T>` as `anyOf` of the type and `null` (or `"type": ["string",
  "null"]`), and leaves `additionalProperties` out unless the Rust type denies unknown fields.
- Without `additionalProperties: false` in the generator's options every interface would gain
  `[k: string]: unknown`, and a misspelt field name would compile.
- The check compares after turning CRLF into LF, so a Windows checkout with line-ending
  conversion does not report drift that is not there.
- The output's formatting comes from `prettier`, which the generator depends on. The lock file
  holds its version. Run the script only after `npm ci`, never after a bare `npm install`.

- [ ] **Step 1: See the check fail**

```
npm run check:types
```

Expected: node cannot find the script: `Cannot find module '…\scripts\gen-types.mjs'`, exit 1.

- [ ] **Step 2: Create `scripts/gen-types.mjs`**

````js
// Generate the cockpit's API types from the JSON Schema that `fleet-api` commits.
//
//   node scripts/gen-types.mjs           write src/lib/generated/fleet-api.ts
//   node scripts/gen-types.mjs --check   exit 1 if that file is not what would be written
//
// Run from cockpit/ui (as `npm run gen:types` and `npm run check:types` do). The schema is the
// contract: nothing in src/ declares an API type by hand.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compile } from 'json-schema-to-typescript';

const here = dirname(fileURLToPath(import.meta.url));
const SCHEMA = resolve(here, '../../../crates/fleet-api/contract/fleet-api.schema.json');
const OUT = resolve(here, '../src/lib/generated/fleet-api.ts');

const BANNER = `// GENERATED FILE. Do not edit.
//
// Source: crates/fleet-api/contract/fleet-api.schema.json
// Regenerate: npm run gen:types   (in cockpit/ui)
// The check \`npm run check:types\` fails when this file and the schema disagree.`;

/** Line endings as git stores them, so a checkout with CRLF conversion still compares equal. */
const lf = (text) => text.replace(/\r\n/g, '\n');

async function generate() {
  if (!existsSync(SCHEMA)) {
    throw new Error(
      `${SCHEMA} does not exist. It is committed by the fleet-api crate; ` +
        'this script never writes it.',
    );
  }
  const schema = JSON.parse(readFileSync(SCHEMA, 'utf8'));
  if (!schema.$defs || typeof schema.$defs !== 'object') {
    throw new Error('the schema has no $defs: this is not the fleet-api contract');
  }
  const body = await compile(schema, schema.title ?? 'ApiSchema', {
    // schemars leaves `additionalProperties` out unless a type denies unknown fields. Without
    // this, every interface would gain an index signature and stop catching misspelt keys.
    additionalProperties: false,
    // Emit every $defs entry, including ones only reachable through another definition.
    unreachableDefinitions: true,
    bannerComment: BANNER,
    format: true,
    style: { singleQuote: true, printWidth: 100 },
  });
  return lf(body);
}

const check = process.argv.includes('--check');
try {
  const wanted = await generate();
  if (!check) {
    mkdirSync(dirname(OUT), { recursive: true });
    writeFileSync(OUT, wanted);
    console.log(`wrote ${OUT}`);
  } else {
    const have = existsSync(OUT) ? lf(readFileSync(OUT, 'utf8')) : null;
    if (have === wanted) {
      console.log('generated types are up to date');
    } else {
      console.error(
        have === null
          ? `${OUT} is missing.`
          : 'src/lib/generated/fleet-api.ts is not what the committed schema generates.',
      );
      console.error('Run `npm run gen:types` in cockpit/ui and commit the result.');
      process.exit(1);
    }
  }
} catch (error) {
  console.error(`gen-types: ${error.message}`);
  process.exit(1);
}
````

- [ ] **Step 3: See the check fail for the right reason, then generate**

```
npm run check:types
```

Expected: a line ending `src\lib\generated\fleet-api.ts is missing.`, then a line telling you
to run `npm run gen:types` and commit the result; exit 1.

```
npm run gen:types
npm run check:types
```

Expected: `wrote …\src\lib\generated\fleet-api.ts`, then `generated types are up to date`.

- [ ] **Step 4: Check what was generated**

*Git-Bash:*

```
grep "^export " src/lib/generated/fleet-api.ts | sed 's/ =.*//; s/ {.*//' | LC_ALL=C sort
```

Expected, exactly these 33 lines (the 32 definitions and the root):

````
export interface ApiError
export interface ApiSchema
export interface Capabilities
export interface EventEnvelopeDto
export interface HarnessInfo
export interface HarnessView
export interface LegacyMissionRequest
export interface MissionRequest
export interface MissionResponse
export interface PresetInfo
export interface ProblemView
export interface ProfileInfo
export interface SignRequest
export interface SignResponse
export interface SignatureView
export interface Snapshot
export interface SpecView
export interface UnitDetail
export interface UnitSummary
export type CommandDto
export type ControlKind
export type Delivery
export type GateKind
export type Isolation
export type Metering
export type MissionBody
export type Network
export type PhaseDto
export type Stage
export type StageStatus
export type TierDto
export type UnitEventDto
export type UnitKind
````

Then open the file and check four shapes, because the client and the lanes rely on them:

| In the generated file | Must read |
|---|---|
| `UnitEventDto` | a union of eleven object types, each with a literal `type`: `'phase_changed'`, `'oracle_proposed'`, `'iteration'`, `'log'`, `'metric'`, `'finding'`, `'artifact'`, `'blocked'`, `'error'`, `'done'`, `'stage'` |
| `CommandDto` | a union of seven object types, each with `cmd_id: string` and a literal `command`: `'halt'`, `'resume'`, `'abandon'`, `'ship'`, `'approve_oracle'`, `'reject_oracle'`, `'reverify'` |
| `MissionBody` | `MissionRequest \| LegacyMissionRequest` |
| `ApiError` | `code: string; message: string; problems?: ProblemView[]` with no index signature |

If a shape differs, stop and report. Do not edit the generated file.

- [ ] **Step 5: Prove the check catches drift**

Append one line to `src/lib/generated/fleet-api.ts` with your file tool (`// drift`), then:

```
npm run check:types
```

Expected: `src/lib/generated/fleet-api.ts is not what the committed schema generates.`, exit 1.
Then `npm run gen:types` to restore it, and `npm run check:types` once more: up to date.

- [ ] **Step 6: Commit**

```
git add cockpit/ui/scripts/gen-types.mjs cockpit/ui/src/lib/generated/fleet-api.ts
git commit -m "feat(cockpit): generate the API types from fleet-api's schema, with a drift check"
```

### Task 3: The typed client

**Files:**
- Create: `src/lib/auth.ts` (a stub; lane UI-TOKEN owns it afterwards)
- Create: `src/lib/api.test.ts`
- Replace: `src/lib/api.ts`

**Produces:** `FleetApi`, `createFleetApi`, `api`, `ApiFailure`, `FLEET_API_METHODS`,
`newCommand`, `CommandName`, `CommandOutcome`, `CreateReq`, `StreamHandlers`, `StreamHandle`,
`FleetApiOptions`, `DEFAULT_FLEET_URL`, `FLEET_BASE`, `WS_PROTOCOL`, `WS_TOKEN_PREFIX`; and
from `auth.ts`: `currentToken`, `clearToken`, `loadToken`.

What to know:

- `FleetApi` has eight methods and no more: the eight milestone 1's surfaces call. `GET
  /details`, `GET /units/{id}`, `GET /units/{id}/detail`, `GET /units/{id}/events` and `GET
  /schema` have generated types and no method; a later milestone adds a method when a view
  needs one, and adds it to `FLEET_API_METHODS` and to the fake in the same change.
- The client asks for the token on every call (`options.token()`), so a token loaded after the
  client was built is used. With no token it throws `ApiFailure` with code `no_token` and makes
  no request.
- A failed `fetch` is rethrown as `ApiFailure` with code `network` and **without** the
  original error as its cause: a fetch error's text can quote the request.
- `openStream` does not reconnect. It reports a close once through `onClose`, and does not
  report a close the caller asked for. The store decides whether to reopen (Task 5).
- The old `api.ts` exported five functions (`createMission`, `sendCommand`, `listUnits`,
  `health`, `openStream`) that every test mocked. They are gone: there is one `api` object, and
  tests use the fake.

- [ ] **Step 1: Create the stub `src/lib/auth.ts`**

````ts
// Where the cockpit's page gets the daemon's token.
//
// WAVE 0 STUB. Lane UI-TOKEN replaces the body of `loadToken` and adds what it needs below it.
// The three signatures here do not change. Until then there is no token, and the page shows
// its "not connected" state.
//
// The token is held in this module's memory only. It is never written to localStorage,
// sessionStorage, a cookie, the URL, the DOM or the console.

let token: string | null = null;

/** The token, if one has been loaded. Synchronous: the API client calls it on every request. */
export function currentToken(): string | null {
  return token;
}

/** Forget the token. The next `loadToken` asks for it again. */
export function clearToken(): void {
  token = null;
}

/**
 * Resolve the token from wherever this build gets it, remember it, and return it.
 * Resolves to null when there is none. Never rejects.
 */
export async function loadToken(): Promise<string | null> {
  return token;
}
````

- [ ] **Step 2: Write the failing test**

Create `src/lib/api.test.ts`:

````ts
import { describe, it, expect, vi } from 'vitest';
import {
  ApiFailure,
  createFleetApi,
  newCommand,
  WS_PROTOCOL,
  WS_TOKEN_PREFIX,
  type FleetApi,
} from './api';
import type { EventEnvelopeDto } from './generated/fleet-api';

// The real client against a stubbed `fetch` and a stubbed `WebSocket`: no daemon, no network.
// (docs/testing/PLAN.md GAP-078: until this file, nothing executed api.ts at all.)

const TOKEN = 'test-token-0123456789abcdef0123456789abcdef';
const BASE = 'http://127.0.0.1:8787';

type FetchArgs = { url: string; init: RequestInit };

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

/** A client whose `fetch` records each call and answers with `answer`. */
function client(answer: () => Response | Promise<Response>, token: string | null = TOKEN) {
  const seen: FetchArgs[] = [];
  const fetchStub = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    seen.push({ url: String(input), init: init ?? {} });
    return answer();
  });
  const api = createFleetApi({
    baseUrl: BASE,
    token: () => token,
    fetch: fetchStub as unknown as typeof fetch,
  });
  return { api, seen, fetchStub };
}

function header(args: FetchArgs, name: string): string | undefined {
  return (args.init.headers as Record<string, string> | undefined)?.[name];
}

/** A stand-in for the browser's WebSocket that records how it was constructed. */
class SocketStub {
  static made: SocketStub[] = [];
  onmessage: ((message: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;
  readonly url: string;
  readonly protocols: string[];
  constructor(url: string, protocols?: string | string[]) {
    this.url = url;
    this.protocols = protocols === undefined ? [] : ([] as string[]).concat(protocols);
    SocketStub.made.push(this);
  }
  close(): void {
    this.closed = true;
  }
}

function streamClient(token: string | null = TOKEN): FleetApi {
  SocketStub.made = [];
  return createFleetApi({
    baseUrl: BASE,
    token: () => token,
    fetch: (() => {
      throw new Error('no fetch in a stream test');
    }) as unknown as typeof fetch,
    WebSocket: SocketStub as unknown as typeof WebSocket,
  });
}

/** Every HTTP method of the client, with a call that exercises it. */
const HTTP_CALLS: [string, (api: FleetApi) => Promise<unknown>][] = [
  ['listUnits', (api) => api.listUnits()],
  ['createMission', (api) => api.createMission({ task: 'x' })],
  ['sendCommand', (api) => api.sendCommand('u1', newCommand('halt'))],
  ['listHarnesses', (api) => api.listHarnesses()],
  ['getSpec', (api) => api.getSpec({ repo: 'o/r', spec_id: 'SPEC-1' })],
  ['sign', (api) => api.sign({ repo: 'o/r', spec_id: 'SPEC-1', commit: 'c', sha256: 'h' })],
  ['listSignatures', (api) => api.listSignatures('o/r')],
];

describe('the token', () => {
  it.each(HTTP_CALLS)('%s sends the token as a bearer header', async (_name, call) => {
    const { api, seen } = client(() => jsonResponse(202, []));
    await call(api);
    expect(seen).toHaveLength(1);
    expect(header(seen[0], 'authorization')).toBe(`Bearer ${TOKEN}`);
    expect(seen[0].url).not.toContain(TOKEN);
  });

  it.each(HTTP_CALLS)('%s makes no request without a token', async (_name, call) => {
    const { api, fetchStub } = client(() => jsonResponse(200, []), null);
    const failure = await call(api).catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect((failure as ApiFailure).code).toBe('no_token');
    expect((failure as ApiFailure).status).toBe(0);
    expect(fetchStub).not.toHaveBeenCalled();
  });

  it('asks for the token on every request, so one loaded later is used', async () => {
    let token: string | null = null;
    const fetchStub = vi.fn(async () => jsonResponse(200, []));
    const api = createFleetApi({
      baseUrl: BASE,
      token: () => token,
      fetch: fetchStub as unknown as typeof fetch,
    });
    await expect(api.listUnits()).rejects.toMatchObject({ code: 'no_token' });
    token = TOKEN;
    await expect(api.listUnits()).resolves.toEqual([]);
    expect(fetchStub).toHaveBeenCalledTimes(1);
  });
});

describe('failures', () => {
  it('turns the API error shape into an ApiFailure with its code, message and problems', async () => {
    const { api } = client(() =>
      jsonResponse(422, {
        code: 'spec_not_ready',
        message: 'the spec is not ready to sign',
        problems: [{ code: 'open_questions', message: 'one open question remains' }],
      }),
    );
    const failure = (await api
      .sign({ repo: 'o/r', spec_id: 'SPEC-1', commit: 'c', sha256: 'h' })
      .catch((e: unknown) => e)) as ApiFailure;
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure.status).toBe(422);
    expect(failure.code).toBe('spec_not_ready');
    expect(failure.message).toBe('the spec is not ready to sign');
    expect(failure.problems).toEqual([
      { code: 'open_questions', message: 'one open question remains' },
    ]);
  });

  it('an error body without problems has an empty list, not undefined', async () => {
    const { api } = client(() =>
      jsonResponse(409, { code: 'duplicate_key', message: 'this key already started unit u1' }),
    );
    const failure = (await api
      .createMission({ task: 'x' })
      .catch((e: unknown) => e)) as ApiFailure;
    expect(failure.code).toBe('duplicate_key');
    expect(failure.problems).toEqual([]);
  });

  it('a plain-text refusal keeps its text and gets the code http_<status>', async () => {
    const { api } = client(() => new Response('unknown mode: sideways', { status: 400 }));
    const failure = (await api
      .createMission({ task: 'x', mode: 'sideways' })
      .catch((e: unknown) => e)) as ApiFailure;
    expect(failure.status).toBe(400);
    expect(failure.code).toBe('http_400');
    expect(failure.message).toBe('unknown mode: sideways');
  });

  it('an empty error body still says which status came back', async () => {
    const { api } = client(() => new Response('', { status: 503 }));
    const failure = (await api.listUnits().catch((e: unknown) => e)) as ApiFailure;
    expect(failure.code).toBe('http_503');
    expect(failure.message).toContain('503');
  });

  it('JSON that is not the error shape is not mistaken for one', async () => {
    const { api } = client(() => jsonResponse(500, { error: 'boom' }));
    const failure = (await api.listUnits().catch((e: unknown) => e)) as ApiFailure;
    expect(failure.code).toBe('http_500');
  });

  it('a request that gets no answer is a network failure that never quotes the request', async () => {
    const fetchStub = vi.fn(async () => {
      throw new TypeError(`Failed to fetch with Bearer ${TOKEN}`);
    });
    const api = createFleetApi({
      baseUrl: BASE,
      token: () => TOKEN,
      fetch: fetchStub as unknown as typeof fetch,
    });
    const failure = (await api.listUnits().catch((e: unknown) => e)) as ApiFailure;
    expect(failure.code).toBe('network');
    expect(failure.status).toBe(0);
    expect(failure.message).not.toContain(TOKEN);
    expect(String(failure.stack ?? '')).not.toContain(TOKEN);
    expect((failure as { cause?: unknown }).cause).toBeUndefined();
  });

  it('a 401 is the API error shape with code unauthorized', async () => {
    const { api } = client(() =>
      jsonResponse(401, { code: 'unauthorized', message: 'this request does not carry the daemon\'s token' }),
    );
    await expect(api.listHarnesses()).rejects.toMatchObject({ status: 401, code: 'unauthorized' });
  });
});

describe('requests', () => {
  it('posts a mission as JSON and returns the response body', async () => {
    const { api, seen } = client(() => jsonResponse(200, { unit_id: 'u1', waits_for_slot: false }));
    const body = {
      work_item: 'owner/sandbox#1',
      repo: 'owner/sandbox',
      spec_sha256: 'a'.repeat(64),
      harness: 'reqdrive',
      idempotency_key: 'k1',
    };
    await expect(api.createMission(body)).resolves.toEqual({ unit_id: 'u1', waits_for_slot: false });
    expect(seen[0].url).toBe(`${BASE}/missions`);
    expect(seen[0].init.method).toBe('POST');
    expect(header(seen[0], 'content-type')).toBe('application/json');
    expect(JSON.parse(String(seen[0].init.body))).toEqual(body);
  });

  it('a GET sends no body and no content type', async () => {
    const { api, seen } = client(() => jsonResponse(200, []));
    await api.listUnits();
    expect(seen[0].init.method).toBe('GET');
    expect(seen[0].init.body).toBeUndefined();
    expect(header(seen[0], 'content-type')).toBeUndefined();
  });

  it.each([
    [202, 'accepted'],
    [404, 'unknown_unit'],
    [410, 'gone'],
    [422, 'invalid'],
  ] as const)('a command answered %i resolves to %s', async (status, outcome) => {
    const { api, seen } = client(() => new Response('', { status }));
    const command = newCommand('reverify');
    await expect(api.sendCommand('u 1/x', command)).resolves.toBe(outcome);
    expect(seen[0].url).toBe(`${BASE}/units/u%201%2Fx/commands`);
    expect(JSON.parse(String(seen[0].init.body))).toEqual(command);
  });

  it('a command answered with any other status rejects', async () => {
    const { api } = client(() => jsonResponse(503, { code: 'store', message: 'the store is down' }));
    await expect(api.sendCommand('u1', newCommand('halt'))).rejects.toMatchObject({
      status: 503,
      code: 'store',
    });
  });

  it('newCommand gives each command its own id', () => {
    const a = newCommand('halt');
    const b = newCommand('halt');
    expect(a.command).toBe('halt');
    expect(a.cmd_id).not.toBe(b.cmd_id);
  });

  it('reads a spec with an encoded query, and leaves commit out when it has none', async () => {
    const { api, seen } = client(() => jsonResponse(200, {}));
    await api.getSpec({ repo: 'owner/sand box', spec_id: 'SPEC-0001' });
    await api.getSpec({ repo: 'owner/sandbox', spec_id: 'SPEC-0001', commit: 'abc123' });
    expect(seen[0].url).toBe(`${BASE}/specs?repo=owner%2Fsand+box&spec_id=SPEC-0001`);
    expect(seen[1].url).toBe(`${BASE}/specs?repo=owner%2Fsandbox&spec_id=SPEC-0001&commit=abc123`);
  });

  it('lists signatures for one repository', async () => {
    const { api, seen } = client(() => jsonResponse(200, []));
    await api.listSignatures('owner/sandbox');
    expect(seen[0].url).toBe(`${BASE}/signatures?repo=owner%2Fsandbox`);
  });

  it('a trailing slash on the base address is dropped', async () => {
    const urls: string[] = [];
    const api = createFleetApi({
      baseUrl: `${BASE}/`,
      token: () => TOKEN,
      fetch: (async (input: RequestInfo | URL) => {
        urls.push(String(input));
        return jsonResponse(200, []);
      }) as unknown as typeof fetch,
    });
    await api.listUnits();
    expect(urls).toEqual([`${BASE}/units`]);
    expect(api.baseUrl).toBe(BASE);
  });
});

describe('the stream', () => {
  it('opens a WebSocket with the token as a subprotocol and never in the address', () => {
    const api = streamClient();
    api.openStream('u1', 7, { onEvent: () => {} });
    const socket = SocketStub.made[0];
    expect(socket.url).toBe('ws://127.0.0.1:8787/units/u1/stream?since=7');
    expect(socket.protocols).toEqual([WS_PROTOCOL, `${WS_TOKEN_PREFIX}${TOKEN}`]);
    expect(socket.protocols).toEqual(['fleetd.v1', `fleetd.token.${TOKEN}`]);
    expect(socket.url).not.toContain(TOKEN);
  });

  it('an https address becomes wss', () => {
    SocketStub.made = [];
    const api = createFleetApi({
      baseUrl: 'https://fleet.example',
      token: () => TOKEN,
      WebSocket: SocketStub as unknown as typeof WebSocket,
    });
    api.openStream('u1', 0, { onEvent: () => {} });
    expect(SocketStub.made[0].url).toBe('wss://fleet.example/units/u1/stream?since=0');
  });

  it('opens nothing and throws without a token', () => {
    const api = streamClient(null);
    expect(() => api.openStream('u1', 0, { onEvent: () => {} })).toThrowError(ApiFailure);
    expect(SocketStub.made).toHaveLength(0);
  });

  it('delivers each frame as an envelope and drops a frame that is not JSON', () => {
    const api = streamClient();
    const seen: EventEnvelopeDto[] = [];
    api.openStream('u1', 0, { onEvent: (e) => seen.push(e) });
    const socket = SocketStub.made[0];
    const envelope: EventEnvelopeDto = {
      unit_id: 'u1',
      seq: 1,
      event: { type: 'stage', stage: 'red', status: 'started' },
    };
    socket.onmessage?.({ data: '{not json' });
    socket.onmessage?.({ data: JSON.stringify(envelope) });
    expect(seen).toEqual([envelope]);
  });

  it('reports a close once, whether the socket errored, closed, or both', () => {
    const api = streamClient();
    const onClose = vi.fn();
    api.openStream('u1', 0, { onEvent: () => {}, onClose });
    const socket = SocketStub.made[0];
    socket.onerror?.();
    socket.onclose?.();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not report a close the caller asked for', () => {
    const api = streamClient();
    const onClose = vi.fn();
    const handle = api.openStream('u1', 0, { onEvent: () => {}, onClose });
    const socket = SocketStub.made[0];
    handle.close();
    expect(socket.closed).toBe(true);
    socket.onclose?.();
    expect(onClose).not.toHaveBeenCalled();
  });
});
````

- [ ] **Step 3: Run it and see it fail**

```
npx vitest run src/lib/api.test.ts
```

Expected: `Tests  39 failed (39)`. Thirty-eight fail with `TypeError: (0 , createFleetApi) is
not a function` and one with `TypeError: (0 , newCommand) is not a function`: the old `api.ts`
exports neither.

- [ ] **Step 4: Replace `src/lib/api.ts`**

````ts
// The typed client for the daemon's HTTP and WebSocket API.
//
// Every request and response type comes from ./generated/fleet-api, which is generated from
// the JSON Schema the `fleet-api` crate commits. Nothing here declares a wire type by hand.
//
// The token is sent as `Authorization: Bearer <token>` on every HTTP request and as a
// WebSocket subprotocol on the stream (a browser cannot set a header on a WebSocket). Without
// a token the client makes no request at all.

import type {
  ApiError,
  CommandDto,
  EventEnvelopeDto,
  HarnessView,
  LegacyMissionRequest,
  MissionBody,
  MissionResponse,
  ProblemView,
  SignRequest,
  SignResponse,
  SignatureView,
  SpecView,
  UnitSummary,
} from './generated/fleet-api';
import { currentToken } from './auth';

/** Where the daemon listens unless a build says otherwise. Pinned to the CSP by a test. */
export const DEFAULT_FLEET_URL = 'http://127.0.0.1:8787';

/** The daemon's address for this build: `VITE_FLEET_URL`, or the default. */
export const FLEET_BASE: string = import.meta.env?.VITE_FLEET_URL ?? DEFAULT_FLEET_URL;

/** The WebSocket subprotocol the daemon selects (`fleet_api::WS_PROTOCOL`). */
export const WS_PROTOCOL = 'fleetd.v1';
/** What the token-carrying subprotocol starts with (`fleet_api::WS_TOKEN_PREFIX`). */
export const WS_TOKEN_PREFIX = 'fleetd.token.';

/** The name of a command, as the schema spells it. */
export type CommandName = CommandDto['command'];

/** The request the legacy launch form sends: `LegacyMissionRequest` with every field chosen. */
export type CreateReq = Required<
  Pick<LegacyMissionRequest, 'task' | 'tier' | 'min_review_rounds'>
> & {
  mode: 'demo' | 'real';
};

/** What the daemon said to a command: 202, 410, 404 and 422 in that order. */
export type CommandOutcome = 'accepted' | 'gone' | 'unknown_unit' | 'invalid';

/**
 * Any failure of a request. `code` is the API's own (`ApiError.code`) when the daemon sent one,
 * and otherwise one of:
 *
 * - `no_token`: there is no token, so no request was made (`status` 0);
 * - `network`: the request got no answer (`status` 0);
 * - `http_<status>`: the daemon answered with a status and a body that is not an `ApiError`
 *   (the deprecated mission shape answers in plain text; a body that fits no request shape is
 *   answered by the framework). `message` is then the body's text.
 */
export class ApiFailure extends Error {
  readonly status: number;
  readonly code: string;
  readonly problems: ProblemView[];

  constructor(status: number, code: string, message: string, problems: ProblemView[] = []) {
    super(message);
    this.name = 'ApiFailure';
    this.status = status;
    this.code = code;
    this.problems = problems;
  }
}

export interface StreamHandlers {
  /** One record of the unit's log. */
  onEvent(envelope: EventEnvelopeDto): void;
  /** The socket closed and `close()` was not what closed it. Called at most once. */
  onClose?(): void;
}

export interface StreamHandle {
  /** Close the socket. `onClose` is not called. */
  close(): void;
}

/** The daemon's API as the cockpit uses it. The real client and the test fake both satisfy it. */
export interface FleetApi {
  /** The daemon's address, for display. */
  readonly baseUrl: string;
  /** `GET /units`. */
  listUnits(): Promise<UnitSummary[]>;
  /** `POST /missions`, either request shape. A refusal rejects with an `ApiFailure`. */
  createMission(body: MissionBody): Promise<MissionResponse>;
  /** `POST /units/{id}/commands`. */
  sendCommand(unitId: string, command: CommandDto): Promise<CommandOutcome>;
  /** `GET /harnesses`. */
  listHarnesses(): Promise<HarnessView[]>;
  /** `GET /specs`. Leave `commit` out to read the base branch's tip. */
  getSpec(query: { repo: string; spec_id: string; commit?: string }): Promise<SpecView>;
  /** `POST /signatures`. */
  sign(request: SignRequest): Promise<SignResponse>;
  /** `GET /signatures`. */
  listSignatures(repo: string): Promise<SignatureView[]>;
  /**
   * `GET /units/{id}/stream`: every record after `since`, then what follows. Throws an
   * `ApiFailure` with code `no_token` when there is no token.
   */
  openStream(unitId: string, since: number, handlers: StreamHandlers): StreamHandle;
}

/** The method names of `FleetApi`, for the test that compares the fake with the real client. */
export const FLEET_API_METHODS = [
  'listUnits',
  'createMission',
  'sendCommand',
  'listHarnesses',
  'getSpec',
  'sign',
  'listSignatures',
  'openStream',
] as const satisfies readonly (keyof FleetApi)[];

export interface FleetApiOptions {
  baseUrl: string;
  /** Asked on every request, so a token loaded later is picked up. */
  token: () => string | null;
  fetch?: typeof fetch;
  WebSocket?: typeof WebSocket;
}

let commandSeq = 0;

/** A command with a fresh `cmd_id`. */
export function newCommand(name: CommandName): CommandDto {
  commandSeq += 1;
  return { command: name, cmd_id: `c${Date.now()}-${commandSeq}` };
}

function isApiError(value: unknown): value is ApiError {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.code === 'string' && typeof v.message === 'string';
}

/** Turn a response that is not a success into the `ApiFailure` to throw. */
async function failureOf(response: Response): Promise<ApiFailure> {
  const text = await response.text().catch(() => '');
  try {
    const parsed: unknown = JSON.parse(text);
    if (isApiError(parsed)) {
      return new ApiFailure(response.status, parsed.code, parsed.message, parsed.problems ?? []);
    }
  } catch {
    /* not JSON: fall through */
  }
  const message = text.trim() || `the daemon answered ${response.status}`;
  return new ApiFailure(response.status, `http_${response.status}`, message);
}

export function createFleetApi(options: FleetApiOptions): FleetApi {
  const baseUrl = options.baseUrl.replace(/\/+$/, '');

  function requireToken(): string {
    const token = options.token();
    if (!token) throw new ApiFailure(0, 'no_token', 'there is no daemon token: not connected');
    return token;
  }

  async function send(method: 'GET' | 'POST', path: string, body?: unknown): Promise<Response> {
    const token = requireToken();
    const headers: Record<string, string> = { authorization: `Bearer ${token}` };
    if (body !== undefined) headers['content-type'] = 'application/json';
    const doFetch = options.fetch ?? fetch;
    try {
      return await doFetch(`${baseUrl}${path}`, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch {
      // The cause is deliberately dropped: a fetch error's text can carry the request.
      throw new ApiFailure(0, 'network', `the daemon at ${baseUrl} did not answer`);
    }
  }

  async function json<T>(method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
    const response = await send(method, path, body);
    if (!response.ok) throw await failureOf(response);
    return (await response.json()) as T;
  }

  return {
    baseUrl,

    listUnits: () => json<UnitSummary[]>('GET', '/units'),

    createMission: (body) => json<MissionResponse>('POST', '/missions', body),

    async sendCommand(unitId, command) {
      const path = `/units/${encodeURIComponent(unitId)}/commands`;
      const response = await send('POST', path, command);
      switch (response.status) {
        case 202:
          return 'accepted';
        case 404:
          return 'unknown_unit';
        case 410:
          return 'gone';
        case 422:
          return 'invalid';
        default:
          throw await failureOf(response);
      }
    },

    listHarnesses: () => json<HarnessView[]>('GET', '/harnesses'),

    getSpec(query) {
      const params = new URLSearchParams({ repo: query.repo, spec_id: query.spec_id });
      if (query.commit) params.set('commit', query.commit);
      return json<SpecView>('GET', `/specs?${params.toString()}`);
    },

    sign: (request) => json<SignResponse>('POST', '/signatures', request),

    listSignatures(repo) {
      const params = new URLSearchParams({ repo });
      return json<SignatureView[]>('GET', `/signatures?${params.toString()}`);
    },

    openStream(unitId, since, handlers) {
      const token = requireToken();
      const Socket = options.WebSocket ?? WebSocket;
      const wsBase = baseUrl.replace(/^http/, 'ws');
      const url = `${wsBase}/units/${encodeURIComponent(unitId)}/stream?since=${since}`;
      // The token goes in the subprotocol list and never in the URL: a URL is logged.
      const socket = new Socket(url, [WS_PROTOCOL, `${WS_TOKEN_PREFIX}${token}`]);
      let closedByCaller = false;
      let reported = false;
      const report = () => {
        if (closedByCaller || reported) return;
        reported = true;
        handlers.onClose?.();
      };
      socket.onmessage = (message: MessageEvent) => {
        try {
          handlers.onEvent(JSON.parse(String(message.data)) as EventEnvelopeDto);
        } catch {
          /* a frame that is not JSON is dropped */
        }
      };
      socket.onclose = report;
      socket.onerror = report;
      return {
        close() {
          closedByCaller = true;
          socket.close();
        },
      };
    },
  };
}

/** The one client the app uses. Tests build their own, or use `src/lib/testkit/fakeApi`. */
export const api: FleetApi = createFleetApi({ baseUrl: FLEET_BASE, token: currentToken });
````

- [ ] **Step 5: Run the test**

```
npx vitest run src/lib/api.test.ts
```

Expected: `Tests  39 passed (39)`. (The rest of the suite is red until Task 5: the store still
imports the five functions that are gone. Do not commit yet.)

### Task 4: The fake client

**Files:**
- Create: `src/lib/testkit/fakeApi.test.ts`
- Create: `src/lib/testkit/fakeApi.ts`

**Produces:** `createFakeApi`, `FakeFleetApi`, `FakeCall`, `FakeStream`, `specView`,
`harnessView`, `sha256Hex`, `TEST_TOKEN`, `SIGNED_AT_MS`.

What to know:

- The fake is a `FleetApi` plus what a test needs to script it: the lists it answers from
  (`units`, `harnesses`, `specs`, `signatures`), `fail` and `failNext`, and `emit` and `drop`
  to play a unit's stream.
- It answers as the daemon does for the cases the page handles, with the daemon's status and
  code: `duplicate_key` (409) for a repeated idempotency key, `not_signed` (403), `hash_mismatch`
  (409), `spec_not_ready` (422, with problems), `spec_not_found` (404), and `unknown_unit` for a
  command to a unit it does not list. Its `message` texts are its own; a test asserts on a
  code, or that a message is shown unedited.
- `openStream` delivers the replay in a microtask, after the caller has its handle, as a
  socket would. A test that opens a stream awaits once before it looks.
- It imports `node:crypto` for `sha256Hex`. It is test support: nothing outside a test file
  imports `src/lib/testkit`, so none of it reaches the bundle.

- [ ] **Step 1: Write the failing test**

Create `src/lib/testkit/fakeApi.test.ts`:

````ts
import { describe, it, expect, vi } from 'vitest';
import { ApiFailure, createFleetApi, FLEET_API_METHODS, newCommand, type FleetApi } from '../api';
import type { EventEnvelopeDto, MissionRequest } from '../generated/fleet-api';
import { createFakeApi, harnessView, sha256Hex, specView, SIGNED_AT_MS, TEST_TOKEN } from './fakeApi';

// The fake is only worth having if it is the same thing, to a caller, as the real client.

function real(): FleetApi {
  return createFleetApi({
    baseUrl: 'http://127.0.0.1:8787',
    token: () => TEST_TOKEN,
    fetch: (async () => new Response('[]', { status: 200 })) as unknown as typeof fetch,
  });
}

function mission(key: string, sha256: string): MissionRequest {
  return {
    work_item: 'owner/sandbox#1',
    repo: 'owner/sandbox',
    spec_sha256: sha256,
    harness: 'reqdrive',
    idempotency_key: key,
  };
}

describe('the fake satisfies the interface of the real client', () => {
  it('both are a FleetApi to the compiler', () => {
    // These two lines are the test: `npm run check` fails if either stops being a FleetApi.
    const a = real() satisfies FleetApi;
    const b = createFakeApi() satisfies FleetApi;
    expect(typeof a.baseUrl).toBe('string');
    expect(typeof b.baseUrl).toBe('string');
  });

  it('both have every method the interface names, and the real client has no other', () => {
    const a = real() as unknown as Record<string, unknown>;
    const b = createFakeApi() as unknown as Record<string, unknown>;
    for (const name of FLEET_API_METHODS) {
      expect(typeof a[name], `the real client has no ${name}`).toBe('function');
      expect(typeof b[name], `the fake has no ${name}`).toBe('function');
    }
    const realMethods = Object.keys(a)
      .filter((key) => typeof a[key] === 'function')
      .sort();
    expect(realMethods).toEqual([...FLEET_API_METHODS].sort());
  });

  it('a call the real client would reject without a token can be made to reject the same way', async () => {
    const fake = createFakeApi();
    fake.fail('listUnits', new ApiFailure(0, 'no_token', 'there is no daemon token: not connected'));
    await expect(fake.listUnits()).rejects.toMatchObject({ code: 'no_token', status: 0 });
    await expect(fake.listUnits()).rejects.toBeInstanceOf(ApiFailure);
    fake.heal('listUnits');
    await expect(fake.listUnits()).resolves.toEqual([]);
  });
});

describe('the fake answers as the daemon does', () => {
  it('signs a ready spec, lists the signature, and shows it on the spec', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    expect(spec.sha256).toBe(sha256Hex(spec.text));
    const answer = await fake.sign({
      repo: spec.repo,
      spec_id: spec.spec_id,
      commit: spec.commit,
      sha256: spec.sha256,
    });
    expect(answer).toEqual({
      signature: {
        repo: 'owner/sandbox',
        spec_id: 'SPEC-0001',
        commit: spec.commit,
        sha256: spec.sha256,
        signed_by: 'tester',
        signed_at_ms: SIGNED_AT_MS,
      },
      tier: 't1',
      work_item: 'owner/sandbox#1',
    });
    await expect(fake.listSignatures('owner/sandbox')).resolves.toHaveLength(1);
    await expect(fake.listSignatures('other/repo')).resolves.toEqual([]);
    const again = await fake.getSpec({ repo: spec.repo, spec_id: spec.spec_id });
    expect(again.signature?.sha256).toBe(spec.sha256);
  });

  it('refuses a hash that does not match, a spec that is not ready, and a spec that is not there', async () => {
    const draft = specView({
      spec_id: 'SPEC-0002',
      problems: [{ code: 'open_questions', message: 'Which currency?' }],
    });
    const fake = createFakeApi({ specs: [specView(), draft] });
    const good = fake.specs[0];
    await expect(
      fake.sign({ repo: good.repo, spec_id: good.spec_id, commit: good.commit, sha256: '0'.repeat(64) }),
    ).rejects.toMatchObject({ status: 409, code: 'hash_mismatch' });
    await expect(
      fake.sign({ repo: draft.repo, spec_id: draft.spec_id, commit: draft.commit, sha256: draft.sha256 }),
    ).rejects.toMatchObject({ status: 422, code: 'spec_not_ready', problems: draft.problems });
    await expect(fake.getSpec({ repo: 'owner/sandbox', spec_id: 'SPEC-9999' })).rejects.toMatchObject({
      status: 404,
      code: 'spec_not_found',
    });
    expect(fake.signatures).toEqual([]);
  });

  it('starts a unit from a signed spec once per idempotency key', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    await expect(fake.createMission(mission('k1', spec.sha256))).rejects.toMatchObject({
      status: 403,
      code: 'not_signed',
    });
    await fake.sign({ repo: spec.repo, spec_id: spec.spec_id, commit: spec.commit, sha256: spec.sha256 });
    await expect(fake.createMission(mission('k1', spec.sha256))).resolves.toEqual({
      unit_id: 'u1',
      waits_for_slot: false,
    });
    const repeat = (await fake
      .createMission(mission('k1', spec.sha256))
      .catch((e: unknown) => e)) as ApiFailure;
    expect(repeat.status).toBe(409);
    expect(repeat.code).toBe('duplicate_key');
    expect(repeat.message).toContain('u1');
    expect(fake.missions).toHaveLength(1);
    await expect(fake.listUnits()).resolves.toHaveLength(1);
    await expect(fake.createMission(mission('k2', spec.sha256))).resolves.toMatchObject({
      unit_id: 'u2',
    });
  });

  it('answers the deprecated mission shape with a unit id and no slot field', async () => {
    const fake = createFakeApi();
    await expect(fake.createMission({ task: 'add sum', tier: 't2', mode: 'demo' })).resolves.toEqual({
      unit_id: 'u1',
    });
    await expect(fake.createMission({ task: 'x', mode: 'sideways' })).rejects.toMatchObject({
      status: 400,
      code: 'http_400',
      message: 'unknown mode: sideways',
    });
  });

  it('answers a command for a unit nobody knows, and otherwise as scripted', async () => {
    const fake = createFakeApi({
      units: [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 't', last_seq: 0 }],
    });
    await expect(fake.sendCommand('u404', newCommand('halt'))).resolves.toBe('unknown_unit');
    await expect(fake.sendCommand('u1', newCommand('reverify'))).resolves.toBe('accepted');
    fake.commandOutcome = 'gone';
    await expect(fake.sendCommand('u1', newCommand('halt'))).resolves.toBe('gone');
    expect(fake.commands.map((c) => c.command.command)).toEqual(['halt', 'reverify', 'halt']);
  });

  it('lists the harnesses it was given', async () => {
    const fake = createFakeApi({ harnesses: [harnessView(), harnessView({ name: 'fake', state: 'unprobed', harness: null, capabilities: null })] });
    const rows = await fake.listHarnesses();
    expect(rows.map((r) => [r.name, r.state])).toEqual([
      ['reqdrive', 'healthy'],
      ['fake', 'unprobed'],
    ]);
  });

  it('replays a unit log after `since`, then follows it, and reports a dropped stream once', async () => {
    const fake = createFakeApi();
    fake.emit('u1', { type: 'stage', stage: 'provision', status: 'started' });
    fake.emit('u1', { type: 'stage', stage: 'provision', status: 'finished' });
    const seen: EventEnvelopeDto[] = [];
    const onClose = vi.fn();
    const handle = fake.openStream('u1', 1, { onEvent: (e) => seen.push(e), onClose });
    expect(seen).toEqual([]); // the replay arrives after the caller has its handle
    await Promise.resolve();
    expect(seen.map((e) => e.seq)).toEqual([2]);
    fake.emit('u1', { type: 'stage', stage: 'red', status: 'started' });
    expect(seen.map((e) => e.seq)).toEqual([2, 3]);
    expect(fake.openStreams('u1')).toBe(1);

    fake.drop('u1');
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(fake.openStreams('u1')).toBe(0);
    fake.emit('u1', { type: 'stage', stage: 'red', status: 'finished' });
    expect(seen.map((e) => e.seq)).toEqual([2, 3]); // a closed stream gets nothing

    // A stream the caller closes is not reported as dropped.
    const quiet = vi.fn();
    const second = fake.openStream('u1', 4, { onEvent: () => {}, onClose: quiet });
    second.close();
    fake.drop('u1');
    expect(quiet).not.toHaveBeenCalled();
    handle.close();
  });

  it('records every call, and failNext fails exactly one', async () => {
    const fake = createFakeApi();
    fake.failNext('listHarnesses', new ApiFailure(503, 'store', 'the store is down'));
    await expect(fake.listHarnesses()).rejects.toMatchObject({ code: 'store' });
    await expect(fake.listHarnesses()).resolves.toEqual([]);
    expect(fake.calls.map((c) => c.method)).toEqual(['listHarnesses', 'listHarnesses']);
  });
});
````

- [ ] **Step 2: Run it and see it fail**

```
npx vitest run src/lib/testkit
```

Expected: `Error: Failed to resolve import "./fakeApi" from
"src/lib/testkit/fakeApi.test.ts". Does the file exist?`, and no test runs.

- [ ] **Step 3: Create `src/lib/testkit/fakeApi.ts`**

````ts
// An in-memory `FleetApi` for component and store tests. No network, no timers.
//
// It answers as the daemon does for the cases the cockpit handles: a repeated idempotency key,
// an unsigned spec, a hash that does not match, a spec that is not ready, a unit nobody knows.
// The `message` texts are this fake's own; tests assert on `code`, or on the text being shown
// verbatim, never on the daemon's wording.

import { createHash } from 'node:crypto';
import { ApiFailure, type CommandOutcome, type FleetApi, type StreamHandlers } from '../api';
import type {
  CommandDto,
  EventEnvelopeDto,
  HarnessView,
  MissionBody,
  SignatureView,
  SpecView,
  UnitEventDto,
  UnitSummary,
} from '../generated/fleet-api';

/** A token long enough for the daemon to accept, for tests that need one. */
export const TEST_TOKEN = 'test-token-0123456789abcdef0123456789abcdef';

/** The time every fake signature is made at. */
export const SIGNED_AT_MS = 1_700_000_000_000;

export interface FakeCall {
  method: string;
  args: unknown[];
}

export interface FakeStream {
  unitId: string;
  since: number;
  open: boolean;
}

export interface FakeFleetApi extends FleetApi {
  /** What `GET /units` answers. A dispatched unit is appended. */
  units: UnitSummary[];
  /** What `GET /harnesses` answers. */
  harnesses: HarnessView[];
  /** The specs `GET /specs` can find. Signing one sets its `signature`. */
  specs: SpecView[];
  /** What `GET /signatures` answers from. */
  signatures: SignatureView[];
  /** The name recorded as the signer. */
  operator: string;
  /** What the next commands are answered with. */
  commandOutcome: CommandOutcome;
  /** Every call made, in order. */
  readonly calls: FakeCall[];
  /** Every mission body that was admitted, in order. */
  readonly missions: MissionBody[];
  /** Every command sent, in order, whatever it was answered with. */
  readonly commands: { unitId: string; command: CommandDto }[];
  /** Every stream opened, in order. */
  readonly streams: FakeStream[];
  /** Make the next call of `method` reject with `failure`. */
  failNext(method: keyof FleetApi, failure: ApiFailure): void;
  /** Make every call of `method` reject with `failure` until `heal(method)`. */
  fail(method: keyof FleetApi, failure: ApiFailure): void;
  heal(method: keyof FleetApi): void;
  /** Append one record to the unit's log and deliver it to the unit's open streams. */
  emit(unitId: string, event: UnitEventDto): EventEnvelopeDto;
  /** Close the unit's open streams, as the daemon does when no run holds the unit. */
  drop(unitId: string): void;
  /** How many of the unit's streams are open now. */
  openStreams(unitId: string): number;
}

interface LiveStream extends FakeStream {
  handlers: StreamHandlers;
}

export function createFakeApi(seed: Partial<Pick<FakeFleetApi, 'units' | 'harnesses' | 'specs' | 'signatures'>> = {}): FakeFleetApi {
  const calls: FakeCall[] = [];
  const missions: MissionBody[] = [];
  const commands: { unitId: string; command: CommandDto }[] = [];
  const live: LiveStream[] = [];
  const logs: Record<string, EventEnvelopeDto[]> = {};
  const keys = new Map<string, string>();
  const once = new Map<string, ApiFailure>();
  const always = new Map<string, ApiFailure>();
  let nextUnit = 1;

  function enter(method: keyof FleetApi, args: unknown[]): void {
    calls.push({ method, args });
    const sticky = always.get(method);
    if (sticky) throw sticky;
    const failure = once.get(method);
    if (failure) {
      once.delete(method);
      throw failure;
    }
  }

  const fake: FakeFleetApi = {
    baseUrl: 'http://fake.invalid',
    units: seed.units ?? [],
    harnesses: seed.harnesses ?? [],
    specs: seed.specs ?? [],
    signatures: seed.signatures ?? [],
    operator: 'tester',
    commandOutcome: 'accepted',
    calls,
    missions,
    commands,
    streams: live,

    failNext(method, failure) {
      once.set(method, failure);
    },
    fail(method, failure) {
      always.set(method, failure);
    },
    heal(method) {
      always.delete(method);
    },

    async listUnits() {
      enter('listUnits', []);
      return fake.units.map((u) => ({ ...u }));
    },

    async createMission(body) {
      enter('createMission', [body]);
      const unitId = `u${nextUnit}`;
      if ('spec_sha256' in body) {
        const earlier = keys.get(body.idempotency_key);
        if (earlier) {
          throw new ApiFailure(409, 'duplicate_key', `this key already started unit ${earlier}`);
        }
        const signed = fake.signatures.some(
          (s) => s.repo === body.repo && s.sha256 === body.spec_sha256,
        );
        if (!signed) {
          throw new ApiFailure(403, 'not_signed', `no signature is stored for ${body.spec_sha256}`);
        }
        keys.set(body.idempotency_key, unitId);
      } else if (body.mode !== undefined && body.mode !== 'demo' && body.mode !== 'real') {
        throw new ApiFailure(400, 'http_400', `unknown mode: ${body.mode}`);
      }
      nextUnit += 1;
      missions.push(body);
      fake.units.push({
        unit_id: unitId,
        phase: 'queued',
        cost: 0,
        usd_cap: 5,
        tier: 'spec_sha256' in body ? 't1' : (body.tier ?? 't1'),
        task: 'spec_sha256' in body ? body.work_item : body.task,
        last_seq: 0,
      });
      return 'spec_sha256' in body
        ? { unit_id: unitId, waits_for_slot: false }
        : { unit_id: unitId };
    },

    async sendCommand(unitId, command) {
      enter('sendCommand', [unitId, command]);
      commands.push({ unitId, command });
      if (!fake.units.some((u) => u.unit_id === unitId)) return 'unknown_unit';
      return fake.commandOutcome;
    },

    async listHarnesses() {
      enter('listHarnesses', []);
      return fake.harnesses.map((h) => ({ ...h }));
    },

    async getSpec(query) {
      enter('getSpec', [query]);
      const found = fake.specs.find(
        (s) =>
          s.repo === query.repo &&
          s.spec_id === query.spec_id &&
          (query.commit === undefined || s.commit === query.commit),
      );
      if (!found) {
        throw new ApiFailure(404, 'spec_not_found', `no spec ${query.spec_id} in ${query.repo}`);
      }
      return { ...found };
    },

    async sign(request) {
      enter('sign', [request]);
      const spec = fake.specs.find(
        (s) =>
          s.repo === request.repo && s.spec_id === request.spec_id && s.commit === request.commit,
      );
      if (!spec) {
        throw new ApiFailure(404, 'spec_not_found', `no spec ${request.spec_id} at ${request.commit}`);
      }
      if (spec.sha256 !== request.sha256) {
        throw new ApiFailure(409, 'hash_mismatch', 'the file at that commit has another hash');
      }
      if (spec.problems.length > 0) {
        throw new ApiFailure(422, 'spec_not_ready', 'the spec is not ready to sign', spec.problems);
      }
      const signature: SignatureView = {
        repo: spec.repo,
        spec_id: spec.spec_id,
        commit: spec.commit,
        sha256: spec.sha256,
        signed_by: fake.operator,
        signed_at_ms: SIGNED_AT_MS,
      };
      if (!fake.signatures.some((s) => s.repo === signature.repo && s.sha256 === signature.sha256)) {
        fake.signatures.push(signature);
      }
      spec.signature = signature;
      return { signature, tier: spec.tier ?? 't1', work_item: spec.work_item ?? '' };
    },

    async listSignatures(repo) {
      enter('listSignatures', [repo]);
      return fake.signatures.filter((s) => s.repo === repo).map((s) => ({ ...s }));
    },

    openStream(unitId, since, handlers) {
      enter('openStream', [unitId, since]);
      const stream: LiveStream = { unitId, since, open: true, handlers };
      live.push(stream);
      // The replay is delivered after the caller has its handle, as a socket would deliver it.
      const replay = (logs[unitId] ?? []).filter((e) => e.seq > since);
      queueMicrotask(() => {
        for (const envelope of replay) if (stream.open) handlers.onEvent(envelope);
      });
      return {
        close() {
          stream.open = false;
        },
      };
    },

    emit(unitId, event) {
      const log = (logs[unitId] ??= []);
      const envelope: EventEnvelopeDto = { unit_id: unitId, seq: log.length + 1, event };
      log.push(envelope);
      for (const stream of live) {
        if (stream.open && stream.unitId === unitId) stream.handlers.onEvent(envelope);
      }
      return envelope;
    },

    drop(unitId) {
      for (const stream of live) {
        if (stream.open && stream.unitId === unitId) {
          stream.open = false;
          stream.handlers.onClose?.();
        }
      }
    },

    openStreams(unitId) {
      return live.filter((s) => s.open && s.unitId === unitId).length;
    },
  };
  return fake;
}

/** SHA-256 of the UTF-8 bytes of `text`, as lowercase hex: what the daemon calls `sha256`. */
export function sha256Hex(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

/**
 * A spec view that is ready to sign. Override what a test cares about; `sha256` is computed
 * from `text` unless the override names one.
 */
export function specView(overrides: Partial<SpecView> = {}): SpecView {
  const text = overrides.text ?? '---\nid: SPEC-0001\n---\n# Cart total\n';
  return {
    repo: 'owner/sandbox',
    spec_id: 'SPEC-0001',
    commit: '4f2a9c1d7e8b3a6c5d4e3f2a1b0c9d8e7f6a5b4c',
    sha256: sha256Hex(text),
    text,
    tier: 't1',
    work_item: 'owner/sandbox#1',
    problems: [],
    signature: null,
    ...overrides,
  };
}

/** A healthy harness row with the reference guarantees. Override what a test cares about. */
export function harnessView(overrides: Partial<HarnessView> = {}): HarnessView {
  return {
    name: 'reqdrive',
    state: 'healthy',
    harness: { name: 'reqdrive', version: '0.1.0' },
    capabilities: {
      isolation: 'container',
      metering: 'usd',
      gates: ['oracle'],
      delivery: 'bundle',
      resume: true,
      halt: true,
      holdouts: false,
      controls: ['scope', 'protected'],
      network: 'open',
      profiles: [{ name: 'claude', priced: true }],
      kinds: ['build'],
      presets: [
        { name: 'cargo', version: '0.1.0' },
        { name: 'node', version: '0.1.0' },
      ],
    },
    ...overrides,
  };
}
````

- [ ] **Step 4: Run the tests**

```
npx vitest run src/lib/api.test.ts src/lib/testkit
```

Expected: `Tests  50 passed (50)`.

### Task 5: The store on the client, and the end of `types.ts`

**Files:**
- Create: `src/lib/unit/rail.ts` (a stub; lane UI-RAIL owns it afterwards)
- Create: `src/lib/store.connection.svelte.test.ts`
- Replace: `src/lib/fleet.ts`, `src/lib/store.svelte.ts`, `src/lib/store.sink.svelte.test.ts`
- Modify: `src/lib/bridge.ts`, `src/lib/bridge.test.ts`, `src/lib/store.svelte.test.ts`,
  `src/lib/store.overlay.svelte.test.ts`, `src/lib/dashboard/store.ts`,
  `src/lib/dashboard/store.test.ts`, `src/lib/dashboard/adapters/fleet.ts`,
  `src/lib/dashboard/adapters/adapters.test.ts`, `src/views/Dashboard.svelte`
- Delete: `src/lib/types.ts`

**Produces:** in `fleet.ts`: `PHASES`, `asPhase`, `fromSummary` (the old `fromSnapshot`), and
three new fields on `Unit`: `pauseReason`, `pausedFrom`, `rail`. In `store.svelte.ts`:
`Connection`, `FleetStore.connection`, `FleetStore.dispatch`, a constructor that takes a client
and a token loader, `cmd` returning the daemon's answer, and four timing constants. In
`unit/rail.ts`: `STAGES`, `StageState`, `StageCell`, `RailState`, `newRail`, `foldRail`.

Where the old names went:

| `src/lib/types.ts` | Now |
|---|---|
| `Phase` | `PhaseDto` from the generated file |
| `FleetEvent` | `UnitEventDto` |
| `Envelope` | `EventEnvelopeDto` |
| `Snapshot` (a row of `GET /units`) | `UnitSummary`. The generated `Snapshot` is a different thing: the answer of `GET /units/{id}` |
| `CommandName` | `CommandName` from `src/lib/api.ts`, which is `CommandDto['command']` and so includes `reverify` |
| `Health` | Gone. `FleetStore.daemon` is replaced by `FleetStore.connection` |
| `IterationKind`, `LogStream`, `Severity`, `ArtifactKind` | Gone: the schema types those fields as strings, and nothing else used the names |

What to know:

- **The store makes no request without a token.** `reconnect()` asks the token loader first;
  with none, `connection` is `no_token` and nothing else happens. It does not retry by itself
  in that state, or after a 401 (`refused`): only a person, or a new mount, tries again. While
  the daemon does not answer (`unreachable`) it retries with a wait that doubles from one
  second to ten.
- **A stream that closes is reopened from the last record folded.** The daemon closes a stream
  after the replay when no run holds the unit, so a close is ordinary. The store reopens after
  half a second, doubling to fifteen while attempts bring no record, and back to half a second
  as soon as one does. It leaves a unit alone only when the unit is finished *and* its log has
  been read as far as its summary's `last_seq`.
- **`dispose()` makes the store restartable.** It closes streams, cancels timers and clears
  `started`, so a second `start()` (a remount, or hot reload) connects again. Units and
  selection are kept.
- **`cmd` resolves to what the daemon said** (`accepted`, `gone`, `unknown_unit`, `invalid`),
  or to `undefined` when there is no unit. It used to discard the status.
- **`UnitLite` does not change.** `src/lib/bridge.ts` defines the unit a sandboxed view plugin
  is sent as `Unit` without its log. The three new fields are left out of it too, so the plugin
  protocol is byte for byte what it was.
- **`bridge.ts` `degraded()`** was "Docker or the API key is missing". It is now "the page is
  not connected to the daemon", the only health the page still knows.
- The store's token loader defaults to `() => loadToken()`, a call through the module, not the
  function itself: a test that spies on `auth.loadToken` after the store was built is still
  seen.

- [ ] **Step 1: Create the stub `src/lib/unit/rail.ts`**

````ts
// The seven-stage rail of one unit, folded from its event log.
//
// WAVE 0 STUB. The types, `STAGES` and `newRail` are final. Lane UI-RAIL replaces the body of
// `foldRail`; until then the rail never leaves its initial state.

import type { Stage, UnitEventDto } from '../generated/fleet-api';

/** The seven stages, in the order a unit passes through them. */
export const STAGES = [
  'provision',
  'red',
  'plan',
  'green',
  'check',
  'review',
  'deliver',
] as const satisfies readonly Stage[];

/**
 * - `pending`: no `started` note has arrived for the stage;
 * - `running`: started and not finished;
 * - `done`: finished;
 * - `stopped`: it was running when the unit stopped for a person, was halted, or failed.
 */
export type StageState = 'pending' | 'running' | 'done' | 'stopped';

export interface StageCell {
  stage: Stage;
  state: StageState;
  /** The `detail` of the stage's latest note, if it had one. */
  detail: string | null;
  /** Metered agent time spent while this stage ran, in milliseconds. Null until a metric says. */
  elapsedMs: number | null;
  /** Spend while this stage ran, in USD. Null until a metric says. */
  costUsd: number | null;
  /** The runtime adapter the stage's latest metric named, when a metric names one. */
  adapter: string | null;
  /** The model the stage's latest metric named, when a metric names one. */
  model: string | null;
}

export interface RailState {
  /** Always seven cells, in `STAGES` order. */
  cells: StageCell[];
  /** The stage that is running, if one is. */
  current: Stage | null;
  /** The stage that most recently finished or stopped, if `current` is null. */
  last: Stage | null;
  /** The unit's cumulative spend at the latest metric. */
  seenCostUsd: number;
  /** The unit's cumulative metered time at the latest metric. */
  seenElapsedMs: number;
}

export function newRail(): RailState {
  return {
    cells: STAGES.map((stage) => ({
      stage,
      state: 'pending',
      detail: null,
      elapsedMs: null,
      costUsd: null,
      adapter: null,
      model: null,
    })),
    current: null,
    last: null,
    seenCostUsd: 0,
    seenElapsedMs: 0,
  };
}

/** Apply one event of a unit's log. Returns a new state; never mutates `rail`. */
export function foldRail(rail: RailState, event: UnitEventDto): RailState {
  void event;
  return rail;
}
````

- [ ] **Step 2: Write the failing test**

Create `src/lib/store.connection.svelte.test.ts`:

````ts
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { ApiFailure } from './api';
import {
  CONNECT_RETRY_BASE_MS,
  FleetStore,
  STREAM_RETRY_BASE_MS,
  STREAM_RETRY_CAP_MS,
} from './store.svelte';
import { createFakeApi, specView, TEST_TOKEN, type FakeFleetApi } from './testkit/fakeApi';
import type { UnitSummary } from './generated/fleet-api';

// The store's connection to the daemon: when it asks, when it does not, and what it does when
// a stream or the daemon goes away. (docs/testing/PLAN.md GAP-015, GAP-078, GAP-079.)

function row(id: string, phase = 'building', lastSeq = 0): UnitSummary {
  return { unit_id: id, phase, cost: 0, usd_cap: 5, tier: 't1', task: `task ${id}`, last_seq: lastSeq };
}

const withToken = async () => TEST_TOKEN;
const withoutToken = async () => null;
const opened = (fake: FakeFleetApi, id?: string) =>
  fake.calls.filter((c) => c.method === 'openStream' && (id === undefined || c.args[0] === id));

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('without a token', () => {
  it('makes no request at all and says so', async () => {
    const fake = createFakeApi({ units: [row('u1')] });
    const s = new FleetStore(fake, withoutToken);
    expect(s.connection).toBe('starting');
    s.start();
    await vi.runAllTimersAsync();
    expect(s.connection).toBe('no_token');
    expect(fake.calls).toEqual([]);
    expect(s.order).toEqual([]);
  });

  it('does not retry by itself: only a person, or a new mount, tries again', async () => {
    const fake = createFakeApi();
    let asked = 0;
    const s = new FleetStore(fake, async () => {
      asked += 1;
      return null;
    });
    s.start();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(asked).toBe(1);
    expect(fake.calls).toEqual([]);
  });

  it('connects on a retry once there is a token', async () => {
    const fake = createFakeApi({ units: [row('u1')] });
    let token: string | null = null;
    const s = new FleetStore(fake, async () => token);
    s.start();
    await vi.runAllTimersAsync();
    expect(s.connection).toBe('no_token');
    token = TEST_TOKEN;
    await s.reconnect();
    expect(s.connection).toBe('connected');
    expect(s.order).toEqual(['u1']);
    expect(opened(fake, 'u1')).toHaveLength(1);
  });
});

describe('a daemon that refuses or does not answer', () => {
  it('a 401 is refused and is not retried', async () => {
    const fake = createFakeApi();
    fake.fail('listUnits', new ApiFailure(401, 'unauthorized', 'no'));
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(s.connection).toBe('refused');
    expect(fake.calls.filter((c) => c.method === 'listUnits')).toHaveLength(1);
  });

  it('no answer is unreachable, and is retried until the daemon answers', async () => {
    const fake = createFakeApi({ units: [row('u1')] });
    fake.fail('listUnits', new ApiFailure(0, 'network', 'the daemon did not answer'));
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(s.connection).toBe('unreachable');

    await vi.advanceTimersByTimeAsync(CONNECT_RETRY_BASE_MS);
    expect(fake.calls.filter((c) => c.method === 'listUnits')).toHaveLength(2);
    expect(s.connection).toBe('unreachable');

    fake.heal('listUnits');
    await vi.advanceTimersByTimeAsync(CONNECT_RETRY_BASE_MS * 2);
    expect(s.connection).toBe('connected');
    expect(s.order).toEqual(['u1']);
    // Connected: the retry timer is not left running.
    const calls = fake.calls.length;
    await vi.advanceTimersByTimeAsync(120_000);
    expect(fake.calls.filter((c) => c.method === 'listUnits')).toHaveLength(3);
    expect(fake.calls.length).toBe(calls);
  });

  it('a store-side error from the daemon is unreachable too, never an empty fleet shown as fine', async () => {
    const fake = createFakeApi();
    fake.fail('listUnits', new ApiFailure(503, 'store', 'the store cannot be read'));
    const s = new FleetStore(fake, withToken);
    await s.reconnect();
    expect(s.connection).toBe('unreachable');
  });
});

describe('streams', () => {
  it('reopens a dropped stream from the last record folded, not from the start', async () => {
    const fake = createFakeApi({ units: [row('u1')] });
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);
    fake.emit('u1', { type: 'phase_changed', from: 'queued', to: 'building' });
    fake.emit('u1', { type: 'log', stream: 'agent', line: 'one' });
    expect(s.units['u1'].lastSeq).toBe(2);

    fake.drop('u1'); // the daemon restarted
    fake.emit('u1', { type: 'log', stream: 'agent', line: 'missed while away' });
    expect(fake.openStreams('u1')).toBe(0);

    await vi.advanceTimersByTimeAsync(STREAM_RETRY_BASE_MS);
    expect(opened(fake, 'u1').map((c) => c.args[1])).toEqual([0, 2]);
    expect(fake.openStreams('u1')).toBe(1);
    expect(s.units['u1'].lastSeq).toBe(3);
    expect(s.units['u1'].log.map((l) => l.line)).toEqual(['one', 'missed while away']);
  });

  it('waits longer each time a reopened stream brings nothing, up to a cap', async () => {
    const fake = createFakeApi({ units: [row('u1', 'needs_human')] });
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);

    const waits: number[] = [];
    for (let i = 0; i < 8; i += 1) {
      const before = opened(fake, 'u1').length;
      fake.drop('u1'); // no run holds the unit: the daemon closes after the replay
      let waited = 0;
      while (opened(fake, 'u1').length === before) {
        await vi.advanceTimersByTimeAsync(STREAM_RETRY_BASE_MS);
        waited += STREAM_RETRY_BASE_MS;
        expect(waited).toBeLessThanOrEqual(STREAM_RETRY_CAP_MS);
      }
      waits.push(waited);
    }
    expect(waits[0]).toBeLessThan(waits[3]);
    expect(waits[7]).toBe(STREAM_RETRY_CAP_MS);
    expect(fake.openStreams('u1')).toBe(1); // never two sockets for one unit
  });

  it('leaves a finished unit alone once its log has been read to the end', async () => {
    const fake = createFakeApi({ units: [row('u1', 'done', 2)] });
    fake.emit('u1', { type: 'phase_changed', from: 'merge_check', to: 'pr_open' });
    fake.emit('u1', { type: 'phase_changed', from: 'pr_open', to: 'done' });
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(s.units['u1'].lastSeq).toBe(2);
    fake.drop('u1');
    await vi.advanceTimersByTimeAsync(STREAM_RETRY_CAP_MS * 4);
    expect(opened(fake, 'u1')).toHaveLength(1);
  });

  it('reopens the stream of a finished unit whose log was cut short', async () => {
    const fake = createFakeApi({ units: [row('u1', 'done', 2)] });
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);
    fake.drop('u1'); // nothing was delivered: lastSeq 0 of 2
    fake.emit('u1', { type: 'phase_changed', from: 'merge_check', to: 'pr_open' });
    fake.emit('u1', { type: 'phase_changed', from: 'pr_open', to: 'done' });
    await vi.advanceTimersByTimeAsync(STREAM_RETRY_CAP_MS);
    expect(opened(fake, 'u1').length).toBeGreaterThan(1);
    expect(s.units['u1'].lastSeq).toBe(2);
  });

  it('dispose closes every stream and cancels every retry, and start connects again', async () => {
    const fake = createFakeApi({ units: [row('u1'), row('u2')] });
    const s = new FleetStore(fake, withToken);
    s.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(fake.openStreams('u1') + fake.openStreams('u2')).toBe(2);

    fake.drop('u2'); // a retry is now pending for u2
    s.dispose();
    expect(fake.openStreams('u1')).toBe(0);
    await vi.advanceTimersByTimeAsync(STREAM_RETRY_CAP_MS * 2);
    expect(opened(fake)).toHaveLength(2); // the pending retry did not fire

    s.start(); // a remount
    await vi.advanceTimersByTimeAsync(0);
    expect(opened(fake)).toHaveLength(4);
    expect(fake.openStreams('u1') + fake.openStreams('u2')).toBe(2);
    expect(s.order).toEqual(['u1', 'u2']); // units survived the unmount
  });
});

describe('dispatch and commands', () => {
  it('dispatch starts a unit from a signed spec, adds its tile, opens its stream and selects it', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    await fake.sign({ repo: spec.repo, spec_id: spec.spec_id, commit: spec.commit, sha256: spec.sha256 });
    const s = new FleetStore(fake, withToken);
    const request = {
      work_item: 'owner/sandbox#1',
      repo: 'owner/sandbox',
      spec_sha256: spec.sha256,
      harness: 'reqdrive',
      idempotency_key: 'k1',
    };
    await expect(s.dispatch(request, 't1')).resolves.toEqual({ unit_id: 'u1', waits_for_slot: false });
    expect(s.selectedId).toBe('u1');
    expect(s.units['u1']).toMatchObject({ task: 'owner/sandbox#1', tier: 'T1', phase: 'queued' });
    expect(opened(fake, 'u1')).toHaveLength(1);
  });

  it('a refused dispatch rejects with the failure the daemon sent and adds nothing', async () => {
    const fake = createFakeApi();
    const s = new FleetStore(fake, withToken);
    const failure = await s
      .dispatch(
        { work_item: 'o/r#1', repo: 'o/r', spec_sha256: 'f'.repeat(64), harness: 'reqdrive', idempotency_key: 'k1' },
        null,
      )
      .catch((e: unknown) => e);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect((failure as ApiFailure).code).toBe('not_signed');
    expect(s.order).toEqual([]);
    expect(opened(fake)).toHaveLength(0);
  });

  it('cmd sends a command with its own id and resolves to what the daemon said', async () => {
    const fake = createFakeApi({ units: [row('u1', 'needs_human')] });
    const s = new FleetStore(fake, withToken);
    await s.reconnect();
    await expect(s.cmd('reverify', 'u1')).resolves.toBe('accepted');
    fake.commandOutcome = 'gone';
    await expect(s.cmd('halt')).resolves.toBe('gone'); // defaults to the selected unit
    expect(fake.commands.map((c) => [c.unitId, c.command.command])).toEqual([
      ['u1', 'reverify'],
      ['u1', 'halt'],
    ]);
    expect(fake.commands[0].command.cmd_id).not.toBe(fake.commands[1].command.cmd_id);
  });
});

describe('what a phase change records', () => {
  it('keeps why and from where a unit stopped for a person, and clears both when it moves on', async () => {
    const fake = createFakeApi({ units: [row('u1', 'merge_check')] });
    const s = new FleetStore(fake, withToken);
    await s.reconnect();
    fake.emit('u1', {
      type: 'phase_changed',
      from: 'merge_check',
      to: 'needs_human',
      reason: 'verification stopped',
    });
    expect(s.units['u1']).toMatchObject({
      phase: 'needs_human',
      pauseReason: 'verification stopped',
      pausedFrom: 'merge_check',
    });
    fake.emit('u1', { type: 'phase_changed', from: 'needs_human', to: 'merge_check' });
    expect(s.units['u1']).toMatchObject({ pauseReason: null, pausedFrom: null });
  });

  it('a summary row with a phase this build does not know reads as queued', async () => {
    const fake = createFakeApi({ units: [row('u1', 'a_phase_from_the_future')] });
    const s = new FleetStore(fake, withToken);
    await s.reconnect();
    expect(s.units['u1'].phase).toBe('queued');
  });
});
````

- [ ] **Step 3: Run it and see it fail**

```
npx vitest run src/lib/store.connection.svelte.test.ts
```

Expected: all 16 fail against the old store. The first reads `AssertionError: expected
undefined to be 'starting'` (there is no `connection` field); others read `TypeError:
s.dispatch is not a function`, `expected undefined to be 'unreachable'`, and `TypeError: (0 ,
sendCommand) is not a function`.

- [ ] **Step 4: Replace `src/lib/fleet.ts`**

````ts
// Folds the daemon event stream into per-unit view state.

import type { PhaseDto, UnitEventDto, UnitSummary } from './generated/fleet-api';
import { foldRail, newRail, type RailState } from './unit/rail';

export interface LogLine {
  stream: string;
  line: string;
}
export interface Finding {
  round: number;
  title: string;
  severity: string;
  resolved: boolean;
}

export interface Unit {
  id: string;
  task: string;
  tier: string;
  phase: PhaseDto;
  history: PhaseDto[];
  cost: number;
  /** Per-unit USD ceiling (from the summary); drives the cost bar. */
  usdCap: number;
  tokensIn: number;
  tokensOut: number;
  iters: { build: number; check: number; review: number };
  log: LogLine[];
  findings: Finding[];
  oracleFiles: string[];
  branch?: string;
  pr?: string;
  blocked?: string;
  /** True while the unit is parked waiting for a concurrency slot. */
  awaitingSlot: boolean;
  /** True while the unit is backing off through an Anthropic rate-limit. */
  rateLimited: boolean;
  error?: string;
  result?: string;
  /** Highest event seq folded in, so reconnects replay only what's missing. */
  lastSeq: number;
  /** Why the unit is waiting for a person, while it is (`needs_human` or `halted`). */
  pauseReason: string | null;
  /** The phase the unit was in when it stopped for a person. */
  pausedFrom: PhaseDto | null;
  /** The seven-stage rail (lane UI-RAIL's fold). */
  rail: RailState;
}

/** Every phase, in the schema's order. `satisfies` fails the build if the schema drops one. */
export const PHASES = [
  'queued',
  'provisioning',
  'spec',
  'awaiting_oracle_approval',
  'building',
  'checking',
  'reviewing',
  'merge_check',
  'pr_open',
  'done',
  'no_change',
  'failed',
  'needs_human',
  'halted',
] as const satisfies readonly PhaseDto[];

// Fails the build if the schema gains a phase that `PHASES` does not list.
const everyPhaseIsListed: Exclude<PhaseDto, (typeof PHASES)[number]> extends never ? true : never =
  true;
void everyPhaseIsListed;

/**
 * `GET /units` types `phase` as a string (the route keeps its original keys). Narrow it; a
 * value this build does not know reads as `queued` until the unit's own events correct it.
 */
export function asPhase(value: string): PhaseDto {
  return (PHASES as readonly string[]).includes(value) ? (value as PhaseDto) : 'queued';
}

export function newUnit(id: string, task: string, tier: string, usdCap = 5): Unit {
  return {
    id,
    task,
    tier,
    phase: 'queued',
    history: ['queued'],
    cost: 0,
    usdCap,
    tokensIn: 0,
    tokensOut: 0,
    iters: { build: 0, check: 0, review: 0 },
    log: [],
    findings: [],
    oracleFiles: [],
    awaitingSlot: false,
    rateLimited: false,
    lastSeq: 0,
    pauseReason: null,
    pausedFrom: null,
    rail: newRail(),
  };
}

/**
 * Seed a unit from a `GET /units` row (on cockpit load). `lastSeq` stays 0 so
 * the WS stream replays the full event log to rebuild the detail view; the
 * summary just gives the tile its phase/cost/cap immediately.
 */
export function fromSummary(s: UnitSummary): Unit {
  const u = newUnit(s.unit_id, s.task, s.tier.toUpperCase(), s.usd_cap);
  u.phase = asPhase(s.phase);
  u.history = [u.phase];
  u.cost = s.cost;
  return u;
}

const ACTIVE: PhaseDto[] = ['provisioning', 'spec', 'building', 'checking', 'reviewing', 'merge_check', 'pr_open'];
const ATTENTION: PhaseDto[] = ['awaiting_oracle_approval', 'needs_human', 'halted'];
const GOOD: PhaseDto[] = ['done', 'no_change'];

export function phaseClass(p: PhaseDto): 'active' | 'attention' | 'good' | 'bad' | 'idle' {
  if (p === 'failed') return 'bad';
  if (GOOD.includes(p)) return 'good';
  if (ATTENTION.includes(p)) return 'attention';
  if (ACTIVE.includes(p)) return 'active';
  return 'idle';
}

export function isTerminal(p: PhaseDto): boolean {
  return p === 'done' || p === 'no_change' || p === 'failed';
}

/** Approximate progress (0..1) along the happy path, for the tile rail. */
export function progress(p: PhaseDto): number {
  const order: PhaseDto[] = [
    'queued', 'provisioning', 'spec', 'awaiting_oracle_approval',
    'building', 'checking', 'reviewing', 'merge_check', 'pr_open', 'done',
  ];
  const i = order.indexOf(p);
  if (p === 'failed') return 1;
  return i < 0 ? 0 : i / (order.length - 1);
}

/** Apply one event to a unit (mutates and returns it). */
export function fold(u: Unit, ev: UnitEventDto): Unit {
  u.rail = foldRail(u.rail, ev);
  switch (ev.type) {
    case 'phase_changed':
      u.phase = ev.to;
      u.history = [...u.history, ev.to];
      u.awaitingSlot = false;
      u.rateLimited = false; // any phase change means the wait resolved
      // As the daemon's own projection does: the reason and origin of a pause are those of
      // the transition into it, and leaving the pause clears both.
      if (ev.to === 'needs_human' || ev.to === 'halted') {
        u.pauseReason = ev.reason ?? null;
        u.pausedFrom = ev.from;
      } else {
        u.pauseReason = null;
        u.pausedFrom = null;
      }
      break;
    case 'oracle_proposed':
      u.oracleFiles = ev.test_files;
      break;
    case 'iteration':
      if (ev.kind === 'build' || ev.kind === 'check' || ev.kind === 'review') {
        u.iters = { ...u.iters, [ev.kind]: ev.n };
      }
      break;
    case 'log':
      u.log = [...u.log.slice(-400), { stream: ev.stream, line: ev.line }];
      break;
    case 'metric':
      u.cost = ev.cost_usd;
      u.tokensIn = ev.tokens_in;
      u.tokensOut = ev.tokens_out;
      break;
    case 'finding':
      u.findings = [
        ...u.findings,
        { round: ev.round, title: ev.title, severity: ev.severity, resolved: ev.resolved },
      ];
      break;
    case 'artifact':
      if (ev.kind === 'pr') u.pr = ev.ref;
      if (ev.kind === 'branch') u.branch = ev.ref;
      break;
    case 'blocked':
      u.blocked = ev.reason;
      if (ev.reason === 'awaiting concurrency slot') u.awaitingSlot = true;
      if (ev.reason === 'rate limited') u.rateLimited = true; // exact match to RL_REASON
      break;
    case 'error':
      u.error = `${ev.scope}: ${ev.detail}`;
      break;
    case 'done':
      u.result = ev.result;
      break;
    case 'stage':
      // Folded into `u.rail` above; it changes nothing else.
      break;
  }
  return u;
}
````

- [ ] **Step 5: Replace `src/lib/store.svelte.ts`**

````ts
// Shared fleet state, extracted out of App.svelte (Lane A / SHELL §"keep store.svelte.ts
// the one source of fleet state"). A single `FleetStore` instance owns the units,
// their live WebSocket streams, the selection, and the connection to the daemon, so that
// every view (the inline Fleet cockpit + the Project Dashboard) projects the SAME state.
// Switching views never tears the store down, so units/selection/streams survive a round trip.
//
// This is a `.svelte.ts` rune module: `$state`/`$derived` fields on a class give us
// fine-grained reactivity that any component reading the instance picks up.

import {
  api as defaultApi,
  ApiFailure,
  newCommand,
  type CommandName,
  type CommandOutcome,
  type CreateReq,
  type FleetApi,
  type StreamHandle,
} from './api';
import { loadToken } from './auth';
import { fold, fromSummary, newUnit, isTerminal, type Unit } from './fleet';
import type {
  EventEnvelopeDto,
  MissionRequest,
  MissionResponse,
  PhaseDto,
  TierDto,
  UnitEventDto,
  UnitSummary,
} from './generated/fleet-api';

/** A live `phase_changed` listener — the seam the Dashboard's `onFleetPhase` prop wires to. */
type PhaseListener = (unitId: string, to: PhaseDto, task: string, tier: string) => void;

/**
 * One seq-tagged log line, retained by the store so the view-plugin bridge can build
 * `log-append` deltas since a plugin's per-unit seq cursor. Kept deliberately OUTSIDE
 * the reactive `Unit` (whose `fleet.ts` `log` is capped + untagged): the bridge needs
 * the daemon `seq` to diff against a cursor, and this tail must not drive a component
 * re-render, so it lives in a plain (non-`$state`) store field.
 */
export interface LogDelta {
  seq: number;
  stream: string;
  line: string;
}

/**
 * Whether the page can talk to the daemon.
 *
 * - `starting`: nothing has been tried yet;
 * - `connected`: the last `GET /units` was answered;
 * - `no_token`: there is no token, so no request is made;
 * - `refused`: the daemon answered 401: it does not accept this token;
 * - `unreachable`: the daemon did not answer, or answered with an error.
 *
 * The last three are the one "not connected" state the page shows.
 */
export type Connection = 'starting' | 'connected' | 'no_token' | 'refused' | 'unreachable';

/** How many seq-tagged log lines the store retains per unit for plugin `log-append`. */
const LOG_TAIL_CAP = 500;

/** The first wait before a stream that closed is opened again; doubled each empty attempt. */
export const STREAM_RETRY_BASE_MS = 500;
/** The longest wait between two attempts to reopen one unit's stream. */
export const STREAM_RETRY_CAP_MS = 15_000;
/** The first wait before `GET /units` is tried again while the daemon is unreachable. */
export const CONNECT_RETRY_BASE_MS = 1_000;
/** The longest wait between two attempts to reach the daemon. */
export const CONNECT_RETRY_CAP_MS = 10_000;

function backoff(base: number, cap: number, attempt: number): number {
  return Math.min(cap, base * 2 ** Math.min(attempt, 10));
}

export class FleetStore {
  units = $state<Record<string, Unit>>({});
  order = $state<string[]>([]);
  selectedId = $state<string | null>(null);
  connection = $state<Connection>('starting');

  private readonly api: FleetApi;
  private readonly token: () => Promise<string | null>;
  // Live streams, one per unit. Not reactive (never rendered), just owned for cleanup.
  private streams: Record<string, StreamHandle> = {};
  // Pending reopen timers and the count of consecutive attempts that brought no record.
  private streamTimers: Record<string, ReturnType<typeof setTimeout>> = {};
  private streamAttempts: Record<string, number> = {};
  // The `last_seq` each unit's summary named: a terminal unit's log is complete only once
  // the stream has delivered up to here.
  private knownSeq: Record<string, number> = {};
  private connectTimer: ReturnType<typeof setTimeout> | null = null;
  private connectAttempts = 0;
  // Live `phase_changed` subscribers (the Dashboard wires one here so its fleet lane
  // advances off the very same stream the cockpit already consumes — no second socket).
  private phaseListeners = new Set<PhaseListener>();
  // Guards against double-connecting (e.g. a re-mounted Fleet view re-calling reconnect).
  private started = false;

  // ── view-plugin command-sink support (Lane V) ──────────────────────────────
  // The store is the SINGLE command sink: every daemon write (built-in ops grid AND
  // policed plugin commands) flows through `launch`/`cmd`, which own `cmd_id`, the
  // optimistic insert, and socket-open. Plugin-originated commands are validated by the
  // bridge's command policy BEFORE they reach these methods; trusted built-in calls skip
  // it. These plain (non-reactive) accumulators are what the bridge drains on its tick —
  // Svelte-5 runes tell *components* what changed but hand a plain consumer nothing, so
  // the fold/command path records touched ids here explicitly.
  //
  // `dirty` = ids touched since the bridge last drained (drives per-unit `state` deltas).
  private dirty = new Set<string>();
  // `logTail` = per-unit seq-tagged log lines, for `log-append` since a plugin's cursor.
  private logTail: Record<string, LogDelta[]> = {};

  // A REAL-mode launch the host has staged but not yet confirmed. While set, the host
  // overlay (Lane O) shows a confirm modal; nothing is sent to the daemon until the human
  // confirms. The store stays the single source of fleet state, so the modal is driven
  // purely off this field — independent of which view/plugin is mounted.
  pendingLaunch = $state<CreateReq | null>(null);

  // ── derived projections ────────────────────────────────────────────────────
  readonly selected = $derived(this.selectedId ? this.units[this.selectedId] : null);
  readonly list = $derived(this.order.map((id) => this.units[id]).filter(Boolean));
  readonly activeCount = $derived(this.list.filter((u) => !isTerminal(u.phase)).length);
  readonly totalBurn = $derived(this.list.reduce((s, u) => s + u.cost, 0));

  /**
   * The first unit (in fleet order) parked in `awaiting_oracle_approval`, or null.
   * This is the host overlay's trigger (Lane O): when non-null, the host pops a
   * focus-stealing Oracle Approval modal regardless of the active view/plugin. Driven
   * purely by host-owned fleet state, so a plugin cannot suppress or satisfy it.
   */
  readonly awaitingApproval = $derived(
    this.list.find((u) => u.phase === 'awaiting_oracle_approval') ?? null,
  );

  /**
   * `api` is the daemon client and `token` resolves the daemon token; tests pass
   * `src/lib/testkit/fakeApi` and a function that returns a fixed value.
   */
  constructor(
    api: FleetApi = defaultApi,
    token: () => Promise<string | null> = () => loadToken(),
  ) {
    this.api = api;
    this.token = token;
  }

  /** All current units as `GET /units`-shaped rows (seeds the Dashboard's fleet lane). */
  snapshots(): UnitSummary[] {
    return this.list.map((u) => ({
      unit_id: u.id,
      phase: u.phase,
      cost: u.cost,
      usd_cap: u.usdCap,
      tier: u.tier,
      task: u.task,
      last_seq: u.lastSeq,
    }));
  }

  /**
   * Load the token, repopulate the fleet from the daemon and (re)connect a stream to each
   * unit. Idempotent: safe to call from `onMount` even if a previous mount already
   * connected — existing units/streams are left untouched. Without a token it makes no
   * request. While the daemon is unreachable it tries again by itself, with a growing wait.
   */
  async reconnect(): Promise<void> {
    this.clearConnectTimer();
    const token = await this.token();
    if (!token) {
      this.connection = 'no_token';
      return;
    }
    let rows: UnitSummary[];
    try {
      rows = await this.api.listUnits();
    } catch (error) {
      const failure = error instanceof ApiFailure ? error : null;
      if (failure?.code === 'no_token') {
        this.connection = 'no_token';
      } else if (failure?.status === 401) {
        // A token the daemon refuses will be refused again: wait for a person.
        this.connection = 'refused';
      } else {
        this.connection = 'unreachable';
        this.scheduleConnect();
      }
      return;
    }
    this.connection = 'connected';
    this.connectAttempts = 0;
    for (const s of rows) {
      this.knownSeq[s.unit_id] = Math.max(this.knownSeq[s.unit_id] ?? 0, s.last_seq);
      if (!this.units[s.unit_id]) {
        this.units[s.unit_id] = fromSummary(s);
        this.order = [...this.order, s.unit_id];
        this.markDirty(s.unit_id);
      }
      this.ensureStream(s.unit_id);
    }
    if (!this.selectedId && this.order.length) this.selectedId = this.order[0];
  }

  private scheduleConnect(): void {
    if (!this.started) return; // only a started store keeps itself connected
    const wait = backoff(CONNECT_RETRY_BASE_MS, CONNECT_RETRY_CAP_MS, this.connectAttempts);
    this.connectAttempts += 1;
    this.connectTimer = setTimeout(() => {
      this.connectTimer = null;
      void this.reconnect();
    }, wait);
  }

  private clearConnectTimer(): void {
    if (this.connectTimer !== null) clearTimeout(this.connectTimer);
    this.connectTimer = null;
  }

  /**
   * Open the WS stream for a unit exactly once. Both `reconnect` (summary repopulate)
   * and `launch` (optimistic new unit) route through here so a bridge `launch` racing a
   * concurrent `reconnect()` can never open a second socket for the same unit — the
   * single-socket half of the single-sink guarantee.
   */
  private ensureStream(id: string): void {
    if (this.streams[id] || this.streamTimers[id]) return;
    this.openUnitStream(id);
  }

  /** Open one unit's stream from the last record folded, and reopen it when it closes. */
  private openUnitStream(id: string): void {
    const unit = this.units[id];
    if (!unit) return;
    let delivered = false;
    try {
      this.streams[id] = this.api.openStream(id, unit.lastSeq, {
        onEvent: (e) => {
          delivered = true;
          this.onEvt(id, e);
        },
        onClose: () => this.onStreamClosed(id, delivered),
      });
    } catch {
      // No token: nothing was opened. `reconnect()` opens it once there is one.
      delete this.streams[id];
    }
  }

  /**
   * A stream closed that this store did not close. The daemon closes a stream after the
   * replay when no run holds the unit, so a close is normal; what matters is whether there
   * is anything left to read. A finished unit whose log has been read to the end is left
   * alone. Anything else is reopened from the last record folded, after a wait that grows
   * while attempts bring nothing and resets as soon as one brings a record.
   */
  private onStreamClosed(id: string, delivered: boolean): void {
    delete this.streams[id];
    if (!this.started) return;
    const unit = this.units[id];
    if (!unit) return;
    const logComplete = unit.lastSeq >= (this.knownSeq[id] ?? 0);
    if (isTerminal(unit.phase) && logComplete) return;
    const attempt = delivered ? 0 : (this.streamAttempts[id] ?? 0) + 1;
    this.streamAttempts[id] = attempt;
    const wait = backoff(STREAM_RETRY_BASE_MS, STREAM_RETRY_CAP_MS, attempt);
    this.streamTimers[id] = setTimeout(() => {
      delete this.streamTimers[id];
      if (this.started && !this.streams[id]) this.openUnitStream(id);
    }, wait);
  }

  /** Record that a unit changed so the plugin bridge picks it up on its next drain. */
  private markDirty(id: string): void {
    this.dirty.add(id);
  }

  /**
   * Drain the set of unit ids touched since the last call (the bridge invokes this on its
   * ~60ms tick to build per-unit `state` deltas). Returns the ids and clears the set.
   */
  drainDirty(): string[] {
    const ids = [...this.dirty];
    this.dirty.clear();
    return ids;
  }

  /**
   * Seq-tagged log lines for a unit with `seq > sinceSeq` — the bridge's `log-append`
   * source, diffed against the plugin's per-unit cursor. Bounded by `LOG_TAIL_CAP`.
   */
  logsSince(id: string, sinceSeq: number): LogDelta[] {
    const tail = this.logTail[id];
    if (!tail) return [];
    return sinceSeq <= 0 ? tail.slice() : tail.filter((l) => l.seq > sinceSeq);
  }

  /** First-mount entry point: connect once. `dispose()` makes it callable again. */
  start(): void {
    if (this.started) return;
    this.started = true;
    void this.reconnect();
  }

  /** Fold one stream event into its unit; broadcast `phase_changed` to subscribers. */
  onEvt(id: string, e: EventEnvelopeDto): void {
    const prev = this.units[id];
    if (!prev) return;
    if (e.seq <= prev.lastSeq) return; // dedup replayed/overlapping frames
    const next = fold({ ...prev }, e.event);
    next.lastSeq = e.seq;
    this.units[id] = next;
    this.markDirty(id);
    if (e.event.type === 'log') {
      const tail = (this.logTail[id] ??= []);
      tail.push({ seq: e.seq, stream: e.event.stream, line: e.event.line });
      if (tail.length > LOG_TAIL_CAP) tail.splice(0, tail.length - LOG_TAIL_CAP);
    }
    if (e.event.type === 'phase_changed') this.emitPhase(id, e.event);
  }

  private emitPhase(id: string, ev: Extract<UnitEventDto, { type: 'phase_changed' }>): void {
    const u = this.units[id];
    if (!u) return;
    for (const cb of this.phaseListeners) cb(id, ev.to, u.task, u.tier);
  }

  /**
   * Subscribe to live `phase_changed` events (the Dashboard's `onFleetPhase` seam).
   * Returns an unsubscribe. The board advances its fleet cards off this — no second WS.
   */
  onPhase(cb: PhaseListener): () => void {
    this.phaseListeners.add(cb);
    return () => this.phaseListeners.delete(cb);
  }

  /** Seed a tile for a unit the daemon just created, open its stream, select it. */
  private adopt(id: string, task: string, tier: string): void {
    if (!this.units[id]) {
      this.units[id] = newUnit(id, task, tier);
      this.order = [id, ...this.order];
      this.markDirty(id);
    }
    this.selectedId = id;
    this.ensureStream(id);
  }

  /**
   * Launch a new mission, optimistically seed its tile, open its stream, select it. The
   * single command sink for new units: the ops grid calls it directly (trusted); the
   * plugin bridge calls it only AFTER its command policy passes. Guards the optimistic
   * insert against a concurrent `reconnect()` that already materialised this id from a
   * summary, so a bridge `launch` racing a reconnect yields exactly one unit + one
   * socket (`ensureStream` enforces the socket half).
   */
  async launch(req: CreateReq): Promise<string> {
    const { unit_id } = await this.api.createMission(req);
    this.adopt(unit_id, req.task, req.tier.toUpperCase());
    return unit_id;
  }

  /**
   * Start a unit from a signed spec. `tier` is the signed spec's tier, for the tile only:
   * the request cannot name one. A refusal rejects with the `ApiFailure` the daemon sent,
   * and no tile is added.
   */
  async dispatch(req: MissionRequest, tier: TierDto | null): Promise<MissionResponse> {
    const response = await this.api.createMission(req);
    this.adopt(response.unit_id, req.work_item, (tier ?? '').toUpperCase());
    return response;
  }

  /**
   * Stage a REAL-mode launch for human confirmation instead of firing it immediately.
   * Sets `pendingLaunch`, which the host overlay (Lane O) renders as a confirm modal.
   * DEMO launches don't need confirmation — call `launch()` directly for those.
   */
  requestRealLaunch(req: CreateReq): void {
    this.pendingLaunch = req;
  }

  /**
   * Confirm the staged REAL launch: actually create the mission and clear the pending
   * state. Returns the new unit id, or null if nothing was staged. Clears
   * `pendingLaunch` even if the launch throws, so a failed confirm closes the modal and
   * surfaces the error through the normal launch path.
   */
  async confirmLaunch(): Promise<string | null> {
    const req = this.pendingLaunch;
    if (!req) return null;
    this.pendingLaunch = null;
    return this.launch(req);
  }

  /** Dismiss a staged REAL launch without sending anything to the daemon. */
  cancelLaunch(): void {
    this.pendingLaunch = null;
  }

  /**
   * Send a control command to a unit (defaults to the selected one). Resolves to what the
   * daemon said, or to undefined when there is no unit to send it to.
   */
  async cmd(name: CommandName, unitId = this.selectedId): Promise<CommandOutcome | undefined> {
    if (!unitId) return undefined;
    return this.api.sendCommand(unitId, newCommand(name));
  }

  select(id: string): void {
    this.selectedId = id;
  }

  /**
   * Close every stream and cancel every pending retry. Units and selection are kept, and
   * `start()` connects again: a remount picks up where the unmount left off.
   */
  dispose(): void {
    this.started = false;
    this.clearConnectTimer();
    for (const timer of Object.values(this.streamTimers)) clearTimeout(timer);
    this.streamTimers = {};
    for (const stream of Object.values(this.streams)) stream.close();
    this.streams = {};
  }
}

/**
 * The one shared instance. Importing this module anywhere yields the same store, so
 * the Fleet view and the Project Dashboard project a single source of fleet truth and
 * switching between them preserves units, sockets, and selection.
 */
export const fleet = new FleetStore();
````

- [ ] **Step 6: Replace `src/lib/store.sink.svelte.test.ts`**

It mocked `./api` function by function; it now uses the fake. The four tests assert what they
always did.

````ts
import { describe, it, expect } from 'vitest';
import { FleetStore } from './store.svelte';
import { createFakeApi, TEST_TOKEN, type FakeFleetApi } from './testkit/fakeApi';
import type { EventEnvelopeDto as Envelope } from './generated/fleet-api';

// Lane V: the store is the SINGLE command sink. These tests exercise the paths that reach
// the daemon (`launch`/`reconnect`) against the in-memory fake — jsdom has no fleetd. The
// load-bearing assertion is the single-sink regression: a bridge `launch` concurrent with
// a `reconnect()` must yield exactly ONE unit and ONE socket (no duplicate-unit /
// double-socket, which a second optimistic writer would otherwise cause).

const req = { task: 'do the thing', tier: 't1' as const, mode: 'demo' as const, min_review_rounds: 2 };

function evt(unitId: string, seq: number, event: Envelope['event']): Envelope {
  return { unit_id: unitId, seq, event };
}

function store(fake: FakeFleetApi): FleetStore {
  return new FleetStore(fake, async () => TEST_TOKEN);
}

const opened = (fake: FakeFleetApi) => fake.calls.filter((c) => c.method === 'openStream');

describe('FleetStore single command sink', () => {
  it('a bridge launch concurrent with reconnect yields exactly one unit + one socket', async () => {
    // The daemon's list already names the unit `launch` is about to be handed: the fake
    // numbers its first unit u1.
    const fake = createFakeApi({
      units: [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'do the thing', last_seq: 0 }],
    });
    const s = store(fake);

    // Fire both writers at once — the exact interleaving must not matter.
    await Promise.all([s.launch(req), s.reconnect()]);

    expect(s.order.filter((id) => id === 'u1')).toHaveLength(1);
    expect(Object.keys(s.units)).toEqual(['u1']);
    expect(opened(fake)).toHaveLength(1);
  });

  it('reconnect and a later launch of the same id still open only one socket', async () => {
    const fake = createFakeApi({
      units: [{ unit_id: 'u1', phase: 'queued', cost: 0, usd_cap: 5, tier: 't1', task: 't', last_seq: 0 }],
    });
    const s = store(fake);
    await s.reconnect();
    expect(opened(fake)).toHaveLength(1);

    await s.launch(req); // the daemon hands back an already-known id
    expect(s.order.filter((id) => id === 'u1')).toHaveLength(1);
    expect(opened(fake)).toHaveLength(1); // no second socket
  });
});

describe('FleetStore dirty-set accumulator (bridge drain seam)', () => {
  it('records touched ids on fold and clears them on drain', async () => {
    const s = store(createFakeApi());
    await s.launch(req); // optimistic insert marks dirty
    expect(s.drainDirty()).toEqual(['u1']);
    expect(s.drainDirty()).toEqual([]); // drained set is now empty

    s.onEvt('u1', evt('u1', 1, { type: 'phase_changed', from: 'queued', to: 'building' }));
    s.onEvt('u1', evt('u1', 2, { type: 'metric', tokens_in: 1, tokens_out: 2, cost_usd: 0.1, elapsed_ms: 1 }));
    expect(s.drainDirty()).toEqual(['u1']); // deduped to the single touched id

    // A deduped/replayed frame (seq <= lastSeq) does not touch the unit.
    s.onEvt('u1', evt('u1', 2, { type: 'log', stream: 'agent', line: 'x' }));
    expect(s.drainDirty()).toEqual([]);
  });
});

describe('FleetStore seq-tagged log tail (log-append source)', () => {
  it('returns only lines after the cursor, with seq preserved', async () => {
    const s = store(createFakeApi());
    await s.launch(req);
    s.onEvt('u1', evt('u1', 1, { type: 'log', stream: 'agent', line: 'first' }));
    s.onEvt('u1', evt('u1', 2, { type: 'log', stream: 'check', line: 'second' }));

    expect(s.logsSince('u1', 0)).toEqual([
      { seq: 1, stream: 'agent', line: 'first' },
      { seq: 2, stream: 'check', line: 'second' },
    ]);
    expect(s.logsSince('u1', 1)).toEqual([{ seq: 2, stream: 'check', line: 'second' }]);
    expect(s.logsSince('u1', 2)).toEqual([]);
    expect(s.logsSince('unknown', 0)).toEqual([]);
  });
});
````

- [ ] **Step 7: Move the remaining imports off `types.ts`**

`src/lib/bridge.ts`:

````diff
--- a/cockpit/ui/src/lib/bridge.ts
+++ b/cockpit/ui/src/lib/bridge.ts
@@ -16,9 +16,8 @@
 //   • `PluginSession` — everything that happens over an established MessagePort,
 //   • `PluginBridge` — the window `plugin-hello` → `init`+port dance around a `PluginSession`.
 
-import type { CreateReq } from './api';
+import type { CommandName, CreateReq } from './api';
 import type { Unit } from './fleet';
-import type { CommandName } from './types';
 import type { FleetStore, LogDelta } from './store.svelte';
 
 export const PROTOCOL_VERSION = 1 as const;
@@ -98,7 +97,7 @@
 }
 
 /** `UnitLite` = a unit minus its (heavy, untagged) `log`; `history` is capped by the bridge. */
-export type UnitLite = Omit<Unit, 'log'>;
+export type UnitLite = Omit<Unit, 'log' | 'rail' | 'pauseReason' | 'pausedFrom'>;
 
 export function toUnitLite(u: Unit, historyCap = HISTORY_CAP): UnitLite {
   return {
@@ -271,12 +270,11 @@
     unit: (id) => store.units[id],
     drainDirty: () => store.drainDirty(),
     logsSince: (id, since) => store.logsSince(id, since),
-    degraded: () => {
-      const d = store.daemon;
-      return !d || !d.docker || !d.anthropic_key;
+    degraded: () => store.connection !== 'connected',
+    launch: (req) => store.launch(req),
+    command: async (id, name) => {
+      await store.cmd(name, id);
     },
-    launch: (req) => store.launch(req),
-    command: (id, name) => store.cmd(name, id),
     requestRealLaunch: (req) => store.requestRealLaunch(req),
   };
 }
````

`src/lib/bridge.test.ts`:

````diff
--- a/cockpit/ui/src/lib/bridge.test.ts
+++ b/cockpit/ui/src/lib/bridge.test.ts
@@ -14,8 +14,7 @@
   type UnitLite,
 } from './bridge';
 import { newUnit, type Unit } from './fleet';
-import type { CreateReq } from './api';
-import type { CommandName } from './types';
+import type { CommandName, CreateReq } from './api';
 
 // ── a minimal in-memory BridgeHost fake ───────────────────────────────────────
 interface HostBag {
````

`src/lib/store.svelte.test.ts`:

````diff
--- a/cockpit/ui/src/lib/store.svelte.test.ts
+++ b/cockpit/ui/src/lib/store.svelte.test.ts
@@ -2,7 +2,7 @@
 import { flushSync } from 'svelte';
 import { FleetStore } from './store.svelte';
 import { newUnit } from './fleet';
-import type { Envelope } from './types';
+import type { EventEnvelopeDto as Envelope } from './generated/fleet-api';
 
 // Seed a unit directly into a fresh store (bypasses the network `reconnect`/`launch`
 // paths, which we don't exercise here — jsdom has no fleetd).
````

`src/lib/store.overlay.svelte.test.ts`:

````diff
--- a/cockpit/ui/src/lib/store.overlay.svelte.test.ts
+++ b/cockpit/ui/src/lib/store.overlay.svelte.test.ts
@@ -2,7 +2,7 @@
 import { flushSync } from 'svelte';
 import { FleetStore } from './store.svelte';
 import { newUnit } from './fleet';
-import type { Envelope } from './types';
+import type { EventEnvelopeDto as Envelope } from './generated/fleet-api';
 import type { CreateReq } from './api';
 
 // Lane O: the host-overlay trigger lives entirely in host-owned fleet state, so these
````

`src/lib/dashboard/adapters/fleet.ts`:

````diff
--- a/cockpit/ui/src/lib/dashboard/adapters/fleet.ts
+++ b/cockpit/ui/src/lib/dashboard/adapters/fleet.ts
@@ -6,13 +6,14 @@
 // mission unit (R3 fix #1).
 
 import type { ProjectCard, Source, StageOverride, BlockedInfo } from '../model';
-import type { Phase, Snapshot } from '../../types';
+import type { PhaseDto, UnitSummary } from '../../generated/fleet-api';
+import { asPhase } from '../../fleet';
 import { resolveStage, applyOverride } from '../stage';
 
 export const FLEET_SOURCE: Source = 'fleet';
 
 // §6.3 mapping table — fleet mission phase → canonical pipeline stage.
-function pipelineFor(phase: Phase): {
+function pipelineFor(phase: PhaseDto): {
   stage: 'Plan' | 'Spec' | 'Build' | 'Review' | 'Ship' | 'Archived' | null;
   gate: boolean;
   terminal: boolean;
@@ -47,7 +48,7 @@
 }
 
 /** Which human gate a blocked phase represents (drives `blocked.gate`/action). */
-function gateFor(phase: Phase): { gate: 'oracle-approval' | 'manual'; action: string } {
+function gateFor(phase: PhaseDto): { gate: 'oracle-approval' | 'manual'; action: string } {
   if (phase === 'awaiting_oracle_approval') {
     return { gate: 'oracle-approval', action: 'Approve the frozen oracle test set' };
   }
@@ -56,7 +57,7 @@
 
 export interface FleetUnitState {
   id: string;
-  phase: Phase;
+  phase: PhaseDto;
   task: string;
   tier: string;
 }
@@ -106,8 +107,8 @@
 }
 
 /** Build cards from a snapshot list (`GET /units`) on cockpit load. */
-export function fleetCardsFromSnapshots(snaps: Snapshot[], opts: FleetAdapterOpts = {}): ProjectCard[] {
+export function fleetCardsFromSnapshots(snaps: UnitSummary[], opts: FleetAdapterOpts = {}): ProjectCard[] {
   return snaps.map((s) =>
-    fleetCard({ id: s.unit_id, phase: s.phase, task: s.task, tier: (s.tier ?? '').toUpperCase() }, opts),
+    fleetCard({ id: s.unit_id, phase: asPhase(s.phase), task: s.task, tier: (s.tier ?? '').toUpperCase() }, opts),
   );
 }
````

`src/lib/dashboard/adapters/adapters.test.ts`:

````diff
--- a/cockpit/ui/src/lib/dashboard/adapters/adapters.test.ts
+++ b/cockpit/ui/src/lib/dashboard/adapters/adapters.test.ts
@@ -3,7 +3,7 @@
 import { audienceCards, type AudienceReader } from './audience';
 import { fleetCard, fleetCardsFromSnapshots } from './fleet';
 import { appPluginCard, appPluginCards } from './appPlugin';
-import type { Snapshot } from '../../types';
+import type { UnitSummary } from '../../generated/fleet-api';
 
 const NOW = () => new Date('2026-06-09T12:00:00Z');
 
@@ -184,7 +184,7 @@
   });
 
   it('builds cards from snapshots', () => {
-    const snaps: Snapshot[] = [
+    const snaps: UnitSummary[] = [
       { unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'build x', last_seq: 0 },
     ];
     const cards = fleetCardsFromSnapshots(snaps, { now: NOW });
````

`src/lib/dashboard/store.ts`:

````diff
--- a/cockpit/ui/src/lib/dashboard/store.ts
+++ b/cockpit/ui/src/lib/dashboard/store.ts
@@ -8,7 +8,7 @@
 
 import type { ProjectCard, Source, Stage, StageOverride } from './model';
 import { STAGE_ORDINAL, isPipelineStage } from './model';
-import type { Phase, Snapshot } from '../types';
+import type { PhaseDto, UnitSummary } from '../generated/fleet-api';
 import { halyardCards, type HalyardReader } from './adapters/halyard';
 import { audienceCards, type AudienceReader } from './adapters/audience';
 import { feedbackCards, type FeedbackReader } from './adapters/feedback';
@@ -101,7 +101,7 @@
 /** Seed fleet cards from `GET /units` snapshots on load. */
 export function seedFleet(
   state: BoardState,
-  snaps: Snapshot[],
+  snaps: UnitSummary[],
   overrides: Record<string, StageOverride> = {},
   now: () => Date = () => new Date(),
 ): BoardState {
@@ -115,7 +115,7 @@
 export function applyFleetPhase(
   state: BoardState,
   unitId: string,
-  to: Phase,
+  to: PhaseDto,
   fallback: { task: string; tier: string },
   overrides: Record<string, StageOverride> = {},
   now: () => Date = () => new Date(),
````

`src/lib/dashboard/store.test.ts`:

````diff
--- a/cockpit/ui/src/lib/dashboard/store.test.ts
+++ b/cockpit/ui/src/lib/dashboard/store.test.ts
@@ -18,7 +18,7 @@
 import type { AudienceReader } from './adapters/audience';
 import type { FeedbackReader, TelltaleIssue } from './adapters/feedback';
 import type { LocalReader } from './adapters/local';
-import type { Snapshot } from '../types';
+import type { UnitSummary } from '../generated/fleet-api';
 
 const NOW = () => new Date('2026-06-09T12:00:00Z');
 const NOW_MS = Date.parse('2026-06-09T12:00:00Z');
@@ -122,7 +122,7 @@
 describe('live fleet phase advance (§6.3 push)', () => {
   it('a phase_changed event advances a mission card live', () => {
     let board = newBoard();
-    const snaps: Snapshot[] = [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'ship it', last_seq: 0 }];
+    const snaps: UnitSummary[] = [{ unit_id: 'u1', phase: 'building', cost: 0, usd_cap: 5, tier: 't1', task: 'ship it', last_seq: 0 }];
     board = seedFleet(board, snaps, {}, NOW);
     expect(board.cards['fleet:u1'].stage).toBe('Build');
 
````

`src/views/Dashboard.svelte`:

````diff
--- a/cockpit/ui/src/views/Dashboard.svelte
+++ b/cockpit/ui/src/views/Dashboard.svelte
@@ -24,7 +24,7 @@
   import type { AudienceReader } from '../lib/dashboard/adapters/audience';
   import type { FeedbackReader } from '../lib/dashboard/adapters/feedback';
   import type { LocalReader } from '../lib/dashboard/adapters/local';
-  import type { Snapshot, Phase } from '../lib/types';
+  import type { PhaseDto, UnitSummary } from '../lib/generated/fleet-api';
   import type { PluginUnit } from '../lib/dashboard/adapters/appPlugin';
 
   // Injectable seams so the board renders/tests in isolation (no Tauri required).
@@ -34,9 +34,9 @@
     audienceReader?: AudienceReader;
     feedbackReader?: FeedbackReader;
     localReader?: LocalReader;
-    fleetSnapshots?: Snapshot[];
+    fleetSnapshots?: UnitSummary[];
     /** Subscribe to fleet `phase_changed`; returns an unsubscribe. */
-    onFleetPhase?: (cb: (unitId: string, to: Phase, task: string, tier: string) => void) => () => void;
+    onFleetPhase?: (cb: (unitId: string, to: PhaseDto, task: string, tier: string) => void) => () => void;
     /** Subscribe to `plugin://state`; returns an unsubscribe. */
     onPluginState?: (cb: (unit: PluginUnit) => void) => () => void;
     pollIntervalMs?: number;
````

- [ ] **Step 8: Delete `src/lib/types.ts`**

```
git rm cockpit/ui/src/lib/types.ts
```

- [ ] **Step 9: Run the tests that do not render `App.svelte`**

```
npx vitest run src/lib src/views
```

Expected: every file under `src/lib` and `src/views` passes, 225 tests in 24 files. (`App.svelte` still
imports `./lib/types` and reads `fleet.daemon`; the two existing `src/App.*.test.ts` files are red
until Task 6.)

### Task 6: The seams in `App.svelte`

**Files:**
- Create (stubs): `src/lib/NotConnected.svelte`, `src/lib/harness/HarnessBadge.svelte`,
  `src/lib/unit/StageRail.svelte`, `src/lib/unit/ReverifyButton.svelte`,
  `src/lib/unit/FactoryPanel.svelte`
- Create: `src/App.connection.test.ts`
- Modify: `src/App.svelte`, `src/App.overlay.test.ts`, `src/App.appPlugin.test.ts`

**Produces:** five mount points, each a component with its final props:

| Component | Owner after wave 0 | Props | Mounted |
|---|---|---|---|
| `NotConnected` | UI-TOKEN | `connection: Connection`, `baseUrl: string`, `onretry: () => void` | Under the header, in every view, when `fleet.connection` is `no_token`, `refused` or `unreachable` |
| `HarnessBadge` | UI-HARNESS | `api: FleetApi`, `connected: boolean` | In the header's stats, where the Docker and API-key badges were |
| `ReverifyButton` | UI-RAIL | `unit: Unit`, `onreverify: () => Promise<CommandOutcome \| undefined>` | Under the four command buttons of the selected unit |
| `StageRail` | UI-RAIL | `rail: RailState` | Under that, above the alerts |
| `FactoryPanel` | UI-RAIL | `api: FleetApi`, `ondispatch: (request: MissionRequest, tier: TierDto \| null) => Promise<MissionResponse>` | In the left panel, under a new tab `SIGNED SPEC`; the existing task form moves under the tab `LEGACY TASK` |

What to know:

- The Docker and API-key badges go in this task, with their two CSS rules. Until lane
  UI-HARNESS merges, the header shows nothing in their place.
- The left panel opens on `SIGNED SPEC`. The legacy form is unchanged, one tab away, and stays
  until the driver path is deleted in milestone 2.
- The connection chip (`◉ 127.0.0.1:8787`) gets `data-connection` and turns red when not
  connected. It used to be green whatever happened.
- `App.overlay.test.ts` and `App.appPlugin.test.ts` spied on `api.health` and `api.listUnits`
  to keep `fleet.start()` quiet. They no longer need to: in jsdom there is no shell and no
  development token, so the store stops at `no_token` and asks for nothing.

- [ ] **Step 1: Create the five stubs**

`src/lib/NotConnected.svelte`:

````svelte
<script lang="ts">
  // WAVE 0 STUB. Lane UI-TOKEN replaces the markup and the styles; the props do not change.
  import type { Connection } from './store.svelte';

  interface Props {
    /** Never `connected` or `starting`: the parent renders this only when not connected. */
    connection: Connection;
    /** The daemon's address, for the message. */
    baseUrl: string;
    /** Try again. */
    onretry: () => void;
  }

  let { connection, baseUrl, onretry }: Props = $props();
</script>

<div role="alert" data-testid="not-connected" data-reason={connection}>
  NOT CONNECTED to {baseUrl}
  <button type="button" data-testid="not-connected-retry" onclick={onretry}>RETRY</button>
</div>
````

`src/lib/harness/HarnessBadge.svelte`:

````svelte
<script lang="ts">
  // WAVE 0 STUB. Lane UI-HARNESS replaces the markup and adds its own files beside this one;
  // the props do not change.
  import type { FleetApi } from '../api';

  interface Props {
    /** The daemon client: the badge reads `GET /harnesses` through it. */
    api: FleetApi;
    /** False while the page is not connected: the badge then makes no request. */
    connected: boolean;
  }

  let { api, connected }: Props = $props();
</script>

<span data-testid="harness-badge" hidden></span>
````

`src/lib/unit/StageRail.svelte`:

````svelte
<script lang="ts">
  // WAVE 0 STUB. Lane UI-RAIL replaces the markup; the props do not change.
  import type { RailState } from './rail';

  interface Props {
    rail: RailState;
  }

  let { rail }: Props = $props();
</script>

<div data-testid="stage-rail" hidden></div>
````

`src/lib/unit/ReverifyButton.svelte`:

````svelte
<script lang="ts">
  // WAVE 0 STUB. Lane UI-RAIL replaces the markup; the props do not change.
  import type { CommandOutcome } from '../api';
  import type { Unit } from '../fleet';

  interface Props {
    unit: Unit;
    /** Send `reverify` to this unit. Resolves to what the daemon said. */
    onreverify: () => Promise<CommandOutcome | undefined>;
  }

  let { unit, onreverify }: Props = $props();
</script>
````

`src/lib/unit/FactoryPanel.svelte`:

````svelte
<script lang="ts">
  // WAVE 0 STUB. Lane UI-RAIL replaces the markup and adds its own files beside this one;
  // the props do not change.
  import type { FleetApi } from '../api';
  import type { MissionRequest, MissionResponse, TierDto } from '../generated/fleet-api';

  interface Props {
    /** The daemon client: the panel reads specs, signatures and harnesses through it. */
    api: FleetApi;
    /**
     * Start a unit from a signed spec. `tier` is the signed spec's tier, for the tile.
     * Rejects with the `ApiFailure` the daemon sent when the request is refused.
     */
    ondispatch: (request: MissionRequest, tier: TierDto | null) => Promise<MissionResponse>;
  }

  let { api, ondispatch }: Props = $props();
</script>

<div data-testid="factory-panel">Sign and dispatch arrive with lane UI-RAIL.</div>
````

- [ ] **Step 2: Write the failing test**

Create `src/App.connection.test.ts`:

````ts
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte';
import App from './App.svelte';
import { fleet } from './lib/store.svelte';
import { api } from './lib/api';
import * as auth from './lib/auth';
import { TEST_TOKEN } from './lib/testkit/fakeApi';

// The page's connection to the daemon, as a person sees it: one "not connected" state, no
// request without a token, and the seams the milestone's lanes fill.

beforeEach(() => {
  fleet.dispose();
  fleet.units = {};
  fleet.order = [];
  fleet.selectedId = null;
  fleet.pendingLaunch = null;
  fleet.connection = 'starting';
});

afterEach(() => {
  cleanup();
  fleet.dispose();
  vi.restoreAllMocks();
});

describe('App without a token', () => {
  it('shows the not-connected state and asks the daemon for nothing', async () => {
    vi.spyOn(auth, 'loadToken').mockResolvedValue(null);
    const listUnits = vi.spyOn(api, 'listUnits');
    const listHarnesses = vi.spyOn(api, 'listHarnesses');
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('not-connected')).not.toBeNull());
    expect(getByTestId('not-connected').getAttribute('data-reason')).toBe('no_token');
    expect(getByTestId('conn').getAttribute('data-connection')).toBe('no_token');
    expect(listUnits).not.toHaveBeenCalled();
    expect(listHarnesses).not.toHaveBeenCalled();
  });

  it('RETRY asks for the token again and connects when there is one', async () => {
    const loadToken = vi.spyOn(auth, 'loadToken').mockResolvedValue(null);
    const listUnits = vi.spyOn(api, 'listUnits').mockResolvedValue([]);
    const { getByTestId, queryByTestId } = render(App);
    await waitFor(() => expect(getByTestId('not-connected')).not.toBeNull());

    loadToken.mockResolvedValue(TEST_TOKEN);
    await fireEvent.click(getByTestId('not-connected-retry'));
    await waitFor(() => expect(queryByTestId('not-connected')).toBeNull());
    expect(getByTestId('conn').getAttribute('data-connection')).toBe('connected');
    expect(listUnits).toHaveBeenCalledTimes(1);
  });
});

describe('App seams', () => {
  beforeEach(() => {
    vi.spyOn(auth, 'loadToken').mockResolvedValue(TEST_TOKEN);
    vi.spyOn(api, 'listUnits').mockResolvedValue([]);
    vi.spyOn(api, 'listHarnesses').mockResolvedValue([]);
  });

  it('opens on the signed-spec panel and keeps the legacy task form one tab away', async () => {
    const { getByTestId, queryByTestId, getByText } = render(App);
    await waitFor(() => expect(getByTestId('factory-panel')).not.toBeNull());
    expect(getByTestId('left-tab-factory').getAttribute('aria-selected')).toBe('true');
    expect(queryByTestId('not-connected')).toBeNull();

    await fireEvent.click(getByTestId('left-tab-legacy'));
    expect(queryByTestId('factory-panel')).toBeNull();
    expect(getByText('NEW MISSION')).not.toBeNull();
    expect(getByTestId('left-tab-legacy').getAttribute('aria-selected')).toBe('true');
  });

  it('mounts the harness badge in the header', async () => {
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('harness-badge')).not.toBeNull());
  });
});
````

- [ ] **Step 3: Run it and see it fail**

```
npx vitest run src/App.connection.test.ts
```

Expected: `Tests  4 failed (4)`: two with `Unable to find an element by:
[data-testid="not-connected"]`, one with `[data-testid="factory-panel"]`, one with
`[data-testid="harness-badge"]`.

- [ ] **Step 4: Change `src/App.svelte`**

````diff
--- a/cockpit/ui/src/App.svelte
+++ b/cockpit/ui/src/App.svelte
@@ -2,10 +2,14 @@
   import { onMount, tick } from 'svelte';
   import { invoke } from '@tauri-apps/api/core';
   import { listen } from '@tauri-apps/api/event';
-  import { FLEET_BASE } from './lib/api';
+  import { api, FLEET_BASE, type CommandName } from './lib/api';
   import { phaseClass, progress, isTerminal } from './lib/fleet';
-  import type { CommandName } from './lib/types';
   import { fleet } from './lib/store.svelte';
+  import NotConnected from './lib/NotConnected.svelte';
+  import HarnessBadge from './lib/harness/HarnessBadge.svelte';
+  import StageRail from './lib/unit/StageRail.svelte';
+  import ReverifyButton from './lib/unit/ReverifyButton.svelte';
+  import FactoryPanel from './lib/unit/FactoryPanel.svelte';
   import Switcher, { type ViewEntry } from './lib/Switcher.svelte';
   import Dashboard from './views/Dashboard.svelte';
   import ApprovalOverlay, { type ApprovalRequest } from './lib/ApprovalOverlay.svelte';
@@ -172,8 +176,15 @@
   const selected = $derived(fleet.selected);
   const activeCount = $derived(fleet.activeCount);
   const totalBurn = $derived(fleet.totalBurn);
-  const daemon = $derived(fleet.daemon);
+  const connection = $derived(fleet.connection);
+  // The one "not connected" state: no token, a refused token, or no answer.
+  const notConnected = $derived(
+    connection === 'no_token' || connection === 'refused' || connection === 'unreachable',
+  );
   const selectedId = $derived(fleet.selectedId);
+
+  // ── left panel: SIGNED SPEC (sign a spec, dispatch it) or LEGACY TASK (the task form) ──
+  let leftTab = $state<'factory' | 'legacy'>('factory');
 
   // ── new-mission form ─────────────────────────────────────────────
   let task = $state('Add a function sum(a, b) in src/index.js (module.exports.sum) returning a + b.');
@@ -341,18 +352,23 @@
         <div class="stat"><b class="disp">{activeCount}</b><span>ACTIVE</span></div>
         <div class="stat"><b class="disp">{list.length}</b><span>UNITS</span></div>
         <div class="stat"><b class="disp">${totalBurn.toFixed(2)}</b><span>BURN</span></div>
-        <div class="badges mono">
-          <span class="hb" class:ok={daemon?.docker} class:bad={daemon && !daemon.docker} title="Docker daemon">
-            ◉ DOCKER
-          </span>
-          <span class="hb" class:ok={daemon?.anthropic_key} class:bad={daemon && !daemon.anthropic_key} title="ANTHROPIC_API_KEY set">
-            ◉ KEY
-          </span>
+        <HarnessBadge {api} connected={connection === 'connected'} />
+        <div
+          class="conn mono"
+          class:down={notConnected}
+          title={FLEET_BASE}
+          data-testid="conn"
+          data-connection={connection}
+        >
+          ◉ {FLEET_BASE.replace(/^https?:\/\//, '')}
         </div>
-        <div class="conn mono" title={FLEET_BASE}>◉ {FLEET_BASE.replace(/^https?:\/\//, '')}</div>
       </div>
     {/if}
   </header>
+
+  {#if notConnected}
+    <NotConnected {connection} baseUrl={FLEET_BASE} onretry={() => void fleet.reconnect()} />
+  {/if}
 
   {#if activeViewPlugin}
     <!-- View-plugin: untrusted, sandboxed iframe (opaque "null" origin — allow-scripts only,
@@ -388,6 +404,25 @@
   <main class="grid">
     <!-- left: new mission -->
     <aside class="panel left">
+      <div class="seg" role="tablist" aria-label="How to start a unit">
+        <button
+          class="disp"
+          role="tab"
+          aria-selected={leftTab === 'factory'}
+          class:on={leftTab === 'factory'}
+          data-testid="left-tab-factory"
+          onclick={() => (leftTab = 'factory')}>SIGNED SPEC</button>
+        <button
+          class="disp"
+          role="tab"
+          aria-selected={leftTab === 'legacy'}
+          class:on={leftTab === 'legacy'}
+          data-testid="left-tab-legacy"
+          onclick={() => (leftTab = 'legacy')}>LEGACY TASK</button>
+      </div>
+      {#if leftTab === 'factory'}
+      <FactoryPanel {api} ondispatch={(request, tier) => fleet.dispatch(request, tier)} />
+      {:else}
       <h2 class="disp ph">NEW MISSION</h2>
       <label class="fld">
         <span>TASK</span>
@@ -425,6 +460,7 @@
       </button>
       {#if launchErr}<div class="err mono">{launchErr}</div>{/if}
       {#if mode === 'real'}<div class="warn mono">REAL mode needs ANTHROPIC_API_KEY + Docker; spends real $.</div>{/if}
+      {/if}
     </aside>
 
     <!-- center: fleet grid -->
@@ -481,6 +517,9 @@
           <button class="cmd warn" disabled={!canHalt} onclick={() => cmd('halt')}>HALT</button>
           <button class="cmd bad" disabled={isTerminal(selected.phase)} onclick={() => cmd('abandon')}>ABANDON</button>
         </div>
+        <ReverifyButton unit={selected} onreverify={() => fleet.cmd('reverify', selected.id)} />
+
+        <StageRail rail={selected.rail} />
 
         {#if selected.blocked}<div class="alert">⚠ {selected.blocked}</div>{/if}
         {#if selected.error}<div class="alert bad">✕ {selected.error}</div>{/if}
@@ -542,10 +581,7 @@
   .stat b { font-size: 18px; color: var(--cyan); font-weight: 700; }
   .stat span { font-size: 9px; letter-spacing: 1.5px; color: var(--text-faint); margin-top: 3px; }
   .conn { font-size: 11px; color: var(--green); letter-spacing: 0.5px; }
-  .badges { display: flex; gap: 8px; }
-  .hb { font-size: 10px; letter-spacing: 1px; color: var(--text-faint); border: 1px solid var(--line-2); padding: 2px 6px; border-radius: 2px; }
-  .hb.ok { color: var(--green); border-color: rgba(53, 208, 127, 0.4); }
-  .hb.bad { color: var(--red); border-color: rgba(255, 93, 108, 0.45); }
+  .conn.down { color: var(--red); }
 
   .grid { flex: 1; display: grid; grid-template-columns: 300px 1fr 360px; min-height: 0; }
 
````

- [ ] **Step 5: Change the two existing App tests**

`src/App.overlay.test.ts`:

````diff
--- a/cockpit/ui/src/App.overlay.test.ts
+++ b/cockpit/ui/src/App.overlay.test.ts
@@ -3,15 +3,15 @@
 import App from './App.svelte';
 import { fleet } from './lib/store.svelte';
 import { newUnit } from './lib/fleet';
-import * as api from './lib/api';
+import { api } from './lib/api';
 
 // App.overlay integration: the host overlay (Lane O) must pop off the shared `fleet`
 // store regardless of the active view, and freeze the rest of the app with `inert`.
 // We stub the network so App.onMount's fleet.start() is inert in jsdom.
 beforeEach(() => {
-  vi.spyOn(api, 'health').mockRejectedValue(new Error('no daemon in test'));
-  vi.spyOn(api, 'listUnits').mockResolvedValue([]);
-  vi.spyOn(api, 'sendCommand').mockResolvedValue(200);
+  // jsdom has no Tauri shell and no dev token, so the store stays not connected and makes
+  // no request; `sendCommand` is the one call these tests drive.
+  vi.spyOn(api, 'sendCommand').mockResolvedValue('accepted');
   // Reset shared-store state between tests (it's a module singleton).
   fleet.units = {};
   fleet.order = [];
@@ -55,7 +55,10 @@
     const { getByTestId } = render(App);
     await waitFor(() => expect(getByTestId('approval-approve')).not.toBeNull());
     await fireEvent.click(getByTestId('approval-approve'));
-    expect(api.sendCommand).toHaveBeenCalledWith('u7', 'approve_oracle');
+    expect(api.sendCommand).toHaveBeenCalledWith(
+      'u7',
+      expect.objectContaining({ command: 'approve_oracle' }),
+    );
   });
 
   it('wires REJECT to fleet.cmd(id, reject_oracle)', async () => {
@@ -63,7 +66,10 @@
     const { getByTestId } = render(App);
     await waitFor(() => expect(getByTestId('approval-reject')).not.toBeNull());
     await fireEvent.click(getByTestId('approval-reject'));
-    expect(api.sendCommand).toHaveBeenCalledWith('u8', 'reject_oracle');
+    expect(api.sendCommand).toHaveBeenCalledWith(
+      'u8',
+      expect.objectContaining({ command: 'reject_oracle' }),
+    );
   });
 
   it('shows the modal even while the Projects view is active (view-independent)', async () => {
````

`src/App.appPlugin.test.ts`:

````diff
--- a/cockpit/ui/src/App.appPlugin.test.ts
+++ b/cockpit/ui/src/App.appPlugin.test.ts
@@ -31,7 +31,6 @@
 import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte';
 import App from './App.svelte';
 import { fleet } from './lib/store.svelte';
-import * as api from './lib/api';
 
 const AUDIENCE = { id: 'audience', name: 'Audience', icon: '', url: 'http://localhost:3000' };
 const TAB = 'switch-app:audience';
@@ -57,9 +56,8 @@
     },
   );
 
-  // Stub the network so App.onMount's fleet.start() is inert in jsdom (as in App.overlay.test).
-  vi.spyOn(api, 'health').mockRejectedValue(new Error('no daemon in test'));
-  vi.spyOn(api, 'listUnits').mockResolvedValue([]);
+  // App.onMount's fleet.start() is inert here: `invoke` is mocked and hands back no token, so
+  // the store stays not connected and makes no request (as in App.overlay.test).
   fleet.units = {};
   fleet.order = [];
   fleet.selectedId = null;
````

- [ ] **Step 6: Run everything**

```
npm run check
npx vitest run
```

Expected: `check` exits 0 with no error and no warning; `Test Files  27 passed (27)`,
`Tests  236 passed (236)`.

- [ ] **Step 7: Commit**

```
git add -A cockpit/ui/src
git commit -m "feat(cockpit): a typed API client over the generated types; retire types.ts; mount points for the M1 lanes"
```

### Task 7: Verify, push, open the pull request

- [ ] Run every command of this part's **Verify** table and keep the output.
- [ ] `git push -u origin feat/m1-ui-types`
- [ ] `gh pr create -R adbarc92/command-center --base factory/m1 --head feat/m1-ui-types --title "feat(cockpit): generated API types, the typed client, and the M1 seams" --body-file <a file outside the worktree>`.
      The body says what changed, gives the real output of each Verify command, says that the
      desktop app shows "not connected" until lane UI-TOKEN merges, and lists anything the real
      schema changed against this plan. It closes no issue.

**Done when**

- [ ] The Verify table is green, with real output in the pull request.
- [ ] `src/lib/types.ts` does not exist and nothing imports it.
- [ ] `src/lib/generated/fleet-api.ts` is what `npm run gen:types` writes from the committed
      schema, and `npm run check` fails when it is not.
- [ ] The seven stubs exist with exactly the props and signatures printed above.

---

# Part 2 — The lanes

Every lane's brief starts with the rules of the program plan, section 2, and this plan's Global
Constraints. Each lane cuts its branch from `origin/factory/m1` after Part 1 has merged:

```
git -C D:/MajorProjects/INFRASTRUCTURE/command-center fetch origin
git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --no-track -b <branch> <worktree> origin/factory/m1
```

and ends with `git push -u origin <branch>` and
`gh pr create -R adbarc92/command-center --base factory/m1 --head <branch> --title "<title>" --body-file <file outside the worktree>`.
The body says what changed, how it was verified (the real output of every **Verify** command),
which issue it closes, and what it asks of the coordinator. No lane merges.

Lanes UI-TOKEN, UI-HARNESS and UI-RAIL can run at the same time: each needs only Part 1, and no
two of them own a file in common. In every lane the steps are the same and are not repeated
below:

- [ ] In `cockpit/ui`: `npm ci`, then `npx vitest run`. Expected: every file passes, with
      `Tests  236 passed (236)` if no other lane has merged yet. Write the count down: your
      **Verify** is stated relative to it. A failing test is a finding about what is already
      on the branch: stop and report.
- [ ] Create the test files this lane's **Behaviours** list, exactly as printed.
- [ ] Run the lane's test command and see the tests fail as its **Behaviours** section says.
      A test that fails for another reason, or passes when the section says it fails, is a
      finding: stop and report.
- [ ] Build the implementation, one behaviour at a time, until the tests pass. Commit after
      each behaviour with a conventional subject (`feat(cockpit): …`).
- [ ] Run **Verify**, push, open the pull request.

---

## Lane UI-TYPES

This lane is Part 1. It is listed here so that the four lanes of the program plan each have a
section.

**Owns, Reads, Worktree and branch, Needs, Blocks, Behaviours, Verify, Done when:** as Part 1
states them. Its behaviours are the 70 tests of `src/lib/api.test.ts`,
`src/lib/testkit/fakeApi.test.ts`, `src/lib/store.connection.svelte.test.ts` and
`src/App.connection.test.ts`, printed there in full, and the drift check of Task 2.

**After Part 1 has merged, this lane has one standing duty.** When `fleet-api`'s schema
changes (the control-plane plan's behaviour 32 fails and is re-blessed), the same pull request
runs `npm run gen:types` in `cockpit/ui` and commits the result, or `svelte-check + tsc` fails
on it. Whoever changes the schema regenerates; nobody edits the generated file.

---

## Lane UI-TOKEN

The shell mints the daemon's token, gives it to the daemon and to the page, and the page shows
one clear state when it has none. This lane, with lane CC-WIRE of the control-plane plan,
closes issue #91.

**Owns:** `src-tauri/src/token.rs` (new); `src-tauri/src/sidecar.rs`; in `src-tauri/src/lib.rs`
the three additions printed below and nothing else; `src/lib/auth.ts` and
`src/lib/auth.test.ts`; `src/lib/NotConnected.svelte` and `src/lib/NotConnected.test.ts`;
`src/lib/csp.test.ts`.

**Reads:** the control-plane plan, lane CC-API ("The token and the wrapper": what the daemon
accepts and where it looks for the token) and lane CC-WIRE ("The settings": `FLEETD_TOKEN`,
`CC_ADDR`, `FLEETD_ALLOWED_ORIGINS`); `src-tauri/src/sidecar.rs` (all of it);
`src-tauri/tests/tauri_command_threading.rs` (a new command must be `async`);
`src-tauri/tauri.conf.json` (the policy, read only); `src/lib/api.ts` and
`src/lib/store.svelte.ts` (how the token is used; read only); issue #91;
`docs/testing/PLAN.md` GAP-015.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-ui-token`, branch `feat/m1-ui-token`,
created with `git worktree add --no-track`, cut from `origin/factory/m1` after wave 0 merges.
Pull request against `factory/m1`.

**Needs:** Part 1 merged; request C3 (`getrandom`) on `factory/m1`.

**Blocks:** the page reaching the daemon at all once lane CC-WIRE has merged; the operator's
smoke.

### Behaviours

In the shell (`cargo test --lib`, in `cockpit/ui/src-tauri`):

1. A minted token is 64 lowercase hexadecimal characters: long enough for the daemon, and
   legal as a WebSocket subprotocol name. `token::tests::a_minted_token_is_64_lowercase_hex_characters`
2. Two tokens differ. `token::tests::two_minted_tokens_differ`
3. Encoding keeps leading zeroes. `token::tests::hex_encoding_keeps_leading_zeroes`
4. Formatting a token for debugging never shows it. `token::tests::debug_never_shows_the_value`
5. A line that contains the token is redacted. `token::tests::redact_removes_every_occurrence_and_leaves_other_text`
6. Only the webview labelled `main` may read the token. `token::tests::only_the_main_webview_may_read_the_token`
7. The token is read in exactly three places in the crate: the command that hands it to the
   page, the daemon's environment, and the health check's header.
   `token::tests::the_token_is_exposed_in_exactly_the_places_this_module_names`
8. The health check carries the token as a bearer header and not in its address.
   `sidecar::tests::the_health_check_carries_the_token_as_a_bearer_header`
9. A line of the daemon's output that contains the token is logged redacted.
   `sidecar::tests::a_daemon_line_that_contains_the_token_is_logged_redacted`
10. The daemon gets the token through its environment, as `FLEETD_TOKEN`, and the sidecar is
    given no arguments.
    `sidecar::tests::the_token_reaches_the_daemon_through_its_environment_and_never_its_arguments`
11. No logging call in the supervisor names the token.
    `sidecar::tests::nothing_in_this_module_logs_the_token_itself`

In the page (`npx vitest run`, in `cockpit/ui`):

12. The page asks the shell for the token with the `fleetd_token` command, once, and remembers
    it. `src/lib/auth.test.ts` › *the token from the shell* (three tests)
13. An answer that is not a usable token is not a token. *the token from the shell* ›
    `an answer that is … is not a token` (six cases)
14. With no shell, a development build uses `VITE_FLEETD_TOKEN`; a production build never
    does; a value that could not be sent on a stream is ignored; with neither source there is
    no token and no rejection. *the token from the development variable* (four tests)
15. A usable token is 32 or more characters, each legal in a WebSocket subprotocol name; and
    everything it accepts really can be offered to a `WebSocket`. *what counts as a usable
    token* (23 tests)
16. The token is kept in memory only. *where the token is kept* (three tests)
17. The "not connected" state says which of three things is wrong, names the development
    variable when there is no token, and offers one action. `src/lib/NotConnected.test.ts`
    (five tests)
18. The content-security policy allows the daemon's HTTP address and its WebSocket address,
    and nothing else. `src/lib/csp.test.ts` (four tests)

| Tests | Command | The failure to expect first |
|---|---|---|
| 1 to 7 | `cargo test --lib token` | Create `token.rs` with only the `#[cfg(test)] mod tests` block printed below, and add `mod token;` to `lib.rs`. The crate does not compile, with nine errors: `E0433` (use of undeclared type `DaemonToken`) six times, `E0425` (cannot find function `may_read_token`) twice, and one `E0282` that follows from them |
| 8 to 11 | `cargo test --lib sidecar` | With `token.rs` written, add only the `#[cfg(test)] mod tests` block to `sidecar.rs`. The crate does not compile, with seven errors: `E0425` for the functions `health_request` and `log_line`, the value `TOKEN_ENV` and the type `DaemonToken` (the module does not import them yet), `E0433` for `DaemonToken`, and one `E0282` that follows |
| 12 to 16 | `npx vitest run src/lib/auth.test.ts` | `Tests  35 failed \| 4 passed (39)`. Most read `TypeError: isUsableToken is not a function` or `tokenSource is not a function`; the three *from the shell* tests fail because the stub never asks the shell. The four that pass are the ones a stub with no token satisfies by accident: *is ignored when it could not be sent as a WebSocket subprotocol*, *with neither source there is no token…*, and the first two of *where the token is kept* |
| 17 | `npx vitest run src/lib/NotConnected.test.ts` | `Tests  4 failed \| 1 passed (5)`: the stub says `NOT CONNECTED to …` and none of the three reasons. The retry test passes against the stub |
| 18 | `npx vitest run src/lib/csp.test.ts` | **It passes from the start.** It is a pin, not a behaviour to build: it fails only when someone changes the address or the policy without the other |

`src-tauri/src/token.rs`, in full. The code above the test module is the implementation; write
the test module first.

````rust
//! The daemon's token: minted once per run of the shell, handed to `fleetd` in its
//! environment, and handed to the cockpit's own page through one command.
//!
//! The value lives in this process's memory and in the daemon's environment, and nowhere else:
//! it is never a command-line argument, never written to a file, and never written to a log.

use tauri::{State, Webview};

/// The environment variable `fleetd` reads its token from (`fleet_api::TOKEN_ENV`).
pub const TOKEN_ENV: &str = "FLEETD_TOKEN";

/// The label of the one webview that may read the token: the cockpit's own page.
/// App-plugin child webviews are labelled `app::<id>` and are refused.
pub const MAIN_WEBVIEW: &str = "main";

/// What is shown wherever the token would otherwise appear.
pub const REDACTED: &str = "***";

/// How many random bytes a token is made from. As hex that is 64 characters, twice the
/// 32 the daemon requires.
const TOKEN_BYTES: usize = 32;

/// The token every request to `fleetd` must carry. `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct DaemonToken(String);

impl std::fmt::Debug for DaemonToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DaemonToken(***)")
    }
}

impl DaemonToken {
    /// A new token from the operating system's random source, as lowercase hex.
    ///
    /// Hex and nothing else: the same value travels in an HTTP header and in a WebSocket
    /// subprotocol name, and a subprotocol name may not contain `/`, `=`, or most punctuation.
    pub fn mint() -> Result<Self, String> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|e| format!("no random source: {e}"))?;
        Ok(Self::from_bytes(&bytes))
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            hex.push_str(&format!("{byte:02x}"));
        }
        DaemonToken(hex)
    }

    /// The value itself. Call this only to hand the token to the daemon or to the page.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `line` with every occurrence of the token replaced by [`REDACTED`].
    pub fn redact(&self, line: &str) -> String {
        line.replace(&self.0, REDACTED)
    }
}

/// True for the one webview that may read the token.
pub fn may_read_token(webview_label: &str) -> bool {
    webview_label == MAIN_WEBVIEW
}

/// Hand the token to the cockpit's page. Refused for any other webview.
///
/// `async` so that it runs off the main event loop (`tests/tauri_command_threading.rs`).
#[tauri::command]
pub async fn fleetd_token(
    webview: Webview,
    token: State<'_, DaemonToken>,
) -> Result<String, String> {
    if !may_read_token(webview.label()) {
        return Err("this webview may not read the daemon token".into());
    }
    Ok(token.expose().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_token_is_64_lowercase_hex_characters() {
        let token = DaemonToken::mint().unwrap();
        assert_eq!(token.expose().len(), 64);
        assert!(token
            .expose()
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn two_minted_tokens_differ() {
        let a = DaemonToken::mint().unwrap();
        let b = DaemonToken::mint().unwrap();
        assert_ne!(a.expose(), b.expose());
    }

    #[test]
    fn hex_encoding_keeps_leading_zeroes() {
        let token = DaemonToken::from_bytes(&[0x00, 0x0f, 0xa0, 0xff]);
        assert_eq!(token.expose(), "000fa0ff");
    }

    #[test]
    fn debug_never_shows_the_value() {
        let token = DaemonToken::from_bytes(&[0xab; 32]);
        let shown = format!("{token:?}");
        assert_eq!(shown, "DaemonToken(***)");
        assert!(!shown.contains(token.expose()));
    }

    #[test]
    fn redact_removes_every_occurrence_and_leaves_other_text() {
        let token = DaemonToken::from_bytes(&[0xcd; 32]);
        let value = token.expose().to_string();
        let line = format!("GET /units Bearer {value} then fleetd.token.{value} end");
        assert_eq!(
            token.redact(&line),
            "GET /units Bearer *** then fleetd.token.*** end"
        );
        assert_eq!(token.redact("no secret here"), "no secret here");
    }

    #[test]
    fn only_the_main_webview_may_read_the_token() {
        assert!(may_read_token("main"));
        for other in ["app::audience", "app::main", "main2", "Main", "", " main"] {
            assert!(!may_read_token(other), "{other:?} was allowed");
        }
    }

    /// The token must not be reachable any other way: no second command returns it and
    /// nothing in this crate prints it. A source scan, in the style of
    /// `tests/tauri_command_threading.rs`.
    #[test]
    fn the_token_is_exposed_in_exactly_the_places_this_module_names() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut uses = Vec::new();
        for entry in walkdir::WalkDir::new(&src) {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(entry.path()).unwrap();
            // The code that runs, without any test module's own text.
            let text = text.split("#[cfg(test)]").next().unwrap().to_string();
            let name = entry
                .path()
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for (index, line) in text.lines().enumerate() {
                if line.contains(".expose()") && !line.trim_start().starts_with("//") {
                    uses.push(format!("{name}:{}", index + 1));
                }
            }
        }
        let outside: Vec<&String> = uses
            .iter()
            .filter(|u| !u.starts_with("token.rs:") && !u.starts_with("sidecar.rs:"))
            .collect();
        assert!(
            outside.is_empty(),
            "the token is read outside token.rs and sidecar.rs: {outside:?}"
        );
        let in_sidecar = uses.iter().filter(|u| u.starts_with("sidecar.rs:")).count();
        assert_eq!(
            in_sidecar, 2,
            "sidecar.rs reads the token in exactly two places: the daemon's environment and \
             the health check's header. Found: {uses:?}"
        );
    }
}
````

`src-tauri/src/sidecar.rs`, the whole change, tests included:

````diff
--- a/cockpit/ui/src-tauri/src/sidecar.rs
+++ b/cockpit/ui/src-tauri/src/sidecar.rs
@@ -25,6 +25,8 @@
 use tauri_plugin_shell::process::{CommandChild, CommandEvent};
 use tauri_plugin_shell::ShellExt;
 
+use crate::token::{DaemonToken, TOKEN_ENV};
+
 /// fleetd's bound address; mirrors `crates/fleetd/src/bin/serve.rs` (CC_ADDR
 /// default). The supervisor health-gates against `http://{ADDR}/health`.
 const FLEETD_ADDR: &str = "127.0.0.1:8787";
@@ -116,9 +118,12 @@
 
         emit_status(&app, Status::Starting, None);
 
-        // Spawn the bundled sidecar (binaries/fleetd-serve-<triple>).
+        // Spawn the bundled sidecar (binaries/fleetd-serve-<triple>) with the token in its
+        // environment. Never as an argument: a process's arguments are visible to every
+        // other process of the same user.
+        let token = daemon_token(&app);
         let spawned = match app.shell().sidecar("fleetd-serve") {
-            Ok(cmd) => cmd.spawn(),
+            Ok(cmd) => cmd.env(TOKEN_ENV, token.expose()).spawn(),
             Err(e) => {
                 log::error!("fleetd: failed to resolve sidecar: {e}");
                 emit_status(&app, Status::Down, None);
@@ -156,7 +161,7 @@
 
         // Pipe logs and watch for termination. The event loop runs until the
         // child's stdout/stderr close (i.e. it exits), giving us `exit_code`.
-        let exit_code = pump_events(&app, &mut rx).await;
+        let exit_code = pump_events(&app, &token, &mut rx).await;
 
         // Drop the (now-dead) child handle.
         let _ = supervisor_state(&app).take_child();
@@ -180,14 +185,16 @@
 /// the child's exit code (when known) once its event stream closes.
 async fn pump_events(
     app: &AppHandle,
+    token: &DaemonToken,
     rx: &mut tauri::async_runtime::Receiver<CommandEvent>,
 ) -> Option<i32> {
     // Kick off the health gate concurrently with log piping so a slow-binding
     // fleetd still streams its logs while we wait.
     {
         let app = app.clone();
+        let token = token.clone();
         tauri::async_runtime::spawn(async move {
-            if health_gate(&app).await {
+            if health_gate(&app, &token).await {
                 emit_status(&app, Status::Ready, None);
                 log::info!("fleetd: health OK — cockpit may connect");
             } else {
@@ -199,12 +206,8 @@
     let mut exit_code = None;
     while let Some(event) = rx.recv().await {
         match event {
-            CommandEvent::Stdout(b) => {
-                log::info!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
-            }
-            CommandEvent::Stderr(b) => {
-                log::warn!("fleetd: {}", String::from_utf8_lossy(&b).trim_end())
-            }
+            CommandEvent::Stdout(b) => log::info!("fleetd: {}", log_line(token, &b)),
+            CommandEvent::Stderr(b) => log::warn!("fleetd: {}", log_line(token, &b)),
             CommandEvent::Terminated(t) => {
                 exit_code = t.code;
                 log::warn!("fleetd exited: {:?}", t.code);
@@ -217,7 +220,7 @@
 
 /// Poll `GET /health` until it answers 2xx or the gate times out. Returns
 /// `true` if fleetd became healthy.
-async fn health_gate(app: &AppHandle) -> bool {
+async fn health_gate(app: &AppHandle, token: &DaemonToken) -> bool {
     let url = format!("http://{FLEETD_ADDR}/health");
     let deadline = std::time::Instant::now() + HEALTH_GATE_TIMEOUT;
     let client = reqwest::Client::new();
@@ -226,7 +229,7 @@
         if supervisor_state(app).is_shutting_down() {
             return false;
         }
-        match client.get(&url).send().await {
+        match health_request(&client, &url, token).send().await {
             Ok(resp) if resp.status().is_success() => return true,
             _ => {}
         }
@@ -247,6 +250,26 @@
     supervisor_state(app).is_shutting_down()
 }
 
+/// One line of the daemon's output as it is written to the log: trimmed, and with the token
+/// replaced if the daemon ever printed it.
+fn log_line(token: &DaemonToken, bytes: &[u8]) -> String {
+    token.redact(String::from_utf8_lossy(bytes).trim_end())
+}
+
+/// The health check. `/health` is behind the token like every other route, so a request
+/// without the header is answered 401 and the gate would never open.
+fn health_request(
+    client: &reqwest::Client,
+    url: &str,
+    token: &DaemonToken,
+) -> reqwest::RequestBuilder {
+    client.get(url).bearer_auth(token.expose())
+}
+
+fn daemon_token(app: &AppHandle) -> DaemonToken {
+    app.state::<DaemonToken>().inner().clone()
+}
+
 fn supervisor_state(app: &AppHandle) -> tauri::State<'_, SidecarSupervisor> {
     app.state::<SidecarSupervisor>()
 }
@@ -254,3 +277,82 @@
 fn emit_status(app: &AppHandle, status: Status, code: Option<i32>) {
     let _ = app.emit(STATUS_EVENT, StatusPayload { status, code });
 }
+
+#[cfg(test)]
+mod tests {
+    use super::*;
+
+    fn token() -> DaemonToken {
+        DaemonToken::mint().unwrap()
+    }
+
+    #[test]
+    fn the_health_check_carries_the_token_as_a_bearer_header() {
+        let token = token();
+        let request = health_request(
+            &reqwest::Client::new(),
+            &format!("http://{FLEETD_ADDR}/health"),
+            &token,
+        )
+        .build()
+        .unwrap();
+        assert_eq!(request.url().as_str(), "http://127.0.0.1:8787/health");
+        assert_eq!(
+            request
+                .headers()
+                .get("authorization")
+                .and_then(|v| v.to_str().ok()),
+            Some(format!("Bearer {}", token.expose()).as_str())
+        );
+        assert!(
+            !request.url().as_str().contains(token.expose()),
+            "the token is in the URL"
+        );
+    }
+
+    #[test]
+    fn a_daemon_line_that_contains_the_token_is_logged_redacted() {
+        let token = token();
+        let line = format!("listening; token={}\r\n", token.expose());
+        assert_eq!(log_line(&token, line.as_bytes()), "listening; token=***");
+        assert_eq!(log_line(&token, b"plain line\n"), "plain line");
+    }
+
+    #[test]
+    fn the_token_reaches_the_daemon_through_its_environment_and_never_its_arguments() {
+        // Everything above this module: the code that runs, without these tests' own text.
+        let production = include_str!("sidecar.rs")
+            .split("#[cfg(test)]")
+            .next()
+            .unwrap();
+        let spawn = production
+            .lines()
+            .find(|line| line.contains(".spawn()"))
+            .expect("the line that spawns the sidecar");
+        assert!(
+            spawn.contains(".env(TOKEN_ENV, token.expose())"),
+            "the sidecar is spawned without the token in its environment: {spawn}"
+        );
+        assert!(
+            !production.contains(".args(") && !production.contains(".arg("),
+            "the sidecar is given arguments; the token must never be one"
+        );
+        assert_eq!(TOKEN_ENV, "FLEETD_TOKEN");
+    }
+
+    #[test]
+    fn nothing_in_this_module_logs_the_token_itself() {
+        let production = include_str!("sidecar.rs")
+            .split("#[cfg(test)]")
+            .next()
+            .unwrap();
+        for (index, line) in production.lines().enumerate() {
+            let logs = line.contains("log::") || line.contains("println!") || line.contains("dbg!");
+            assert!(
+                !(logs && line.contains("expose()")),
+                "sidecar.rs line {} writes the token to a log: {line}",
+                index + 1
+            );
+        }
+    }
+}
````

`src-tauri/src/lib.rs`, the three additions:

````diff
--- a/cockpit/ui/src-tauri/src/lib.rs
+++ b/cockpit/ui/src-tauri/src/lib.rs
@@ -28,6 +28,8 @@
 mod dashboard;
 // LANE-B → HOST: fleetd-serve sidecar supervisor (health-gate / restart / kill).
 mod sidecar;
+// UI-TOKEN: the daemon token (minted here, handed to fleetd and to the cockpit's own page).
+mod token;
 // U4 (spec §4, §6): filesystem discovery + raw reads for the `local` dashboard source.
 mod local_projects;
 // PLUGIN RUNTIME (Lane S integration):
@@ -68,6 +70,7 @@
             dashboard::audience_posts,
             dashboard::feedback_issues,
             local_projects::scan_local_projects,
+            token::fleetd_token,
         ])
         .setup(|app| {
             // LANE-P → HOST: activate the updater runtime. Lane B already wired
@@ -87,6 +90,11 @@
                         .build(),
                 )?;
             }
+
+            // Mint the daemon's token before the sidecar starts: the supervisor reads it from
+            // managed state on every (re)spawn, so a restarted daemon keeps the same token
+            // and the page's copy stays valid.
+            app.manage(token::DaemonToken::mint()?);
 
             // Babysit the fleetd daemon as a sidecar: the cockpit talks to it on
             // 127.0.0.1:8787. Bundled as binaries/fleetd-serve-<target-triple>.
````

`src/lib/auth.test.ts`:

````ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// The Tauri mock must be installed before auth.ts is imported (it imports `invoke` at module
// scope), hence `vi.hoisted`, as in App.appPlugin.test.ts.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import {
  clearToken,
  currentToken,
  isUsableToken,
  loadToken,
  tokenSource,
  TOKEN_COMMAND,
} from './auth';
import authSource from './auth.ts?raw';

const SHELL_TOKEN = 'a'.repeat(64);
const ENV_TOKEN = 'dev-token-0123456789abcdef0123456789abcdef';

beforeEach(() => {
  clearToken();
  invokeMock.mockReset();
  localStorage.clear();
  sessionStorage.clear();
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

describe('the token from the shell', () => {
  it('asks the shell with the fleetd_token command and remembers the answer', async () => {
    invokeMock.mockResolvedValue(SHELL_TOKEN);
    expect(currentToken()).toBeNull();
    await expect(loadToken()).resolves.toBe(SHELL_TOKEN);
    expect(invokeMock).toHaveBeenCalledWith(TOKEN_COMMAND, undefined);
    expect(TOKEN_COMMAND).toBe('fleetd_token');
    expect(currentToken()).toBe(SHELL_TOKEN);
    expect(tokenSource()).toBe('shell');
  });

  it('asks the shell once: a loaded token is not asked for again', async () => {
    invokeMock.mockResolvedValue(SHELL_TOKEN);
    await loadToken();
    await loadToken();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('prefers the shell to the development variable', async () => {
    vi.stubEnv('VITE_FLEETD_TOKEN', ENV_TOKEN);
    invokeMock.mockResolvedValue(SHELL_TOKEN);
    await expect(loadToken()).resolves.toBe(SHELL_TOKEN);
    expect(tokenSource()).toBe('shell');
  });

  it.each([
    ['undefined', undefined],
    ['null', null],
    ['a number', 42],
    ['an object', { token: SHELL_TOKEN }],
    ['a short string', 'short'],
    ['a string with a space', `${'a'.repeat(40)} b`],
  ])('an answer that is %s is not a token', async (_name, answer) => {
    invokeMock.mockResolvedValue(answer);
    await expect(loadToken()).resolves.toBeNull();
    expect(currentToken()).toBeNull();
    expect(tokenSource()).toBe('none');
  });
});

describe('the token from the development variable', () => {
  it('is used when there is no shell, in a development build', async () => {
    vi.stubEnv('DEV', true);
    vi.stubEnv('VITE_FLEETD_TOKEN', ENV_TOKEN);
    invokeMock.mockRejectedValue(new Error('not running in Tauri'));
    await expect(loadToken()).resolves.toBe(ENV_TOKEN);
    expect(tokenSource()).toBe('env');
  });

  it('is ignored in a production build, even when the variable is set', async () => {
    vi.stubEnv('DEV', false);
    vi.stubEnv('VITE_FLEETD_TOKEN', ENV_TOKEN);
    invokeMock.mockRejectedValue(new Error('not running in Tauri'));
    await expect(loadToken()).resolves.toBeNull();
    expect(tokenSource()).toBe('none');
  });

  it('is ignored when it could not be sent as a WebSocket subprotocol', async () => {
    vi.stubEnv('DEV', true);
    vi.stubEnv('VITE_FLEETD_TOKEN', 'dev/token=0123456789abcdef0123456789abcdef');
    invokeMock.mockRejectedValue(new Error('not running in Tauri'));
    await expect(loadToken()).resolves.toBeNull();
  });

  it('with neither source there is no token, and loadToken does not reject', async () => {
    vi.stubEnv('DEV', true);
    vi.stubEnv('VITE_FLEETD_TOKEN', '');
    invokeMock.mockRejectedValue(new Error('not running in Tauri'));
    await expect(loadToken()).resolves.toBeNull();
  });
});

describe('what counts as a usable token', () => {
  it('accepts what the shell mints and what the proof checklist mints', () => {
    expect(isUsableToken('0123456789abcdef'.repeat(4))).toBe(true); // 64 lowercase hex
    expect(isUsableToken('0123456789ABCDEF'.repeat(3))).toBe(true); // 48 uppercase hex
    expect(isUsableToken(ENV_TOKEN)).toBe(true);
  });

  it('refuses anything shorter than 32 characters', () => {
    expect(isUsableToken('a'.repeat(31))).toBe(false);
    expect(isUsableToken('a'.repeat(32))).toBe(true);
  });

  it.each(['/', '=', '"', ',', ';', ':', '@', '(', ')', '[', ']', '{', '}', '<', '>', '?', '\\', ' ', '\t', 'é'])(
    'refuses a token containing %j, which no WebSocket subprotocol name may contain',
    (bad) => {
      expect(isUsableToken(`${'a'.repeat(40)}${bad}`)).toBe(false);
    },
  );

  it('every token it accepts can be offered as a subprotocol', () => {
    // jsdom's WebSocket applies the same rule a browser does: an illegal name throws.
    const offer = (value: string) => new WebSocket('ws://127.0.0.1:9/', ['fleetd.v1', `fleetd.token.${value}`]);
    const accepted = "abcXYZ019!#$%&'*+-.^_`|~".padEnd(32, 'a');
    expect(isUsableToken(accepted)).toBe(true);
    const socket = offer(accepted);
    socket.close();
    expect(() => offer(`${'a'.repeat(40)}/`)).toThrow();
  });
});

describe('where the token is kept', () => {
  it('is in memory only: nothing is written to storage, the URL, a cookie or the console', async () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem');
    const log = vi.spyOn(console, 'log');
    const info = vi.spyOn(console, 'info');
    const warn = vi.spyOn(console, 'warn');
    const error = vi.spyOn(console, 'error');
    const debug = vi.spyOn(console, 'debug');
    invokeMock.mockResolvedValue(SHELL_TOKEN);
    await loadToken();
    expect(setItem).not.toHaveBeenCalled();
    expect(localStorage.length + sessionStorage.length).toBe(0);
    expect(document.cookie).toBe('');
    expect(window.location.href).not.toContain(SHELL_TOKEN);
    expect(document.documentElement.outerHTML).not.toContain(SHELL_TOKEN);
    for (const spy of [log, info, warn, error, debug]) expect(spy).not.toHaveBeenCalled();
  });

  it('the module names no storage, cookie or console API at all', () => {
    // Comments are stripped first: the file's own header says where the token must not go.
    const code = authSource
      .split('\n')
      .filter((line) => !line.trim().startsWith('//') && !line.trim().startsWith('*') && !line.trim().startsWith('/*'))
      .join('\n');
    for (const forbidden of ['localStorage', 'sessionStorage', 'indexedDB', 'document.cookie', 'console.', 'location']) {
      expect(code, `auth.ts uses ${forbidden}`).not.toContain(forbidden);
    }
  });

  it('clearToken forgets it, so the next load asks again', async () => {
    invokeMock.mockResolvedValue(SHELL_TOKEN);
    await loadToken();
    clearToken();
    expect(currentToken()).toBeNull();
    expect(tokenSource()).toBe('none');
    await loadToken();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});
````

`src/lib/NotConnected.test.ts`:

````ts
import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import NotConnected from './NotConnected.svelte';

afterEach(cleanup);

const BASE = 'http://127.0.0.1:8787';

describe('NotConnected.svelte', () => {
  it.each([
    ['no_token', 'NO DAEMON TOKEN'],
    ['refused', 'TOKEN REFUSED'],
    ['unreachable', 'DAEMON NOT ANSWERING'],
  ] as const)('for %s it says NOT CONNECTED and %s', (connection, title) => {
    const { getByTestId } = render(NotConnected, {
      props: { connection, baseUrl: BASE, onretry: () => {} },
    });
    const banner = getByTestId('not-connected');
    expect(banner.getAttribute('role')).toBe('alert');
    expect(banner.getAttribute('data-reason')).toBe(connection);
    expect(banner.textContent).toContain('NOT CONNECTED');
    expect(banner.textContent).toContain(title);
    expect(banner.textContent).toContain(BASE);
  });

  it('without a token it names the development variable and the file to put it in', () => {
    const { getByTestId } = render(NotConnected, {
      props: { connection: 'no_token', baseUrl: BASE, onretry: () => {} },
    });
    const text = getByTestId('not-connected').textContent ?? '';
    expect(text).toContain('VITE_FLEETD_TOKEN');
    expect(text).toContain('.env.development.local');
  });

  it('RETRY is a real button, reachable by keyboard, that calls onretry', async () => {
    const onretry = vi.fn();
    const { getByTestId } = render(NotConnected, {
      props: { connection: 'unreachable', baseUrl: BASE, onretry },
    });
    const retry = getByTestId('not-connected-retry') as HTMLButtonElement;
    expect(retry.tagName).toBe('BUTTON');
    expect(retry.type).toBe('button');
    expect(retry.textContent?.trim()).toBe('RETRY');
    retry.focus();
    expect(document.activeElement).toBe(retry);
    await fireEvent.click(retry);
    expect(onretry).toHaveBeenCalledOnce();
  });
});
````

`src/lib/csp.test.ts`:

````ts
import { describe, it, expect } from 'vitest';
import { DEFAULT_FLEET_URL } from './api';
import raw from '../../src-tauri/tauri.conf.json?raw';

// The packaged page may only connect where its content-security policy says. jsdom enforces
// no policy, so nothing else in this suite would notice the daemon's address and the policy
// drifting apart; in the packaged app every request and every stream would then fail with no
// error a test could see.

const conf = JSON.parse(raw) as { app: { security: { csp: string } } };

function directive(name: string): string[] {
  const found = conf.app.security.csp
    .split(';')
    .map((part) => part.trim().split(/\s+/))
    .find((words) => words[0] === name);
  return found ? found.slice(1) : [];
}

describe('the content-security policy and the daemon address', () => {
  it('allows HTTP requests to the daemon', () => {
    expect(directive('connect-src')).toContain(DEFAULT_FLEET_URL);
  });

  it('allows the WebSocket stream to the daemon', () => {
    // A subprotocol list does not change the origin a stream connects to: the policy needs
    // the ws:// form of the same address and nothing more.
    expect(directive('connect-src')).toContain(DEFAULT_FLEET_URL.replace(/^http/, 'ws'));
  });

  it('allows connections to nothing else', () => {
    expect(directive('connect-src').sort()).toEqual(
      ["'self'", DEFAULT_FLEET_URL, DEFAULT_FLEET_URL.replace(/^http/, 'ws')].sort(),
    );
  });

  it('the daemon address is loopback', () => {
    expect(new URL(DEFAULT_FLEET_URL).hostname).toBe('127.0.0.1');
  });
});
````

### Implementation notes

**`src/lib/auth.ts`, in full.** It replaces wave 0's stub. The three functions the stub
declared keep their signatures; `isUsableToken`, `tokenSource`, `TOKEN_COMMAND` and
`TokenSource` are new.

````ts
// Where the cockpit's page gets the daemon's token.
//
// Two sources, tried in this order:
//
// 1. The desktop shell. The Tauri shell mints the token, starts the daemon with it, and hands
//    it to this page through the `fleetd_token` command.
// 2. A development build only: `VITE_FLEETD_TOKEN`, read by Vite when the dev server starts.
//    This is for running the page in a browser against a daemon started by hand. The read is
//    behind `import.meta.env.DEV`, so a production build contains neither the value nor the
//    branch.
//
// The token is held in this module's memory only. It is never written to localStorage,
// sessionStorage, a cookie, the URL, the DOM or the console.

import { invoke } from '@tauri-apps/api/core';

/** The Tauri command that hands the page its token (`src-tauri/src/token.rs`). */
export const TOKEN_COMMAND = 'fleetd_token';

/** Where the token in memory came from. */
export type TokenSource = 'shell' | 'env' | 'none';

/**
 * A usable token: 32 or more characters, each one legal in a WebSocket subprotocol name
 * (RFC 7230 `tchar`). The daemon accepts any printable ASCII, but the same value is sent as
 * `fleetd.token.<token>` in `Sec-WebSocket-Protocol`, and a browser throws on a name that
 * contains `/`, `=`, `"`, `,`, `;`, `:`, `@`, brackets, braces or a space.
 */
const USABLE = /^[A-Za-z0-9!#$%&'*+\-.^_`|~]{32,}$/;

export function isUsableToken(value: unknown): value is string {
  return typeof value === 'string' && USABLE.test(value);
}

let token: string | null = null;
let source: TokenSource = 'none';

/** The token, if one has been loaded. Synchronous: the API client calls it on every request. */
export function currentToken(): string | null {
  return token;
}

/** Where the loaded token came from, for the "not connected" message. Never the value. */
export function tokenSource(): TokenSource {
  return source;
}

/** Forget the token. The next `loadToken` asks for it again. */
export function clearToken(): void {
  token = null;
  source = 'none';
}

async function fromShell(): Promise<string | null> {
  try {
    const value: unknown = await invoke(TOKEN_COMMAND);
    return isUsableToken(value) ? value : null;
  } catch {
    // Not running in the shell (a browser), or the shell refused this webview.
    return null;
  }
}

function fromDevEnv(): string | null {
  if (!import.meta.env.DEV) return null;
  const value: unknown = import.meta.env.VITE_FLEETD_TOKEN;
  return isUsableToken(value) ? value : null;
}

/**
 * Resolve the token from wherever this build gets it, remember it, and return it.
 * Resolves to null when there is none. Never rejects.
 */
export async function loadToken(): Promise<string | null> {
  if (token) return token;
  const shell = await fromShell();
  if (shell) {
    token = shell;
    source = 'shell';
    return token;
  }
  const env = fromDevEnv();
  if (env) {
    token = env;
    source = 'env';
    return token;
  }
  return null;
}
````

**The development path, exactly.** For running the page in a browser against a daemon started
by hand:

1. Mint a value and start the daemon with it. PowerShell, at the repository root:

   ```powershell
   $env:FLEETD_TOKEN = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLower()
   cargo run -p fleetd --bin serve
   ```

2. Put the same value in `cockpit/ui/.env.development.local`, one line:
   `VITE_FLEETD_TOKEN=<the value>`. The file is ignored by git (`*.local` in
   `cockpit/ui/.gitignore`). Vite reads it when the development server starts, and only in
   development mode: `vite build` does not load a `.env.development*` file.
3. `npm run dev`, and open `http://localhost:5173`, which is on the daemon's default list of
   allowed origins. Changing the value needs the development server restarted, because Vite
   substitutes `import.meta.env.VITE_FLEETD_TOKEN` when it transforms the module.

In the desktop app the shell answers first, so the variable is not consulted even if it is
set. In a production build the read is behind `import.meta.env.DEV`, which Vite replaces with
`false`, so the branch and the value are removed from the bundle. **Verify** builds with a
canary value and searches the output for it.

**`src/lib/NotConnected.svelte`.** Keep the stub's three props. What the tests require:

| Element | `data-testid` | Must have |
|---|---|---|
| The container | `not-connected` | `role="alert"`; `data-reason` equal to the `connection` prop; text containing `NOT CONNECTED`, the reason's title, and `baseUrl` |
| The button | `not-connected-retry` | a `<button type="button">` whose text is `RETRY`, calling `onretry` |

The three titles are `NO DAEMON TOKEN` (`no_token`), `TOKEN REFUSED` (`refused`) and `DAEMON
NOT ANSWERING` (`unreachable`). For `no_token` the text also names `VITE_FLEETD_TOKEN` and
`.env.development.local`. Say one sentence about what to do for each; write it yourself. It is
one line high, under the header, across the full width: a red-tinted band
(`rgba(255, 93, 108, 0.1)` with a `rgba(255, 93, 108, 0.45)` bottom border), the title in
`--red`, the sentence in `--text`, the address in `--text-dim`, the button on `--elev` with
`--text`. It never shows a token and never receives one.

**Changes to existing code**

| File and place | Change |
|---|---|
| `src-tauri/src/sidecar.rs` line 120, `app.shell().sidecar("fleetd-serve")` | The spawned command gets `.env(TOKEN_ENV, token.expose())`. `tauri_plugin_shell`'s `Command::env` sets one variable and leaves the inherited environment alone, which is how `FLEETD_CONFIG`, `FLEETD_REGISTRY` and the model credential reach the daemon from the operator's shell |
| `sidecar.rs` line 229, the health poll | `health_request` adds the bearer header. Without it the gate never opens once CC-WIRE has merged: `/health` answers 401 and the supervisor logs a timeout every twenty seconds |
| `sidecar.rs` lines 202 to 207, the log pump | Each line goes through `log_line`, which redacts. The daemon is specified never to print its token; this is the second lock |
| `src-tauri/src/lib.rs` | `mod token;`, the command in `generate_handler!`, and `app.manage(token::DaemonToken::mint()?)` in `setup` **before** `sidecar::spawn_supervisor` |
| `src/lib/auth.ts` | Replaced in full, above |
| `src/lib/NotConnected.svelte` | The stub's markup replaced; props unchanged |

**Pitfalls**

- **Mint once per run of the shell, not once per spawn.** The supervisor restarts a crashed
  daemon. If each spawn minted a new token, the page's copy would be refused after the first
  crash and nothing would tell it. The token lives in Tauri's managed state and every spawn
  reads the same one.
- **The command must be `async`.** `tests/tauri_command_threading.rs` fails the build for a
  synchronous command that does not dispatch its work. An `async` command that takes
  `State<'_, T>` must return a `Result`, which this one does.
- **Any webview can call an app command.** Tauri checks its permission lists for plugin
  commands; a command registered with `generate_handler!` is callable from every webview of
  the app unless the build declares an app manifest, which this crate does not. The app-plugin
  child webviews (labelled `app::<id>`) are other programs' pages. That is why `fleetd_token`
  takes the calling `Webview` and refuses any label but `main`. Do not remove the parameter,
  and do not add a second way to read the token.
- **A view plugin's iframe is inside the `main` webview.** The label check does not tell it
  apart from the page. It is sandboxed without `allow-same-origin`, and whether Tauri's bridge
  is reachable from such a frame on WebView2 was not established while writing this plan. Part
  3's checklist has the operator look. If the bridge is reachable there, stop and report: the
  fix is an app manifest in `build.rs`, which is outside this lane.
- **The content-security policy and the subprotocol.** A policy has no notion of a
  subprotocol: `connect-src` names origins. The stream connects to `ws://127.0.0.1:8787`,
  which the policy already lists, whatever is offered in `Sec-WebSocket-Protocol`. Nothing in
  `tauri.conf.json` changes. `csp.test.ts` is what notices if the address ever does.
- **The alphabet.** The daemon accepts any printable ASCII. A browser throws a `SyntaxError`
  from `new WebSocket(url, [name])` when `name` has a character outside the HTTP token set.
  Base64 has `/` and `=`. So the shell mints hex, and `isUsableToken` refuses a development
  value the page could not send on a stream, rather than half-working.
- **`invoke` in tests.** `App.appPlugin.test.ts` mocks `invoke` to resolve `undefined` for
  commands it does not know. `loadToken` must treat any answer that is not a usable token as
  no token, which is what behaviour 13 pins.
- **Never log it, never store it.** No `console.*` in `auth.ts`, at any level, for any reason.
  No `localStorage`. An error message never includes the value: `loadToken` swallows a failed
  `invoke` and returns null, and the reason shown is `no_token`.
- **The reconnect gap.** `docs/testing/PLAN.md` GAP-015 records that the page went dead when
  the sidecar restarted. Wave 0's store now reopens streams and retries the unit list. This
  lane's part is only that the token survives the restart (the first pitfall). Do not add a
  second retry loop here, and do not listen for `fleetd://status` to trigger one.
- **Svelte 5.** `NotConnected.svelte` uses `$props()` and `$derived`; a lookup of the reason by
  `connection` inside `$derived`, not a top-level `const` that reads a prop once.

### Verify

In `cockpit/ui`, after `npm ci` and `npm run sidecar`:

| Command | Expected |
|---|---|
| `npm run check` | exit 0: types up to date, 0 errors, 0 warnings |
| `npm run check:types` | `generated types are up to date`: this lane does not touch them |
| `npx vitest run src/lib/auth.test.ts src/lib/NotConnected.test.ts src/lib/csp.test.ts` | `Tests  48 passed (48)` |
| `npx vitest run` | every file passes; 48 more tests than at the branch point (`Test Files  30 passed (30)`, `Tests  284 passed (284)` if no other lane had merged) |
| *Git-Bash:* `VITE_FLEETD_TOKEN=canary-token-0123456789abcdef0123456789 npx vite build && ! grep -rl "canary-token" dist && echo "no token in the bundle"` | the build succeeds and `no token in the bundle` is printed |
| *Git-Bash:* `grep -nE "console\.\|localStorage\|sessionStorage\|document\.cookie" src/lib/auth.ts \| grep -v "^[0-9]*://"` | no output |

In `cockpit/ui/src-tauri`:

| Command | Expected |
|---|---|
| `cargo fmt --all -- --check` | no output |
| `cargo clippy --all-targets -- -D warnings` | exit 0, no warning |
| `cargo test` | `test result: ok. 46 passed` for the library (35 before, 11 new), then `3 passed` (`packaged_plugin_root`) and `2 passed` (`tauri_command_threading`) |
| *Git-Bash:* `git diff origin/factory/m1...HEAD -- cockpit/ui/src-tauri/Cargo.toml cockpit/ui/src-tauri/Cargo.lock cockpit/ui/src-tauri/tauri.conf.json cockpit/ui/src-tauri/capabilities` | no output: those are the coordinator's |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 18 pass, and the Verify tables are green with real output in the pull
      request.
- [ ] The token is minted once per run, reaches the daemon only through `FLEETD_TOKEN` in its
      environment, and reaches the page only through `fleetd_token`, for the `main` webview.
- [ ] A production bundle built with `VITE_FLEETD_TOKEN` set does not contain the value.
- [ ] The pull request says `Closes #91` if lane CC-WIRE has merged, and otherwise says that
      #91 closes when both halves are in. It tells the coordinator that the app-command
      question in **Pitfalls** is open for the operator's smoke.

---

## Lane UI-HARNESS

The header's Docker and API-key badges become one badge that summarises the harness registry
and opens a panel showing each harness's name, health and declared capabilities, with a plain
statement of anything weaker than a container with metering.

**Owns:** `src/lib/harness/**` (the stub `HarnessBadge.svelte`, which it fills, and every new
file beside it); `src/App.harness.test.ts`.

**Reads:** the control-plane plan, Part 1 Task 8 (`HarnessView`), lane CC-SUPERVISOR's
`eligibility` (which shortfalls need the opt-in and which are refused outright), and the M0
contracts plan's `Capabilities`; `src/lib/generated/fleet-api.ts` (`HarnessView`,
`Capabilities` and the enums); `src/lib/api.ts`; `src/lib/testkit/fakeApi.ts` (`harnessView`);
`src/App.svelte` (where the badge is mounted and the styles of the header around it; read
only); `src/lib/Switcher.svelte` and `src/lib/ApprovalOverlay.svelte` for the house style.

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-ui-harness`, branch
`feat/m1-ui-harness`, created with `git worktree add --no-track`, cut from `origin/factory/m1`
after wave 0 merges. Pull request against `factory/m1`.

**Needs:** Part 1 merged. Nothing from the other lanes: every test runs on the fake.

**Blocks:** nothing. Lane UI-RAIL's dispatch form reads `GET /harnesses` through the client
itself and does not import from this lane.

### Behaviours

1. Every declared capability is shown, in a fixed order, as a label and a value; an empty list
   or an undeclared value says so; each profile is marked priced or unpriced.
   `src/lib/harness/capabilities.test.ts` › *facts* (three tests)
2. Each guarantee weaker than a container with metering is stated in words: agents on the
   host, no isolation, no metering, an unpriced profile. The reference harness has none.
   *weaker than a container with metering* (six tests)
3. What a harness cannot do, and the risk it declares, are stated too and kept apart from
   "weaker": no oracle gate, push delivery, no build kind, no preset, no resume, no halt; open
   or undeclared egress; and the three kinds come in a fixed order. *limits and declared
   risks* (nine tests)
4. The registry is summarised for the badge: healthy of total, how many are weaker, and one of
   four tones. *summarise* (five tests; 23 in the file)
5. The badge reads `GET /harnesses` when the page is connected and shows healthy of total; it
   is green only when every harness is healthy and none is weaker; it counts the weaker ones.
   `src/lib/harness/HarnessBadge.test.ts` › *the header badge* (first three tests)
6. The badge makes no request while the page is not connected. *makes no request while the
   page is not connected*
7. A list that cannot be read is shown as that, never as an empty registry. *a list that
   cannot be read…*
8. The badge is a button that says whether its panel is open; the panel takes focus; Escape
   closes it and returns focus to the badge. *is a button that says…*
9. Opening the panel reads the registry again, and so does REFRESH. *opening the panel reads…*
10. The panel shows each harness with its name, version, health and every capability.
    *the harness panel* › *shows each harness…*
11. The panel says plainly when nothing is weaker, and states each weaker guarantee in words
    with the word `WEAKER`, not by colour alone. *says plainly that a container…*, *states
    each weaker guarantee…*
12. An unhealthy harness shows its error; an unprobed one says its capabilities are unknown;
    neither shows capabilities. *shows an unhealthy harness…*
13. An empty registry says so. *says so when no harness is registered*
14. In the real page the badge is in the header with the registry's count, and the Docker and
    API-key badges are gone. `src/App.harness.test.ts` (two tests)

| Tests | Command | The failure to expect first |
|---|---|---|
| 1 to 4 | `npx vitest run src/lib/harness/capabilities.test.ts` | `Error: Failed to resolve import "./capabilities" from "src/lib/harness/capabilities.test.ts". Does the file exist?` and no test runs |
| 5 to 13 | `npx vitest run src/lib/harness/HarnessBadge.test.ts` | `12 failed`. The stub renders a hidden empty `<span>`: the tests read `expected '' to contain 'HARNESS 1/2'`, `expected null to be 'ok'`, `expected 'SPAN' to be 'BUTTON'`, and the ones that open the panel time out in `waitFor` on `expected '' to match /\d\/\d/` |
| 14 | `npx vitest run src/App.harness.test.ts` | `2 failed`: `expected '' to contain 'HARNESS 1/2'` and `'HARNESS 1/1'` |

`src/lib/harness/capabilities.test.ts`:

````ts
import { describe, it, expect } from 'vitest';
import { facts, notes, summarise, weaker } from './capabilities';
import { harnessView } from '../testkit/fakeApi';
import type { Capabilities } from '../generated/fleet-api';

/** The reference: a container with metering, and nothing else to remark on but open egress. */
function reference(overrides: Partial<Capabilities> = {}): Capabilities {
  return { ...harnessView().capabilities!, ...overrides };
}

describe('facts', () => {
  it('lists every declared capability, in a fixed order, as label and value', () => {
    expect(facts(reference())).toEqual([
      { label: 'ISOLATION', value: 'container' },
      { label: 'METERING', value: 'usd' },
      { label: 'NETWORK', value: 'open' },
      { label: 'GATES', value: 'oracle' },
      { label: 'CONTROLS', value: 'scope, protected' },
      { label: 'KINDS', value: 'build' },
      { label: 'PRESETS', value: 'cargo 0.1.0, node 0.1.0' },
      { label: 'PROFILES', value: 'claude (priced)' },
      { label: 'DELIVERY', value: 'bundle' },
      { label: 'HOLDOUTS', value: 'no' },
      { label: 'RESUME', value: 'yes' },
      { label: 'HALT', value: 'yes' },
    ]);
  });

  it('says so when a list is empty or a value was not declared', () => {
    // A 0.1 harness: only the six original keys are on the wire.
    const old: Capabilities = {
      isolation: 'host',
      metering: 'none',
      gates: [],
      delivery: 'bundle',
      resume: false,
      halt: false,
    };
    const byLabel = Object.fromEntries(facts(old).map((f) => [f.label, f.value]));
    expect(byLabel).toMatchObject({
      NETWORK: 'not declared',
      GATES: 'none declared',
      CONTROLS: 'none declared',
      KINDS: 'build',
      PRESETS: 'none declared',
      PROFILES: 'none declared',
      HOLDOUTS: 'no',
    });
  });

  it('marks each profile priced or unpriced', () => {
    const c = reference({
      profiles: [
        { name: 'claude', priced: true },
        { name: 'hybrid-local', priced: false },
      ],
    });
    expect(facts(c).find((f) => f.label === 'PROFILES')?.value).toBe(
      'claude (priced), hybrid-local (unpriced)',
    );
  });
});

describe('weaker than a container with metering', () => {
  it('the reference has no weaker guarantee', () => {
    expect(weaker(reference())).toEqual([]);
  });

  it.each([
    ['isolation', { isolation: 'host' }, 'not in a container'],
    ['isolation', { isolation: 'none' }, 'no isolation'],
    ['metering', { metering: 'none' }, 'not metered'],
    ['profiles', { profiles: [{ name: 'hybrid-local', priced: false }] }, 'hybrid-local is unpriced'],
  ] as const)('%s: %j is stated plainly', (capability, change, phrase) => {
    const found = weaker(reference(change as Partial<Capabilities>));
    expect(found).toHaveLength(1);
    expect(found[0].capability).toBe(capability);
    expect(found[0].text).toContain(phrase);
  });

  it('a harness that is weaker in three ways gets three statements, isolation first', () => {
    const found = weaker(
      reference({
        isolation: 'host',
        metering: 'none',
        profiles: [
          { name: 'claude', priced: true },
          { name: 'local', priced: false },
        ],
      }),
    );
    expect(found.map((n) => n.capability)).toEqual(['isolation', 'metering', 'profiles']);
  });
});

describe('limits and declared risks', () => {
  it.each([
    ['gates', { gates: [] }, 'T2 and T3 units are refused'],
    ['delivery', { delivery: 'push' }, 'refuses this harness'],
    ['kinds', { kinds: ['draft'] }, 'Does not run build units'],
    ['presets', { presets: [] }, 'no unit from a signed spec'],
    ['resume', { resume: false }, 'cannot be resumed'],
    ['halt', { halt: false }, 'cannot be halted'],
  ] as const)('%s: %j is a limit', (capability, change, phrase) => {
    const found = notes(reference(change as Partial<Capabilities>)).filter((n) => n.level === 'limit');
    expect(found.map((n) => n.capability)).toEqual([capability]);
    expect(found[0].text).toContain(phrase);
  });

  it('empty kinds means build only, which is not a limit', () => {
    expect(notes(reference({ kinds: [] })).filter((n) => n.capability === 'kinds')).toEqual([]);
  });

  it('open egress is a declared risk, and an undeclared network is read as open', () => {
    expect(notes(reference({ network: 'open' })).filter((n) => n.level === 'risk')).toEqual([
      { level: 'risk', capability: 'network', text: 'Agent containers have open network egress.' },
    ]);
    const undeclared = notes(reference({ network: null })).filter((n) => n.level === 'risk');
    expect(undeclared[0].text).toContain('not declared');
    expect(notes(reference({ network: 'filtered' })).filter((n) => n.level === 'risk')).toEqual([]);
  });

  it('orders notes weaker, then limit, then risk', () => {
    const levels = notes(reference({ isolation: 'host', halt: false, network: 'open' })).map((n) => n.level);
    expect(levels).toEqual(['weaker', 'limit', 'risk']);
  });
});

describe('summarise', () => {
  const unhealthy = harnessView({ name: 'fake', state: 'unhealthy', harness: null, capabilities: null, error: 'not found' });
  const unprobed = harnessView({ name: 'later', state: 'unprobed', harness: null, capabilities: null });
  const weak = harnessView({ name: 'bare', capabilities: reference({ isolation: 'host' }) });

  it.each([
    ['no harness registered', [], { healthy: 0, total: 0, weaker: 0, tone: 'idle' }],
    ['every harness healthy and none weaker', [harnessView()], { healthy: 1, total: 1, weaker: 0, tone: 'ok' }],
    ['a healthy harness that is weaker', [harnessView(), weak], { healthy: 2, total: 2, weaker: 1, tone: 'attention' }],
    ['one of two not healthy', [harnessView(), unhealthy], { healthy: 1, total: 2, weaker: 0, tone: 'attention' }],
    ['none healthy', [unhealthy, unprobed], { healthy: 0, total: 2, weaker: 0, tone: 'bad' }],
  ] as const)('%s', (_name, rows, expected) => {
    expect(summarise(rows)).toEqual(expected);
  });
});
````

`src/lib/harness/HarnessBadge.test.ts`:

````ts
import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup, waitFor, fireEvent, within } from '@testing-library/svelte';
import HarnessBadge from './HarnessBadge.svelte';
import { ApiFailure } from '../api';
import { createFakeApi, harnessView, type FakeFleetApi } from '../testkit/fakeApi';
import type { HarnessView } from '../generated/fleet-api';

afterEach(cleanup);

const reference = harnessView();
const bare = harnessView({
  name: 'bare-host',
  harness: { name: 'bare-host', version: '0.0.3' },
  capabilities: {
    ...reference.capabilities!,
    isolation: 'host',
    metering: 'none',
    network: null,
    profiles: [
      { name: 'claude', priced: true },
      { name: 'hybrid-local', priced: false },
    ],
  },
});
const unhealthy: HarnessView = { name: 'fake', state: 'unhealthy', error: 'harness-fake not found' };
const unprobed: HarnessView = { name: 'later', state: 'unprobed' };

function mount(fake: FakeFleetApi, connected = true) {
  return render(HarnessBadge, { props: { api: fake, connected } });
}

const listed = (fake: FakeFleetApi) => fake.calls.filter((c) => c.method === 'listHarnesses').length;

async function openPanel(view: ReturnType<typeof mount>) {
  await waitFor(() => expect(view.getByTestId('harness-badge').textContent).toMatch(/\d\/\d/));
  await fireEvent.click(view.getByTestId('harness-badge'));
  return view.getByTestId('harness-panel');
}

describe('the header badge', () => {
  it('reads GET /harnesses when the page is connected and shows healthy of total', async () => {
    const fake = createFakeApi({ harnesses: [reference, unhealthy] });
    const { getByTestId } = mount(fake);
    const badge = getByTestId('harness-badge');
    await waitFor(() => expect(badge.textContent).toContain('HARNESS 1/2'));
    expect(badge.getAttribute('data-tone')).toBe('attention');
    expect(badge.getAttribute('aria-label')).toBe('Harnesses: 1 of 2 healthy');
    expect(listed(fake)).toBe(1);
  });

  it('is green only when every harness is healthy and none is weaker', async () => {
    const { getByTestId } = mount(createFakeApi({ harnesses: [reference] }));
    await waitFor(() => expect(getByTestId('harness-badge').getAttribute('data-tone')).toBe('ok'));
    expect(getByTestId('harness-badge').textContent).toContain('HARNESS 1/1');
  });

  it('counts healthy harnesses that are weaker than a container with metering', async () => {
    const { getByTestId } = mount(createFakeApi({ harnesses: [reference, bare] }));
    const badge = getByTestId('harness-badge');
    await waitFor(() => expect(badge.textContent).toContain('1 WEAKER'));
    expect(badge.getAttribute('data-tone')).toBe('attention');
    expect(badge.getAttribute('aria-label')).toBe(
      'Harnesses: 2 of 2 healthy, 1 weaker than a container with metering',
    );
  });

  it('makes no request while the page is not connected', async () => {
    const fake = createFakeApi({ harnesses: [reference] });
    const { getByTestId } = mount(fake, false);
    await Promise.resolve();
    expect(listed(fake)).toBe(0);
    expect(getByTestId('harness-badge').getAttribute('data-tone')).toBe('idle');
    expect(getByTestId('harness-badge').getAttribute('aria-label')).toBe('Harnesses: not connected');
    await fireEvent.click(getByTestId('harness-badge')); // opening the panel asks for nothing either
    expect(listed(fake)).toBe(0);
  });

  it('a list that cannot be read is shown as that, never as an empty registry', async () => {
    const fake = createFakeApi({ harnesses: [reference] });
    fake.fail('listHarnesses', new ApiFailure(503, 'store', 'the registry cannot be read right now'));
    const { getByTestId, queryByTestId } = mount(fake);
    const badge = getByTestId('harness-badge');
    await waitFor(() => expect(badge.getAttribute('data-tone')).toBe('bad'));
    expect(badge.textContent).toContain('HARNESS ?');
    await fireEvent.click(badge);
    expect(getByTestId('harness-error').textContent).toContain('the registry cannot be read right now');
    expect(queryByTestId('harness-empty')).toBeNull();
  });

  it('is a button that says whether its panel is open, and Escape closes the panel and returns focus', async () => {
    const view = mount(createFakeApi({ harnesses: [reference] }));
    const badge = view.getByTestId('harness-badge') as HTMLButtonElement;
    expect(badge.tagName).toBe('BUTTON');
    expect(badge.getAttribute('aria-expanded')).toBe('false');
    expect(badge.getAttribute('aria-controls')).toBe('harness-panel');

    const panel = await openPanel(view);
    expect(badge.getAttribute('aria-expanded')).toBe('true');
    expect(panel.id).toBe('harness-panel');
    expect(panel.tagName).toBe('SECTION'); // a labelled section is a region landmark
    expect(panel.getAttribute('aria-label')).toBe('Harnesses');
    await waitFor(() => expect(document.activeElement).toBe(panel));

    await fireEvent.keyDown(panel, { key: 'Escape' });
    expect(view.queryByTestId('harness-panel')).toBeNull();
    expect(badge.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(badge);
  });

  it('opening the panel reads the registry again, and REFRESH reads it once more', async () => {
    const fake = createFakeApi({ harnesses: [reference] });
    const view = mount(fake);
    await openPanel(view);
    expect(listed(fake)).toBe(2);
    fake.harnesses = [reference, unprobed];
    await fireEvent.click(view.getByTestId('harness-refresh'));
    await waitFor(() => expect(view.getAllByTestId('harness-row')).toHaveLength(2));
    expect(listed(fake)).toBe(3);
  });
});

describe('the harness panel', () => {
  it('shows each harness with its name, version, health and every declared capability', async () => {
    const view = mount(createFakeApi({ harnesses: [reference] }));
    const panel = await openPanel(view);
    const row = within(panel).getByTestId('harness-row');
    expect(row.getAttribute('data-name')).toBe('reqdrive');
    expect(row.textContent).toContain('reqdrive 0.1.0');
    expect(within(row).getByTestId('harness-state').textContent).toBe('HEALTHY');

    const pairs = [...within(row).getByTestId('harness-facts').querySelectorAll('.hp-fact')].map((el) => [
      el.querySelector('dt')?.textContent,
      el.querySelector('dd')?.textContent,
    ]);
    expect(pairs).toEqual([
      ['ISOLATION', 'container'],
      ['METERING', 'usd'],
      ['NETWORK', 'open'],
      ['GATES', 'oracle'],
      ['CONTROLS', 'scope, protected'],
      ['KINDS', 'build'],
      ['PRESETS', 'cargo 0.1.0, node 0.1.0'],
      ['PROFILES', 'claude (priced)'],
      ['DELIVERY', 'bundle'],
      ['HOLDOUTS', 'no'],
      ['RESUME', 'yes'],
      ['HALT', 'yes'],
    ]);
  });

  it('says plainly that a container with metering is not weaker, and still names its declared risk', async () => {
    const view = mount(createFakeApi({ harnesses: [reference] }));
    const panel = await openPanel(view);
    expect(within(panel).getByText('WEAKER THAN A CONTAINER WITH METERING')).not.toBeNull();
    expect(within(panel).getByTestId('harness-not-weaker').textContent).toContain(
      'agents run in a container and spend is metered',
    );
    expect(within(panel).queryByTestId('harness-weaker')).toBeNull();
    expect(within(panel).getByTestId('harness-notes').textContent).toContain('RISK');
    expect(within(panel).getByTestId('harness-notes').textContent).toContain('open network egress');
  });

  it('states each weaker guarantee in words, with the word WEAKER and not by colour alone', async () => {
    const view = mount(createFakeApi({ harnesses: [bare] }));
    const panel = await openPanel(view);
    const items = [...within(panel).getByTestId('harness-weaker').querySelectorAll('li')].map(
      (li) => li.textContent?.replace(/\s+/g, ' ').trim() ?? '',
    );
    expect(items).toHaveLength(3);
    expect(items.every((text) => text.startsWith('WEAKER '))).toBe(true);
    expect(items[0]).toContain('not in a container');
    expect(items[1]).toContain('Spend is not metered');
    expect(items[2]).toContain('hybrid-local is unpriced');
    expect(within(panel).queryByTestId('harness-not-weaker')).toBeNull();
    expect(within(panel).getByTestId('harness-facts').textContent).toContain('hybrid-local (unpriced)');
  });

  it('shows an unhealthy harness with its error and an unprobed one as unknown, with no capabilities', async () => {
    const view = mount(createFakeApi({ harnesses: [reference, unhealthy, unprobed] }));
    const panel = await openPanel(view);
    const rows = within(panel).getAllByTestId('harness-row');
    expect(rows.map((r) => r.getAttribute('data-state'))).toEqual(['healthy', 'unhealthy', 'unprobed']);

    expect(within(rows[1]).getByTestId('harness-state').textContent).toBe('UNHEALTHY');
    expect(within(rows[1]).getByTestId('harness-row-error').textContent).toBe('harness-fake not found');
    expect(within(rows[1]).queryByTestId('harness-facts')).toBeNull();

    expect(within(rows[2]).getByTestId('harness-state').textContent).toBe('UNPROBED');
    expect(rows[2].textContent).toContain('Not probed yet');
    expect(within(rows[2]).queryByTestId('harness-facts')).toBeNull();
  });

  it('says so when no harness is registered', async () => {
    const fake = createFakeApi({ harnesses: [] });
    const { getByTestId } = mount(fake);
    await waitFor(() => expect(getByTestId('harness-badge').textContent).toContain('HARNESS 0/0'));
    await fireEvent.click(getByTestId('harness-badge'));
    expect(getByTestId('harness-empty').textContent).toContain('No harness is registered');
  });
});
````

`src/App.harness.test.ts`:

````ts
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { render, cleanup, waitFor } from '@testing-library/svelte';
import App from './App.svelte';
import { fleet } from './lib/store.svelte';
import { api } from './lib/api';
import * as auth from './lib/auth';
import { harnessView, TEST_TOKEN } from './lib/testkit/fakeApi';

// The header in the real app: the harness badge is there, fed by GET /harnesses, and the two
// badges it replaces are gone.

beforeEach(() => {
  fleet.dispose();
  fleet.units = {};
  fleet.order = [];
  fleet.selectedId = null;
  fleet.pendingLaunch = null;
  fleet.connection = 'starting';
  vi.spyOn(auth, 'loadToken').mockResolvedValue(TEST_TOKEN);
  vi.spyOn(api, 'listUnits').mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  fleet.dispose();
  vi.restoreAllMocks();
});

describe('App header', () => {
  it('shows the harness badge with the registry count once connected', async () => {
    const listHarnesses = vi.spyOn(api, 'listHarnesses').mockResolvedValue([
      harnessView(),
      { name: 'fake', state: 'unhealthy', error: 'harness-fake not found' },
    ]);
    const { getByTestId } = render(App);
    await waitFor(() => expect(getByTestId('harness-badge').textContent).toContain('HARNESS 1/2'));
    expect(listHarnesses).toHaveBeenCalled();
    expect(getByTestId('harness-badge').closest('header')).not.toBeNull();
  });

  it('has no Docker badge and no API-key badge', async () => {
    vi.spyOn(api, 'listHarnesses').mockResolvedValue([harnessView()]);
    const { getByTestId, queryByText, queryByTitle } = render(App);
    await waitFor(() => expect(getByTestId('harness-badge').textContent).toContain('HARNESS 1/1'));
    expect(queryByText(/DOCKER/)).toBeNull();
    expect(queryByText(/◉ KEY/)).toBeNull();
    expect(queryByTitle('Docker daemon')).toBeNull();
    expect(queryByTitle('ANTHROPIC_API_KEY set')).toBeNull();
  });
});
````

### Implementation notes

**`src/lib/harness/capabilities.ts`, in full.** This is the part that is easy to get wrong: it
is where the page says something about a guarantee, and it must say only what is declared. It
decides nothing. The daemon decides whether a harness may run a unit.

````ts
// What a harness declares, read for a person: each capability as a labelled value, and a
// plain statement of every guarantee that is weaker than the reference.
//
// The reference is a harness that runs its agents in a container and meters their spend in
// USD. The control plane refuses a harness that falls short of it unless a unit opts in.
// This module decides nothing: the daemon decides eligibility. It only says what is declared.

import type { Capabilities, HarnessView } from '../generated/fleet-api';

/** How much a note matters to a person choosing a harness. */
export type NoteLevel =
  /** Weaker than a container with metering: a unit needs the opt-in to run here. */
  | 'weaker'
  /** Something this harness cannot do; no opt-in changes it. */
  | 'limit'
  /** A risk that is accepted and declared, not a shortfall against the reference. */
  | 'risk';

export interface Note {
  level: NoteLevel;
  /** Which capability the note is about, as the schema names it. */
  capability: keyof Capabilities;
  text: string;
}

export interface Fact {
  /** The label shown beside the value. */
  label: string;
  /** The value as declared, or `none declared` / `not declared`. */
  value: string;
}

const list = (values: readonly string[] | undefined): string =>
  values && values.length > 0 ? values.join(', ') : 'none declared';

/** Every declared capability, in a fixed order, as label and value. */
export function facts(c: Capabilities): Fact[] {
  return [
    { label: 'ISOLATION', value: c.isolation },
    { label: 'METERING', value: c.metering },
    { label: 'NETWORK', value: c.network ?? 'not declared' },
    { label: 'GATES', value: list(c.gates) },
    { label: 'CONTROLS', value: list(c.controls) },
    { label: 'KINDS', value: c.kinds && c.kinds.length > 0 ? c.kinds.join(', ') : 'build' },
    { label: 'PRESETS', value: list((c.presets ?? []).map((p) => `${p.name} ${p.version}`)) },
    {
      label: 'PROFILES',
      value: list((c.profiles ?? []).map((p) => `${p.name} (${p.priced ? 'priced' : 'unpriced'})`)),
    },
    { label: 'DELIVERY', value: c.delivery },
    { label: 'HOLDOUTS', value: c.holdouts ? 'yes' : 'no' },
    { label: 'RESUME', value: c.resume ? 'yes' : 'no' },
    { label: 'HALT', value: c.halt ? 'yes' : 'no' },
  ];
}

/**
 * Every way these capabilities fall short, in a fixed order: the weaker guarantees first,
 * then the limits, then the declared risks.
 */
export function notes(c: Capabilities): Note[] {
  const out: Note[] = [];

  if (c.isolation === 'host') {
    out.push({
      level: 'weaker',
      capability: 'isolation',
      text: 'Agents run on this machine, not in a container: they can read and change whatever this user can.',
    });
  } else if (c.isolation === 'none') {
    out.push({
      level: 'weaker',
      capability: 'isolation',
      text: 'Agents run with no isolation at all.',
    });
  }
  if (c.metering === 'none') {
    out.push({
      level: 'weaker',
      capability: 'metering',
      text: 'Spend is not metered: the USD cap cannot stop a unit on this harness.',
    });
  }
  for (const profile of c.profiles ?? []) {
    if (!profile.priced) {
      out.push({
        level: 'weaker',
        capability: 'profiles',
        text: `Profile ${profile.name} is unpriced: its usage does not count against the USD cap.`,
      });
    }
  }

  if (!c.gates.includes('oracle')) {
    out.push({
      level: 'limit',
      capability: 'gates',
      text: 'No oracle gate: T2 and T3 units are refused.',
    });
  }
  if (c.delivery === 'push') {
    out.push({
      level: 'limit',
      capability: 'delivery',
      text: 'Delivers by pushing: the control plane refuses this harness.',
    });
  }
  if (c.kinds && c.kinds.length > 0 && !c.kinds.includes('build')) {
    out.push({
      level: 'limit',
      capability: 'kinds',
      text: 'Does not run build units.',
    });
  }
  if (!c.presets || c.presets.length === 0) {
    out.push({
      level: 'limit',
      capability: 'presets',
      text: 'Declares no preset: no unit from a signed spec can run here.',
    });
  }
  if (!c.resume) {
    out.push({ level: 'limit', capability: 'resume', text: 'A stopped unit cannot be resumed.' });
  }
  if (!c.halt) {
    out.push({ level: 'limit', capability: 'halt', text: 'A running unit cannot be halted, only abandoned.' });
  }

  if (c.network === 'open') {
    out.push({
      level: 'risk',
      capability: 'network',
      text: 'Agent containers have open network egress.',
    });
  } else if (c.network === undefined || c.network === null) {
    out.push({
      level: 'risk',
      capability: 'network',
      text: 'Network policy is not declared: treat egress as open.',
    });
  }
  return out;
}

/** The notes that make a harness weaker than a container with metering. */
export function weaker(c: Capabilities): Note[] {
  return notes(c).filter((n) => n.level === 'weaker');
}

/** How the header badge should read the whole registry. */
export type RegistryTone = 'ok' | 'attention' | 'bad' | 'idle';

export interface RegistrySummary {
  healthy: number;
  total: number;
  /** Healthy harnesses with at least one weaker guarantee. */
  weaker: number;
  tone: RegistryTone;
}

/**
 * - `idle`: no harness is registered, or none has been listed yet;
 * - `bad`: harnesses are registered and none is healthy;
 * - `attention`: at least one healthy harness is weaker than the reference, or at least one
 *   registered harness is not healthy;
 * - `ok`: every harness is healthy and none is weaker.
 */
export function summarise(rows: readonly HarnessView[]): RegistrySummary {
  const total = rows.length;
  const healthyRows = rows.filter((r) => r.state === 'healthy');
  const healthy = healthyRows.length;
  const weak = healthyRows.filter((r) => r.capabilities && weaker(r.capabilities).length > 0).length;
  let tone: RegistryTone;
  if (total === 0) tone = 'idle';
  else if (healthy === 0) tone = 'bad';
  else if (weak > 0 || healthy < total) tone = 'attention';
  else tone = 'ok';
  return { healthy, total, weaker: weak, tone };
}
````

The wording of each note is this plan's, and the tests match on a phrase from each. A lane may
improve a sentence if it keeps the phrase the test looks for and keeps it a plain statement of
fact. It may not soften one ("may be less isolated") or turn one into advice.

**The two components.** `HarnessBadge.svelte` keeps the stub's props (`api`, `connected`) and
owns the list; `HarnessPanel.svelte` (new) only renders what it is given. What the tests
require:

| Element | `data-testid` | Must have |
|---|---|---|
| The badge | `harness-badge` | a `<button type="button">`; `aria-expanded` (`"true"`/`"false"`); `aria-controls="harness-panel"`; `data-tone` one of `ok`, `attention`, `bad`, `idle`; `aria-label` as below; text `HARNESS <healthy>/<total>`, then `· <n> WEAKER` when `n > 0`; `HARNESS ?` when the list could not be read |
| The panel | `harness-panel` | a `<section id="harness-panel" aria-label="Harnesses" tabindex="-1">` (a labelled section is a region landmark; do not add `role="region"`, the compiler warns that it is redundant); takes focus when it opens; Escape closes it |
| Its buttons | `harness-refresh`, `harness-close` | real buttons |
| One per harness | `harness-row` | `data-name`, `data-state`; the harness's `name` and, when present, `harness.name harness.version` |
| Its health | `harness-state` | text `HEALTHY`, `UNHEALTHY` or `UNPROBED` |
| An unhealthy row's reason | `harness-row-error` | exactly `error` |
| The capabilities | `harness-facts` | a `<dl>` with one `.hp-fact` per fact, each a `<dt>` (the label) and a `<dd>` (the value), in the order `facts` returns |
| The weaker list | `harness-weaker` | a `<ul>`; each `<li>` begins with the word `WEAKER`, then the note's text. Present only when there is one |
| When nothing is weaker | `harness-not-weaker` | text containing `agents run in a container and spend is metered` |
| Limits and risks | `harness-notes` | a `<ul>`; each `<li>` begins with `LIMIT` or `RISK` |
| States of the list | `harness-error`, `harness-loading`, `harness-empty` | `harness-error` has `role="alert"` and contains the failure's message; `harness-empty` contains `No harness is registered` |

The heading above the weaker list is the text `WEAKER THAN A CONTAINER WITH METERING`.

The badge's `aria-label` is `Harnesses: not connected` when not connected; `Harnesses: the
list could not be read` after a failure; otherwise `Harnesses: <healthy> of <total> healthy`,
with `, <n> weaker than a container with metering` appended when `n > 0`.

**Loading, the part with a trap in it.** The badge loads when `connected` becomes true, again
when the panel opens, and on REFRESH. It forgets the list when `connected` becomes false.

````ts
  let rows = $state<HarnessView[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);

  async function load(): Promise<void> {
    try {
      rows = await api.listHarnesses();
      error = null;
    } catch (failure) {
      // Keep the last list on screen, but say that it is stale: an error is never shown as
      // an empty registry.
      error = failure instanceof ApiFailure ? failure.message : 'the request failed';
    }
    loaded = true;
  }

  // Read the registry when the page connects; forget it when the page is not connected.
  $effect(() => {
    if (connected) {
      void load();
    } else {
      rows = [];
      loaded = false;
      error = null;
    }
  });
````

The effect must read `connected` and nothing it writes. If it read `rows`, `loaded` or `error`
it would run again after every load, and the badge would ask the daemon in a loop.

**Existing code.** This lane changes no existing file. Wave 0 removed the two badges
(`src/App.svelte` lines 344 to 351 on `main`) and their four CSS rules (lines 545 to 548), and
mounted `<HarnessBadge {api} connected={connection === 'connected'} />` in their place. It
also stopped the page calling `GET /health`, which is where those badges got their values.

**Where it sits.** The badge is inside the header's `.stats` row, which is a flex row with a
22-pixel gap. The panel is positioned under the badge (`position: absolute; top: calc(100% +
10px); right: 0` inside a `position: relative` wrapper), about 560 pixels wide, at most 72% of
the viewport high, scrolling inside itself. It is inside the page's `.hud` element, so the
approval overlay makes it inert with everything else.

**Pitfalls**

- **Scoped styles.** The old `.hb` rules were deleted from `App.svelte` in wave 0, and would
  not have reached this component anyway. State the badge's and the panel's rules here.
- **Colour is not the message.** Amber for a weaker harness is fine as emphasis. The count and
  the word carry it: `1 WEAKER` on the badge, `WEAKER` at the start of each line.
- **`harness` and `capabilities` are optional *and* nullable** in the generated type
  (`capabilities?: Capabilities | null`). The daemon leaves them out for a harness that is not
  healthy. Test for a value, not for the key.
- **Do not poll.** The registry changes when the daemon starts and when a harness is probed.
  Loading on connect, on open and on REFRESH is enough for milestone 1; a timer here would be
  one more thing to stop on unmount.
- **Do not decide eligibility.** No "this harness can run T2" badge, no disabling of anything
  outside this panel. A note says what is declared, and the daemon's refusal says what follows.
- **The reconnect gap.** The badge has no socket. When the daemon goes away the store's
  `connection` leaves `connected`, the `connected` prop turns false and the badge forgets its
  list; when the store connects again the prop turns true and the effect loads once more. Do
  not add a retry of your own.

### Verify

| Command | Expected |
|---|---|
| `npm run check` | exit 0, 0 errors, 0 warnings (a redundant `role` is a warning: fix it, do not ignore it) |
| `npm run check:types` | `generated types are up to date`: this lane does not touch them |
| `npx vitest run src/lib/harness src/App.harness.test.ts` | `Tests  37 passed (37)` |
| `npx vitest run` | every file passes; 37 more tests than at the branch point |
| `npm run sidecar`, then `cargo test` in `cockpit/ui/src-tauri` | passes, unchanged by this lane: 35 library tests, or 46 if lane UI-TOKEN has merged |
| *Git-Bash:* `grep -rnE "fetch\(\|new WebSocket\(\|text-faint" src/lib/harness` | no output |
| *Git-Bash:* `grep -rnE 'export let\|on:click\|\$:' src/lib/harness` | no output: Svelte 5 only |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 14 pass, with real output in the pull request.
- [ ] The header shows one harness badge and neither of the two badges it replaces.
- [ ] Every capability the schema declares appears in the panel, and every shortfall against a
      container with metering is a sentence beginning `WEAKER`.
- [ ] The pull request includes one screenshot of the open panel, from `npm run dev` with a
      development token against any daemon, or says that no daemon was available to take one.

---

## Lane UI-RAIL

A unit's seven stages, folded from its events; a minimal surface for signing a spec and
dispatching it; and the `reverify` command on a unit that waits for it.

**Owns:** `src/lib/unit/**` (the four stubs `rail.ts`, `StageRail.svelte`,
`ReverifyButton.svelte` and `FactoryPanel.svelte`, which it fills, and every new file beside
them); `src/App.unit.test.ts`.

**Reads:** the control-plane plan, Part 1 Task 8 (`UnitEventDto`, `SpecView`, `SignRequest`,
`SignResponse`, `MissionRequest`, `MissionResponse`, `ApiError`), lane CC-API ("Reading and
signing a spec" and the refusal statuses) and lane CC-SUPERVISOR's wording table (the reason
`verification stopped`); `src/lib/generated/fleet-api.ts`; `src/lib/api.ts`;
`src/lib/fleet.ts` (`Unit`, and where `foldRail` is called); `src/lib/store.svelte.ts`
(`dispatch`, `cmd`; read only); `src/lib/testkit/fakeApi.ts`; `src/App.svelte` (where the
three components are mounted and the styles around them; read only).

**Worktree and branch:** `D:\MajorProjects\.swarm-wt\m1-ui-rail`, branch `feat/m1-ui-rail`,
created with `git worktree add --no-track`, cut from `origin/factory/m1` after wave 0 merges.
Pull request against `factory/m1`.

**Needs:** Part 1 merged. Nothing from the other lanes.

**Blocks:** the operator's smoke.

### Behaviours

The fold (`src/lib/unit/rail.test.ts`, 36 tests):

1. There are seven stages, in order, and a new rail has seven pending cells with nothing
   measured. *the seven stages* (two tests)
2. A `started` note makes a stage running, a `finished` note makes it done, a note's detail is
   kept, a unit that passes all seven ends with seven done cells, later stages stay pending,
   and a stage that starts again is running again. *stage notes* (five tests)
3. Each stage gets the spend and the metered time the unit added while it ran, and the stages
   together account for exactly the unit's total. A stage no metric arrived in shows nothing,
   not zero. A metric between two stages belongs to the one that just finished. A metric
   before any stage is charged to none. A repeated metric adds nothing. A stage visited twice
   accumulates. *cost and time per stage* (seven tests)
4. Adapter and model are absent when no metric names them, which is always in milestone 1, and
   shown when one does. A metric that names its own stage is attributed there. A stage name
   the page does not know is ignored. *adapter and model* (four tests)
5. When a unit stops for a person, is halted or fails, the running stage is marked stopped; it
   runs again when it starts again; a phase change that is not a stop, or a stop with nothing
   running, changes nothing. *a unit that stops* (six tests)
6. The fold never mutates its input, and returns the same object for a record it does not
   read. *the fold itself* (two tests)
7. Time reads `—`, `0.8s`, `42s`, `3m 05s`, `1h 02m`; cost reads `—` or dollars to three
   places. *formatting* (ten tests)

The rail (`src/lib/unit/StageRail.test.ts`, six tests):

8. Seven rows in order, each with its state as a word; a table with a caption and scoped
   headers; done, running, stopped and waiting as different words, the running row marked;
   each stage's time and cost, with a dash where nothing was metered; a dash for adapter and
   model unless a metric named them.

Re-verify (`src/lib/unit/ReverifyButton.test.ts`, eight tests):

9. A unit waits for a re-verify exactly when it is `needs_human` with the reason
   `verification stopped`. *which units wait for a re-verify*
10. The control renders nothing for any other unit; for a waiting one it offers RE-VERIFY and
    says where the unit stopped; it sends the command once per press and not again while one
    is in flight; it says so when the daemon answers `gone`, `unknown_unit` or `invalid`; and
    it shows the daemon's own words when the request is refused.

Sign and dispatch (`src/lib/unit/FactoryPanel.test.ts`, 20 tests):

11. The page's hash is the SHA-256 of the UTF-8 bytes, as the daemon computes it. *the page
    hash*
12. A spec is shown exactly as returned (trailing spaces, tabs, CRLF, no final newline), with
    the commit it was read from; a named commit is asked for when typed; a refusal is shown in
    the daemon's words; nothing is asked without a repository and a spec id; every input has a
    visible label. *reading a spec* (five tests)
13. Signing sends the commit that was read and the SHA-256 of the bytes shown. *signing* ›
    *sends the commit…*
14. The page will not sign when the text shown is not the bytes the daemon hashed. *will not
    sign when…*
15. The problems the daemon found are shown, and a spec with any is not offered for signing;
    a refused signature is shown in the daemon's words with the problems it returned; a hash
    mismatch marks nothing as signed; a spec already signed is shown as signed. *signing*
    (four more tests)
16. Dispatch sends the work item, the signed hash, the harness and an idempotency key made on
    the page, and no tier. *dispatching* › *sends the work item…*
17. Only harnesses from the daemon's list are offered, and only healthy ones can be chosen;
    the chosen harness's profiles are offered, marked priced or unpriced, and the one picked
    is sent with the opt-in. *offers only harnesses…*, *offers the chosen harness's profiles…*
18. A refusal's reason is shown verbatim, and the same key is used for the next press; after a
    unit starts, the next dispatch uses a new key; a second press while the first is in flight
    sends nothing. *shows a refusal's reason verbatim…*, *after a unit starts…*, *a second
    press…*
19. With no healthy harness nothing can be dispatched, and a list that cannot be read says
    why; reading another spec clears the last signature, refusal and result. *cannot dispatch
    when…*, *reading another spec…*

The real page (`src/App.unit.test.ts`, three tests):

20. The selected unit's rail moves through all seven stages as records arrive, then the
    pull-request link appears; RE-VERIFY is offered only while the unit waits for it and sends
    `reverify` for that unit; a unit stopped for another reason is not offered it.

| Tests | Command | The failure to expect first |
|---|---|---|
| 1 to 7 | `npx vitest run src/lib/unit/rail.test.ts` | `29 failed \| 7 passed (36)`. The stub's `foldRail` returns its input, so every test that expects a change fails on its first assertion (`expected 'pending' to be 'running'`); the ten *formatting* tests read `TypeError: (0 , formatElapsed) is not a function` or `formatCost`. The seven that pass are the ones an identity fold satisfies: the two of *the seven stages*, *are absent in milestone 1*, the two "changes nothing" tests, and the two of *the fold itself* |
| 8 | `npx vitest run src/lib/unit/StageRail.test.ts` | `6 failed`: the stub is a hidden empty `<div>`; `Unable to find an element by: [data-testid="stage-provision"]` and the like |
| 9, 10 | `npx vitest run src/lib/unit/ReverifyButton.test.ts` | `7 failed \| 1 passed (8)`: `awaitsReverify` and `VERIFY_STOPPED` are not exported, and `Unable to find an element by: [data-testid="reverify-go"]`. *renders nothing for a unit that is not waiting* passes: the stub renders nothing for any unit |
| 11 to 19 | `npx vitest run src/lib/unit/FactoryPanel.test.ts` | `Error: Failed to resolve import "./digest" from "src/lib/unit/FactoryPanel.test.ts". Does the file exist?` and no test runs. Once `digest.ts` exists: `19 failed \| 1 passed (20)`. *the page hash* passes; eighteen read `Unable to find an element by: [data-testid="spec-repo"]` and one `[data-testid="spec-load"]` |
| 20 | `npx vitest run src/App.unit.test.ts` | `2 failed \| 1 passed (3)`: `[data-testid="stage-provision"]` and `[data-testid="reverify-go"]` are not found. The third passes against the stubs, because it asserts an absence |

`src/lib/unit/rail.test.ts`:

````ts
import { describe, it, expect } from 'vitest';
import { foldRail, formatCost, formatElapsed, newRail, STAGES, type RailState } from './rail';
import type { Stage, UnitEventDto } from '../generated/fleet-api';

const started = (stage: Stage, detail?: string): UnitEventDto =>
  detail === undefined ? { type: 'stage', stage, status: 'started' } : { type: 'stage', stage, status: 'started', detail };
const finished = (stage: Stage): UnitEventDto => ({ type: 'stage', stage, status: 'finished' });
/** A metric as the daemon sends it: cumulative for the unit. */
const metric = (cost_usd: number, elapsed_ms: number): UnitEventDto => ({
  type: 'metric',
  tokens_in: 0,
  tokens_out: 0,
  cost_usd,
  elapsed_ms,
});

/**
 * A metric with fields a later schema adds (`stage`, `adapter`, `model`). The schema does not
 * declare them yet, so the cast is the only way to build one.
 */
const metricWith = (cost_usd: number, elapsed_ms: number, extra: Record<string, string>): UnitEventDto =>
  ({ ...metric(cost_usd, elapsed_ms), ...extra }) as unknown as UnitEventDto;

function run(events: UnitEventDto[], from: RailState = newRail()): RailState {
  return events.reduce(foldRail, from);
}

const cell = (rail: RailState, stage: Stage) => rail.cells.find((c) => c.stage === stage)!;
const states = (rail: RailState) => rail.cells.map((c) => c.state);

describe('the seven stages', () => {
  it('are provision, red, plan, green, check, review and deliver, in that order', () => {
    expect([...STAGES]).toEqual(['provision', 'red', 'plan', 'green', 'check', 'review', 'deliver']);
  });

  it('a new rail has seven pending cells with nothing measured', () => {
    const rail = newRail();
    expect(rail.cells.map((c) => c.stage)).toEqual([...STAGES]);
    expect(states(rail)).toEqual(Array(7).fill('pending'));
    for (const c of rail.cells) {
      expect(c).toMatchObject({ detail: null, elapsedMs: null, costUsd: null, adapter: null, model: null });
    }
    expect(rail.current).toBeNull();
  });
});

describe('stage notes', () => {
  it('started makes a stage running and finished makes it done', () => {
    let rail = run([started('provision')]);
    expect(cell(rail, 'provision').state).toBe('running');
    expect(rail.current).toBe('provision');
    rail = run([finished('provision')], rail);
    expect(cell(rail, 'provision').state).toBe('done');
    expect(rail.current).toBeNull();
    expect(rail.last).toBe('provision');
  });

  it('keeps the detail of a note', () => {
    const rail = run([started('red', 'baseline')]);
    expect(cell(rail, 'red').detail).toBe('baseline');
    // A finish with no detail keeps what the start said.
    expect(cell(run([finished('red')], rail), 'red').detail).toBe('baseline');
  });

  it('a unit that passes all seven stages ends with seven done cells', () => {
    const rail = run(STAGES.flatMap((s) => [started(s), finished(s)]));
    expect(states(rail)).toEqual(Array(7).fill('done'));
    expect(rail.current).toBeNull();
    expect(rail.last).toBe('deliver');
  });

  it('stages not yet reached stay pending while one runs', () => {
    const rail = run([started('provision'), finished('provision'), started('red')]);
    expect(states(rail)).toEqual(['done', 'running', 'pending', 'pending', 'pending', 'pending', 'pending']);
  });

  it('a stage that starts again after finishing is running again', () => {
    // A failed check sends the unit back to green.
    const rail = run([started('green'), finished('green'), started('check'), finished('check'), started('green')]);
    expect(cell(rail, 'green').state).toBe('running');
    expect(cell(rail, 'check').state).toBe('done');
    expect(rail.current).toBe('green');
  });
});

describe('cost and time per stage', () => {
  it('gives each stage what the unit spent while it ran', () => {
    const rail = run([
      started('red'),
      metric(0.1, 4_000),
      metric(0.25, 9_000),
      finished('red'),
      started('plan'),
      metric(0.3, 11_500),
      finished('plan'),
    ]);
    expect(cell(rail, 'red').costUsd).toBeCloseTo(0.25, 10);
    expect(cell(rail, 'red').elapsedMs).toBe(9_000);
    expect(cell(rail, 'plan').costUsd).toBeCloseTo(0.05, 10);
    expect(cell(rail, 'plan').elapsedMs).toBe(2_500);
  });

  it('the stages together account for exactly the unit total', () => {
    const rail = run([
      started('provision'),
      finished('provision'),
      started('red'),
      metric(0.2, 5_000),
      finished('red'),
      started('green'),
      metric(0.9, 61_000),
      metric(1.4, 95_000),
      finished('green'),
    ]);
    const cost = rail.cells.reduce((sum, c) => sum + (c.costUsd ?? 0), 0);
    const elapsed = rail.cells.reduce((sum, c) => sum + (c.elapsedMs ?? 0), 0);
    expect(cost).toBeCloseTo(1.4, 10);
    expect(elapsed).toBe(95_000);
    expect(rail.seenCostUsd).toBe(1.4);
  });

  it('a stage no metric arrived in shows nothing, not zero', () => {
    const rail = run([started('provision'), finished('provision'), started('red'), metric(0.1, 1_000)]);
    expect(cell(rail, 'provision').costUsd).toBeNull();
    expect(cell(rail, 'provision').elapsedMs).toBeNull();
    expect(cell(rail, 'red').costUsd).toBeCloseTo(0.1, 10);
  });

  it('a metric between two stages belongs to the one that just finished', () => {
    const rail = run([started('review'), finished('review'), metric(0.4, 20_000), started('deliver')]);
    expect(cell(rail, 'review').costUsd).toBeCloseTo(0.4, 10);
    expect(cell(rail, 'deliver').costUsd).toBeNull();
  });

  it('a metric before any stage is attributed to none, and is not charged to the first stage later', () => {
    const rail = run([metric(0.05, 500), started('provision'), metric(0.07, 900)]);
    expect(cell(rail, 'provision').costUsd).toBeCloseTo(0.02, 10);
    expect(cell(rail, 'provision').elapsedMs).toBe(400);
  });

  it('a repeated metric adds nothing, and a counter that goes backwards is not subtracted', () => {
    const rail = run([started('green'), metric(0.5, 10_000), metric(0.5, 10_000), metric(0.3, 8_000)]);
    expect(cell(rail, 'green').costUsd).toBeCloseTo(0.5, 10);
    expect(cell(rail, 'green').elapsedMs).toBe(10_000);
    expect(rail.seenCostUsd).toBe(0.5);
  });

  it('a stage visited twice accumulates across both visits', () => {
    const rail = run([
      started('green'),
      metric(0.5, 10_000),
      finished('green'),
      started('check'),
      metric(0.6, 12_000),
      finished('check'),
      started('green'),
      metric(0.9, 20_000),
    ]);
    expect(cell(rail, 'green').costUsd).toBeCloseTo(0.8, 10);
    expect(cell(rail, 'green').elapsedMs).toBe(18_000);
    expect(cell(rail, 'check').costUsd).toBeCloseTo(0.1, 10);
  });
});

describe('adapter and model', () => {
  it('are absent in milestone 1, where a metric names neither', () => {
    const rail = run([started('green'), metric(0.5, 10_000)]);
    expect(cell(rail, 'green')).toMatchObject({ adapter: null, model: null });
  });

  it('are shown for the stage when a metric carries them', () => {
    const withNames = metricWith(0.5, 10_000, { adapter: 'claude-code', model: 'sonnet' });
    const rail = run([started('green'), withNames, metric(0.6, 11_000)]);
    expect(cell(rail, 'green')).toMatchObject({ adapter: 'claude-code', model: 'sonnet' });
    expect(cell(rail, 'green').costUsd).toBeCloseTo(0.6, 10);
  });

  it('a metric that names its own stage is attributed there, whatever is running', () => {
    const late = metricWith(0.2, 3_000, { stage: 'red' });
    const rail = run([started('red'), finished('red'), started('plan'), late]);
    expect(cell(rail, 'red').costUsd).toBeCloseTo(0.2, 10);
    expect(cell(rail, 'plan').costUsd).toBeNull();
  });

  it('a stage name it does not know is ignored, not trusted', () => {
    const odd = metricWith(0.2, 3_000, { stage: 'teleport', adapter: '' });
    const rail = run([started('plan'), odd]);
    expect(cell(rail, 'plan').costUsd).toBeCloseTo(0.2, 10);
    expect(cell(rail, 'plan').adapter).toBeNull();
  });
});

describe('a unit that stops', () => {
  it.each(['needs_human', 'halted', 'failed'] as const)(
    'marks the running stage stopped when the unit becomes %s',
    (to) => {
      const rail = run([
        started('provision'),
        finished('provision'),
        started('check'),
        { type: 'phase_changed', from: 'merge_check', to, reason: 'verification stopped' },
      ]);
      expect(cell(rail, 'check').state).toBe('stopped');
      expect(cell(rail, 'provision').state).toBe('done');
      expect(rail.current).toBeNull();
      expect(rail.last).toBe('check');
    },
  );

  it('a stage that was stopped runs again when it starts again', () => {
    const rail = run([
      started('check'),
      { type: 'phase_changed', from: 'checking', to: 'needs_human' },
      { type: 'phase_changed', from: 'needs_human', to: 'checking' },
      started('check'),
    ]);
    expect(cell(rail, 'check').state).toBe('running');
  });

  it('a phase change that is not a stop changes nothing', () => {
    const before = run([started('green')]);
    const after = foldRail(before, { type: 'phase_changed', from: 'building', to: 'checking' });
    expect(after).toBe(before);
  });

  it('a stop with no stage running changes nothing', () => {
    const before = run([started('red'), finished('red')]);
    expect(foldRail(before, { type: 'phase_changed', from: 'spec', to: 'halted' })).toBe(before);
  });
});

describe('the fold itself', () => {
  it('never mutates the rail it is given', () => {
    const frozen = newRail();
    Object.freeze(frozen);
    Object.freeze(frozen.cells);
    frozen.cells.forEach((c) => Object.freeze(c));
    expect(() =>
      run([started('red'), metric(0.1, 100), finished('red'), { type: 'phase_changed', from: 'spec', to: 'failed' }], frozen),
    ).not.toThrow();
    expect(states(frozen)).toEqual(Array(7).fill('pending'));
  });

  it('returns the same object for a record the rail does not read', () => {
    const rail = run([started('red')]);
    expect(foldRail(rail, { type: 'log', stream: 'agent', line: 'hello' })).toBe(rail);
    expect(foldRail(rail, { type: 'done', result: 'done' })).toBe(rail);
  });
});

describe('formatting', () => {
  it.each([
    [null, '—'],
    [0, '0.0s'],
    [850, '0.8s'],
    [42_000, '42s'],
    [185_000, '3m 05s'],
    [3_725_000, '1h 02m'],
  ] as const)('formatElapsed(%j) is %s', (ms, text) => {
    expect(formatElapsed(ms)).toBe(text);
  });

  it.each([
    [null, '—'],
    [0, '$0.000'],
    [0.0421, '$0.042'],
    [12.5, '$12.500'],
  ] as const)('formatCost(%j) is %s', (usd, text) => {
    expect(formatCost(usd)).toBe(text);
  });
});
````

`src/lib/unit/StageRail.test.ts`:

````ts
import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import StageRail from './StageRail.svelte';
import { foldRail, newRail, STAGES, type RailState } from './rail';
import type { UnitEventDto } from '../generated/fleet-api';

afterEach(cleanup);

function rail(events: UnitEventDto[]): RailState {
  return events.reduce(foldRail, newRail());
}

const metric = (cost_usd: number, elapsed_ms: number): UnitEventDto => ({
  type: 'metric',
  tokens_in: 0,
  tokens_out: 0,
  cost_usd,
  elapsed_ms,
});

/**
 * A metric with fields a later schema adds (`stage`, `adapter`, `model`). The schema does not
 * declare them yet, so the cast is the only way to build one.
 */
const metricWith = (cost_usd: number, elapsed_ms: number, extra: Record<string, string>): UnitEventDto =>
  ({ ...metric(cost_usd, elapsed_ms), ...extra }) as unknown as UnitEventDto;

describe('StageRail.svelte', () => {
  it('shows seven stages in order, each with a state in words', () => {
    const { getByTestId } = render(StageRail, { props: { rail: newRail() } });
    const rows = [...getByTestId('stage-rail').querySelectorAll('tbody tr')];
    expect(rows.map((r) => r.getAttribute('data-testid'))).toEqual(STAGES.map((s) => `stage-${s}`));
    expect(rows.map((r) => r.querySelector('th')?.textContent)).toEqual([
      'PROVISION', 'RED', 'PLAN', 'GREEN', 'CHECK', 'REVIEW', 'DELIVER',
    ]);
    for (const row of rows) {
      expect(row.getAttribute('data-state')).toBe('pending');
      expect(row.querySelector('.state')?.textContent).toContain('WAIT');
    }
  });

  it('is a table with a caption and column headers a screen reader can use', () => {
    const { getByTestId } = render(StageRail, { props: { rail: newRail() } });
    const table = getByTestId('stage-rail');
    expect(table.tagName).toBe('TABLE');
    expect(table.querySelector('caption')?.textContent).toContain('STAGES');
    expect([...table.querySelectorAll('thead th')].map((th) => [th.textContent, th.getAttribute('scope')])).toEqual([
      ['STAGE', 'col'],
      ['STATE', 'col'],
      ['TIME', 'col'],
      ['COST', 'col'],
      ['ADAPTER · MODEL', 'col'],
    ]);
    expect(table.querySelector('tbody th')?.getAttribute('scope')).toBe('row');
  });

  it('shows done, running, stopped and waiting stages as different words, and marks the running one', () => {
    const { getByTestId } = render(StageRail, {
      props: {
        rail: rail([
          { type: 'stage', stage: 'provision', status: 'started' },
          { type: 'stage', stage: 'provision', status: 'finished' },
          { type: 'stage', stage: 'red', status: 'started', detail: 'baseline' },
        ]),
      },
    });
    expect(getByTestId('stage-provision').textContent).toContain('DONE');
    const red = getByTestId('stage-red');
    expect(red.textContent).toContain('RUN');
    expect(red.getAttribute('aria-current')).toBe('step');
    expect(red.getAttribute('title')).toBe('baseline');
    expect(getByTestId('stage-plan').textContent).toContain('WAIT');
    expect(getByTestId('stage-provision').getAttribute('aria-current')).toBeNull();
  });

  it('shows STOP for the stage a unit stopped in', () => {
    const { getByTestId } = render(StageRail, {
      props: {
        rail: rail([
          { type: 'stage', stage: 'check', status: 'started' },
          { type: 'phase_changed', from: 'merge_check', to: 'needs_human', reason: 'verification stopped' },
        ]),
      },
    });
    expect(getByTestId('stage-check').getAttribute('data-state')).toBe('stopped');
    expect(getByTestId('stage-check').textContent).toContain('STOP');
  });

  it('shows each stage time and cost, and a dash where nothing was metered', () => {
    const { getByTestId } = render(StageRail, {
      props: {
        rail: rail([
          { type: 'stage', stage: 'provision', status: 'started' },
          { type: 'stage', stage: 'provision', status: 'finished' },
          { type: 'stage', stage: 'red', status: 'started' },
          metric(0.0421, 185_000),
        ]),
      },
    });
    expect(getByTestId('stage-red-time').textContent).toBe('3m 05s');
    expect(getByTestId('stage-red-cost').textContent).toBe('$0.042');
    expect(getByTestId('stage-provision-time').textContent).toBe('—');
    expect(getByTestId('stage-provision-cost').textContent).toBe('—');
  });

  it('shows a dash for adapter and model when no metric names them, and the names when one does', () => {
    const named = metricWith(0.5, 10_000, { adapter: 'claude-code', model: 'sonnet' });
    const { getByTestId } = render(StageRail, {
      props: {
        rail: rail([
          { type: 'stage', stage: 'red', status: 'started' },
          metric(0.1, 1_000),
          { type: 'stage', stage: 'red', status: 'finished' },
          { type: 'stage', stage: 'green', status: 'started' },
          named,
        ]),
      },
    });
    expect(getByTestId('stage-red-runtime').textContent?.trim()).toBe('—');
    expect(getByTestId('stage-green-runtime').textContent?.trim()).toBe('claude-code · sonnet');
  });
});
````

`src/lib/unit/ReverifyButton.test.ts`:

````ts
import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import ReverifyButton, { awaitsReverify, VERIFY_STOPPED } from './ReverifyButton.svelte';
import { ApiFailure, type CommandOutcome } from '../api';
import { newUnit, type Unit } from '../fleet';
import type { PhaseDto } from '../generated/fleet-api';

afterEach(cleanup);

function unit(phase: PhaseDto, pauseReason: string | null, pausedFrom: PhaseDto | null = null): Unit {
  const u = newUnit('u1', 'owner/sandbox#1', 'T1');
  u.phase = phase;
  u.pauseReason = pauseReason;
  u.pausedFrom = pausedFrom;
  return u;
}

const waiting = () => unit('needs_human', VERIFY_STOPPED, 'merge_check');

describe('which units wait for a re-verify', () => {
  it('is exactly: stopped for a person, with the reason the daemon gives a stopped verification', () => {
    expect(VERIFY_STOPPED).toBe('verification stopped');
    expect(awaitsReverify(waiting())).toBe(true);
    expect(awaitsReverify(unit('needs_human', 'usd cap', 'building'))).toBe(false);
    expect(awaitsReverify(unit('needs_human', 'oracle tampering', 'merge_check'))).toBe(false);
    expect(awaitsReverify(unit('needs_human', null))).toBe(false);
    expect(awaitsReverify(unit('halted', VERIFY_STOPPED, 'merge_check'))).toBe(false);
    expect(awaitsReverify(unit('merge_check', null))).toBe(false);
    expect(awaitsReverify(unit('failed', VERIFY_STOPPED))).toBe(false);
  });
});

describe('ReverifyButton.svelte', () => {
  it('renders nothing for a unit that is not waiting for it', () => {
    const { queryByTestId } = render(ReverifyButton, {
      props: { unit: unit('needs_human', 'usd cap', 'building'), onreverify: async () => 'accepted' as const },
    });
    expect(queryByTestId('reverify')).toBeNull();
    expect(queryByTestId('reverify-go')).toBeNull();
  });

  it('offers RE-VERIFY on a unit whose verification stopped, and says where it stopped', () => {
    const { getByTestId } = render(ReverifyButton, {
      props: { unit: waiting(), onreverify: async () => 'accepted' as const },
    });
    expect(getByTestId('reverify').textContent).toContain('Verification stopped');
    expect(getByTestId('reverify').textContent).toContain('merge_check');
    const go = getByTestId('reverify-go') as HTMLButtonElement;
    expect(go.tagName).toBe('BUTTON');
    expect(go.type).toBe('button');
    expect(go.textContent?.trim()).toBe('RE-VERIFY');
    expect(go.disabled).toBe(false);
  });

  it('sends the command once per press and cannot be pressed twice while it is being sent', async () => {
    let release: (outcome: CommandOutcome) => void = () => {};
    const onreverify = vi.fn(
      () => new Promise<CommandOutcome>((resolve) => { release = resolve; }),
    );
    const { getByTestId, queryByTestId } = render(ReverifyButton, { props: { unit: waiting(), onreverify } });
    const go = getByTestId('reverify-go') as HTMLButtonElement;
    await fireEvent.click(go);
    expect(go.disabled).toBe(true);
    expect(go.textContent?.trim()).toBe('RE-VERIFYING…');
    await fireEvent.click(go);
    expect(onreverify).toHaveBeenCalledTimes(1);
    release('accepted');
    await waitFor(() => expect(go.disabled).toBe(false));
    expect(queryByTestId('reverify-problem')).toBeNull();
  });

  it.each([
    ['gone', 'no longer holds'],
    ['unknown_unit', 'does not know this unit'],
    ['invalid', 'did not accept'],
  ] as const)('says so when the daemon answers %s', async (outcome, phrase) => {
    const { getByTestId } = render(ReverifyButton, {
      props: { unit: waiting(), onreverify: async () => outcome },
    });
    await fireEvent.click(getByTestId('reverify-go'));
    await waitFor(() => expect(getByTestId('reverify-problem').textContent).toContain(phrase));
    expect(getByTestId('reverify-problem').getAttribute('role')).toBe('alert');
  });

  it('shows the daemon\'s own words when the request is refused', async () => {
    const { getByTestId } = render(ReverifyButton, {
      props: {
        unit: waiting(),
        onreverify: async () => {
          throw new ApiFailure(503, 'store', 'the store cannot be read right now');
        },
      },
    });
    await fireEvent.click(getByTestId('reverify-go'));
    await waitFor(() =>
      expect(getByTestId('reverify-problem').textContent).toBe('the store cannot be read right now'),
    );
  });
});
````

`src/lib/unit/FactoryPanel.test.ts`:

````ts
import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, cleanup, fireEvent, waitFor } from '@testing-library/svelte';
import FactoryPanel from './FactoryPanel.svelte';
import { sha256Hex as pageHash } from './digest';
import { ApiFailure } from '../api';
import {
  createFakeApi,
  harnessView,
  sha256Hex,
  SIGNED_AT_MS,
  specView,
  type FakeFleetApi,
} from '../testkit/fakeApi';
import type { MissionRequest, MissionResponse, SpecView, TierDto } from '../generated/fleet-api';

afterEach(cleanup);

type Dispatch = (request: MissionRequest, tier: TierDto | null) => Promise<MissionResponse>;

/** Mount the panel over a fake whose dispatches go to the fake's own `createMission`. */
function mount(fake: FakeFleetApi, ondispatch?: Dispatch) {
  const dispatched: [MissionRequest, TierDto | null][] = [];
  const view = render(FactoryPanel, {
    props: {
      api: fake,
      ondispatch:
        ondispatch ??
        (async (request, tier) => {
          dispatched.push([request, tier]);
          return fake.createMission(request);
        }),
    },
  });
  return { ...view, dispatched };
}

async function type(el: HTMLElement, value: string) {
  await fireEvent.input(el, { target: { value } });
}

async function read(view: ReturnType<typeof mount>, spec: SpecView, commit = '') {
  await type(view.getByTestId('spec-repo'), spec.repo);
  await type(view.getByTestId('spec-id'), spec.spec_id);
  await type(view.getByTestId('spec-commit'), commit);
  await fireEvent.click(view.getByTestId('spec-load'));
  await waitFor(() => expect(view.getByTestId('spec-text')).not.toBeNull());
}

async function readAndSign(view: ReturnType<typeof mount>, spec: SpecView) {
  await read(view, spec);
  await waitFor(() => expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(false));
  await fireEvent.click(view.getByTestId('spec-sign'));
  await waitFor(() => expect(view.getByTestId('dispatch-form')).not.toBeNull());
  await waitFor(() => expect((view.getByTestId('dispatch-go') as HTMLButtonElement).disabled).toBe(false));
}

const calls = (fake: FakeFleetApi, method: string) => fake.calls.filter((c) => c.method === method);

describe('the page hash', () => {
  it('is the SHA-256 of the UTF-8 bytes, as lowercase hex, the same as the daemon computes', async () => {
    expect(await pageHash('')).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
    expect(await pageHash('abc')).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    const text = 'café ✓\r\nline two\n';
    expect(await pageHash(text)).toBe(sha256Hex(text));
  });
});

describe('reading a spec', () => {
  it('shows the text exactly as the daemon returned it, and the commit it was read from', async () => {
    // Trailing spaces, a tab, CRLF and no final newline: none of it may be tidied.
    const text = '---\r\nid: SPEC-0001\r\n---\r\n# Cart total  \r\n\tindented\r\nlast line';
    const spec = specView({ text });
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);

    expect(view.getByTestId('spec-text').textContent).toBe(text);
    expect(view.getByTestId('spec-text').tagName).toBe('PRE');
    expect(view.getByTestId('spec-commit-read').textContent).toBe(spec.commit);
    expect(view.getByTestId('spec-sha256').textContent).toBe(spec.sha256);
    expect(view.getByTestId('spec-tier').textContent).toBe('T1');
    expect(view.getByTestId('spec-work-item').textContent).toBe('owner/sandbox#1');
    expect(calls(fake, 'getSpec')[0].args[0]).toEqual({ repo: 'owner/sandbox', spec_id: 'SPEC-0001' });
  });

  it('asks for a named commit when one is typed, and shows the commit the answer names', async () => {
    const spec = specView({ commit: 'abc1234abc1234abc1234abc1234abc1234abc12' });
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec, ' abc1234abc1234abc1234abc1234abc1234abc12 ');
    expect(calls(fake, 'getSpec')[0].args[0]).toEqual({
      repo: 'owner/sandbox',
      spec_id: 'SPEC-0001',
      commit: 'abc1234abc1234abc1234abc1234abc1234abc12',
    });
    expect(view.getByTestId('spec-commit-read').textContent).toBe('abc1234abc1234abc1234abc1234abc1234abc12');
  });

  it('shows the daemon\'s refusal in its own words when the spec cannot be read', async () => {
    const fake = createFakeApi();
    fake.fail('getSpec', new ApiFailure(403, 'repo_not_allowed', 'owner/elsewhere is not a repository this daemon serves'));
    const view = mount(fake);
    await type(view.getByTestId('spec-repo'), 'owner/elsewhere');
    await type(view.getByTestId('spec-id'), 'SPEC-0001');
    await fireEvent.click(view.getByTestId('spec-load'));
    await waitFor(() => expect(view.getByTestId('spec-load-error')).not.toBeNull());
    const error = view.getByTestId('spec-load-error');
    expect(error.textContent).toBe('owner/elsewhere is not a repository this daemon serves');
    expect(error.getAttribute('data-code')).toBe('repo_not_allowed');
    expect(error.getAttribute('role')).toBe('alert');
    expect(view.queryByTestId('spec-text')).toBeNull();
  });

  it('cannot be asked to read without a repository and a spec id', async () => {
    const view = mount(createFakeApi());
    const load = view.getByTestId('spec-load') as HTMLButtonElement;
    expect(load.disabled).toBe(true);
    await type(view.getByTestId('spec-repo'), 'owner/sandbox');
    expect(load.disabled).toBe(true);
    await type(view.getByTestId('spec-id'), 'SPEC-0001');
    expect(load.disabled).toBe(false);
  });

  it('every input has a visible label', () => {
    const view = mount(createFakeApi());
    for (const id of ['spec-repo', 'spec-id', 'spec-commit']) {
      const input = view.getByTestId(id);
      expect(input.closest('label')?.querySelector('span')?.textContent?.length, id).toBeGreaterThan(0);
    }
  });
});

describe('signing', () => {
  it('sends the commit that was read and the SHA-256 of the bytes shown', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);
    await waitFor(() => expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(false));
    await fireEvent.click(view.getByTestId('spec-sign'));
    await waitFor(() => expect(view.getByTestId('spec-signature')).not.toBeNull());

    expect(calls(fake, 'sign')).toHaveLength(1);
    expect(calls(fake, 'sign')[0].args[0]).toEqual({
      repo: 'owner/sandbox',
      spec_id: 'SPEC-0001',
      commit: spec.commit,
      sha256: sha256Hex(view.getByTestId('spec-text').textContent ?? ''),
    });
    expect(view.getByTestId('spec-signature').textContent).toContain('SIGNED by tester');
    expect(view.getByTestId('spec-signature').textContent).toContain(new Date(SIGNED_AT_MS).toISOString());
    expect(view.queryByTestId('spec-sign')).toBeNull();
  });

  it('will not sign when the text shown is not the bytes the daemon hashed', async () => {
    // What the daemon sends when the file is not UTF-8: replacement characters, and the hash
    // of the real bytes. The page must not offer to sign text nobody could have read.
    const spec = specView({ text: 'price: ��\n', sha256: 'f'.repeat(64) });
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);
    expect(view.getByTestId('spec-integrity').getAttribute('role')).toBe('alert');
    expect(view.getByTestId('spec-integrity').textContent).toContain('f'.repeat(64));
    expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(view.getByTestId('spec-sign'));
    expect(calls(fake, 'sign')).toHaveLength(0);
  });

  it('shows the problems the daemon found on reading, and will not sign a spec that has any', async () => {
    const spec = specView({
      problems: [
        { code: 'open_questions', message: 'An open question remains: Which currency?' },
        { code: 'unconfirmed_invariants', message: 'An invariant is unconfirmed: Totals round down.' },
      ],
    });
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);
    const items = [...view.getByTestId('spec-problems').querySelectorAll('li')];
    expect(items.map((li) => li.getAttribute('data-code'))).toEqual(['open_questions', 'unconfirmed_invariants']);
    expect(items[0].textContent).toContain('An open question remains: Which currency?');
    expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(true);
    expect(view.queryByTestId('dispatch-form')).toBeNull();
  });

  it('shows a refused signature in the daemon\'s words, with the problems it returned', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);
    await waitFor(() => expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(false));
    fake.failNext(
      'sign',
      new ApiFailure(422, 'spec_not_ready', 'the spec is not ready to sign', [
        { code: 'form_not_declared', message: 'src/cart.js is an interface file of Form cart' },
      ]),
    );
    await fireEvent.click(view.getByTestId('spec-sign'));
    await waitFor(() => expect(view.getByTestId('sign-error')).not.toBeNull());
    expect(view.getByTestId('sign-error').textContent).toBe('the spec is not ready to sign');
    expect(view.getByTestId('sign-error').getAttribute('data-code')).toBe('spec_not_ready');
    expect(view.getByTestId('spec-problems').textContent).toContain('src/cart.js is an interface file of Form cart');
    expect(view.queryByTestId('spec-signature')).toBeNull();
    expect(view.queryByTestId('dispatch-form')).toBeNull();
  });

  it('a hash mismatch from the daemon is shown verbatim and nothing is marked signed', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec] });
    const view = mount(fake);
    await read(view, spec);
    await waitFor(() => expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(false));
    fake.failNext('sign', new ApiFailure(409, 'hash_mismatch', 'the file at that commit has another hash'));
    await fireEvent.click(view.getByTestId('spec-sign'));
    await waitFor(() => expect(view.getByTestId('sign-error').textContent).toBe('the file at that commit has another hash'));
    expect(view.queryByTestId('spec-signature')).toBeNull();
  });

  it('a spec that is already signed is shown as signed and offers dispatch at once', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: [harnessView()] });
    await fake.sign({ repo: spec.repo, spec_id: spec.spec_id, commit: spec.commit, sha256: spec.sha256 });
    const view = mount(fake);
    await read(view, fake.specs[0]);
    expect(view.getByTestId('spec-signature').textContent).toContain('SIGNED by tester');
    expect(view.queryByTestId('spec-sign')).toBeNull();
    await waitFor(() => expect(view.getByTestId('dispatch-form')).not.toBeNull());
    expect(calls(fake, 'sign')).toHaveLength(1); // the one above: the page signed nothing
  });
});

describe('dispatching', () => {
  const twoHarnesses = () => [
    harnessView({
      capabilities: {
        ...harnessView().capabilities!,
        profiles: [
          { name: 'claude', priced: true },
          { name: 'hybrid-local', priced: false },
        ],
      },
    }),
    harnessView({ name: 'fake', state: 'unhealthy', harness: null, capabilities: null, error: 'not found' }),
  ];

  it('sends the work item, the signed hash, the harness and a key made on the page', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    const view = mount(fake);
    await readAndSign(view, spec);
    const key = view.getByTestId('dispatch-key').textContent ?? '';
    expect(key).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    expect(view.getByTestId('dispatch-work-item').textContent).toBe('owner/sandbox#1');
    expect(view.getByTestId('dispatch-spec').getAttribute('title')).toBe(spec.sha256);

    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.getByTestId('dispatch-done')).not.toBeNull());

    expect(view.dispatched).toEqual([
      [
        {
          work_item: 'owner/sandbox#1',
          repo: 'owner/sandbox',
          spec_sha256: spec.sha256,
          harness: 'reqdrive',
          idempotency_key: key,
        },
        't1',
      ],
    ]);
    // The request names no tier, and no key the schema does not have.
    expect(Object.keys(view.dispatched[0][0]).sort()).toEqual(
      ['harness', 'idempotency_key', 'repo', 'spec_sha256', 'work_item'],
    );
    expect(view.getByTestId('dispatch-done').textContent).toContain('Started u1');
    expect(view.getByTestId('dispatch-done').getAttribute('role')).toBe('status');
  });

  it('offers only harnesses from the daemon\'s list, and only healthy ones can be chosen', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    const view = mount(fake);
    await readAndSign(view, spec);
    const options = [...(view.getByTestId('dispatch-harness') as HTMLSelectElement).options];
    expect(options.map((o) => [o.value, o.disabled, o.textContent])).toEqual([
      ['reqdrive', false, 'reqdrive · healthy'],
      ['fake', true, 'fake · unhealthy'],
    ]);
    expect((view.getByTestId('dispatch-harness') as HTMLSelectElement).value).toBe('reqdrive');
  });

  it('offers the chosen harness\'s profiles, marked priced or unpriced, and sends the one picked with the opt-in', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    const view = mount(fake);
    await readAndSign(view, spec);
    const profile = view.getByTestId('dispatch-profile') as HTMLSelectElement;
    expect([...profile.options].map((o) => o.textContent)).toEqual([
      'harness default',
      'claude (priced)',
      'hybrid-local (unpriced)',
    ]);
    await fireEvent.change(profile, { target: { value: 'hybrid-local' } });
    await fireEvent.click(view.getByTestId('dispatch-opt-in'));
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.dispatched).toHaveLength(1));
    expect(view.dispatched[0][0]).toMatchObject({ profile: 'hybrid-local', opt_in: true });
  });

  it('shows a refusal\'s reason verbatim and keeps the same key for the next press', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    const reason = 'harness reqdrive may not run this unit: profile hybrid-local is unpriced and the unit did not opt in';
    const keys: string[] = [];
    let refuse = true;
    const view = mount(fake, async (request) => {
      keys.push(request.idempotency_key);
      if (refuse) throw new ApiFailure(422, 'ineligible', reason);
      return fake.createMission(request);
    });
    await readAndSign(view, spec);
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.getByTestId('dispatch-refusal')).not.toBeNull());
    const refusal = view.getByTestId('dispatch-refusal');
    expect(refusal.textContent).toBe(reason);
    expect(refusal.getAttribute('data-code')).toBe('ineligible');
    expect(refusal.getAttribute('role')).toBe('alert');
    expect(view.queryByTestId('dispatch-done')).toBeNull();

    refuse = false;
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.getByTestId('dispatch-done')).not.toBeNull());
    expect(keys).toHaveLength(2);
    expect(keys[1]).toBe(keys[0]);
    expect(view.queryByTestId('dispatch-refusal')).toBeNull();
  });

  it('after a unit starts, the next dispatch of the same spec uses a new key', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    const view = mount(fake);
    await readAndSign(view, spec);
    const first = view.getByTestId('dispatch-key').textContent;
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.getByTestId('dispatch-done')).not.toBeNull());
    const second = view.getByTestId('dispatch-key').textContent;
    expect(second).not.toBe(first);
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.dispatched).toHaveLength(2));
    expect(view.dispatched[1][0].idempotency_key).toBe(second);
    await waitFor(() => expect(view.getByTestId('dispatch-done').textContent).toContain('Started u2'));
  });

  it('a second press while the first is in flight sends nothing', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: twoHarnesses() });
    let release: (r: MissionResponse) => void = () => {};
    const ondispatch = vi.fn(() => new Promise<MissionResponse>((resolve) => { release = resolve; }));
    const view = mount(fake, ondispatch);
    await readAndSign(view, spec);
    const go = view.getByTestId('dispatch-go') as HTMLButtonElement;
    await fireEvent.click(go);
    expect(go.disabled).toBe(true);
    await fireEvent.click(go);
    expect(ondispatch).toHaveBeenCalledTimes(1);
    release({ unit_id: 'u1', waits_for_slot: true });
    await waitFor(() => expect(view.getByTestId('dispatch-done').textContent).toContain('waiting for a concurrency slot'));
  });

  it('cannot dispatch when no harness is healthy, and says why the list is empty when it cannot be read', async () => {
    const spec = specView();
    const fake = createFakeApi({ specs: [spec], harnesses: [twoHarnesses()[1]] });
    const view = mount(fake);
    await read(view, spec);
    await waitFor(() => expect((view.getByTestId('spec-sign') as HTMLButtonElement).disabled).toBe(false));
    fake.failNext('listHarnesses', new ApiFailure(503, 'store', 'the registry cannot be read right now'));
    await fireEvent.click(view.getByTestId('spec-sign'));
    await waitFor(() => expect(view.getByTestId('dispatch-harness-error')).not.toBeNull());
    expect(view.getByTestId('dispatch-harness-error').textContent).toBe('the registry cannot be read right now');
    expect((view.getByTestId('dispatch-go') as HTMLButtonElement).disabled).toBe(true);
  });

  it('reading another spec clears the last signature, refusal and result', async () => {
    const first = specView();
    const second = specView({ spec_id: 'SPEC-0002', text: '---\nid: SPEC-0002\n---\n# Other\n', work_item: 'owner/sandbox#2' });
    const fake = createFakeApi({ specs: [first, second], harnesses: twoHarnesses() });
    const view = mount(fake);
    await readAndSign(view, first);
    await fireEvent.click(view.getByTestId('dispatch-go'));
    await waitFor(() => expect(view.getByTestId('dispatch-done')).not.toBeNull());

    await read(view, second);
    await waitFor(() => expect(view.getByTestId('spec-ident').textContent).toContain('SPEC-0002'));
    expect(view.queryByTestId('dispatch-form')).toBeNull();
    expect(view.queryByTestId('dispatch-done')).toBeNull();
    expect(view.queryByTestId('spec-signature')).toBeNull();
  });
});
````

`src/App.unit.test.ts`:

````ts
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte';
import App from './App.svelte';
import { fleet } from './lib/store.svelte';
import { api } from './lib/api';
import * as auth from './lib/auth';
import { newUnit } from './lib/fleet';
import { STAGES } from './lib/unit/rail';
import type { EventEnvelopeDto, UnitEventDto } from './lib/generated/fleet-api';

// The unit view in the real app: a selected unit shows its seven-stage rail folded from the
// same events every other part of the page reads, the pull-request link, and the re-verify
// action when the unit waits for it.

let seq = 0;
function feed(id: string, event: UnitEventDto): void {
  seq += 1;
  const envelope: EventEnvelopeDto = { unit_id: id, seq, event };
  fleet.onEvt(id, envelope);
}

function seed(id = 'u1'): void {
  fleet.units[id] = newUnit(id, 'owner/sandbox#1', 'T1');
  fleet.order = [...fleet.order, id];
  fleet.selectedId = id;
}

beforeEach(() => {
  seq = 0;
  fleet.dispose();
  fleet.units = {};
  fleet.order = [];
  fleet.selectedId = null;
  fleet.pendingLaunch = null;
  fleet.connection = 'starting';
  // No token: the page makes no request, and these tests drive the store directly.
  vi.spyOn(auth, 'loadToken').mockResolvedValue(null);
});

afterEach(() => {
  cleanup();
  fleet.dispose();
  vi.restoreAllMocks();
});

describe('App unit view', () => {
  it('moves the rail through all seven stages as stage records arrive, then shows the PR link', async () => {
    seed();
    const { getByTestId, getByText } = render(App);
    await waitFor(() => expect(getByTestId('stage-rail')).not.toBeNull());
    for (const stage of STAGES) {
      expect(getByTestId(`stage-${stage}`).getAttribute('data-state')).toBe('pending');
    }

    for (const stage of STAGES) {
      feed('u1', { type: 'stage', stage, status: 'started' });
      await waitFor(() => expect(getByTestId(`stage-${stage}`).getAttribute('data-state')).toBe('running'));
      feed('u1', { type: 'metric', tokens_in: 1, tokens_out: 1, cost_usd: seq * 0.01, elapsed_ms: seq * 1000 });
      feed('u1', { type: 'stage', stage, status: 'finished' });
      await waitFor(() => expect(getByTestId(`stage-${stage}`).getAttribute('data-state')).toBe('done'));
    }
    expect(getByTestId('stage-deliver-cost').textContent).not.toBe('—');

    feed('u1', { type: 'artifact', kind: 'pr', ref: 'https://github.com/owner/sandbox/pull/7' });
    await waitFor(() => expect(getByText('▸ OPEN PULL REQUEST')).not.toBeNull());
    expect(getByText('▸ OPEN PULL REQUEST').getAttribute('href')).toBe('https://github.com/owner/sandbox/pull/7');
  });

  it('offers RE-VERIFY only while the unit waits for it, and sends reverify for that unit', async () => {
    const sendCommand = vi.spyOn(api, 'sendCommand').mockResolvedValue('accepted');
    seed();
    const { getByTestId, queryByTestId } = render(App);
    await waitFor(() => expect(getByTestId('stage-rail')).not.toBeNull());
    expect(queryByTestId('reverify-go')).toBeNull();

    feed('u1', { type: 'stage', stage: 'check', status: 'started' });
    feed('u1', { type: 'phase_changed', from: 'merge_check', to: 'needs_human', reason: 'verification stopped' });
    await waitFor(() => expect(getByTestId('reverify-go')).not.toBeNull());
    expect(getByTestId('stage-check').getAttribute('data-state')).toBe('stopped');

    await fireEvent.click(getByTestId('reverify-go'));
    await waitFor(() => expect(sendCommand).toHaveBeenCalledTimes(1));
    expect(sendCommand).toHaveBeenCalledWith('u1', expect.objectContaining({ command: 'reverify' }));

    feed('u1', { type: 'phase_changed', from: 'needs_human', to: 'merge_check' });
    await waitFor(() => expect(queryByTestId('reverify-go')).toBeNull());
  });

  it('a unit stopped for another reason is not offered RE-VERIFY', async () => {
    seed();
    const { getByTestId, queryByTestId } = render(App);
    await waitFor(() => expect(getByTestId('stage-rail')).not.toBeNull());
    feed('u1', { type: 'phase_changed', from: 'building', to: 'needs_human', reason: 'usd cap' });
    await waitFor(() => expect(getByTestId('hud').textContent).toContain('NEEDS YOU'));
    expect(queryByTestId('reverify-go')).toBeNull();
  });
});
````

### Implementation notes

**`src/lib/unit/rail.ts`, in full.** It replaces wave 0's stub. The types, `STAGES` and
`newRail` are unchanged; `foldRail` gets its body, and `formatElapsed` and `formatCost` are
new. `src/lib/fleet.ts` already calls `foldRail` for every record of a unit, before its own
`switch`, so nothing outside this file changes.

````ts
// The seven-stage rail of one unit, folded from its event log.
//
// A `stage` record says a stage started or finished. A `metric` record carries the unit's
// cumulative spend and metered time; the rail attributes what was added since the previous
// metric to the stage that was running. A `phase_changed` record into a stopped phase marks
// the running stage as stopped.
//
// Time here is METERED AGENT TIME, not wall-clock time: a record carries no timestamp, so the
// only durations the log holds are the ones a metric reports.

import type { Stage, UnitEventDto } from '../generated/fleet-api';

/** The seven stages, in the order a unit passes through them. */
export const STAGES = [
  'provision',
  'red',
  'plan',
  'green',
  'check',
  'review',
  'deliver',
] as const satisfies readonly Stage[];

/**
 * - `pending`: no `started` note has arrived for the stage;
 * - `running`: started and not finished;
 * - `done`: finished;
 * - `stopped`: it was running when the unit stopped for a person, was halted, or failed.
 */
export type StageState = 'pending' | 'running' | 'done' | 'stopped';

export interface StageCell {
  stage: Stage;
  state: StageState;
  /** The `detail` of the stage's latest note, if it had one. */
  detail: string | null;
  /** Metered agent time spent while this stage ran, in milliseconds. Null until a metric says. */
  elapsedMs: number | null;
  /** Spend while this stage ran, in USD. Null until a metric says. */
  costUsd: number | null;
  /** The runtime adapter the stage's latest metric named, when a metric names one. */
  adapter: string | null;
  /** The model the stage's latest metric named, when a metric names one. */
  model: string | null;
}

export interface RailState {
  /** Always seven cells, in `STAGES` order. */
  cells: StageCell[];
  /** The stage that is running, if one is. */
  current: Stage | null;
  /** The stage that most recently finished or stopped, if `current` is null. */
  last: Stage | null;
  /** The unit's cumulative spend at the latest metric. */
  seenCostUsd: number;
  /** The unit's cumulative metered time at the latest metric. */
  seenElapsedMs: number;
}

export function newRail(): RailState {
  return {
    cells: STAGES.map((stage) => ({
      stage,
      state: 'pending',
      detail: null,
      elapsedMs: null,
      costUsd: null,
      adapter: null,
      model: null,
    })),
    current: null,
    last: null,
    seenCostUsd: 0,
    seenElapsedMs: 0,
  };
}

/** The phases in which a unit is not running: a stage that was running has stopped. */
const STOPPED_PHASES: readonly string[] = ['needs_human', 'halted', 'failed'];

/** Replace one cell, leaving the others and the input untouched. */
function withCell(rail: RailState, stage: Stage, change: (cell: StageCell) => StageCell): StageCell[] {
  return rail.cells.map((cell) => (cell.stage === stage ? change(cell) : cell));
}

/**
 * A text field the schema does not declare on this record. In milestone 1 a `metric` carries
 * no `stage`, `adapter` or `model`; when `fleet-api` adds them, regenerate the types and read
 * them as ordinary fields instead.
 */
function undeclaredText(event: object, key: string): string | null {
  const value = (event as Record<string, unknown>)[key];
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function asStage(value: string | null): Stage | null {
  return value !== null && (STAGES as readonly string[]).includes(value) ? (value as Stage) : null;
}

/** Apply one event of a unit's log. Returns a new state; never mutates `rail`. */
export function foldRail(rail: RailState, event: UnitEventDto): RailState {
  switch (event.type) {
    case 'stage': {
      const detail = event.detail ?? null;
      if (event.status === 'started') {
        return {
          ...rail,
          cells: withCell(rail, event.stage, (cell) => ({ ...cell, state: 'running', detail })),
          current: event.stage,
        };
      }
      return {
        ...rail,
        cells: withCell(rail, event.stage, (cell) => ({
          ...cell,
          state: 'done',
          detail: detail ?? cell.detail,
        })),
        current: rail.current === event.stage ? null : rail.current,
        last: rail.current === event.stage || rail.current === null ? event.stage : rail.last,
      };
    }

    case 'metric': {
      // Cumulative counters: what this metric adds is what it reports beyond the last one.
      const addedCost = Math.max(0, event.cost_usd - rail.seenCostUsd);
      const addedElapsed = Math.max(0, event.elapsed_ms - rail.seenElapsedMs);
      const seen = {
        seenCostUsd: Math.max(rail.seenCostUsd, event.cost_usd),
        seenElapsedMs: Math.max(rail.seenElapsedMs, event.elapsed_ms),
      };
      const target = asStage(undeclaredText(event, 'stage')) ?? rail.current ?? rail.last;
      if (target === null) return { ...rail, ...seen };
      const adapter = undeclaredText(event, 'adapter');
      const model = undeclaredText(event, 'model');
      return {
        ...rail,
        ...seen,
        cells: withCell(rail, target, (cell) => ({
          ...cell,
          costUsd: (cell.costUsd ?? 0) + addedCost,
          elapsedMs: (cell.elapsedMs ?? 0) + addedElapsed,
          adapter: adapter ?? cell.adapter,
          model: model ?? cell.model,
        })),
      };
    }

    case 'phase_changed': {
      if (!STOPPED_PHASES.includes(event.to) || rail.current === null) return rail;
      const stopped = rail.current;
      return {
        ...rail,
        cells: withCell(rail, stopped, (cell) => ({ ...cell, state: 'stopped' })),
        current: null,
        last: stopped,
      };
    }

    default:
      return rail;
  }
}

/** Metered time for a person: `—`, `0.8s`, `42s`, `3m 05s`, `1h 02m`. */
export function formatElapsed(ms: number | null): string {
  if (ms === null) return '—';
  if (ms < 1000) return `${(ms / 1000).toFixed(1)}s`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${String(seconds % 60).padStart(2, '0')}s`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, '0')}m`;
}

/** Spend for a person: `—` or dollars to three places, as the unit tiles show it. */
export function formatCost(usd: number | null): string {
  return usd === null ? '—' : `$${usd.toFixed(3)}`;
}
````

**Existing code.** This lane changes no existing file. Wave 0 already calls `foldRail` as the
first statement of `fold` in `src/lib/fleet.ts`, mounts `ReverifyButton` and `StageRail` after
the four command buttons of the selected unit in `src/App.svelte` (after line 483 on `main`),
and mounts `FactoryPanel` at the top of the left `<aside>` (line 390 on `main`) under the
`SIGNED SPEC` tab.

Three things in it that are easy to get wrong:

- **A metric is cumulative.** `cost_usd` is the unit's total so far, not what the last step
  cost. The fold keeps the total it last saw and attributes the difference. Adding `cost_usd`
  itself to a stage would count the first stage's spend again in every later stage.
- **The time is metered agent time.** No record carries a timestamp, so there is no wall-clock
  duration to show, and a clock started in the browser would be wrong for every unit loaded
  from its log. The rail's caption says "agent time". Do not add a ticking timer.
- **Adapter and model are not in the schema.** `undeclaredText` reads them from the record if
  they are there, without a type for them. That is deliberate and temporary: it lets the rail
  show them the day the daemon sends them, and the tests pin both cases. When `fleet-api` adds
  the fields, regenerate, delete `undeclaredText`, and read them as fields.

**`src/lib/unit/StageRail.svelte`.** Keep the stub's prop (`rail`). What the tests require:

| Element | `data-testid` | Must have |
|---|---|---|
| The rail | `stage-rail` | a `<table>` with a `<caption>` containing `STAGES`, and five `<th scope="col">` in `<thead>`: `STAGE`, `STATE`, `TIME`, `COST`, `ADAPTER · MODEL` |
| One row per stage, in `rail.cells` order | `stage-<stage>` | a `<tr>` in `<tbody>`; `data-state` equal to the cell's state; `aria-current="step"` on a running row and no `aria-current` otherwise; `title` equal to the cell's detail when it has one; a `<th scope="row">` with the stage name in capitals |
| The state cell | (class `state`) | a glyph marked `aria-hidden="true"` and a word: `WAIT`, `RUN`, `DONE`, `STOP` |
| Time, cost, runtime | `stage-<stage>-time`, `stage-<stage>-cost`, `stage-<stage>-runtime` | `formatElapsed(cell.elapsedMs)`, `formatCost(cell.costUsd)`, and `<adapter> · <model>` or `—` |

It sits in the right-hand panel, 360 pixels wide, under the command buttons. Seven rows of
about eleven-pixel type: do not make it a horizontal strip, there is no room for five columns
seven times. Pending rows in `--text-dim`; the running row's stage and state in `--cyan`;
`DONE` in `--green`; a stopped row's stage and state in `--amber`.

**`src/lib/unit/ReverifyButton.svelte`.** Keep the stub's props. Its two script blocks, in
full; the markup is yours:

````svelte
<script lang="ts" module>
  import type { Unit } from '../fleet';

  /** The pause reason the daemon gives a unit whose verification stopped. */
  export const VERIFY_STOPPED = 'verification stopped';

  /** True when the unit is waiting for a person to re-run the control plane's verification. */
  export function awaitsReverify(unit: Unit): boolean {
    return unit.phase === 'needs_human' && unit.pauseReason === VERIFY_STOPPED;
  }
</script>

<script lang="ts">
  // The `reverify` command, offered only on a unit that is waiting for it.
  import { ApiFailure, type CommandOutcome } from '../api';

  interface Props {
    unit: Unit;
    /** Send `reverify` to this unit. Resolves to what the daemon said. */
    onreverify: () => Promise<CommandOutcome | undefined>;
  }

  let { unit, onreverify }: Props = $props();

  const OUTCOME: Record<Exclude<CommandOutcome, 'accepted'>, string> = {
    gone: 'The daemon no longer holds this unit’s run.',
    unknown_unit: 'The daemon does not know this unit.',
    invalid: 'The daemon did not accept the command.',
  };

  let sending = $state(false);
  let problem = $state<string | null>(null);

  async function send(): Promise<void> {
    // The disabled attribute stops a pointer; this stops a second call that is already queued.
    if (sending) return;
    sending = true;
    problem = null;
    try {
      const outcome = await onreverify();
      if (outcome !== undefined && outcome !== 'accepted') problem = OUTCOME[outcome];
    } catch (failure) {
      problem = failure instanceof ApiFailure ? failure.message : 'The command could not be sent.';
    } finally {
      sending = false;
    }
  }
</script>
````

| Element | `data-testid` | Must have |
|---|---|---|
| The container, rendered only when `awaitsReverify(unit)` | `reverify` | text containing `Verification stopped` and, when `unit.pausedFrom` is set, that phase's name |
| The button | `reverify-go` | a `<button type="button">`; text `RE-VERIFY`, or `RE-VERIFYING…` and `disabled` while sending |
| What went wrong | `reverify-problem` | `role="alert"`; the text from `OUTCOME`, or the failure's message unedited |

The `if (sending) return;` at the top of `send` is not decoration. `disabled` stops a pointer;
it does not stop a handler that is already queued, and the test presses twice.

**`src/lib/unit/digest.ts`, in full:**

````ts
// SHA-256 of the text the page is showing, so that what is signed is what was shown.

/** SHA-256 of the UTF-8 bytes of `text`, as lowercase hex: the daemon's `sha256` of a spec. */
export async function sha256Hex(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

/** A fresh idempotency key for one dispatch. */
export function newIdempotencyKey(): string {
  return crypto.randomUUID();
}
````

`crypto.subtle` exists only in a secure context. The Tauri page (`http://tauri.localhost` on
Windows, `tauri://localhost` elsewhere) and `http://localhost:5173` are both secure contexts;
`http://127.0.0.1:5173` is too. If `crypto.subtle` is ever undefined, `sha256Hex` throws,
`load` shows "The request could not be made.", and no spec can be signed: that is the right
failure. Do not add a JavaScript SHA-256 as a fallback.

**`src/lib/unit/FactoryPanel.svelte`.** Keep the stub's props. Its script block, in full,
because the order of things in it is the behaviour; the markup and the styles are yours:

````svelte
<script lang="ts">
  // Sign a spec, then dispatch it. The minimum a person needs for both:
  //
  // 1. Read a spec exactly as the daemon read it, at a commit the daemon names.
  // 2. Sign it: send that commit and the SHA-256 of the text on screen.
  // 3. Dispatch the signed spec to a harness, once per idempotency key.
  //
  // Nothing here judges a spec or a harness: the daemon's problems and refusals are shown in
  // its own words.
  import { ApiFailure, type FleetApi } from '../api';
  import type {
    HarnessView,
    MissionRequest,
    MissionResponse,
    ProblemView,
    SignatureView,
    SpecView,
    TierDto,
  } from '../generated/fleet-api';
  import { newIdempotencyKey, sha256Hex } from './digest';

  interface Props {
    /** The daemon client: the panel reads specs, signatures and harnesses through it. */
    api: FleetApi;
    /**
     * Start a unit from a signed spec. `tier` is the signed spec's tier, for the tile.
     * Rejects with the `ApiFailure` the daemon sent when the request is refused.
     */
    ondispatch: (request: MissionRequest, tier: TierDto | null) => Promise<MissionResponse>;
  }

  let { api, ondispatch }: Props = $props();

  /** What a failure says, in the daemon's words when it has any. */
  function words(failure: unknown): { code: string; message: string; problems: ProblemView[] } {
    if (failure instanceof ApiFailure) {
      return { code: failure.code, message: failure.message, problems: failure.problems };
    }
    return { code: 'error', message: 'The request could not be made.', problems: [] };
  }

  // ── read ──────────────────────────────────────────────────────────────────
  let repo = $state('');
  let specId = $state('');
  let commit = $state('');
  let loading = $state(false);
  let loadError = $state<{ code: string; message: string } | null>(null);

  let view = $state<SpecView | null>(null);
  /** SHA-256 of `view.text` as this page computed it. Null until computed. */
  let shownHash = $state<string | null>(null);

  async function load(): Promise<void> {
    loading = true;
    loadError = null;
    signError = null;
    view = null;
    shownHash = null;
    signature = null;
    done = null;
    refusal = null;
    try {
      const query = { repo: repo.trim(), spec_id: specId.trim() };
      const answer = await api.getSpec(commit.trim() ? { ...query, commit: commit.trim() } : query);
      shownHash = await sha256Hex(answer.text);
      view = answer;
      signature = answer.signature ?? null;
      problems = answer.problems;
    } catch (failure) {
      loadError = words(failure);
    } finally {
      loading = false;
    }
  }

  // ── sign ──────────────────────────────────────────────────────────────────
  let problems = $state<ProblemView[]>([]);
  let signature = $state<SignatureView | null>(null);
  let signing = $state(false);
  let signError = $state<{ code: string; message: string } | null>(null);

  /** True when the text on screen is, byte for byte, what the daemon hashed. */
  const intact = $derived(view !== null && shownHash !== null && shownHash === view.sha256);
  const canSign = $derived(intact && problems.length === 0 && signature === null && !signing);

  async function sign(): Promise<void> {
    if (!view || !shownHash || !canSign) return;
    signing = true;
    signError = null;
    try {
      // The hash sent is the hash of the text shown, computed here: never a value copied
      // from the answer without checking it against that text.
      const answer = await api.sign({
        repo: view.repo,
        spec_id: view.spec_id,
        commit: view.commit,
        sha256: shownHash,
      });
      signature = answer.signature;
      view = { ...view, tier: answer.tier, work_item: answer.work_item, signature: answer.signature };
    } catch (failure) {
      const said = words(failure);
      signError = said;
      if (said.problems.length > 0) problems = said.problems;
    } finally {
      signing = false;
    }
  }

  // ── dispatch ──────────────────────────────────────────────────────────────
  let harnesses = $state<HarnessView[]>([]);
  let harnessError = $state<string | null>(null);
  let harness = $state('');
  let profile = $state('');
  let optIn = $state(false);
  let key = $state('');
  let dispatching = $state(false);
  let refusal = $state<{ code: string; message: string } | null>(null);
  let done = $state<MissionResponse | null>(null);

  const profiles = $derived(
    harnesses.find((h) => h.name === harness)?.capabilities?.profiles ?? [],
  );

  // A signed spec is dispatchable: give it a key, and read the harnesses it could run on.
  let preparedFor = '';
  $effect(() => {
    const signed = signature?.sha256 ?? '';
    if (signed === preparedFor) return;
    preparedFor = signed;
    if (!signed) return;
    key = newIdempotencyKey();
    void loadHarnesses();
  });

  async function loadHarnesses(): Promise<void> {
    try {
      harnesses = await api.listHarnesses();
      harnessError = null;
      if (!harnesses.some((h) => h.name === harness && h.state === 'healthy')) {
        harness = harnesses.find((h) => h.state === 'healthy')?.name ?? '';
        profile = '';
      }
    } catch (failure) {
      harnessError = words(failure).message;
    }
  }

  const canDispatch = $derived(
    signature !== null && !!view?.work_item && harness !== '' && key !== '' && !dispatching,
  );

  async function dispatch(): Promise<void> {
    if (!view || !signature || !view.work_item || !canDispatch) return;
    dispatching = true;
    refusal = null;
    done = null;
    const request: MissionRequest = {
      work_item: view.work_item,
      repo: view.repo,
      spec_sha256: signature.sha256,
      harness,
      idempotency_key: key,
      ...(profile ? { profile } : {}),
      ...(optIn ? { opt_in: true } : {}),
    };
    try {
      done = await ondispatch(request, view.tier ?? null);
      // The key did its job. A second dispatch of this spec is a new decision with a new key.
      key = newIdempotencyKey();
    } catch (failure) {
      // The key is kept: pressing again repeats the same request, and cannot start a second
      // unit if the first one was started after all.
      refusal = words(failure);
    } finally {
      dispatching = false;
    }
  }
</script>
````

What the markup must have:

| Element | `data-testid` | Must have |
|---|---|---|
| The whole panel | `factory-panel` | |
| Repository, spec id, commit | `spec-repo`, `spec-id`, `spec-commit` | `<input>` bound to `repo`, `specId`, `commit`; each inside a `<label>` whose first child is a `<span>` with the visible label; `autocomplete="off"`, `spellcheck="false"` |
| Read | `spec-load` | a submit button of a `<form>` whose submit calls `load()`; disabled while loading or while repository or spec id is blank |
| A failed read | `spec-load-error` | `role="alert"`; `data-code`; text exactly the failure's message |
| The commit, the hash, the tier, the work item, the identity | `spec-commit-read`, `spec-sha256`, `spec-tier`, `spec-work-item`, `spec-ident` | exactly `view.commit`; exactly `view.sha256`; the tier in capitals; the work item; `<repo> · <spec id>` |
| The text | `spec-text` | a `<pre>` whose text content is exactly `view.text`; `white-space: pre`; `tabindex="0"` and an `aria-label` so that it can be scrolled from the keyboard |
| The integrity warning, only when `!intact` | `spec-integrity` | `role="alert"`; contains both hashes |
| Problems, only when there are any | `spec-problems` | a list; each `<li>` has `data-code` and contains the problem's message |
| Sign, only while there is no signature | `spec-sign` | a button, `disabled={!canSign}`, calling `sign()` |
| A refused signature | `sign-error` | `role="alert"`; `data-code`; text exactly the failure's message |
| The signature | `spec-signature` | contains `SIGNED by <signed_by>` and `new Date(signed_at_ms).toISOString()` |
| The dispatch form, only when there is a signature | `dispatch-form` | a `<form>` whose submit calls `dispatch()` |
| Work item, spec, key | `dispatch-work-item`, `dispatch-spec`, `dispatch-key` | the work item; the spec id with the hash in its `title`; exactly `key` |
| Harness | `dispatch-harness` | a `<select>` bound to `harness`; one `<option>` per row of the list, text `<name> · <state>`, `disabled` unless healthy; changing it clears `profile` |
| Profile | `dispatch-profile` | a `<select>` bound to `profile`; first option `harness default` with an empty value; then one per profile, text `<name> (priced)` or `<name> (unpriced)` |
| The opt-in | `dispatch-opt-in` | a checkbox bound to `optIn`, with a visible label saying what is being accepted |
| Go | `dispatch-go` | the form's submit button; `disabled={!canDispatch}` |
| A harness list that could not be read | `dispatch-harness-error` | `role="alert"`; the failure's message |
| A refusal | `dispatch-refusal` | `role="alert"`; `data-code`; text **exactly** the failure's message, nothing added |
| The result | `dispatch-done` | `role="status"`; `Started <unit id>`, and `waiting for a concurrency slot` when `waits_for_slot` |

It sits in the left-hand panel, 300 pixels wide, which scrolls. Order: the three inputs and
READ; then the facts, the text (at most about 260 pixels high, scrolling inside itself), the
problems, SIGN; then the dispatch form.

**Pitfalls**

- **Show the text, do not render it.** The spec is Markdown with a YAML header. It goes into a
  `<pre>` as text. No `marked`, no `{@html}`, no trimming, no line-ending normalisation, no
  "show more". The person is signing bytes; the page shows bytes.
- **Sign the hash of what is on screen.** `sign()` sends `shownHash`, computed from
  `view.text`, and only when it equals the daemon's `sha256`. Sending `view.sha256` straight
  back would pass every happy-path test and would let a person sign text they were not shown.
- **The key is kept on failure and replaced on success.** A refusal, or a request that got no
  answer, may or may not have started a unit. Pressing again with the same key is safe either
  way: the daemon answers `duplicate_key` and names the unit. A new key after a failure is how
  a second unit gets started by accident.
- **No tier, no cap, no extra keys in the request.** `MissionRequest` denies unknown fields:
  one extra key is a 422 from the framework with a plain-text body. Leave `profile` and
  `opt_in` out when they are not set, rather than sending `null` or `false`; the test checks
  the exact key set. A spend-cap field is not in this milestone's form: the daemon's
  configured ceiling applies.
- **Verbatim.** `dispatch-refusal`, `sign-error` and `spec-load-error` contain the daemon's
  message and nothing else. The code goes in `data-code`. If you want a label, put it outside
  the element.
- **A unit dispatched here appears in the fleet through `ondispatch`.** The panel does not
  touch the store. `App.svelte` passes `fleet.dispatch`, which adds the tile, opens the stream
  and selects the unit. The panel only shows `Started <id>`.
- **The reconnect gap.** A unit dispatched while the daemon is restarting gets its stream
  reopened by the store (wave 0). Do not open a stream here, and do not poll for the unit.
- **Svelte 5 and effects.** The one `$effect` reads `signature` and writes `key` and, through
  `loadHarnesses`, `harnesses`. It guards with a plain variable (`preparedFor`) so that it does
  its work once per signed hash. If it read `key` or `harnesses` it would loop.
- **jsdom fires `click` on a disabled button** when a test calls `fireEvent.click`. Guard in
  the handler as well as with `disabled`: `sign()` and `dispatch()` return at once unless
  `canSign` or `canDispatch`.

### Verify

| Command | Expected |
|---|---|
| `npm run check` | exit 0, 0 errors, 0 warnings |
| `npm run check:types` | `generated types are up to date`: this lane does not touch them |
| `npx vitest run src/lib/unit src/App.unit.test.ts` | `Tests  73 passed (73)` |
| `npx vitest run` | every file passes; 73 more tests than at the branch point |
| `npm run sidecar`, then `cargo test` in `cockpit/ui/src-tauri` | passes, unchanged by this lane: 35 library tests, or 46 if lane UI-TOKEN has merged |
| *Git-Bash:* `grep -rnE "fetch\(\|new WebSocket\(\|@html\|from 'marked'\|text-faint\|setInterval" src/lib/unit \| grep -v "\.test\.ts"` | no output |
| *Git-Bash:* `grep -rnE 'export let\|on:click\|\$:' src/lib/unit` | no output: Svelte 5 only |
| *Git-Bash:* `grep -n "view.sha256" src/lib/unit/FactoryPanel.svelte` | the lines that display it and compare with it; none inside the object passed to `api.sign` |
| `git diff --name-only origin/factory/m1...HEAD` | only paths under **Owns** |

### Done when

- [ ] Behaviours 1 to 20 pass, with real output in the pull request.
- [ ] A selected unit shows seven stages with state, agent time and cost, and a dash for
      adapter and model.
- [ ] A spec can be read, signed and dispatched from the left panel, and every refusal on the
      way is shown in the daemon's words.
- [ ] RE-VERIFY appears on a unit stopped with `verification stopped`, and on no other.
- [ ] The pull request says that issue #92 is not closed by it, and lists what the rail cannot
      show until the API carries it: wall-clock time, adapter and model.

---

# Part 3 — Integration and proof

## Merge order

Everything merges into `factory/m1`, by the coordinator, never by the lane that wrote it.

| Step | What merges | Before it merges | After it merges |
|---|---|---|---|
| 0 | Requests C1 to C3 (the coordinator's own change), and lane CC-API of the control-plane plan (request C4) | `npm ci` succeeds in `cockpit/ui`; `cargo test` passes in `cockpit/ui/src-tauri`; `crates/fleet-api/contract/fleet-api.schema.json` exists | Part 1 can start |
| 1 | Part 1, `feat/m1-ui-types` | Part 1's Verify table, with real output in the pull request; the reviewing session's pass over Review Focus item 4 | Every lane cuts its branch. The desktop app shows "not connected" until step 2 |
| 2 | Lane UI-TOKEN, `feat/m1-ui-token`, together with lane CC-WIRE of the control-plane plan (request C5) | Its two Verify tables; the reviewing session's pass over Review Focus items 1, 2 and 3 | With both in, the desktop app reaches the daemon. With only one in, it does not: the old daemon's cross-origin rules do not list `Authorization`, so the webview refuses to send the request, and the new daemon refuses a request without the token. Neither order of the two is safe on its own |
| 3 | Lanes UI-HARNESS and UI-RAIL, in either order | The lane's Verify table; for UI-RAIL, the reviewing session's pass over Review Focus item 5 | The header shows the harness badge; a unit shows its rail; the left panel signs and dispatches |
| 4 | Nothing | The checklist below, by the operator, after lane CC-WIRE and step 5 of the control-plane plan's merge order | The owner merges `factory/m1` to `main` under the program plan |

Two lanes never edit one file: each lane's **Owns** is disjoint from every other's, and after
step 1 the shared files (`App.svelte`, `api.ts`, `store.svelte.ts`, `fleet.ts`, the fake, the
generated types) belong to nobody. A merge conflict between two lanes therefore means one of
them left its files; stop and find out which.

After each of steps 1 to 3, on a clean checkout of `factory/m1`:

| Command | Where | Expected |
|---|---|---|
| `npm ci` | `cockpit/ui` | exit 0 |
| `npm run check` | `cockpit/ui` | exit 0: types up to date, 0 errors, 0 warnings |
| `npx vitest run` | `cockpit/ui` | every file passes. After step 1: 236 tests. After all of step 3: 394 tests in 38 files |
| `npx vite build` | `cockpit/ui` | exit 0 |
| `npm run sidecar`, then `cargo test` | `cockpit/ui`, then `cockpit/ui/src-tauri` | exit 0. From step 2: 46 library tests |
| `cargo xtask test static` | the repository root | exit 0: it formats and lints the shell's crate too |

If the schema changed between this plan being written and step 1, the test counts above stay
the same and the generated file differs; that is expected. If a count differs, find out why
before going on.

## The operator's smoke checklist

One real unit, signed and dispatched from the desktop app and watched to its pull request.
This is milestone 1's functional smoke for the cockpit, and it is the control-plane plan's
watched proof done through the page instead of through PowerShell: it spends real money and it
opens a real pull request on the sandbox repository. An agent may prepare it; the operator
runs it. Every box is ticked by the operator.

**Before the session**

- [ ] `factory/m1` holds this plan's four merges and the control-plane plan's merge-order step
      5, and the table above is green there.
- [ ] Everything under "Before the session" in the control-plane plan's watched proof is done:
      the sandbox repository has its configuration and one T1 spec, `gh` can push there,
      Docker is running, the pinned `reqdrive` binary is named in a registry file, and **the
      operator has chosen the spend cap** and written it into `fleetd.toml` as both
      `unit_usd` and `global_usd`.
- [ ] A checkout for the smoke that touches no other worktree:
      `git -C D:/MajorProjects/INFRASTRUCTURE/command-center worktree add --detach D:/MajorProjects/.swarm-wt/m1-ui-smoke origin/factory/m1`
- [ ] An empty directory for the daemon's files, holding `fleetd.toml` and `harnesses.toml`.
      Called `<proof>` below.

**Start the desktop app** (PowerShell, in `D:\MajorProjects\.swarm-wt\m1-ui-smoke\cockpit\ui`)

- [ ] Set where the daemon reads and writes, and who signs. The sidecar inherits this shell's
      environment. Do **not** set `FLEETD_TOKEN`: the shell mints it.

      ```powershell
      Remove-Item Env:FLEETD_TOKEN -ErrorAction SilentlyContinue
      $env:FLEETD_CONFIG   = '<proof>\fleetd.toml'
      $env:FLEETD_REGISTRY = '<proof>\harnesses.toml'
      $env:FLEETD_DATA_DIR = '<proof>\fleetd-data'
      $env:CC_DB           = '<proof>\fleet.db'
      $env:FLEETD_OPERATOR = '<your name>'
      npm ci
      npm run desktop
      ```

      The model credential `reqdrive` needs is set in this same shell, by the operator, as the
      `reqdrive` plan's smoke section names it.
- [ ] The window opens. Within about twenty seconds the address chip at the top right is green
      and there is no red `NOT CONNECTED` band under the header.
      If the band says `NO DAEMON TOKEN`, the shell did not hand the page a token: stop, that
      is lane UI-TOKEN's. If it says `DAEMON NOT ANSWERING` for more than thirty seconds, read
      the terminal: the daemon prints why it did not start.
- [ ] The daemon refuses a caller without the token. In a second PowerShell:
      `Invoke-WebRequest http://127.0.0.1:8787/units -SkipHttpErrorCheck | Select-Object StatusCode`
      shows `401`.

**The token stays where it belongs**

- [ ] Open the developer tools (right-click, Inspect). In the Network panel, find a request to
      `/units/<id>/stream` once a unit exists (after the next section, come back to this box).
      Its address contains no token. Its request header `Sec-WebSocket-Protocol` offers
      `fleetd.v1` and `fleetd.token.` followed by 64 hexadecimal characters; the response's
      names `fleetd.v1` only.
- [ ] In the Application panel, Local Storage and Session Storage hold nothing that looks like
      the token.
- [ ] The terminal that runs `npm run desktop` shows the daemon's lines prefixed `fleetd:`,
      and none of them contains a 64-character hexadecimal string.
- [ ] Switch to the `REFERENCE` tab (a sandboxed view plugin). In the developer tools console,
      choose that frame in the context menu and run `typeof window.__TAURI_INTERNALS__`.
      Expected: `"undefined"`. If it is an object, run
      `await window.__TAURI_INTERNALS__.invoke('fleetd_token')` and expect it to be rejected.
      **If a token comes back, stop.** That is a finding for the owner, not something to work
      around: a plugin frame can read the daemon's token.

**The harness panel**

- [ ] The header shows `◉ HARNESS 1/1` (or `n/m` for the registry you wrote), and no `DOCKER`
      or `KEY` badge.
- [ ] Reach it with Tab and open it with Enter. The panel lists `reqdrive` as `HEALTHY` with
      `ISOLATION container`, `METERING usd`, and the sandbox's preset among `PRESETS`. Under
      `WEAKER THAN A CONTAINER WITH METERING` it says that nothing is. If it lists a `RISK`
      line about network egress, that is the harness declaring it.
- [ ] Escape closes the panel and the badge has focus again.

**The rail, without spending**

- [ ] Left panel, `LEGACY TASK`, mode `DEMO`, `▸ LAUNCH UNIT`. A tile appears and is selected.
- [ ] In the right panel the `STAGES` table shows seven rows. They go from `WAIT` to `RUN` to
      `DONE`, top to bottom. If the unit's phase badge advances and every row stays at `WAIT`,
      the scripted harness sent no stage notes: write that down. It is a finding about the
      demo harness, not about the page.

**Sign**

- [ ] Left panel, `SIGNED SPEC`. Type the sandbox repository as `owner/name` and the spec's
      id. `READ SPEC`.
- [ ] `COMMIT` is the tip of the sandbox's `main`
      (`git ls-remote https://github.com/<owner>/<name> refs/heads/main`).
- [ ] *Git-Bash, in a clone of the sandbox:*
      `git show <the commit>:.reqdrive/specs/<spec id>.md | sha256sum` prints the `SHA-256`
      the page shows.
- [ ] **Read the text in the box.** It is what you are about to sign, byte for byte. This is
      the one judgment in the run.
- [ ] `TIER` is `T1`, `WORK ITEM` is the issue you expect, and there is no `NOT READY TO
      SIGN` list.
- [ ] `SIGN THESE BYTES`. The button is replaced by `SIGNED by <your name>` and a time.

**Dispatch**

- [ ] The `DISPATCH` form appears. `WORK ITEM` and `SIGNED SPEC` are filled in; `HARNESS` is
      `reqdrive · healthy`; `KEY` is a UUID. Leave `PROFILE` on `harness default` and the
      opt-in unticked.
- [ ] `▸ DISPATCH UNIT`. It says `Started u…`, a tile for the unit appears and is selected,
      and `KEY` changes to a new value.
- [ ] Press `▸ DISPATCH UNIT` once more while the unit runs. With `global_usd` equal to
      `unit_usd` the daemon refuses a second unit: the form shows the daemon's sentence about
      the spend ceiling in a red box, and no second tile appears. That box is a refusal shown
      verbatim.

**Watch it**

- [ ] The rail moves through `PROVISION`, `RED`, `PLAN`, `GREEN`, `CHECK`, `REVIEW`,
      `DELIVER`: each row `RUN`, then `DONE`. `TIME` and `COST` fill in for the stages an agent
      ran in. `ADAPTER · MODEL` shows a dash in every row: milestone 1's API does not carry
      them.
- [ ] The tile's cost rises and stays under the cap.
- [ ] The phase badge reaches `PR OPEN` and then `DONE`, and `▸ OPEN PULL REQUEST` appears in
      the right panel. Its address is a pull request on the sandbox repository. (If clicking
      it opens nothing, copy the link: how the shell opens an outside link is not this
      milestone's.)

**If the unit stops for you instead**

- [ ] The badge says `NEEDS YOU`. If the amber box under the command buttons says
      `Verification stopped`, there is a `RE-VERIFY` button: the control plane's own run of
      the tests failed or could not run. Read the activity log for why before pressing it.
      Pressing it runs the control plane's checks again on the result it kept, with no agent
      and no spend. Any other reason has no `RE-VERIFY`, by design.

**The daemon goes away and comes back**

- [ ] With a unit listed, end the `fleetd-serve` process in Task Manager. The terminal shows
      the supervisor restarting it within a few seconds.
- [ ] Do not reload the page. Launch one more `DEMO` unit from `LEGACY TASK`. It appears and
      its rail moves. That shows the page still holds a token the restarted daemon accepts,
      and that streams reopen by themselves.

**Stop conditions.** If the tile's cost passes the cap and the unit does not stop itself, or
anything happens that this list does not describe, press `ABANDON` on the unit, close the
window (which stops the daemon), then `docker ps -a --filter label=cc.unit_id` and remove what
is left. Record what happened before trying again.

**After the smoke**

- [ ] The date, the unit id, the pull request, the cap and the final cost go into
      `docs/STATUS.md` in the session's wrap-up, beside the control-plane proof's record.
- [ ] Anything on this list that could not be ticked is written down with what was seen
      instead. An unticked box is a finding, not a failure to hide.
- [ ] The pull request is left open. Whether to merge it is a separate decision.

---

## Self-review

What was actually done to check this plan, and what could not be closed. Nothing below was run
in any repository: every check ran in a scratch directory outside them, on Windows 11 with
Node 22.17.1, npm 11.0.0 and cargo 1.93.1.

### What was run

- **The schema.** `fleet-api` does not exist yet, so its schema was produced in a scratch
  crate: `dto.rs` and `ApiSchema` copied verbatim from the control-plane plan (Part 1, Task 8),
  with the protocol types they use (`Capabilities` and its enums, `HarnessInfo`, `Stage`,
  `StageStatus`) copied from the M0 contracts plan, compiled against `schemars` 1.2.2 and
  written with that plan's `schema_json()`. The result is draft 2020-12 with 32 definitions
  under `$defs`. The shapes this plan describes (`oneOf` with `const`, `anyOf` with `null`,
  no `additionalProperties` unless unknown fields are denied) were read from that output, not
  assumed.
- **The generator.** `json-schema-to-typescript` 16.0.0 was installed and
  `scripts/gen-types.mjs` run against that schema: 33 exports, the four shapes of Part 1 Task
  2 Step 4 as stated. The check was seen to fail for a missing file, to fail for one appended
  line, to pass after regenerating, and to pass on a copy converted to CRLF. `npm run check`
  with request C2's scripts was seen to exit 1 on drift and 0 without.
- **Wave 0.** A copy of `cockpit/ui` from `main` with Part 1 applied and nothing else:
  `npm run check` reports 0 errors and 0 warnings; `npx vitest run` reports 27 files, 236
  tests; `npx vite build` succeeds. The starting count on `main` was confirmed as 166 tests in
  23 files. The red states quoted in Part 1 were observed: the 39 failures of `api.test.ts`
  against the old client, the unresolved import of the fake, the failures of
  `store.connection.svelte.test.ts` against the old store, the four of
  `App.connection.test.ts` against the old page, and 225 passing under `src/lib` and
  `src/views` at the end of Task 5.
- **The lanes.** Each lane was built in full on top of that copy, including the components
  this plan leaves to the lane, to be sure the printed tests can be passed: 394 tests in 38
  files, `svelte-check` with 0 errors and 0 warnings. Then the lanes' implementation files
  were swapped back for wave 0's stubs and the lane tests run again; the failures quoted under
  each lane's **Behaviours** are what that run printed (97 failed, 18 passed, across the
  eleven lane test files).
- **The shell.** A copy of `cockpit/ui/src-tauri` with lane UI-TOKEN's `token.rs` and its
  changes to `sidecar.rs` and `lib.rs`, and `getrandom = "0.3"`: `cargo test` passes (46
  library tests, 11 of them new, and the two existing guard suites, so the new command
  satisfies the threading guard); `cargo clippy --all-targets -- -D warnings` and `cargo fmt
  --check` are clean; the lock file gains one line and no package. The two red states quoted
  for the shell's tests (nine errors, then seven) were observed by compiling with only the
  test modules in place.
- **The canary build.** `vite build` with `VITE_FLEETD_TOKEN` set in the environment: the
  value is not in `dist/`.
- **Contrast.** The ratios in Global Constraints were computed from the hex values in
  `src/app.css`.
- **The verify commands.** Every `grep` in a Verify table was run against the scratch build
  and returns what its row says.

### Gaps that could not be closed

1. **The real schema was not available.** If the workspace turns on `serde_json`'s
   `preserve_order` anywhere, the real file's key order differs from the scratch one and the
   generated file's member order with it. Names and shapes should not differ; Part 1 Task 1
   checks, and stops if they do.
2. **Nothing was run against a daemon.** The milestone-1 daemon does not exist. The fake's
   behaviour is this plan's reading of the control-plane plan's tests. Where the two could
   differ in a way the page would feel: whether a refused request consumes its idempotency
   key (this plan assumes not, and keeps the key); the exact `message` texts (the page shows
   whatever arrives); and whether the scripted demo harness sends stage notes.
3. **The desktop app was never started.** The shell's changes compile and their unit tests
   pass. That `fleetd_token` returns the token to the page, that `Command::env` reaches the
   sidecar, and that the health gate opens were not observed. The smoke checklist is where
   they are.
4. **Whether a sandboxed view-plugin frame can call an app command was not established.** The
   label check stops other webviews; it cannot tell a frame inside the main webview from the
   page. The checklist has the operator test it. If it can, the fix is an app manifest in
   `src-tauri/build.rs` with the capability files to match, which no lane here owns.
5. **`crypto.subtle` in the packaged webview was not observed.** It is expected, because
   `tauri.localhost` is a secure context. If it is missing, no spec can be signed from the
   desktop app, and the page says so rather than signing blind.
6. **Node 20.** CI runs Node 20; everything here ran on 22.17.1. The generator declares
   support from Node 16.
7. **Per-stage numbers are approximations until the API changes.** Cost and time are
   attributed by which stage was running when a cumulative metric arrived. A metric that
   arrives after its stage finished is charged to the stage that just finished; one that
   covers two stages is charged to the later. The time is metered agent time, not elapsed
   time.
8. **The Sign surface takes a typed repository and spec id.** There is no route that lists
   either. Issue #92's "no id is typed by hand" is met for the work item (it comes from the
   signed spec) and not for the spec.
9. **A finished unit whose stream drops mid-replay** is retried until it has read as far as
   its summary's `last_seq`. A unit that finishes while its stream is down is caught by the
   same rule only on the next load, because its summary is not refreshed in between.
10. **`fireEvent.click` on a disabled button** reaches the handler in jsdom, and does not in a
    browser. The handlers guard for both; the tests exercise the jsdom case.

### Decisions left to the owner

- **Add a time to each record, and stage, adapter and model to the API's `metric`.** The store
  already records a time for every event; `EventEnvelopeDto` does not carry it. The harness
  already reports stage, adapter and model on its own metric; the control plane's `metric`
  drops them. Both are additive changes to `fleet-api`'s Form, and both would make the rail
  exact.
- **Put app commands under Tauri's permission lists** (gap 4), whatever the smoke finds.
- **A spend-cap field on the Dispatch form.** Left out: the daemon's configured ceiling
  applies, and the watched proof sets that ceiling to the operator's number.
- **The existing page loads two fonts from Google** (`cockpit/ui/index.html`). Nothing here
  adds to that, and nothing here removes it; it is outside every lane. The content-security
  policy in `tauri.conf.json` does not allow that stylesheet, so the desktop app should already
  be running on the fallback fonts.
- **Issue #92 stays open.** Its per-issue cards and card-level Dispatch are a later
  milestone's.
