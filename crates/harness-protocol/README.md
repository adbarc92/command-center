# harness-protocol

The wire contract between `fleetd` (the control plane) and a **harness**, the process that runs
one unit of work. Design: NEXUS `docs/specs/2026-09-14-swappable-harness-design.md` §3.

## Transport

JSON-RPC 2.0, **one JSON object per line**, over the harness's stdin and stdout. Stderr is free-form
log. One harness process per unit. Closing stdin means "shut down".

## Lifecycle

| Step | Message | Direction |
|---|---|---|
| 1 | `initialize` `{protocol_version}` → `{protocol_version, harness, capabilities}` | control plane → harness |
| 2 | `unit/start` work order → `{}` | → |
| 3 | `unit/event` observations, metrics, logs, findings, artifacts, errors | ← notification |
| 4 | `gate/request` `{gate: oracle, …}` → `{approved}` (T2/T3) | ← request |
| — | `unit/halt`, `unit/abandon`, `unit/resume` → `{}` | → at any time |
| 5 | `unit/result` `{outcome, evidence?, failure?}`, then exit | ← notification |

Refuse an `initialize` from a different protocol **major** with error code `-32001`.

## Rules the control plane enforces

- A harness **never merges** and **never declares done**. `evidence` is re-read from git, CI and
  the test run before any unit reaches `PrOpen`.
- `capabilities` decide eligibility. T2/T3 units need `gates: [oracle]`. Isolation weaker than
  `container`, or `metering: none`, needs per-unit operator opt-in.
- If you declare `metering: usd`, send at least one `metric` before a non-failed result.
- After `unit/result`, send nothing more and exit.
- After answering `unit/halt` or `unit/abandon`, exit **without** `unit/result`.

## The contract

`contract/harness-protocol.contract.json` is the JSON Schema for every message, pinned by canonical
SHA-256 in `tests/contract.rs` and registered in NEXUS `docs/contracts/README.md`. To change the
protocol on purpose, re-bless with `HARNESS_PROTOCOL_BLESS=1 cargo test -p harness-protocol --test
contract`, update the pinned hash, and update the NEXUS registry row in the same change.

## Checking your harness

```bash
cargo build -p harness-conformance --bins
target/debug/harness-conformance -- path/to/your-harness --its --args
```

Exit `0` means no case failed. Cases your harness declares it cannot do are reported `SKIP`, not
`PASS`. `target/debug/harness-fake` is a reference harness; set `HARNESS_FAKE_MODE` to see what each
violation looks like.
