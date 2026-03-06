use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub id: i64,
    pub command: String,
    pub args: String,
    pub cwd: String,
    pub timestamp: String,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub id: i64,
    pub command_id: i64,
    pub path: String,
    pub event_type: String,
    pub timestamp: String,
}

impl Database {
    pub fn db_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".local/share/prv/prv.db")
    }

    pub fn open() -> rusqlite::Result<Self> {
        let path = Self::db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS commands (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
                cwd TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                duration_ms INTEGER,
                exit_code INTEGER
            );
            CREATE TABLE IF NOT EXISTS file_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_id INTEGER NOT NULL REFERENCES commands(id),
                path TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_file_events_path ON file_events(path);
            CREATE INDEX IF NOT EXISTS idx_file_events_cmd ON file_events(command_id);",
        )
    }

    pub fn insert_command(
        &self,
        command: &str,
        args: &[String],
        cwd: &str,
        timestamp: &str,
        duration_ms: Option<i64>,
        exit_code: Option<i32>,
    ) -> rusqlite::Result<i64> {
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
        self.conn.execute(
            "INSERT INTO commands (command, args, cwd, timestamp, duration_ms, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![command, args_json, cwd, timestamp, duration_ms, exit_code],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_file_event(
        &self,
        command_id: i64,
        path: &str,
        event_type: &str,
        timestamp: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO file_events (command_id, path, event_type, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![command_id, path, event_type, timestamp],
        )?;
        Ok(())
    }

    pub fn log_for_file(&self, path: &str) -> rusqlite::Result<Vec<(CommandRecord, FileEvent)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.command, c.args, c.cwd, c.timestamp, c.duration_ms, c.exit_code,
                    fe.id, fe.command_id, fe.path, fe.event_type, fe.timestamp
             FROM file_events fe
             JOIN commands c ON c.id = fe.command_id
             WHERE fe.path = ?1
             ORDER BY fe.timestamp DESC",
        )?;
        let rows = stmt
            .query_map(params![path], |row| {
                Ok((
                    CommandRecord {
                        id: row.get(0)?,
                        command: row.get(1)?,
                        args: row.get(2)?,
                        cwd: row.get(3)?,
                        timestamp: row.get(4)?,
                        duration_ms: row.get(5)?,
                        exit_code: row.get(6)?,
                    },
                    FileEvent {
                        id: row.get(7)?,
                        command_id: row.get(8)?,
                        path: row.get(9)?,
                        event_type: row.get(10)?,
                        timestamp: row.get(11)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Files that this file depends on (inputs to commands that wrote this file).
    pub fn deps_for_file(&self, path: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT fe2.path
             FROM file_events fe
             JOIN file_events fe2 ON fe2.command_id = fe.command_id
             WHERE fe.path = ?1 AND fe.event_type IN ('write', 'create')
               AND fe2.event_type = 'read' AND fe2.path != ?1",
        )?;
        let rows = stmt
            .query_map(params![path], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Files that depend on this file (outputs of commands that read this file).
    pub fn rdeps_for_file(&self, path: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT fe2.path
             FROM file_events fe
             JOIN file_events fe2 ON fe2.command_id = fe.command_id
             WHERE fe.path = ?1 AND fe.event_type = 'read'
               AND fe2.event_type IN ('write', 'create') AND fe2.path != ?1",
        )?;
        let rows = stmt
            .query_map(params![path], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn gc_older_than(&self, duration: std::time::Duration) -> rusqlite::Result<usize> {
        let cutoff: DateTime<Utc> =
            Utc::now() - Duration::from_std(duration).unwrap_or(Duration::zero());
        let cutoff_str = cutoff.to_rfc3339();
        let deleted = self.conn.execute(
            "DELETE FROM file_events WHERE command_id IN
             (SELECT id FROM commands WHERE timestamp < ?1)",
            params![cutoff_str],
        )?;
        self.conn.execute(
            "DELETE FROM commands WHERE timestamp < ?1",
            params![cutoff_str],
        )?;
        Ok(deleted)
    }

    pub fn search_commands(&self, pattern: &str) -> rusqlite::Result<Vec<CommandRecord>> {
        let like_pattern = format!("%{}%", pattern);
        let mut stmt = self.conn.prepare(
            "SELECT id, command, args, cwd, timestamp, duration_ms, exit_code
             FROM commands
             WHERE command LIKE ?1 OR args LIKE ?1
             ORDER BY timestamp DESC",
        )?;
        let rows = stmt
            .query_map(params![like_pattern], |row| {
                Ok(CommandRecord {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    args: row.get(2)?,
                    cwd: row.get(3)?,
                    timestamp: row.get(4)?,
                    duration_ms: row.get(5)?,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get all file events for building a graph.
    pub fn all_events_for_file(&self, path: &str) -> rusqlite::Result<Vec<(CommandRecord, Vec<FileEvent>)>> {
        // Get all commands that touched this file
        let mut cmd_stmt = self.conn.prepare(
            "SELECT DISTINCT c.id, c.command, c.args, c.cwd, c.timestamp, c.duration_ms, c.exit_code
             FROM commands c
             JOIN file_events fe ON fe.command_id = c.id
             WHERE fe.path = ?1
             ORDER BY c.timestamp",
        )?;
        let cmds: Vec<CommandRecord> = cmd_stmt
            .query_map(params![path], |row| {
                Ok(CommandRecord {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    args: row.get(2)?,
                    cwd: row.get(3)?,
                    timestamp: row.get(4)?,
                    duration_ms: row.get(5)?,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for cmd in cmds {
            let mut ev_stmt = self.conn.prepare(
                "SELECT id, command_id, path, event_type, timestamp
                 FROM file_events WHERE command_id = ?1",
            )?;
            let events: Vec<FileEvent> = ev_stmt
                .query_map(params![cmd.id], |row| {
                    Ok(FileEvent {
                        id: row.get(0)?,
                        command_id: row.get(1)?,
                        path: row.get(2)?,
                        event_type: row.get(3)?,
                        timestamp: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            result.push((cmd, events));
        }
        Ok(result)
    }

    /// Get all commands that produced a given file (wrote/created it),
    /// along with their read dependencies.
    pub fn producers_for_file(&self, path: &str) -> rusqlite::Result<Vec<(CommandRecord, Vec<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT c.id, c.command, c.args, c.cwd, c.timestamp, c.duration_ms, c.exit_code
             FROM commands c
             JOIN file_events fe ON fe.command_id = c.id
             WHERE fe.path = ?1 AND fe.event_type IN ('write', 'create')
             ORDER BY c.timestamp",
        )?;
        let cmds: Vec<CommandRecord> = stmt
            .query_map(params![path], |row| {
                Ok(CommandRecord {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    args: row.get(2)?,
                    cwd: row.get(3)?,
                    timestamp: row.get(4)?,
                    duration_ms: row.get(5)?,
                    exit_code: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for cmd in cmds {
            let mut dep_stmt = self.conn.prepare(
                "SELECT DISTINCT path FROM file_events
                 WHERE command_id = ?1 AND event_type = 'read'",
            )?;
            let deps: Vec<String> = dep_stmt
                .query_map(params![cmd.id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            result.push((cmd, deps));
        }
        Ok(result)
    }
}
