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
               phase TEXT, cost REAL, last_seq INTEGER, oracle_frozen INTEGER,
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
               mode,min_review_rounds,swarm_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,?16,?17,?18,?19)
             ON CONFLICT(unit_id) DO UPDATE SET
               phase=?11, cost=?12, last_seq=?13, oracle_frozen=?14, updated_ts=?15, terminal_reason=?16",
            params![
                r.unit_id, r.tier, r.task, r.repo_url, r.repo_slug, r.base_branch, r.branch,
                r.test_cmd, r.usd_cap, r.wall_clock_secs, r.phase, r.cost, r.last_seq,
                r.oracle_frozen as i64, now, r.terminal_reason, r.mode, r.min_review_rounds,
                r.swarm_id
            ],
        )?;
        Ok(())
    }

    /// Update the live projection columns of an existing unit (forwarder hot path).
    pub fn update_unit(
        &self,
        unit_id: &str,
        phase: &str,
        cost: f64,
        last_seq: u64,
        terminal_reason: Option<&str>,
        now: i64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE units SET phase=?2, cost=?3, last_seq=?4, terminal_reason=?5, updated_ts=?6
             WHERE unit_id=?1",
            params![unit_id, phase, cost, last_seq, terminal_reason, now],
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

    /// Rolling-window global spend (for the admission cost cap).
    pub fn spend_since(&self, since_ts: i64) -> rusqlite::Result<f64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(cost),0) FROM units WHERE created_ts>=?1",
            params![since_ts],
            |r| r.get(0),
        )
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
            terminal_reason: r.get(14)?,
            mode: r.get(15)?,
            min_review_rounds: r.get::<_, i64>(16)? as u32,
            swarm_id: r.get(17)?,
        })
    }
}

const SELECT_COLS_ALL: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason,mode,min_review_rounds,swarm_id FROM units";
const SELECT_COLS_WHERE_ID: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason,mode,min_review_rounds,swarm_id FROM units WHERE unit_id=?1";

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
        assert!((s.spend_since(0).unwrap() - 0.5).abs() < 1e-9);
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
    fn append_is_idempotent_on_seq() {
        let s = Store::open_memory().unwrap();
        s.upsert_unit(&row("u1"), 1).unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap();
        s.append_event("u1", 1, 1, "{}").unwrap(); // OR IGNORE → no duplicate
        assert_eq!(s.events_since("u1", 0).unwrap().len(), 1);
    }

    #[test]
    fn spend_window_excludes_old_units() {
        let s = Store::open_memory().unwrap();
        let mut old = row("old");
        old.cost = 9.0;
        s.upsert_unit(&old, 100).unwrap(); // created_ts=100
        let mut new = row("new");
        new.cost = 2.0;
        s.upsert_unit(&new, 1000).unwrap(); // created_ts=1000
        assert!((s.spend_since(500).unwrap() - 2.0).abs() < 1e-9, "only the new unit counts");
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
        let mut over = row("over"); over.phase = "needs_human".into(); over.cost = 9.0; over.usd_cap = 5.0;
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
        assert_eq!(s.committed_spend(500).unwrap(), 0.0, "created before the window is excluded");
    }
}
