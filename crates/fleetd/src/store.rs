//! SQLite persistence (WAL). One writer owns a mutating connection; reads use a
//! separate connection (WAL allows concurrent readers). No async here — callers
//! must never hold the connection across `.await`.

use rusqlite::{params, Connection};
use std::path::Path;

pub struct Store {
    conn: Connection,
}

/// The persisted projection of a unit (mirrors `UnitSpec` + live phase/cost).
#[derive(Debug, Clone)]
pub struct UnitRow {
    pub unit_id: String,
    pub tier: String,
    pub task: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub branch: String,
    pub test_cmd: String,
    pub usd_cap: f64,
    pub wall_clock_secs: u64,
    pub phase: String,
    pub cost: f64,
    pub last_seq: u64,
    pub oracle_frozen: bool,
    pub oracle_hash: Option<String>,
    pub terminal_reason: Option<String>,
    /// Runner mode ("demo" | "real"), set once at create so rehydration picks the
    /// right runner (a demo unit doesn't attempt a real Docker provision).
    pub mode: String,
    /// Review-gate floor, set once at create so a rehydrated unit resumes with the
    /// same `min_review_rounds` it was launched with.
    pub min_review_rounds: u32,
    /// Parent swarm id, set once at create for a lane's unit; `None` for a
    /// standalone mission. Set-once — never updated by the live projection.
    pub swarm_id: Option<String>,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }
    pub fn open_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS units(
               unit_id TEXT PRIMARY KEY, tier TEXT, task TEXT, repo_url TEXT, repo_slug TEXT,
               base_branch TEXT, branch TEXT, test_cmd TEXT, usd_cap REAL, wall_clock_secs INTEGER,
               phase TEXT, cost REAL, last_seq INTEGER, oracle_frozen INTEGER, oracle_hash TEXT,
               created_ts INTEGER, updated_ts INTEGER, terminal_reason TEXT,
               mode TEXT NOT NULL DEFAULT 'demo', min_review_rounds INTEGER NOT NULL DEFAULT 2);
             CREATE TABLE IF NOT EXISTS events(
               unit_id TEXT, seq INTEGER, ts INTEGER, json TEXT,
               PRIMARY KEY(unit_id, seq));
             CREATE TABLE IF NOT EXISTS swarms(
               swarm_id TEXT PRIMARY KEY, repo_url TEXT, repo_slug TEXT, base_branch TEXT,
               doc_path TEXT, tier TEXT, mode TEXT, lane_cap INTEGER, usd_budget REAL, per_lane_cap REAL,
               status TEXT, planner_cost REAL, lanes_launched INTEGER, lanes_dropped INTEGER,
               min_review_rounds INTEGER, terminal_reason TEXT, created_ts INTEGER, updated_ts INTEGER);
             CREATE TABLE IF NOT EXISTS swarm_lanes(
               swarm_id TEXT, idx INTEGER, title TEXT, task TEXT, rationale TEXT,
               decision TEXT, unit_id TEXT, PRIMARY KEY(swarm_id, idx));",
        )?;
        // Migrate DBs created before these columns existed. Idempotent: on a fresh
        // DB the columns already exist (from CREATE TABLE) and the ALTER is a no-op
        // failure we ignore; on an old DB it adds them with the safe defaults.
        for stmt in [
            "ALTER TABLE units ADD COLUMN mode TEXT NOT NULL DEFAULT 'demo'",
            "ALTER TABLE units ADD COLUMN min_review_rounds INTEGER NOT NULL DEFAULT 2",
            "ALTER TABLE units ADD COLUMN swarm_id TEXT",
            "CREATE INDEX IF NOT EXISTS idx_units_swarm ON units(swarm_id)",
            "ALTER TABLE units ADD COLUMN oracle_hash TEXT",
        ] {
            let _ = conn.execute(stmt, []);
        }
        Ok(Self { conn })
    }

    /// Insert a new unit or update the mutable projection columns of an existing one.
    /// `created_ts` is set only on first insert (ON CONFLICT keeps the original).
    pub fn upsert_unit(&self, r: &UnitRow, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO units(unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
               usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,created_ts,updated_ts,terminal_reason,
               mode,min_review_rounds,swarm_id,oracle_hash)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,?16,?17,?18,?19,?20)
             ON CONFLICT(unit_id) DO UPDATE SET
               phase=?11, cost=?12, last_seq=?13, oracle_frozen=?14, updated_ts=?15, terminal_reason=?16",
            params![
                r.unit_id, r.tier, r.task, r.repo_url, r.repo_slug, r.base_branch, r.branch,
                r.test_cmd, r.usd_cap, r.wall_clock_secs, r.phase, r.cost, r.last_seq,
                r.oracle_frozen as i64, now, r.terminal_reason, r.mode, r.min_review_rounds,
                r.swarm_id, r.oracle_hash
            ],
        )?;
        Ok(())
    }

    /// Update the live projection columns of an existing unit (forwarder hot path).
    /// `oracle_hash`, when present, is persisted and flips `oracle_frozen` true;
    /// when `None` a prior hash/frozen state is left untouched (COALESCE/OR).
    #[allow(clippy::too_many_arguments)]
    pub fn update_unit(
        &self,
        unit_id: &str,
        phase: &str,
        cost: f64,
        last_seq: u64,
        terminal_reason: Option<&str>,
        oracle_hash: Option<&str>,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE units SET phase=?2, cost=?3, last_seq=?4, terminal_reason=?5,
               oracle_hash=COALESCE(?6, oracle_hash),
               oracle_frozen=(oracle_frozen OR (?6 IS NOT NULL)),
               updated_ts=?7
             WHERE unit_id=?1",
            params![unit_id, phase, cost, last_seq, terminal_reason, oracle_hash, now],
        )?;
        Ok(())
    }

    pub fn append_event(&self, unit_id: &str, seq: u64, ts: i64, json: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO events(unit_id,seq,ts,json) VALUES(?1,?2,?3,?4)",
            params![unit_id, seq, ts, json],
        )?;
        Ok(())
    }

    pub fn get_unit(&self, id: &str) -> rusqlite::Result<Option<UnitRow>> {
        self.conn
            .query_row(SELECT_COLS_WHERE_ID, params![id], Self::map_row)
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
    }

    pub fn list_units(&self) -> rusqlite::Result<Vec<UnitRow>> {
        let mut s = self.conn.prepare(SELECT_COLS_ALL)?;
        let rows = s.query_map([], Self::map_row)?;
        rows.collect()
    }

    pub fn events_since(&self, id: &str, since: u64) -> rusqlite::Result<Vec<String>> {
        let mut s = self
            .conn
            .prepare("SELECT json FROM events WHERE unit_id=?1 AND seq>?2 ORDER BY seq")?;
        let rows = s.query_map(params![id, since], |r| r.get::<_, String>(0))?;
        rows.collect()
    }

    /// Highest `sw{n}` suffix currently persisted (0 if none). Seeds the swarm-id
    /// allocator on startup so a restart never re-mints a live swarm id.
    pub fn max_swarm_seq(&self) -> rusqlite::Result<u64> {
        let mut s = self.conn.prepare("SELECT swarm_id FROM swarms")?;
        let ids = s.query_map([], |r| r.get::<_, String>(0))?;
        let mut max = 0u64;
        for id in ids {
            if let Some(n) = id?.strip_prefix("sw").and_then(|n| n.parse::<u64>().ok()) {
                max = max.max(n);
            }
        }
        Ok(max)
    }

    /// Highest `u{n}` suffix currently persisted (0 if none). Used to seed the
    /// in-memory id allocator on startup so a restart never re-mints a live id.
    pub fn max_unit_seq(&self) -> rusqlite::Result<u64> {
        let mut s = self.conn.prepare("SELECT unit_id FROM units")?;
        let ids = s.query_map([], |r| r.get::<_, String>(0))?;
        let mut max = 0u64;
        for id in ids {
            if let Some(n) = id?.strip_prefix('u').and_then(|n| n.parse::<u64>().ok()) {
                max = max.max(n);
            }
        }
        Ok(max)
    }

    /// Worst-case committed spend in the rolling window: terminal units count their
    /// final `cost`; non-terminal units count `MAX(usd_cap, cost)` (the driver can
    /// bill a step past the cap before parking, so cost may exceed usd_cap); swarms
    /// count their `planner_cost`. Partitioned by the authoritative terminal list.
    pub fn committed_spend(&self, since_ts: i64) -> rusqlite::Result<f64> {
        let terminal = fleet_core::TERMINAL_PHASE_STRS
            .iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT
               COALESCE((SELECT SUM(cost) FROM units
                           WHERE created_ts>=?1 AND phase IN ({terminal})),0)
             + COALESCE((SELECT SUM(MAX(usd_cap,cost)) FROM units
                           WHERE created_ts>=?1 AND phase NOT IN ({terminal})),0)
             + COALESCE((SELECT SUM(planner_cost) FROM swarms WHERE created_ts>=?1),0)"
        );
        self.conn.query_row(&sql, params![since_ts], |r| r.get(0))
    }

    fn map_row(r: &rusqlite::Row) -> rusqlite::Result<UnitRow> {
        Ok(UnitRow {
            unit_id: r.get(0)?,
            tier: r.get(1)?,
            task: r.get(2)?,
            repo_url: r.get(3)?,
            repo_slug: r.get(4)?,
            base_branch: r.get(5)?,
            branch: r.get(6)?,
            test_cmd: r.get(7)?,
            usd_cap: r.get(8)?,
            wall_clock_secs: r.get::<_, i64>(9)? as u64,
            phase: r.get(10)?,
            cost: r.get(11)?,
            last_seq: r.get::<_, i64>(12)? as u64,
            oracle_frozen: r.get::<_, i64>(13)? != 0,
            oracle_hash: r.get(14)?,
            terminal_reason: r.get(15)?,
            mode: r.get(16)?,
            min_review_rounds: r.get::<_, i64>(17)? as u32,
            swarm_id: r.get(18)?,
        })
    }
}

