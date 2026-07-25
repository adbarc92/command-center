#!/usr/bin/env node
// Durable restart-recovery demo.
//
// Proves the resume claim "recovers durably across restarts": a demo unit's full
// event log, created by one `fleetd` process, is served intact by a SECOND process
// after a HARD kill (SIGKILL — no graceful shutdown) — reconstructed from the
// SQLite store, not from memory.
//
// Prereqs: a debug build of the `serve` binary (`cargo build -p fleetd --bin serve`)
// and Node 18+ (for global `fetch`). Cross-platform; no Docker or API key needed.
//
//   node scripts/demo-restart-recovery.mjs
//
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { rmSync, existsSync } from "node:fs";

const ADDR = process.env.DEMO_ADDR ?? "127.0.0.1:8799";
const BASE = `http://${ADDR}`;
const DB = process.env.DEMO_DB ?? "demo-restart.db";
const SERVE =
  process.platform === "win32" ? "target/debug/serve.exe" : "target/debug/serve";

if (!existsSync(SERVE)) {
  console.error(`missing ${SERVE} — run: cargo build -p fleetd --bin serve`);
  process.exit(2);
}

// Fresh SQLite file so the run is reproducible.
for (const f of [DB, `${DB}-wal`, `${DB}-shm`]) if (existsSync(f)) rmSync(f);

function startDaemon(label) {
  const child = spawn(SERVE, [], {
    // CC_RECONCILE_SECS=0 keeps the periodic loop out of this durability demo.
    env: { ...process.env, CC_ADDR: ADDR, CC_DB: DB, CC_RECONCILE_SECS: "0" },
    stdio: "ignore",
  });
  child.on("error", (e) => {
    console.error(`${label} failed to spawn:`, e.message);
    process.exit(2);
  });
  return child;
}

async function waitHealthy(tries = 100) {
  for (let i = 0; i < tries; i++) {
    try {
      if ((await fetch(`${BASE}/health`)).ok) return;
    } catch {
      /* not up yet */
    }
    await sleep(100);
  }
  throw new Error("daemon never became healthy");
}

const getUnit = async (id) => (await fetch(`${BASE}/units/${id}`)).json();

async function main() {
  // ── Act 1: first daemon dispatches a demo unit ────────────────────────────
  let daemon = startDaemon("daemon #1");
  await waitHealthy();
  console.log("daemon #1 up");

  const { unit_id } = await (
    await fetch(`${BASE}/missions`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        task: "add a sum() helper with tests",
        tier: "t1",
        mode: "demo",
        min_review_rounds: 1,
      }),
    })
  ).json();
  console.log(`dispatched demo unit ${unit_id}`);

  // Let the unit accumulate a persisted history.
  await sleep(600);
  const before = await getUnit(unit_id);
  console.log(
    `  before kill:   phase=${before.phase}  events=${before.events.length}`,
  );

  // ── Act 2: HARD kill — simulate a crash, no graceful shutdown ──────────────
  daemon.kill("SIGKILL");
  await sleep(300);
  console.log("daemon #1 killed (SIGKILL)");

  // ── Act 3: a brand-new process, pointed at the SAME SQLite file ────────────
  daemon = startDaemon("daemon #2");
  await waitHealthy();
  console.log("daemon #2 up (same SQLite db, cold memory)");

  const after = await getUnit(unit_id);
  console.log(
    `  after restart: phase=${after.phase}  events=${after.events.length}`,
  );
  daemon.kill("SIGKILL");

  // ── Verdict ────────────────────────────────────────────────────────────────
  const survived =
    after.events.length > 0 && after.events.length === before.events.length;
  if (survived) {
    console.log(
      `\nPASS — ${after.events.length}-event history for ${unit_id} survived a hard ` +
        `restart, restored from ${DB} by a cold process.`,
    );
    process.exit(0);
  }
  console.log(
    `\nFAIL — event log did not survive (before=${before.events.length}, after=${after.events.length}).`,
  );
  process.exit(1);
}

main().catch((e) => {
  console.error("demo error:", e.message);
  process.exit(1);
});
