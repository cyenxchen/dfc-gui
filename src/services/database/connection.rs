//! Database Connection
//!
//! In-memory SQLite connection management using sqlez.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use sqlez::connection::Connection;

/// Database connection wrapper
pub struct DatabaseConnection {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseConnection {
    /// Create a new in-memory database connection
    pub fn new_in_memory() -> Self {
        let conn = Connection::open_memory(Some("dfc_gui"));
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Initialize database schema
    pub fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();

        // Properties table
        conn.exec(
            r#"
            CREATE TABLE IF NOT EXISTS properties (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                name TEXT NOT NULL,
                topic TEXT NOT NULL,
                mms TEXT,
                hmi TEXT,
                value TEXT,
                prev_value TEXT,
                quality INTEGER DEFAULT 0,
                data_time TEXT NOT NULL,
                created_time TEXT NOT NULL,
                source TEXT
            )
            "#,
        )?()?;

        // Events table
        conn.exec(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                event_code TEXT NOT NULL,
                description TEXT,
                level INTEGER DEFAULT 0,
                state INTEGER DEFAULT 0,
                event_time TEXT NOT NULL,
                created_time TEXT NOT NULL,
                source TEXT
            )
            "#,
        )?()?;

        // Create indexes
        conn.exec(
            "CREATE INDEX IF NOT EXISTS idx_properties_device ON properties(device_id)",
        )?()?;
        conn.exec(
            "CREATE INDEX IF NOT EXISTS idx_events_device ON events(device_id)",
        )?()?;

        tracing::info!("Database schema initialized");
        Ok(())
    }

    /// Get a reference to the connection
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    /// Execute a query using spawn_blocking to avoid blocking the UI
    pub async fn execute_blocking<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            f(&conn)
        })
        .await?
    }
}

impl Clone for DatabaseConnection {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}
