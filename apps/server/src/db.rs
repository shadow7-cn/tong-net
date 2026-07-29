use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )
        .map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;
                PRAGMA busy_timeout = 5000;

                CREATE TABLE IF NOT EXISTS site_settings (
                  id INTEGER PRIMARY KEY CHECK (id = 1),
                  initialized INTEGER NOT NULL DEFAULT 0,
                  site_name TEXT NOT NULL DEFAULT '',
                  public_host TEXT NOT NULL DEFAULT '',
                  mode TEXT NOT NULL DEFAULT 'private',
                  shared_name_ciphertext TEXT NOT NULL DEFAULT '',
                  shared_secret_ciphertext TEXT NOT NULL DEFAULT '',
                  shared_private_key_ciphertext TEXT NOT NULL DEFAULT '',
                  shared_public_key TEXT NOT NULL DEFAULT '',
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS admin_users (
                  id TEXT PRIMARY KEY,
                  username TEXT NOT NULL UNIQUE,
                  password_hash TEXT NOT NULL,
                  session_generation INTEGER NOT NULL DEFAULT 1,
                  last_login_at TEXT,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS admin_sessions (
                  id TEXT PRIMARY KEY,
                  token_hash TEXT NOT NULL UNIQUE,
                  generation INTEGER NOT NULL,
                  expires_at TEXT NOT NULL,
                  created_at TEXT NOT NULL,
                  last_used_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS admin_login_failures (
                  id TEXT PRIMARY KEY,
                  client_key TEXT NOT NULL,
                  created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS private_networks (
                  id TEXT PRIMARY KEY,
                  name TEXT NOT NULL,
                  name_normalized TEXT NOT NULL UNIQUE,
                  password_hash TEXT NOT NULL,
                  internal_name_ciphertext TEXT NOT NULL,
                  internal_secret_ciphertext TEXT NOT NULL,
                  private_key_ciphertext TEXT NOT NULL,
                  status TEXT NOT NULL DEFAULT 'active',
                  slot INTEGER NOT NULL UNIQUE CHECK(slot >= 0 AND slot < 10),
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS devices (
                  id TEXT PRIMARY KEY,
                  client_device_id TEXT NOT NULL UNIQUE,
                  name TEXT NOT NULL,
                  platform TEXT NOT NULL,
                  client_version TEXT NOT NULL,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS network_memberships (
                  id TEXT PRIMARY KEY,
                  network_id TEXT NOT NULL REFERENCES private_networks(id) ON DELETE CASCADE,
                  device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
                  admin_note TEXT NOT NULL DEFAULT '',
                  status TEXT NOT NULL DEFAULT 'active',
                  credential_id TEXT,
                  credential_secret_ciphertext TEXT,
                  virtual_ip TEXT NOT NULL DEFAULT '',
                  protocol TEXT NOT NULL DEFAULT '',
                  latency_ms INTEGER,
                  rx_bytes INTEGER NOT NULL DEFAULT 0,
                  tx_bytes INTEGER NOT NULL DEFAULT 0,
                  last_seen_at TEXT,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL,
                  UNIQUE(network_id, device_id)
                );

                CREATE TABLE IF NOT EXISTS device_sessions (
                  id TEXT PRIMARY KEY,
                  membership_id TEXT NOT NULL REFERENCES network_memberships(id) ON DELETE CASCADE,
                  token_hash TEXT NOT NULL UNIQUE,
                  expires_at TEXT NOT NULL,
                  revoked_at TEXT,
                  created_at TEXT NOT NULL,
                  last_seen_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS audit_logs (
                  id TEXT PRIMARY KEY,
                  actor_type TEXT NOT NULL,
                  actor_id TEXT,
                  action TEXT NOT NULL,
                  target_type TEXT,
                  target_id TEXT,
                  result TEXT NOT NULL,
                  ip_address TEXT,
                  metadata_json TEXT NOT NULL DEFAULT '{}',
                  created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_membership_network ON network_memberships(network_id);
                CREATE INDEX IF NOT EXISTS idx_membership_seen ON network_memberships(last_seen_at);
                CREATE INDEX IF NOT EXISTS idx_device_session_token ON device_sessions(token_hash);
                CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_logs(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_login_failure_key ON admin_login_failures(client_key, created_at);
                "#,
            )
            .map_err(|error| error.to_string())?;
        let now = chrono::Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT OR IGNORE INTO site_settings (id, created_at, updated_at) VALUES (1, ?1, ?1)",
                [&now],
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn read<T>(
        &self,
        callback: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "数据库锁不可用".to_string())?;
        callback(&connection)
    }

    pub fn write<T>(
        &self,
        callback: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "数据库锁不可用".to_string())?;
        callback(&mut connection)
    }

    pub fn is_healthy(&self) -> bool {
        self.read(|connection| {
            connection
                .query_row("SELECT 1", [], |_| Ok(()))
                .map_err(|error| error.to_string())
        })
        .is_ok()
    }
}
