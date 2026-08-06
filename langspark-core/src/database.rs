//! Database module
//!
//! Handles SQLite database operations for LangSpark.
//! All tables include a language field for multi-language support.

use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::Mutex;

/// Database connection wrapper.
///
/// The connection is behind a `Mutex` (rather than a bare `Connection`, which
/// is `Send` but not `Sync`) so `Arc<Database>` — and therefore every
/// repository built on it — can be shared across the background thread pool
/// used for blocking DB work (see `langspark-gui`'s `task::run_blocking`).
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create a database at the specified path
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Lock and get the underlying connection. Returns a guard that derefs
    /// to `Connection`, so existing call sites (`db.conn().prepare(...)`) are
    /// unaffected by the lock.
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database connection mutex poisoned")
    }
}

/// Repository trait for database operations
pub trait Repository: Send + Sync {
    /// Get all items
    fn get_all(&self) -> Result<Vec<serde_json::Value>>;
    /// Get by ID
    fn get_by_id(&self, id: i64) -> Result<Option<serde_json::Value>>;
    /// Create a new item
    fn create(&self, item: &serde_json::Value) -> Result<i64>;
    /// Update an existing item
    fn update(&self, id: i64, item: &serde_json::Value) -> Result<()>;
    /// Delete an item
    fn delete(&self, id: i64) -> Result<()>;
}

/// Initialize database schema
pub fn initialize_schema(conn: &Connection) -> Result<()> {
    // Vocabulary table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS vocabulary (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL,
            reading TEXT,
            meaning TEXT NOT NULL,
            language TEXT NOT NULL,
            level TEXT,
            part_of_speech TEXT,
            tags TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    
    // Kanji table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kanji (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            character TEXT NOT NULL,
            on_readings TEXT,
            kun_readings TEXT,
            meanings TEXT NOT NULL,
            stroke_count INTEGER,
            radical TEXT,
            jlpt_level INTEGER,
            grade INTEGER,
            language TEXT NOT NULL DEFAULT 'ja',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    
    // SRS cards table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS srs_cards (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            vocab_id INTEGER,
            kanji_id INTEGER,
            card_type TEXT NOT NULL CHECK(card_type IN ('vocabulary', 'kanji')),
            state TEXT NOT NULL CHECK(state IN ('new', 'learning', 'review')),
            repetitions INTEGER NOT NULL DEFAULT 0,
            ease_factor REAL NOT NULL DEFAULT 2.5,
            interval_days INTEGER NOT NULL DEFAULT 1,
            next_review_date TIMESTAMP,
            last_reviewed TIMESTAMP,
            language TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (vocab_id) REFERENCES vocabulary(id),
            FOREIGN KEY (kanji_id) REFERENCES kanji(id)
        )",
        [],
    )?;
    
    // Decks table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS decks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            language TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    
    // Deck cards (many-to-many between decks and srs_cards)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS deck_cards (
            deck_id INTEGER NOT NULL,
            card_id INTEGER NOT NULL,
            PRIMARY KEY (deck_id, card_id),
            FOREIGN KEY (deck_id) REFERENCES decks(id),
            FOREIGN KEY (card_id) REFERENCES srs_cards(id)
        )",
        [],
    )?;
    
    // Review history table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS review_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            rating INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 4),
            review_duration_seconds INTEGER,
            reviewed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            language TEXT NOT NULL,
            FOREIGN KEY (card_id) REFERENCES srs_cards(id)
        )",
        [],
    )?;
    
    // Settings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL UNIQUE,
            value TEXT,
            language TEXT
        )",
        [],
    )?;
    
    // Statistics table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS statistics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            cards_reviewed INTEGER DEFAULT 0,
            new_words_added INTEGER DEFAULT 0,
            daily_streak INTEGER DEFAULT 0,
            retention_rate REAL DEFAULT 0.0,
            session_duration_seconds INTEGER DEFAULT 0,
            language TEXT NOT NULL,
            UNIQUE(date, language)
        )",
        [],
    )?;
    
    // Language installation table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS languages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            is_installed BOOLEAN NOT NULL DEFAULT FALSE,
            version TEXT,
            installed_at TIMESTAMP
        )",
        [],
    )?;
    
    Ok(())
}