const SELECT_COLS_ALL: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,oracle_hash,terminal_reason,mode,min_review_rounds,swarm_id FROM units";
const SELECT_COLS_WHERE_ID: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,oracle_hash,terminal_reason,mode,min_review_rounds,swarm_id FROM units WHERE unit_id=?1";
const SELECT_SWARM_ALL: &str = "SELECT swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,min_review_rounds,terminal_reason FROM swarms ORDER BY created_ts DESC";
const SELECT_SWARM_WHERE_ID: &str = "SELECT swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,min_review_rounds,terminal_reason FROM swarms WHERE swarm_id=?1";

/// Persisted swarm config + mutable projection. `created_ts` set once at insert.
#[derive(Debug, Clone)]
pub struct SwarmRow {
    pub swarm_id: String,
    pub repo_url: String,
    pub repo_slug: String,
    pub base_branch: String,
    pub doc_path: String,
    pub tier: String,
    pub mode: String,
    pub lane_cap: u32,
    pub usd_budget: f64,
    pub per_lane_cap: f64,
    pub status: String,
    pub planner_cost: f64,
    pub lanes_launched: u32,
    pub lanes_dropped: u32,
    pub min_review_rounds: u32,
    pub terminal_reason: Option<String>,
}

impl Store {
    pub fn upsert_swarm(&self, r: &SwarmRow, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO swarms(swarm_id,repo_url,repo_slug,base_branch,doc_path,tier,mode,
               lane_cap,usd_budget,per_lane_cap,status,planner_cost,lanes_launched,lanes_dropped,
               min_review_rounds,terminal_reason,created_ts,updated_ts)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)
             ON CONFLICT(swarm_id) DO UPDATE SET
               status=?11, planner_cost=?12, lanes_launched=?13, lanes_dropped=?14,
               terminal_reason=?16, updated_ts=?17",
            params![r.swarm_id, r.repo_url, r.repo_slug, r.base_branch, r.doc_path, r.tier, r.mode,
                r.lane_cap, r.usd_budget, r.per_lane_cap, r.status, r.planner_cost,
                r.lanes_launched, r.lanes_dropped, r.min_review_rounds, r.terminal_reason, now],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_swarm(&self, id: &str, status: &str, planner_cost: f64, lanes_launched: u32,
        lanes_dropped: u32, terminal_reason: Option<&str>, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE swarms SET status=?2, planner_cost=?3, lanes_launched=?4, lanes_dropped=?5,
               terminal_reason=?6, updated_ts=?7 WHERE swarm_id=?1",
            params![id, status, planner_cost, lanes_launched, lanes_dropped, terminal_reason, now],
        )?;
        Ok(())
    }

    pub fn get_swarm(&self, id: &str) -> rusqlite::Result<Option<SwarmRow>> {
        self.conn.query_row(SELECT_SWARM_WHERE_ID, params![id], Self::map_swarm)
            .map(Some).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None), other => Err(other),
            })
    }

    pub fn list_swarms(&self) -> rusqlite::Result<Vec<SwarmRow>> {
        let mut s = self.conn.prepare(SELECT_SWARM_ALL)?;
        let rows = s.query_map([], Self::map_swarm)?;
        rows.collect()
    }

    /// (total, terminal, awaiting_human) over a swarm's child units. `awaiting_human`
    /// = non-terminal parked phases (`needs_human`/`halted`).
    pub fn swarm_rollup(&self, swarm_id: &str) -> rusqlite::Result<(u64, u64, u64)> {
        let terminal = fleet_core::TERMINAL_PHASE_STRS
            .iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT
               COUNT(*),
               COALESCE(SUM(CASE WHEN phase IN ({terminal}) THEN 1 ELSE 0 END),0),
               COALESCE(SUM(CASE WHEN phase IN ('needs_human','halted') THEN 1 ELSE 0 END),0)
             FROM units WHERE swarm_id=?1"
        );
        self.conn.query_row(&sql, params![swarm_id], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64))
        })
    }

    fn map_swarm(r: &rusqlite::Row) -> rusqlite::Result<SwarmRow> {
        Ok(SwarmRow {
            swarm_id: r.get(0)?, repo_url: r.get(1)?, repo_slug: r.get(2)?, base_branch: r.get(3)?,
            doc_path: r.get(4)?, tier: r.get(5)?, mode: r.get(6)?,
            lane_cap: r.get::<_, i64>(7)? as u32, usd_budget: r.get(8)?, per_lane_cap: r.get(9)?,
            status: r.get(10)?, planner_cost: r.get(11)?,
            lanes_launched: r.get::<_, i64>(12)? as u32, lanes_dropped: r.get::<_, i64>(13)? as u32,
            min_review_rounds: r.get::<_, i64>(14)? as u32, terminal_reason: r.get(15)?,
        })
    }
}

