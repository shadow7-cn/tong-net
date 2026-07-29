use chrono::Utc;
use rusqlite::params;
use std::sync::Arc;
use tong_net_server::{
    app,
    config::ServerConfig,
    crypto::{hash_password, load_or_create_master_key, validate_password},
    db::Database,
    easytier::EasyTierSupervisor,
    state::AppState,
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    if let Err(error) = run().await {
        error!(%error, "服务端启动失败");
        eprintln!("同网互通服务端启动失败：{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = ServerConfig::from_env()?;
    config.ensure_directories()?;
    let master_key = load_or_create_master_key(&config.data_dir.join("keys/master.key"))?;
    let database = Arc::new(Database::open(&config.database_path())?);

    if std::env::args().skip(1).collect::<Vec<_>>() == ["admin", "reset-password"] {
        return reset_admin_password(&database);
    }

    let easytier = EasyTierSupervisor::new(config.clone());
    let state = AppState {
        config: config.clone(),
        db: database,
        master_key,
        easytier,
    };
    let snapshot = state.snapshot()?;
    if snapshot.initialized {
        state.apply_runtime().await?;
    }
    let (monitor_stop, monitor_rx) = tokio::sync::watch::channel(false);
    let monitor_state = state.clone();
    let monitor_task = tokio::spawn(async move {
        monitor_runtime(monitor_state, monitor_rx).await;
    });

    let address = format!("0.0.0.0:{}", config.web_port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|error| format!("监听 {address} 失败：{error}"))?;
    info!(%address, "同网互通服务端已启动");

    let shutdown_state = state.clone();
    let result = axum::serve(listener, app(state))
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            let _ = monitor_stop.send(true);
            shutdown_state.easytier.shutdown().await;
        })
        .await
        .map_err(|error| error.to_string());
    monitor_task.abort();
    result
}

async fn monitor_runtime(state: AppState, mut stop: tokio::sync::watch::Receiver<bool>) {
    let mut retry_delay = 1u64;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
                continue;
            }
        }
        let Ok(snapshot) = state.snapshot() else {
            continue;
        };
        if !snapshot.initialized || state.easytier.status().await.healthy {
            retry_delay = 1;
            continue;
        }
        tracing::warn!(retry_delay, "检测到 EasyTier 子进程异常，准备重启");
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(retry_delay)) => {},
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
                continue;
            }
        }
        match state.apply_runtime().await {
            Ok(()) => retry_delay = 1,
            Err(error) => {
                tracing::error!(%error, "自动重启 EasyTier 失败");
                retry_delay = (retry_delay * 2).min(30);
            }
        }
    }
}

fn reset_admin_password(database: &Database) -> Result<(), String> {
    let initialized = database.read(|connection| {
        connection
            .query_row(
                "SELECT initialized FROM site_settings WHERE id = 1",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())
    })?;
    if !initialized {
        return Err("服务尚未完成首次设置，无法重置管理员密码".into());
    }

    let password =
        rpassword::prompt_password("请输入新的管理员密码：").map_err(|error| error.to_string())?;
    validate_password(&password)?;
    let confirmation =
        rpassword::prompt_password("请再次输入新密码：").map_err(|error| error.to_string())?;
    if password != confirmation {
        return Err("两次输入的密码不一致".into());
    }
    let password_hash = hash_password(&password)?;
    let now = Utc::now().to_rfc3339();
    database.write(|connection| {
        let transaction = connection.transaction().map_err(|error| error.to_string())?;
        let updated = transaction
            .execute(
                "UPDATE admin_users SET password_hash = ?1, session_generation = session_generation + 1, updated_at = ?2",
                params![password_hash, now],
            )
            .map_err(|error| error.to_string())?;
        if updated == 0 {
            return Err("没有找到管理员账号".into());
        }
        transaction
            .execute("DELETE FROM admin_sessions", [])
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                r#"
                INSERT INTO audit_logs
                  (id, actor_type, action, target_type, target_id, result, metadata_json, created_at)
                VALUES (?1, 'system', 'admin.reset_password', 'admin', 'all', 'success', '{}', ?2)
                "#,
                params![uuid::Uuid::new_v4().to_string(), now],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    })?;
    println!("管理员密码已重置，已有登录会话已全部失效。");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