/// Database migration system
pub struct Migration {
    version: u32,
    description: String,
    sql: String,
}

impl Migration {
    pub fn new(version: u32, description: &str, sql: &str) -> Self {
        Self {
            version,
            description: description.to_string(),
            sql: sql.to_string(),
        }
    }

    pub fn apply(&self, conn: &Connection) -> Result<()> {
        conn.execute(&self.sql, [])?;
        Ok(())
    }
}

/// Ensure the table that tracks which migrations have run exists.
fn ensure_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    Ok(())
}

/// Apply every migration in `migrations` whose version hasn't been recorded yet,
/// in ascending version order. Each migration runs in its own transaction so a
/// failure partway through doesn't leave that migration half-applied, and
/// already-applied migrations are skipped on subsequent runs.
pub fn run_migrations(conn: &mut Connection, migrations: &[Migration]) -> Result<()> {
    ensure_migrations_table(conn)?;

    let mut sorted: Vec<&Migration> = migrations.iter().collect();
    sorted.sort_by_key(|m| m.version);

    for migration in sorted {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?)",
            [migration.version],
            |row| row.get(0),
        )?;
        if already_applied {
            continue;
        }

        let tx = conn.transaction()?;
        migration.apply(&tx)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, description) VALUES (?, ?)",
            rusqlite::params![migration.version, migration.description],
        )?;
        tx.commit()?;
    }

    Ok(())
}

/// The highest migration version that has been applied, if any.
pub fn current_schema_version(conn: &Connection) -> Result<Option<u32>> {
    ensure_migrations_table(conn)?;
    conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row.get(0))
}

/// Back up the database to `dest_path` using SQLite's atomic online backup API,
/// safe to run against a live connection.
pub fn backup_database(conn: &Connection, dest_path: &Path) -> Result<()> {
    let mut dest = Connection::open(dest_path)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut dest)?;
    backup.run_to_completion(5, std::time::Duration::from_millis(250), None)?;
    Ok(())
}

/// Restore a database by copying a previously-created backup file over `dest_path`.
/// The caller must ensure no connection is open on `dest_path` while this runs.
pub fn restore_database(backup_path: &Path, dest_path: &Path) -> Result<()> {
    std::fs::copy(backup_path, dest_path).map_err(|e| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
            Some(format!("failed to restore database: {e}")),
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_database_open() {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::open(temp.path()).unwrap();
        assert!(db.conn().is_autocommit());
    }
    
    #[test]
    fn test_initialize_schema() {
        let temp = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp.path()).unwrap();
        initialize_schema(&conn).unwrap();
        
        // Verify tables were created
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        
        assert!(tables.contains(&"vocabulary".to_string()));
        assert!(tables.contains(&"kanji".to_string()));
        assert!(tables.contains(&"srs_cards".to_string()));
        assert!(tables.contains(&"decks".to_string()));
        assert!(tables.contains(&"review_history".to_string()));
    }

    #[test]
    fn test_run_migrations_applies_once() {
        let temp = NamedTempFile::new().unwrap();
        let mut conn = Connection::open(temp.path()).unwrap();
        conn.execute("CREATE TABLE widgets (id INTEGER PRIMARY KEY)", []).unwrap();

        let migrations = vec![Migration::new(1, "add name column", "ALTER TABLE widgets ADD COLUMN name TEXT")];

        run_migrations(&mut conn, &migrations).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), Some(1));

        // Running again must not re-apply (would error: duplicate column name)
        run_migrations(&mut conn, &migrations).unwrap();
        assert_eq!(current_schema_version(&conn).unwrap(), Some(1));
    }

    #[test]
    fn test_backup_and_restore_database() {
        let source_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(source_file.path()).unwrap();
        initialize_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO vocabulary (word, meaning, language) VALUES ('受け取る', 'to receive', 'ja')",
            [],
        )
        .unwrap();

        let backup_file = NamedTempFile::new().unwrap();
        backup_database(&conn, backup_file.path()).unwrap();
        drop(conn);

        let restored_file = NamedTempFile::new().unwrap();
        restore_database(backup_file.path(), restored_file.path()).unwrap();

        let restored_conn = Connection::open(restored_file.path()).unwrap();
        let word: String = restored_conn
            .query_row("SELECT word FROM vocabulary LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(word, "受け取る");
    }
}