/// Persisted lane record (admission decision + optional back-link to the unit).
#[derive(Debug, Clone)]
pub struct LaneRow {
    pub swarm_id: String,
    pub idx: u32,
    pub title: String,
    pub task: String,
    pub rationale: String,
    pub decision: String,
    pub unit_id: Option<String>,
}

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_lane(&self, swarm_id: &str, idx: u32, title: &str, task: &str,
        rationale: &str, decision: &str, unit_id: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO swarm_lanes(swarm_id,idx,title,task,rationale,decision,unit_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(swarm_id,idx) DO UPDATE SET decision=?6, unit_id=?7",
            params![swarm_id, idx, title, task, rationale, decision, unit_id],
        )?;
        Ok(())
    }

    pub fn lanes_for_swarm(&self, swarm_id: &str) -> rusqlite::Result<Vec<LaneRow>> {
        let mut s = self.conn.prepare(
            "SELECT swarm_id,idx,title,task,rationale,decision,unit_id
             FROM swarm_lanes WHERE swarm_id=?1 ORDER BY idx")?;
        let rows = s.query_map(params![swarm_id], |r| Ok(LaneRow {
            swarm_id: r.get(0)?, idx: r.get::<_, i64>(1)? as u32, title: r.get(2)?,
            task: r.get(3)?, rationale: r.get(4)?, decision: r.get(5)?, unit_id: r.get(6)?,
        }))?;
        rows.collect()
    }

    /// Insert the lane's unit row AND set `swarm_lanes.unit_id` in ONE transaction,
    /// so a crash never leaves a dangling back-link or an orphan row (spec R2 #5).
    pub fn commit_lane_unit(&self, swarm_id: &str, idx: u32, u: &UnitRow, now: i64)
        -> rusqlite::Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let r: rusqlite::Result<()> = (|| {
            self.upsert_unit(u, now)?;
            self.conn.execute(
                "UPDATE swarm_lanes SET unit_id=?3 WHERE swarm_id=?1 AND idx=?2",
                params![swarm_id, idx, u.unit_id],
            )?;
            Ok(())
        })();
        match r {
            Ok(()) => { self.conn.execute_batch("COMMIT")?; Ok(()) }
            Err(e) => { let _ = self.conn.execute_batch("ROLLBACK"); Err(e) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> UnitRow {
        UnitRow {
            unit_id: id.into(),
            tier: "t1".into(),
            task: "t".into(),
            repo_url: "u".into(),
            repo_slug: "s".into(),
            base_branch: "main".into(),
            branch: format!("agent/{id}"),
            test_cmd: "node --test".into(),
            usd_cap: 1.0,
            wall_clock_secs: 600,
            phase: "queued".into(),
            cost: 0.0,
            last_seq: 0,
            oracle_frozen: false,
            oracle_hash: None,
            terminal_reason: None,
            mode: "demo".into(),
            min_review_rounds: 2,
            swarm_id: None,
        }
    }

    #[test]
    fn upsert_append_list_since_spend() {
        let s = Store::open_memory().unwrap();
        s.upsert_unit(&row("u1"), 1000).unwrap();
        s.append_event("u1", 1, 1000, r#"{"type":"phase_changed"}"#).unwrap();
        s.append_event("u1", 2, 1001, r#"{"type":"metric"}"#).unwrap();
        assert_eq!(s.events_since("u1", 0).unwrap().len(), 2);
        assert_eq!(s.events_since("u1", 1).unwrap().len(), 1);

        let mut r = row("u1");
        r.cost = 0.5;
        r.phase = "done".into();
        s.upsert_unit(&r, 1002).unwrap();
        assert_eq!(s.list_units().unwrap().len(), 1);
        let got = s.get_unit("u1").unwrap().unwrap();
        assert_eq!(got.cost, 0.5);
        assert_eq!(got.phase, "done");
    }

    #[test]
    fn persists_mode_and_review_floor() {
        // mode + gate floor are set-once at create and survive a projection update,
        // so a rehydrated unit can resume as the right runner with the right gate.
        let s = Store::open_memory().unwrap();
        let mut r = row("u1");
        r.mode = "real".into();
        r.min_review_rounds = 3;
        s.upsert_unit(&r, 1000).unwrap();
        // A later live-projection upsert must NOT clobber mode/floor.
        let mut upd = row("u1");
        upd.mode = "demo".into(); // would-be clobber
        upd.min_review_rounds = 1; // would-be clobber
        upd.phase = "building".into();
        s.upsert_unit(&upd, 1001).unwrap();
        let got = s.get_unit("u1").unwrap().unwrap();
        assert_eq!(got.mode, "real", "mode is set-once at create");
        assert_eq!(got.min_review_rounds, 3, "review floor is set-once at create");
        assert_eq!(got.phase, "building", "projection columns still update");
    }

    #[test]
    fn oracle_hash_round_trips_and_freezes_on_update() {
        // The oracle fingerprint round-trips through upsert_unit, and update_unit
        // (the fold's hot path) persists a later hash and flips oracle_frozen.
        let s = Store::open_memory().unwrap();
        let mut r = row("u1");
        r.oracle_hash = Some("h0000000000000abc".into());
        s.upsert_unit(&r, 1000).unwrap();
        let got = s.get_unit("u1").unwrap().unwrap();
        assert_eq!(got.oracle_hash.as_deref(), Some("h0000000000000abc"), "oracle_hash round-trips");
        assert!(!got.oracle_frozen, "upsert_unit alone does not flip oracle_frozen");

        s.update_unit("u1", "checking", got.cost, got.last_seq + 1, None,
            Some("h0000000000000abc"), 1001).unwrap();
        let got2 = s.get_unit("u1").unwrap().unwrap();
        assert_eq!(got2.oracle_hash.as_deref(), Some("h0000000000000abc"));
        assert!(got2.oracle_frozen, "oracle_frozen flips true once update_unit sees a hash");
    }

    #[test]
    fn append_is_idempotent_on_seq() {
        let s = Store::open_memory().unwrap();
        s.upsert_unit(&row("u1"), 1).unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap(); // OR IGNORE → no duplicate
        assert_eq!(s.events_since("u1", 0).unwrap().len(), 1);
    }

    #[test]
    fn swarm_tables_and_unit_swarm_id_exist_after_init() {
        let s = Store::open_memory().unwrap();
        // These inserts only succeed if the schema migrated.
        s.conn.execute(
            "INSERT INTO swarms(swarm_id,status,planner_cost,created_ts,updated_ts) VALUES('s1','planning',0.0,1,1)",
            [],
        ).unwrap();
        s.conn.execute(
            "INSERT INTO swarm_lanes(swarm_id,idx,title,task,rationale,decision) VALUES('s1',0,'t','task','r','admit')",
            [],
        ).unwrap();
        // units.swarm_id column present:
        s.conn.execute("UPDATE units SET swarm_id='s1' WHERE unit_id='nope'", []).unwrap();
    }

    #[test]
    fn unit_swarm_id_round_trips() {
        let s = Store::open_memory().unwrap();
        let mut r = row("u1");
        r.swarm_id = Some("sw1".into());
        s.upsert_unit(&r, 1000).unwrap();
        assert_eq!(s.get_unit("u1").unwrap().unwrap().swarm_id.as_deref(), Some("sw1"));
    }

    #[test]
    fn max_swarm_seq_reads_highest_numeric_suffix() {
        let s = Store::open_memory().unwrap();
        assert_eq!(s.max_swarm_seq().unwrap(), 0);
        s.upsert_swarm(&swarm_row("sw3", "planning"), 1).unwrap();
        s.upsert_swarm(&swarm_row("sw10", "planning"), 1).unwrap();
        assert_eq!(s.max_swarm_seq().unwrap(), 10);
    }

    #[test]
    fn max_unit_seq_reads_highest_numeric_suffix() {
        let s = Store::open_memory().unwrap();
        assert_eq!(s.max_unit_seq().unwrap(), 0, "empty store → 0");
        s.upsert_unit(&row("u3"), 1).unwrap();
        s.upsert_unit(&row("u10"), 1).unwrap();
        s.upsert_unit(&row("u2"), 1).unwrap();
        assert_eq!(s.max_unit_seq().unwrap(), 10, "parses the numeric suffix, not lexical max");
    }

    #[test]
    fn committed_spend_counts_reservations_and_overcap_and_planner() {
        let s = Store::open_memory().unwrap();
        // A terminal unit contributes its final cost.
        let mut done = row("done"); done.phase = "done".into(); done.cost = 1.0; done.usd_cap = 5.0;
        s.upsert_unit(&done, 1000).unwrap();
        // A non-terminal unit under cap contributes its usd_cap (reservation).
        let mut run = row("run"); run.phase = "building".into(); run.cost = 0.5; run.usd_cap = 5.0;
        s.upsert_unit(&run, 1000).unwrap();
        // A non-terminal unit that BILLED PAST its cap contributes cost (MAX), not usd_cap.
        let mut over = row("over"); over.phase = "building".into(); over.cost = 9.0; over.usd_cap = 5.0;
        s.upsert_unit(&over, 1000).unwrap();
        // Planner cost of a swarm counts too.
        s.conn.execute(
            "INSERT INTO swarms(swarm_id,status,planner_cost,created_ts,updated_ts) VALUES('sw',?1,?2,1000,1000)",
            rusqlite::params!["failed", 2.0],
        ).unwrap();
        // 1.0 (done) + 5.0 (run reservation) + 9.0 (over, MAX) + 2.0 (planner) = 17.0
        assert!((s.committed_spend(0).unwrap() - 17.0).abs() < 1e-9);
    }

    #[test]
    fn committed_spend_window_excludes_old() {
        let s = Store::open_memory().unwrap();
        let mut old = row("old"); old.phase = "building".into(); old.usd_cap = 5.0;
        s.upsert_unit(&old, 100).unwrap();
        s.conn.execute(
            "INSERT INTO swarms(swarm_id,status,planner_cost,created_ts,updated_ts) VALUES('oldsw','failed',7.0,100,100)",
            [],
        ).unwrap();
        assert_eq!(s.committed_spend(500).unwrap(), 0.0, "created before the window is excluded");
    }

    fn swarm_row(id: &str, status: &str) -> SwarmRow {
        SwarmRow {
            swarm_id: id.into(), repo_url: "u".into(), repo_slug: "s".into(), base_branch: "main".into(),
            doc_path: "spec.md".into(), tier: "t1".into(), mode: "demo".into(),
            lane_cap: 8, usd_budget: 15.0, per_lane_cap: 5.0, status: status.into(),
            planner_cost: 0.0, lanes_launched: 0, lanes_dropped: 0, min_review_rounds: 2,
            terminal_reason: None,
        }
    }

    #[test]
    fn swarm_row_crud_and_list() {
        let s = Store::open_memory().unwrap();
        let sw = swarm_row("sw1", "planning");
        s.upsert_swarm(&sw, 1000).unwrap();
        assert_eq!(s.get_swarm("sw1").unwrap().unwrap().status, "planning");
        s.update_swarm("sw1", "running", 0.4, 3, 0, None, 1001).unwrap();
        let got = s.get_swarm("sw1").unwrap().unwrap();
        assert_eq!(got.status, "running");
        assert_eq!(got.planner_cost, 0.4);
        assert_eq!(got.lanes_launched, 3);
        assert_eq!(s.list_swarms().unwrap().len(), 1);
    }

    #[test]
    fn lane_crud_and_commit_is_atomic() {
        let s = Store::open_memory().unwrap();
        s.upsert_swarm(&swarm_row("sw1", "fanning_out"), 1000).unwrap();
        // Persist a lane with no unit yet.
        s.upsert_lane("sw1", 0, "Add auth", "do auth", "indep", "admit", None).unwrap();
        let lanes = s.lanes_for_swarm("sw1").unwrap();
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].decision, "admit");
        assert!(lanes[0].unit_id.is_none());

        // commit_lane_unit inserts the unit row AND sets the back-link in one txn.
        let mut u = row("u1"); u.swarm_id = Some("sw1".into());
        s.commit_lane_unit("sw1", 0, &u, 1001).unwrap();
        assert!(s.get_unit("u1").unwrap().is_some(), "unit row inserted");
        assert_eq!(s.lanes_for_swarm("sw1").unwrap()[0].unit_id.as_deref(), Some("u1"), "back-link set");
    }

    #[test]
    fn swarm_rollup_counts_terminal_and_awaiting_human() {
        let s = Store::open_memory().unwrap();
        for (id, phase) in [("u1","done"), ("u2","failed"), ("u3","building"), ("u4","needs_human")] {
            let mut r = row(id); r.phase = phase.into(); r.swarm_id = Some("sw1".into());
            s.upsert_unit(&r, 1000).unwrap();
        }
        let (total, terminal, awaiting) = s.swarm_rollup("sw1").unwrap();
        assert_eq!(total, 4);
        assert_eq!(terminal, 2);          // done + failed
        assert_eq!(awaiting, 1);          // needs_human (halted would also count)
    }
}
