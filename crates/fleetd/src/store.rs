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
               created_ts INTEGER, updated_ts INTEGER, terminal_reason TEXT);
             CREATE TABLE IF NOT EXISTS events(
               unit_id TEXT, seq INTEGER, ts INTEGER, json TEXT,
               PRIMARY KEY(unit_id, seq));",
        )?;
        Ok(Self { conn })
    }

    /// Insert a new unit or update the mutable projection columns of an existing one.
    /// `created_ts` is set only on first insert (ON CONFLICT keeps the original).
    pub fn upsert_unit(&self, r: &UnitRow, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO units(unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,
               usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,created_ts,updated_ts,terminal_reason)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15,?16)
             ON CONFLICT(unit_id) DO UPDATE SET
               phase=?11, cost=?12, last_seq=?13, oracle_frozen=?14, updated_ts=?15, terminal_reason=?16",
            params![
                r.unit_id, r.tier, r.task, r.repo_url, r.repo_slug, r.base_branch, r.branch,
                r.test_cmd, r.usd_cap, r.wall_clock_secs, r.phase, r.cost, r.last_seq,
                r.oracle_frozen as i64, now, r.terminal_reason
            ],
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

    /// Rolling-window global spend (for the admission cost cap).
    pub fn spend_since(&self, since_ts: i64) -> rusqlite::Result<f64> {
        self.conn.query_row(
            "SELECT COALESCE(SUM(cost),0) FROM units WHERE created_ts>=?1",
            params![since_ts],
            |r| r.get(0),
        )
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
        })
    }
}

const SELECT_COLS_ALL: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason FROM units";
const SELECT_COLS_WHERE_ID: &str = "SELECT unit_id,tier,task,repo_url,repo_slug,base_branch,branch,test_cmd,usd_cap,wall_clock_secs,phase,cost,last_seq,oracle_frozen,terminal_reason FROM units WHERE unit_id=?1";

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
}
