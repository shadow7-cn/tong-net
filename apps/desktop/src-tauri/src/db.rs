use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub browser_source: String,
    pub removed: bool,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub from_device_id: String,
    pub to_device_id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub content: String,
    pub file: Option<FileRecord>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRecord {
    pub id: String,
    pub kind: String,
    pub file_name: String,
    pub peer_name: String,
    pub progress: u8,
    pub status: String,
    pub created_at: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub finished_at: Option<String>,
}

pub struct Database(pub Mutex<Connection>);

fn conversation_id(a: &str, b: &str) -> String {
    if a < b {
        format!("{a}:{b}")
    } else {
        format!("{b}:{a}")
    }
}

impl Database {
    pub fn open(path: &Path, host_name: &str) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS devices (
               id TEXT PRIMARY KEY, client_id TEXT UNIQUE, name TEXT NOT NULL, kind TEXT NOT NULL,
               last_seen_at TEXT NOT NULL, created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
               id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, from_device_id TEXT NOT NULL,
               to_device_id TEXT NOT NULL, type TEXT NOT NULL, content TEXT NOT NULL,
               file_id TEXT, created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS files (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, stored_name TEXT NOT NULL, size INTEGER NOT NULL,
               status TEXT NOT NULL, created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS transfers (
               id TEXT PRIMARY KEY, kind TEXT NOT NULL, file_name TEXT NOT NULL, peer_name TEXT NOT NULL,
               progress INTEGER NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL
             );"
        ).map_err(|error| error.to_string())?;
        let _ = connection.execute(
            "ALTER TABLE devices ADD COLUMN browser_source TEXT NOT NULL DEFAULT '其他浏览器'",
            [],
        );
        let _ = connection.execute("ALTER TABLE devices ADD COLUMN removed_at TEXT", []);
        let _ = connection.execute(
            "ALTER TABLE transfers ADD COLUMN total_bytes INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = connection.execute(
            "ALTER TABLE transfers ADD COLUMN transferred_bytes INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = connection.execute("ALTER TABLE transfers ADD COLUMN finished_at TEXT", []);
        let now = Utc::now().to_rfc3339();
        connection
            .execute(
                "UPDATE transfers SET status='failed', finished_at=?1 WHERE status='running'",
                [&now],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO devices(id, client_id, name, kind, last_seen_at, created_at, browser_source, removed_at)
             VALUES('host', NULL, ?1, 'host', ?2, ?2, '桌面应用', NULL)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, browser_source='桌面应用', removed_at=NULL, last_seen_at=excluded.last_seen_at",
                params![host_name, now],
            )
            .map_err(|error| error.to_string())?;
        Ok(Self(Mutex::new(connection)))
    }

    pub fn register_device(
        &self,
        client_id: &str,
        name: &str,
        browser_source: &str,
    ) -> Result<(Device, bool), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let now = Utc::now().to_rfc3339();
        let existing: Option<String> = db
            .query_row(
                "SELECT id FROM devices WHERE client_id=?1",
                [client_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let is_new = existing.is_none();
        let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        db.execute(
            "INSERT INTO devices(id, client_id, name, kind, last_seen_at, created_at, browser_source, removed_at)
             VALUES(?1, ?2, ?3, 'browser', ?4, ?4, ?5, NULL)
             ON CONFLICT(client_id) DO UPDATE SET name=excluded.name, browser_source=excluded.browser_source, removed_at=NULL, last_seen_at=excluded.last_seen_at",
            params![id, client_id, name, now, browser_source],
        ).map_err(|error| error.to_string())?;
        Ok((
            Device {
                id,
                name: name.into(),
                kind: "browser".into(),
                status: "online".into(),
                browser_source: browser_source.into(),
                removed: false,
                last_seen_at: now,
            },
            is_new,
        ))
    }

    pub fn touch_device(&self, id: &str) -> Result<(), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute(
            "UPDATE devices SET last_seen_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn device_exists(&self, id: &str) -> Result<bool, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.query_row(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE id=?1 AND removed_at IS NULL)",
            [id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
    }

    pub fn rename_device(&self, id: &str, name: &str) -> Result<Device, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let now = Utc::now().to_rfc3339();
        db.execute(
            "UPDATE devices SET name=?1, last_seen_at=?2 WHERE id=?3",
            params![name, now, id],
        )
        .map_err(|error| error.to_string())?;
        let (kind, browser_source): (String, String) = db
            .query_row(
                "SELECT kind, browser_source FROM devices WHERE id=?1 AND removed_at IS NULL",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| error.to_string())?;
        Ok(Device {
            id: id.into(),
            name: name.into(),
            kind,
            status: "online".into(),
            browser_source,
            removed: false,
            last_seen_at: now,
        })
    }

    pub fn list_devices(&self, online_ids: &[String]) -> Result<Vec<Device>, String> {
        self.query_devices(online_ids, false)
    }

    pub fn list_all_devices(&self, online_ids: &[String]) -> Result<Vec<Device>, String> {
        self.query_devices(online_ids, true)
    }

    fn query_devices(
        &self,
        online_ids: &[String],
        include_removed: bool,
    ) -> Result<Vec<Device>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let sql = if include_removed {
            "SELECT id, name, kind, last_seen_at, browser_source, removed_at IS NOT NULL FROM devices ORDER BY kind, last_seen_at DESC"
        } else {
            "SELECT id, name, kind, last_seen_at, browser_source, 0 FROM devices WHERE removed_at IS NULL ORDER BY kind, last_seen_at DESC"
        };
        let mut stmt = db.prepare(sql).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let (id, name, kind, last_seen_at, browser_source, removed) =
                row.map_err(|error| error.to_string())?;
            let status = if !removed && (id == "host" || online_ids.contains(&id)) {
                "online"
            } else {
                "offline"
            };
            Ok(Device {
                id,
                name,
                kind,
                status: status.into(),
                browser_source,
                removed,
                last_seen_at,
            })
        })
        .collect()
    }

    pub fn remove_device(&self, id: &str) -> Result<bool, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let changed = db.execute(
            "UPDATE devices SET removed_at=?1 WHERE id=?2 AND kind='browser' AND removed_at IS NULL",
            params![Utc::now().to_rfc3339(), id],
        ).map_err(|error| error.to_string())?;
        Ok(changed > 0)
    }

    pub fn add_text_message(
        &self,
        from: &str,
        to: &str,
        content: &str,
    ) -> Result<MessageRecord, String> {
        self.add_message(from, to, "text", content, None)
    }

    pub fn add_system_message(
        &self,
        from: &str,
        to: &str,
        content: &str,
    ) -> Result<MessageRecord, String> {
        self.add_message(from, to, "system", content, None)
    }

    pub fn add_file_message(
        &self,
        from: &str,
        to: &str,
        file: FileRecord,
    ) -> Result<MessageRecord, String> {
        self.add_message(from, to, "file", "发送了文件", Some(file))
    }

    fn add_message(
        &self,
        from: &str,
        to: &str,
        kind: &str,
        content: &str,
        file: Option<FileRecord>,
    ) -> Result<MessageRecord, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let record = MessageRecord {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id(from, to),
            from_device_id: from.into(),
            to_device_id: to.into(),
            message_type: kind.into(),
            content: content.into(),
            file,
            created_at: Utc::now().to_rfc3339(),
        };
        db.execute(
            "INSERT INTO messages(id, conversation_id, from_device_id, to_device_id, type, content, file_id, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![record.id, record.conversation_id, from, to, kind, content, record.file.as_ref().map(|f| &f.id), record.created_at],
        ).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn list_messages(&self, me: &str, peer: &str) -> Result<Vec<MessageRecord>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let mut stmt = db.prepare(
            "SELECT m.id,m.conversation_id,m.from_device_id,m.to_device_id,m.type,m.content,m.created_at,
                    f.id,f.name,f.size,f.status,f.created_at
             FROM messages m LEFT JOIN files f ON f.id=m.file_id
             WHERE m.conversation_id=?1 ORDER BY m.created_at"
        ).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([conversation_id(me, peer)], |row| {
                let file_id: Option<String> = row.get(7)?;
                Ok(MessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    from_device_id: row.get(2)?,
                    to_device_id: row.get(3)?,
                    message_type: row.get(4)?,
                    content: row.get(5)?,
                    created_at: row.get(6)?,
                    file: file_id.map(|id| FileRecord {
                        id,
                        name: row.get(8).unwrap_or_default(),
                        size: row.get::<_, i64>(9).unwrap_or_default() as u64,
                        status: row.get(10).unwrap_or_default(),
                        created_at: row.get(11).unwrap_or_default(),
                    }),
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn add_file(&self, name: &str, stored_name: &str, size: u64) -> Result<FileRecord, String> {
        let record = FileRecord {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            size,
            status: "available".into(),
            created_at: Utc::now().to_rfc3339(),
        };
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute("INSERT INTO files(id,name,stored_name,size,status,created_at) VALUES(?1,?2,?3,?4,'available',?5)",
            params![record.id, record.name, stored_name, size as i64, record.created_at]).map_err(|error| error.to_string())?;
        Ok(record)
    }

    pub fn file_path_info(&self, id: &str) -> Result<Option<(String, String, u64)>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.query_row(
            "SELECT name,stored_name,size FROM files WHERE id=?1 AND status='available'",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64)),
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    pub fn add_transfer(
        &self,
        kind: &str,
        file_name: &str,
        peer_name: &str,
        status: &str,
    ) -> Result<(), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute("INSERT INTO transfers(id,kind,file_name,peer_name,progress,status,created_at) VALUES(?1,?2,?3,?4,100,?5,?6)",
            params![Uuid::new_v4().to_string(), kind, file_name, peer_name, status, Utc::now().to_rfc3339()]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn start_transfer(
        &self,
        id: &str,
        kind: &str,
        file_name: &str,
        peer_name: &str,
        total_bytes: u64,
    ) -> Result<(), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute(
            "INSERT INTO transfers(id,kind,file_name,peer_name,progress,status,created_at,total_bytes,transferred_bytes,finished_at)
             VALUES(?1,?2,?3,?4,0,'running',?5,?6,0,NULL)
             ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,file_name=excluded.file_name,peer_name=excluded.peer_name,
             total_bytes=excluded.total_bytes WHERE transfers.status='running'",
            params![id, kind, file_name, peer_name, Utc::now().to_rfc3339(), total_bytes as i64],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn update_transfer_progress(&self, id: &str, transferred: u64) -> Result<(), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute(
            "UPDATE transfers SET transferred_bytes=?2,
             progress=CASE WHEN total_bytes > 0 THEN MIN(99, CAST(?2 * 100 / total_bytes AS INTEGER)) ELSE 0 END
             WHERE id=?1 AND status='running'",
            params![id, transferred as i64],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn finish_transfer(&self, id: &str, status: &str, transferred: u64) -> Result<(), String> {
        let progress = if status == "success" { 100 } else { 0 };
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute(
            "UPDATE transfers SET status=?2,progress=?3,transferred_bytes=?4,finished_at=?5 WHERE id=?1 AND status='running'",
            params![id, status, progress, transferred as i64, Utc::now().to_rfc3339()],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn cancel_transfer(&self, id: &str) -> Result<(), String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.execute(
            "UPDATE transfers SET status='canceled',finished_at=?2 WHERE id=?1 AND status!='success'",
            params![id, Utc::now().to_rfc3339()],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_transfers(&self) -> Result<Vec<TransferRecord>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let mut stmt = db.prepare("SELECT id,kind,file_name,peer_name,progress,status,created_at,total_bytes,transferred_bytes,finished_at FROM transfers ORDER BY created_at DESC")
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TransferRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    file_name: row.get(2)?,
                    peer_name: row.get(3)?,
                    progress: row.get::<_, i64>(4)? as u8,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    total_bytes: row.get::<_, i64>(7)? as u64,
                    transferred_bytes: row.get::<_, i64>(8)? as u64,
                    finished_at: row.get(9)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn list_all_messages(&self) -> Result<Vec<MessageRecord>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let mut stmt = db.prepare(
            "SELECT m.id,m.conversation_id,m.from_device_id,m.to_device_id,m.type,m.content,m.created_at,
                    f.id,f.name,f.size,f.status,f.created_at
             FROM messages m LEFT JOIN files f ON f.id=m.file_id ORDER BY m.created_at DESC"
        ).map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let file_id: Option<String> = row.get(7)?;
                Ok(MessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    from_device_id: row.get(2)?,
                    to_device_id: row.get(3)?,
                    message_type: row.get(4)?,
                    content: row.get(5)?,
                    created_at: row.get(6)?,
                    file: file_id.map(|id| FileRecord {
                        id,
                        name: row.get(8).unwrap_or_default(),
                        size: row.get::<_, i64>(9).unwrap_or_default() as u64,
                        status: row.get(10).unwrap_or_default(),
                        created_at: row.get(11).unwrap_or_default(),
                    }),
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn list_files(&self) -> Result<Vec<FileRecord>, String> {
        let db = self.0.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let mut stmt = db
            .prepare("SELECT id,name,size,status,created_at FROM files ORDER BY created_at DESC")
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Database;

    #[test]
    fn canceled_transfer_cannot_be_restarted_by_late_upload_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let db = Database::open(&temp.path().join("test.sqlite3"), "测试主机").unwrap();
        db.start_transfer("transfer-1", "upload", "初始名称", "本机", 100)
            .unwrap();
        db.cancel_transfer("transfer-1").unwrap();
        db.start_transfer("transfer-1", "upload", "文件.mp4", "本机", 100)
            .unwrap();

        let record = db
            .list_transfers()
            .unwrap()
            .into_iter()
            .find(|item| item.id == "transfer-1")
            .unwrap();
        assert_eq!(record.status, "canceled");
    }

    #[test]
    fn unfinished_transfers_are_closed_when_service_restarts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.sqlite3");
        {
            let db = Database::open(&path, "测试主机").unwrap();
            db.start_transfer("transfer-2", "upload", "文件.mp4", "本机", 100)
                .unwrap();
        }

        let reopened = Database::open(&path, "测试主机").unwrap();
        let record = reopened
            .list_transfers()
            .unwrap()
            .into_iter()
            .find(|item| item.id == "transfer-2")
            .unwrap();
        assert_eq!(record.status, "failed");
    }
}
