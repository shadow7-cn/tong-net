use crate::config::ServerConfig;
use crate::crypto::decrypt;
use crate::db::Database;
use crate::easytier::EasyTierSupervisor;
use crate::models::{NetworkSecret, SiteSnapshot};
use rusqlite::OptionalExtension;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: ServerConfig,
    pub db: Arc<Database>,
    pub master_key: [u8; 32],
    pub easytier: EasyTierSupervisor,
}

impl AppState {
    pub fn snapshot(&self) -> Result<SiteSnapshot, String> {
        self.db.read(|connection| {
            let settings = connection
                .query_row(
                    r#"
                    SELECT initialized, site_name, public_host, mode,
                           shared_name_ciphertext, shared_secret_ciphertext,
                           shared_private_key_ciphertext, shared_public_key
                    FROM site_settings WHERE id = 1
                    "#,
                    [],
                    |row| {
                        Ok((
                            row.get::<_, bool>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| error.to_string())?
                .unwrap_or_default();

            let mut statement = connection
                .prepare(
                    r#"
                    SELECT id, name, internal_name_ciphertext, internal_secret_ciphertext,
                           private_key_ciphertext, status, slot
                    FROM private_networks ORDER BY slot
                    "#,
                )
                .map_err(|error| error.to_string())?;
            let networks = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(
                    |(id, name, internal_name, internal_secret, private_key, status, slot)| {
                        Ok(NetworkSecret {
                            id,
                            name,
                            internal_name: decrypt(&self.master_key, &internal_name)?,
                            internal_secret: decrypt(&self.master_key, &internal_secret)?,
                            private_key: decrypt(&self.master_key, &private_key)?,
                            status,
                            slot,
                        })
                    },
                )
                .collect::<Result<Vec<_>, String>>()?;

            let initialized = settings.0;
            Ok(SiteSnapshot {
                initialized,
                site_name: settings.1,
                public_host: settings.2,
                mode: settings.3,
                shared_name: if initialized {
                    decrypt(&self.master_key, &settings.4)?
                } else {
                    String::new()
                },
                shared_secret: if initialized {
                    decrypt(&self.master_key, &settings.5)?
                } else {
                    String::new()
                },
                shared_private_key: if initialized {
                    decrypt(&self.master_key, &settings.6)?
                } else {
                    String::new()
                },
                shared_public_key: settings.7,
                networks,
            })
        })
    }

    pub async fn apply_runtime(&self) -> Result<(), String> {
        self.easytier.apply(&self.snapshot()?).await
    }
}
