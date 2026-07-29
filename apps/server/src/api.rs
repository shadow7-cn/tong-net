use crate::crypto::{
    decrypt, encrypt, generate_x25519_keypair, hash_password, random_token, token_hash,
    validate_device_name, validate_password, verify_password,
};
use crate::models::{AuditRecord, DeviceRecord, NetworkRecord, PublicInfo};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const ADMIN_COOKIE: &str = "tongnet_admin";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MINIMUM_DESKTOP_VERSION: &str = "0.2.0";

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code,
            message: message.into(),
        }
    }

    fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
        }
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
        }
    }

    fn too_many_requests(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "code": self.code,
                "message": self.message,
                "requestId": Uuid::new_v4().to_string()
            })),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/info", get(info))
        .route("/api/v1/setup", post(setup))
        .route("/api/v1/admin/login", post(admin_login))
        .route("/api/v1/admin/logout", post(admin_logout))
        .route("/api/v1/admin/overview", get(admin_overview))
        .route(
            "/api/v1/admin/networks",
            get(admin_networks).post(admin_create_network),
        )
        .route(
            "/api/v1/admin/networks/{id}/disable",
            post(admin_disable_network),
        )
        .route(
            "/api/v1/admin/networks/{id}/enable",
            post(admin_enable_network),
        )
        .route(
            "/api/v1/admin/networks/{id}/reset-password",
            post(admin_reset_network_password),
        )
        .route("/api/v1/admin/networks/{id}", delete(admin_delete_network))
        .route("/api/v1/admin/devices", get(admin_devices))
        .route(
            "/api/v1/admin/memberships/{id}",
            patch(admin_update_membership).delete(admin_delete_membership),
        )
        .route(
            "/api/v1/admin/memberships/{id}/revoke",
            post(admin_revoke_membership),
        )
        .route(
            "/api/v1/admin/audit-logs",
            get(admin_audit_logs).delete(admin_clear_audit_logs),
        )
        .route(
            "/api/v1/admin/settings",
            get(admin_settings).patch(admin_update_settings),
        )
        .route("/api/v1/admin/mode", post(admin_change_mode))
        .route("/api/v1/admin/easytier/retry", post(admin_retry_easytier))
        .route("/api/v1/private/connect", post(private_connect))
        .route("/api/v1/private/heartbeat", post(private_heartbeat))
        .route("/api/v1/private/disconnect", post(private_disconnect))
        .route("/api/v1/private/device", patch(private_rename_device))
        .route("/healthz", get(health))
        .with_state(state)
}

async fn info(State(state): State<AppState>) -> ApiResult<Json<PublicInfo>> {
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    Ok(Json(PublicInfo {
        initialized: snapshot.initialized,
        site_name: snapshot.site_name,
        mode: snapshot.mode,
        version: VERSION.into(),
        minimum_desktop_version: MINIMUM_DESKTOP_VERSION.into(),
        public_host: snapshot.public_host,
        web_port: state.config.web_port,
        easytier_port: state.config.easytier_port,
        shared_public_key: snapshot.shared_public_key,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupRequest {
    admin_username: String,
    admin_password: String,
    site_name: String,
    public_host: String,
    mode: String,
    network_name: Option<String>,
    network_password: Option<String>,
}

async fn setup(
    State(state): State<AppState>,
    Json(request): Json<SetupRequest>,
) -> ApiResult<Json<Value>> {
    validate_username(&request.admin_username)?;
    validate_password(&request.admin_password)
        .map_err(|message| ApiError::bad_request("PASSWORD_TOO_WEAK", message))?;
    let site_name = validate_label(&request.site_name, "站点名称", 40)?;
    let public_host = validate_public_host(&request.public_host)?;
    if !matches!(request.mode.as_str(), "public" | "private") {
        return Err(ApiError::bad_request("MODE_INVALID", "节点模式无效"));
    }
    if request.mode == "private" {
        validate_network_input(
            request.network_name.as_deref().unwrap_or_default(),
            request.network_password.as_deref().unwrap_or_default(),
        )?;
    }

    let admin_hash = hash_password(&request.admin_password).map_err(ApiError::internal)?;
    let shared_name = format!("tongnet-shared-{}", compact_random(12));
    let shared_secret = random_token(32);
    let (shared_private, shared_public) = generate_x25519_keypair();
    let shared_name_encrypted =
        encrypt(&state.master_key, &shared_name).map_err(ApiError::internal)?;
    let shared_secret_encrypted =
        encrypt(&state.master_key, &shared_secret).map_err(ApiError::internal)?;
    let shared_private_encrypted =
        encrypt(&state.master_key, &shared_private).map_err(ApiError::internal)?;
    let now = Utc::now().to_rfc3339();
    let admin_id = Uuid::new_v4().to_string();

    state
        .db
        .write(|connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            let initialized: bool = transaction
                .query_row(
                    "SELECT initialized FROM site_settings WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if initialized {
                return Err("SETUP_ALREADY_COMPLETED".into());
            }
            transaction
                .execute(
                    r#"
                    INSERT INTO admin_users
                      (id, username, password_hash, session_generation, created_at, updated_at)
                    VALUES (?1, ?2, ?3, 1, ?4, ?4)
                    "#,
                    params![admin_id, request.admin_username.trim(), admin_hash, now],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    r#"
                    UPDATE site_settings
                    SET initialized = 1, site_name = ?1, public_host = ?2, mode = ?3,
                        shared_name_ciphertext = ?4, shared_secret_ciphertext = ?5,
                        shared_private_key_ciphertext = ?6, shared_public_key = ?7,
                        updated_at = ?8
                    WHERE id = 1
                    "#,
                    params![
                        site_name,
                        public_host,
                        request.mode,
                        shared_name_encrypted,
                        shared_secret_encrypted,
                        shared_private_encrypted,
                        shared_public,
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
            if request.mode == "private" {
                insert_network(
                    &transaction,
                    &state,
                    request.network_name.as_deref().unwrap(),
                    request.network_password.as_deref().unwrap(),
                    0,
                    &now,
                )?;
            }
            insert_audit(
                &transaction,
                "admin",
                Some(&admin_id),
                "setup.initialize",
                "site",
                "1",
                "success",
                json!({"mode": request.mode}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(|error| {
            if error == "SETUP_ALREADY_COMPLETED" {
                ApiError::conflict("SETUP_ALREADY_COMPLETED", "服务已经完成初始化")
            } else {
                ApiError::internal(error)
            }
        })?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    username: String,
    password: String,
}

async fn admin_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> ApiResult<Response> {
    let now = Utc::now();
    let client_key = request_client_key(&headers);
    enforce_login_limit(&state, &client_key, &now)?;
    let admin = state
        .db
        .read(|connection| {
            connection
                .query_row(
                    "SELECT id, password_hash, session_generation FROM admin_users WHERE username = ?1",
                    [request.username.trim()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    let Some((admin_id, password_hash, generation)) = admin else {
        record_login_failure(&state, &client_key, &now)?;
        audit_failure(&state, "admin.login", "用户名或密码错误")?;
        return Err(ApiError::unauthorized(
            "ADMIN_CREDENTIALS_INVALID",
            "用户名或密码错误",
        ));
    };
    if !verify_password(&password_hash, &request.password) {
        record_login_failure(&state, &client_key, &now)?;
        audit_failure(&state, "admin.login", "用户名或密码错误")?;
        return Err(ApiError::unauthorized(
            "ADMIN_CREDENTIALS_INVALID",
            "用户名或密码错误",
        ));
    }
    let token = random_token(32);
    let session_id = Uuid::new_v4().to_string();
    let expires_at = (now + Duration::hours(12)).to_rfc3339();
    let now_text = now.to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO admin_sessions (id, token_hash, generation, expires_at, created_at, last_used_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![session_id, token_hash(&token), generation, expires_at, now_text],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE admin_users SET last_login_at = ?1 WHERE id = ?2",
                    params![now_text, admin_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "DELETE FROM admin_login_failures WHERE client_key = ?1",
                    [&client_key],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                &transaction,
                "admin",
                Some(&admin_id),
                "admin.login",
                "admin",
                &admin_id,
                "success",
                json!({}),
                &now_text,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"));
    let mut response = Json(json!({"ok": true})).into_response();
    let cookie = format!(
        "{ADMIN_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=43200{}",
        if secure { "; Secure" } else { "" }
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|error| ApiError::internal(error.to_string()))?,
    );
    Ok(response)
}

async fn admin_logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = cookie_token(&headers) {
        state
            .db
            .write(|connection| {
                connection
                    .execute(
                        "DELETE FROM admin_sessions WHERE token_hash = ?1",
                        [token_hash(&token)],
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .map_err(ApiError::internal)?;
    }
    let mut response = Json(json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("tongnet_admin=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    Ok(response)
}

async fn admin_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let _admin = require_admin(&state, &headers)?;
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    let (network_count, device_count, online_count) = state
        .db
        .read(|connection| {
            let network_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM private_networks", [], |row| row.get(0))
                .map_err(|error| error.to_string())?;
            let device_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM network_memberships", [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            let threshold = (Utc::now() - Duration::seconds(30)).to_rfc3339();
            let online_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM network_memberships WHERE last_seen_at >= ?1 AND status = 'active'",
                    [threshold],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            Ok((network_count, device_count, online_count))
        })
        .map_err(ApiError::internal)?;
    let easytier = state.easytier.status().await;
    let public_members = if snapshot.mode == "public" {
        state.easytier.observed_members().await.unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(Json(json!({
        "siteName": snapshot.site_name,
        "mode": snapshot.mode,
        "version": VERSION,
        "webPort": state.config.web_port,
        "easytierPort": state.config.easytier_port,
        "networkCount": network_count,
        "deviceCount": device_count,
        "onlineCount": online_count,
        "easytier": easytier,
        "publicMembers": public_members
    })))
}

async fn admin_networks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<NetworkRecord>>> {
    require_admin(&state, &headers)?;
    let threshold = (Utc::now() - Duration::seconds(30)).to_rfc3339();
    let values = state
        .db
        .read(|connection| {
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT n.id, n.name, n.status, n.slot, n.created_at,
                           COUNT(m.id),
                           SUM(CASE WHEN m.last_seen_at >= ?1 AND m.status = 'active' THEN 1 ELSE 0 END)
                    FROM private_networks n
                    LEFT JOIN network_memberships m ON m.network_id = n.id
                    GROUP BY n.id
                    ORDER BY n.slot
                    "#,
                )
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([threshold], |row| {
                    Ok(NetworkRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        status: row.get(2)?,
                        slot: row.get(3)?,
                        created_at: row.get(4)?,
                        device_count: row.get(5)?,
                        online_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    })
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Ok(rows)
        })
        .map_err(ApiError::internal)?;
    Ok(Json(values))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkRequest {
    name: String,
    password: String,
}

async fn admin_create_network(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NetworkRequest>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    validate_network_input(&request.name, &request.password)?;
    let now = Utc::now().to_rfc3339();
    let network_id = Uuid::new_v4().to_string();
    state
        .db
        .write(|connection| {
            let mode: String = connection
                .query_row("SELECT mode FROM site_settings WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            if mode != "private" {
                return Err("MODE_NOT_PRIVATE".into());
            }
            let used_slots = connection
                .prepare("SELECT slot FROM private_networks ORDER BY slot")
                .map_err(|error| error.to_string())?
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            let slot = (0..10)
                .find(|slot| !used_slots.contains(slot))
                .ok_or_else(|| "NETWORK_LIMIT_REACHED".to_string())?;
            insert_network_with_id(
                connection,
                &state,
                &network_id,
                &request.name,
                &request.password,
                slot,
                &now,
            )?;
            insert_audit(
                connection,
                "admin",
                Some(&admin.id),
                "network.create",
                "network",
                &network_id,
                "success",
                json!({"name": request.name}),
                &now,
            )
        })
        .map_err(map_database_error)?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"id": network_id, "ok": true})))
}

async fn admin_disable_network(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    set_network_status(&state, &admin.id, &id, "disabled")?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_enable_network(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    set_network_status(&state, &admin.id, &id, "active")?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetNetworkPasswordRequest {
    password: String,
    admin_password: String,
}

async fn admin_reset_network_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ResetNetworkPasswordRequest>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    verify_admin_password(&state, &admin.id, &request.admin_password)?;
    validate_password(&request.password)
        .map_err(|message| ApiError::bad_request("PASSWORD_TOO_WEAK", message))?;
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    let network = snapshot
        .networks
        .iter()
        .find(|network| network.id == id)
        .cloned()
        .ok_or_else(|| ApiError::bad_request("NETWORK_NOT_FOUND", "网络不存在"))?;
    let credentials = state
        .db
        .read(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT credential_id FROM network_memberships WHERE network_id = ?1 AND credential_id IS NOT NULL",
                )
                .map_err(|error| error.to_string())?;
            let credentials = statement
                .query_map([&id], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Ok(credentials)
        })
        .map_err(ApiError::internal)?;
    if network.status == "active" {
        for credential in credentials {
            state
                .easytier
                .revoke_credential(&network, &credential)
                .await
                .map_err(ApiError::internal)?;
        }
    } else {
        let credential_file = state
            .config
            .data_dir
            .join("easytier/networks")
            .join(&network.id)
            .join("credentials.json");
        if credential_file.exists() {
            std::fs::remove_file(credential_file)
                .map_err(|error| ApiError::internal(error.to_string()))?;
        }
    }
    let now = Utc::now().to_rfc3339();
    let password_hash = hash_password(&request.password).map_err(ApiError::internal)?;
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE private_networks SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
                    params![password_hash, now, id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    r#"
                    UPDATE network_memberships
                    SET credential_id = NULL, credential_secret_ciphertext = NULL,
                        virtual_ip = '', protocol = '', latency_ms = NULL, updated_at = ?1
                    WHERE network_id = ?2
                    "#,
                    params![now, id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE device_sessions SET revoked_at = ?1 WHERE membership_id IN (SELECT id FROM network_memberships WHERE network_id = ?2) AND revoked_at IS NULL",
                    params![now, id],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "network.reset_password",
                "network",
                &id,
                "success",
                json!({}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_delete_network(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let threshold = (Utc::now() - Duration::seconds(30)).to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            let status: Option<String> = transaction
                .query_row(
                    "SELECT status FROM private_networks WHERE id = ?1",
                    [&id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            if status.as_deref() != Some("disabled") {
                return Err("NETWORK_MUST_BE_DISABLED".into());
            }
            let online: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM network_memberships WHERE network_id = ?1 AND last_seen_at >= ?2",
                    params![id, threshold],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if online > 0 {
                return Err("NETWORK_HAS_ONLINE_DEVICES".into());
            }
            transaction
                .execute("DELETE FROM private_networks WHERE id = ?1", [&id])
                .map_err(|error| error.to_string())?;
            let now = Utc::now().to_rfc3339();
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "network.delete",
                "network",
                &id,
                "success",
                json!({}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(map_database_error)?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceQuery {
    network_id: Option<String>,
}

async fn admin_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeviceQuery>,
) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers)?;
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    if snapshot.mode == "public" {
        let members = state.easytier.observed_members().await.unwrap_or_default();
        return Ok(Json(json!({"mode": "public", "members": members})));
    }
    let threshold = (Utc::now() - Duration::seconds(30)).to_rfc3339();
    let values = state
        .db
        .read(|connection| {
            let mut sql = r#"
                SELECT m.id, d.id, n.id, n.name, d.name, m.admin_note, d.platform,
                       d.client_version, m.status, m.virtual_ip, m.protocol,
                       m.latency_ms, m.rx_bytes, m.tx_bytes, m.last_seen_at, m.created_at
                FROM network_memberships m
                JOIN devices d ON d.id = m.device_id
                JOIN private_networks n ON n.id = m.network_id
            "#
            .to_string();
            if query.network_id.is_some() {
                sql.push_str(" WHERE n.id = ?1");
            }
            sql.push_str(" ORDER BY m.last_seen_at DESC, m.created_at DESC");
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| error.to_string())?;
            let map = |row: &rusqlite::Row<'_>| {
                let last_seen_at = row.get::<_, Option<String>>(14)?;
                Ok(DeviceRecord {
                    membership_id: row.get(0)?,
                    device_id: row.get(1)?,
                    network_id: row.get(2)?,
                    network_name: row.get(3)?,
                    name: row.get(4)?,
                    admin_note: row.get(5)?,
                    platform: row.get(6)?,
                    client_version: row.get(7)?,
                    status: row.get(8)?,
                    online: last_seen_at
                        .as_ref()
                        .is_some_and(|value| value >= &threshold),
                    virtual_ip: row.get(9)?,
                    protocol: row.get(10)?,
                    latency_ms: row.get(11)?,
                    rx_bytes: row.get(12)?,
                    tx_bytes: row.get(13)?,
                    last_seen_at,
                    created_at: row.get(15)?,
                })
            };
            if let Some(network_id) = query.network_id.as_deref() {
                statement
                    .query_map([network_id], map)
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())
            } else {
                statement
                    .query_map([], map)
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())
            }
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"mode": "private", "members": values})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMembershipRequest {
    admin_note: String,
}

async fn admin_update_membership(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateMembershipRequest>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    if request.admin_note.chars().count() > 100 || request.admin_note.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "ADMIN_NOTE_INVALID",
            "管理员备注最多 100 个字符",
        ));
    }
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let changed = connection
                .execute(
                    "UPDATE network_memberships SET admin_note = ?1, updated_at = ?2 WHERE id = ?3",
                    params![request.admin_note.trim(), now, id],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err("MEMBERSHIP_NOT_FOUND".into());
            }
            insert_audit(
                connection,
                "admin",
                Some(&admin.id),
                "membership.note",
                "membership",
                &id,
                "success",
                json!({}),
                &now,
            )
        })
        .map_err(map_database_error)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_revoke_membership(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let membership = membership_credential(&state, &id)?;
    if let Some(credential_id) = membership.1.as_deref() {
        let network = state
            .snapshot()
            .map_err(ApiError::internal)?
            .networks
            .into_iter()
            .find(|network| network.id == membership.0)
            .ok_or_else(|| ApiError::bad_request("NETWORK_NOT_FOUND", "网络不存在"))?;
        state
            .easytier
            .revoke_credential(&network, credential_id)
            .await
            .map_err(ApiError::internal)?;
    }
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE network_memberships SET status = 'revoked', updated_at = ?1 WHERE id = ?2",
                    params![now, id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE device_sessions SET revoked_at = ?1 WHERE membership_id = ?2 AND revoked_at IS NULL",
                    params![now, id],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "membership.revoke",
                "membership",
                &id,
                "success",
                json!({}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_delete_membership(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let threshold = (Utc::now() - Duration::seconds(30)).to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            let value: Option<(String, Option<String>)> = transaction
                .query_row(
                    "SELECT status, last_seen_at FROM network_memberships WHERE id = ?1",
                    [&id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            let Some((status, last_seen)) = value else {
                return Err("MEMBERSHIP_NOT_FOUND".into());
            };
            if status != "revoked" {
                return Err("MEMBERSHIP_MUST_BE_REVOKED".into());
            }
            if last_seen.as_ref().is_some_and(|value| value >= &threshold) {
                return Err("MEMBERSHIP_STILL_ONLINE".into());
            }
            transaction
                .execute("DELETE FROM network_memberships WHERE id = ?1", [&id])
                .map_err(|error| error.to_string())?;
            let now = Utc::now().to_rfc3339();
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "membership.delete",
                "membership",
                &id,
                "success",
                json!({}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(map_database_error)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Default, Deserialize)]
struct PageQuery {
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn admin_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers)?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
    let offset = (page - 1) * page_size;
    let (total, items) = state
        .db
        .read(|connection| {
            let total: i64 = connection
                .query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))
                .map_err(|error| error.to_string())?;
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT id, actor_type, action, COALESCE(target_type, ''),
                           COALESCE(target_id, ''), result, COALESCE(ip_address, ''),
                           metadata_json, created_at
                    FROM audit_logs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2
                    "#,
                )
                .map_err(|error| error.to_string())?;
            let items = statement
                .query_map(params![page_size, offset], |row| {
                    let metadata: String = row.get(7)?;
                    Ok(AuditRecord {
                        id: row.get(0)?,
                        actor_type: row.get(1)?,
                        action: row.get(2)?,
                        target_type: row.get(3)?,
                        target_id: row.get(4)?,
                        result: row.get(5)?,
                        ip_address: row.get(6)?,
                        metadata: serde_json::from_str(&metadata).unwrap_or_else(|_| json!({})),
                        created_at: row.get(8)?,
                    })
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            Ok((total, items))
        })
        .map_err(ApiError::internal)?;
    Ok(Json(
        json!({"items": items, "total": total, "page": page, "pageSize": page_size}),
    ))
}

async fn admin_clear_audit_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            transaction
                .execute("DELETE FROM audit_logs", [])
                .map_err(|error| error.to_string())?;
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "audit.clear",
                "audit",
                "all",
                "success",
                json!({}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    Ok(Json(json!({
        "siteName": snapshot.site_name,
        "publicHost": snapshot.public_host,
        "mode": snapshot.mode,
        "adminUsername": admin.username,
        "webPort": state.config.web_port,
        "easytierPort": state.config.easytier_port,
        "version": VERSION,
        "easytierVersion": "2.6.4"
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSettingsRequest {
    site_name: String,
    public_host: String,
    admin_username: Option<String>,
    current_password: Option<String>,
    new_password: Option<String>,
}

async fn admin_update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateSettingsRequest>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    let previous_site_name = state.snapshot().map_err(ApiError::internal)?.site_name;
    let site_name = validate_label(&request.site_name, "站点名称", 40)?;
    let public_host = validate_public_host(&request.public_host)?;
    let changing_admin = request
        .admin_username
        .as_deref()
        .is_some_and(|value| value.trim() != admin.username)
        || request.new_password.is_some();
    if changing_admin {
        verify_admin_password(
            &state,
            &admin.id,
            request.current_password.as_deref().unwrap_or_default(),
        )?;
    }
    let username = request
        .admin_username
        .as_deref()
        .unwrap_or(&admin.username)
        .trim()
        .to_string();
    validate_username(&username)?;
    let password_hash = if let Some(password) = request.new_password.as_deref() {
        validate_password(password)
            .map_err(|message| ApiError::bad_request("PASSWORD_TOO_WEAK", message))?;
        Some(hash_password(password).map_err(ApiError::internal)?)
    } else {
        None
    };
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE site_settings SET site_name = ?1, public_host = ?2, updated_at = ?3 WHERE id = 1",
                    params![site_name, public_host, now],
                )
                .map_err(|error| error.to_string())?;
            if changing_admin {
                if let Some(hash) = password_hash {
                    transaction
                        .execute(
                            "UPDATE admin_users SET username = ?1, password_hash = ?2, session_generation = session_generation + 1, updated_at = ?3 WHERE id = ?4",
                            params![username, hash, now, admin.id],
                        )
                        .map_err(|error| error.to_string())?;
                } else {
                    transaction
                        .execute(
                            "UPDATE admin_users SET username = ?1, session_generation = session_generation + 1, updated_at = ?2 WHERE id = ?3",
                            params![username, now, admin.id],
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
            insert_audit(
                &transaction,
                "admin",
                Some(&admin.id),
                "settings.update",
                "site",
                "1",
                "success",
                json!({"adminChanged": changing_admin}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    if site_name != previous_site_name {
        state.apply_runtime().await.map_err(ApiError::internal)?;
    }
    Ok(Json(json!({"ok": true, "reauthRequired": changing_admin})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangeModeRequest {
    mode: String,
    admin_password: String,
}

async fn admin_change_mode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangeModeRequest>,
) -> ApiResult<Json<Value>> {
    let admin = require_admin(&state, &headers)?;
    verify_admin_password(&state, &admin.id, &request.admin_password)?;
    if !matches!(request.mode.as_str(), "public" | "private") {
        return Err(ApiError::bad_request("MODE_INVALID", "节点模式无效"));
    }
    if request.mode == "private" {
        let count = state
            .db
            .read(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM private_networks", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|error| error.to_string())
            })
            .map_err(ApiError::internal)?;
        if count == 0 {
            return Err(ApiError::conflict(
                "PRIVATE_NETWORK_REQUIRED",
                "切换私有模式前至少需要创建一个私有网络",
            ));
        }
    }
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            connection
                .execute(
                    "UPDATE site_settings SET mode = ?1, updated_at = ?2 WHERE id = 1",
                    params![request.mode, now],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                connection,
                "admin",
                Some(&admin.id),
                "mode.change",
                "site",
                "1",
                "success",
                json!({"mode": request.mode}),
                &now,
            )
        })
        .map_err(ApiError::internal)?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

async fn admin_retry_easytier(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    require_admin(&state, &headers)?;
    state.apply_runtime().await.map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivateConnectRequest {
    network_name: String,
    network_password: String,
    client_device_id: String,
    device_name: String,
    platform: String,
    client_version: String,
    replace_credential: Option<bool>,
}

async fn private_connect(
    State(state): State<AppState>,
    Json(request): Json<PrivateConnectRequest>,
) -> ApiResult<Json<Value>> {
    let snapshot = state.snapshot().map_err(ApiError::internal)?;
    if !snapshot.initialized {
        return Err(ApiError::conflict("SETUP_REQUIRED", "组网服务尚未初始化"));
    }
    if snapshot.mode != "private" {
        return Err(ApiError::conflict(
            "MODE_NOT_PRIVATE",
            "当前服务器运行在公共节点模式",
        ));
    }
    let name = normalize_network_name(&request.network_name);
    let device_name = validate_device_name(&request.device_name)
        .map_err(|message| ApiError::bad_request("DEVICE_NAME_INVALID", message))?;
    validate_client_device_id(&request.client_device_id)?;

    let network_auth = state
        .db
        .read(|connection| {
            connection
                .query_row(
                    "SELECT id, password_hash, status FROM private_networks WHERE name_normalized = ?1",
                    [&name],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    let Some((network_id, password_hash, network_status)) = network_auth else {
        return Err(ApiError::unauthorized(
            "NETWORK_CREDENTIALS_INVALID",
            "网络名称或密码错误",
        ));
    };
    if network_status != "active" {
        return Err(ApiError::conflict(
            "NETWORK_DISABLED",
            "该网络已被管理员停用",
        ));
    }
    if !verify_password(&password_hash, &request.network_password) {
        return Err(ApiError::unauthorized(
            "NETWORK_CREDENTIALS_INVALID",
            "网络名称或密码错误",
        ));
    }
    let now = Utc::now().to_rfc3339();
    let (device_id, membership_id, existing_credential_id, existing_credential_secret, status) =
        state
            .db
            .write(|connection| {
                let transaction =
                    connection.transaction().map_err(|error| error.to_string())?;
                let device = transaction
                    .query_row(
                        "SELECT id FROM devices WHERE client_device_id = ?1",
                        [&request.client_device_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(|error| error.to_string())?;
                let device_id = device.unwrap_or_else(|| Uuid::new_v4().to_string());
                transaction
                    .execute(
                        r#"
                        INSERT INTO devices
                          (id, client_device_id, name, platform, client_version, created_at, updated_at)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                        ON CONFLICT(client_device_id) DO UPDATE SET
                          name = excluded.name, platform = excluded.platform,
                          client_version = excluded.client_version, updated_at = excluded.updated_at
                        "#,
                        params![
                            device_id,
                            request.client_device_id,
                            device_name,
                            request.platform,
                            request.client_version,
                            now
                        ],
                    )
                    .map_err(|error| error.to_string())?;
                let membership = transaction
                    .query_row(
                        "SELECT id, credential_id, credential_secret_ciphertext, status FROM network_memberships WHERE network_id = ?1 AND device_id = ?2",
                        params![network_id, device_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| error.to_string())?;
                let result = if let Some(value) = membership {
                    value
                } else {
                    let membership_id = Uuid::new_v4().to_string();
                    transaction
                        .execute(
                            "INSERT INTO network_memberships (id, network_id, device_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                            params![membership_id, network_id, device_id, now],
                        )
                        .map_err(|error| error.to_string())?;
                    (membership_id, None, None, "active".into())
                };
                transaction.commit().map_err(|error| error.to_string())?;
                Ok((device_id, result.0, result.1, result.2, result.3))
            })
            .map_err(ApiError::internal)?;
    if status == "revoked" {
        return Err(ApiError::forbidden(
            "DEVICE_REVOKED",
            "此设备已被管理员撤销",
        ));
    }
    let network = snapshot
        .networks
        .iter()
        .find(|network| network.id == network_id)
        .cloned()
        .ok_or_else(|| ApiError::internal("网络运行配置不存在"))?;
    let should_replace = request.replace_credential.unwrap_or(false);
    if should_replace {
        if let Some(credential_id) = existing_credential_id.as_deref() {
            state
                .easytier
                .revoke_credential(&network, credential_id)
                .await
                .map_err(ApiError::internal)?;
        }
    }
    let credential = if !should_replace {
        match (
            existing_credential_id.as_deref(),
            existing_credential_secret.as_deref(),
        ) {
            (Some(id), Some(secret)) => crate::models::CredentialResult {
                credential_id: id.into(),
                credential_secret: decrypt(&state.master_key, secret)
                    .map_err(ApiError::internal)?,
            },
            _ => issue_membership_credential(&state, &network, &membership_id).await?,
        }
    } else {
        issue_membership_credential(&state, &network, &membership_id).await?
    };
    let encrypted_credential =
        encrypt(&state.master_key, &credential.credential_secret).map_err(ApiError::internal)?;
    let session_token = random_token(32);
    let session_id = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::hours(24)).to_rfc3339();
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection.transaction().map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE network_memberships SET credential_id = ?1, credential_secret_ciphertext = ?2, updated_at = ?3 WHERE id = ?4",
                    params![credential.credential_id, encrypted_credential, now, membership_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE device_sessions SET revoked_at = ?1 WHERE membership_id = ?2 AND revoked_at IS NULL",
                    params![now, membership_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO device_sessions (id, membership_id, token_hash, expires_at, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![session_id, membership_id, token_hash(&session_token), expires_at, now],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                &transaction,
                "device",
                Some(&device_id),
                "device.connect",
                "membership",
                &membership_id,
                "success",
                json!({"networkId": network_id}),
                &now,
            )?;
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({
        "mode": "private",
        "deviceId": device_id,
        "membershipId": membership_id,
        "sessionToken": session_token,
        "network": {
            "name": network.internal_name,
            "credential": credential.credential_secret,
            "peers": [
                format!("tcp://{}:{}", snapshot.public_host, state.config.easytier_port),
                format!("udp://{}:{}", snapshot.public_host, state.config.easytier_port)
            ],
            "peerPublicKey": snapshot.shared_public_key
        }
    })))
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatRequest {
    virtual_ip: Option<String>,
    protocol: Option<String>,
    latency_ms: Option<i64>,
    rx_bytes: Option<i64>,
    tx_bytes: Option<i64>,
    client_version: Option<String>,
}

async fn private_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<HeartbeatRequest>,
) -> ApiResult<Json<Value>> {
    let session = require_device_session(&state, &headers)?;
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "UPDATE device_sessions SET last_seen_at = ?1 WHERE id = ?2",
                    params![now, session.session_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    r#"
                    UPDATE network_memberships
                    SET virtual_ip = COALESCE(?1, virtual_ip),
                        protocol = COALESCE(?2, protocol),
                        latency_ms = COALESCE(?3, latency_ms),
                        rx_bytes = COALESCE(?4, rx_bytes),
                        tx_bytes = COALESCE(?5, tx_bytes),
                        last_seen_at = ?6, updated_at = ?6
                    WHERE id = ?7
                    "#,
                    params![
                        request.virtual_ip,
                        request.protocol,
                        request.latency_ms,
                        request.rx_bytes,
                        request.tx_bytes,
                        now,
                        session.membership_id
                    ],
                )
                .map_err(|error| error.to_string())?;
            if let Some(version) = request.client_version {
                transaction
                    .execute(
                        "UPDATE devices SET client_version = ?1, updated_at = ?2 WHERE id = ?3",
                        params![version, now, session.device_id],
                    )
                    .map_err(|error| error.to_string())?;
            }
            transaction.commit().map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true, "serverTime": now})))
}

async fn private_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let session = require_device_session(&state, &headers)?;
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            connection
                .execute(
                    "UPDATE device_sessions SET revoked_at = ?1 WHERE id = ?2",
                    params![now, session.session_id],
                )
                .map_err(|error| error.to_string())?;
            insert_audit(
                connection,
                "device",
                Some(&session.device_id),
                "device.disconnect",
                "membership",
                &session.membership_id,
                "success",
                json!({}),
                &now,
            )
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameDeviceRequest {
    device_name: String,
}

async fn private_rename_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RenameDeviceRequest>,
) -> ApiResult<Json<Value>> {
    let session = require_device_session(&state, &headers)?;
    let name = validate_device_name(&request.device_name)
        .map_err(|message| ApiError::bad_request("DEVICE_NAME_INVALID", message))?;
    state
        .db
        .write(|connection| {
            connection
                .execute(
                    "UPDATE devices SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![name, Utc::now().to_rfc3339(), session.device_id],
                )
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"ok": true, "name": name})))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let snapshot = state.snapshot().ok();
    let easytier = if snapshot.as_ref().is_some_and(|value| value.initialized) {
        state.easytier.status().await
    } else {
        crate::models::EasyTierStatus::default()
    };
    Json(json!({
        "status": if state.db.is_healthy() && (snapshot.as_ref().is_some_and(|value| !value.initialized) || easytier.healthy) { "healthy" } else { "degraded" },
        "initialized": snapshot.as_ref().is_some_and(|value| value.initialized),
        "database": if state.db.is_healthy() { "ok" } else { "error" },
        "easytier": easytier,
        "version": VERSION
    }))
}

#[derive(Clone)]
struct AdminIdentity {
    id: String,
    username: String,
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> ApiResult<AdminIdentity> {
    let token = cookie_token(headers).ok_or_else(|| {
        ApiError::unauthorized("ADMIN_SESSION_REQUIRED", "管理员登录已失效，请重新登录")
    })?;
    let now = Utc::now().to_rfc3339();
    let value = state
        .db
        .read(|connection| {
            connection
                .query_row(
                    r#"
                    SELECT a.id, a.username
                    FROM admin_sessions s
                    JOIN admin_users a ON a.session_generation = s.generation
                    WHERE s.token_hash = ?1 AND s.expires_at > ?2
                    "#,
                    params![token_hash(&token), now],
                    |row| {
                        Ok(AdminIdentity {
                            id: row.get(0)?,
                            username: row.get(1)?,
                        })
                    },
                )
                .optional()
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    value.ok_or_else(|| {
        ApiError::unauthorized("ADMIN_SESSION_REQUIRED", "管理员登录已失效，请重新登录")
    })
}

struct DeviceSession {
    session_id: String,
    membership_id: String,
    device_id: String,
}

fn require_device_session(state: &AppState, headers: &HeaderMap) -> ApiResult<DeviceSession> {
    let token = bearer_token(headers).ok_or_else(|| {
        ApiError::unauthorized("DEVICE_SESSION_REQUIRED", "设备会话已失效，请重新连接")
    })?;
    let now = Utc::now().to_rfc3339();
    state
        .db
        .read(|connection| {
            connection
                .query_row(
                    r#"
                    SELECT s.id, m.id, d.id
                    FROM device_sessions s
                    JOIN network_memberships m ON m.id = s.membership_id
                    JOIN devices d ON d.id = m.device_id
                    JOIN private_networks n ON n.id = m.network_id
                    WHERE s.token_hash = ?1 AND s.revoked_at IS NULL AND s.expires_at > ?2
                      AND m.status = 'active' AND n.status = 'active'
                    "#,
                    params![token_hash(&token), now],
                    |row| {
                        Ok(DeviceSession {
                            session_id: row.get(0)?,
                            membership_id: row.get(1)?,
                            device_id: row.get(2)?,
                        })
                    },
                )
                .optional()
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError::unauthorized("DEVICE_SESSION_REQUIRED", "设备会话已失效，请重新连接")
        })
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == ADMIN_COOKIE).then(|| value.to_string())
        })
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

fn verify_admin_password(state: &AppState, admin_id: &str, password: &str) -> ApiResult<()> {
    let hash = state
        .db
        .read(|connection| {
            connection
                .query_row(
                    "SELECT password_hash FROM admin_users WHERE id = ?1",
                    [admin_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    if verify_password(&hash, password) {
        Ok(())
    } else {
        Err(ApiError::unauthorized(
            "ADMIN_PASSWORD_INVALID",
            "管理员密码错误",
        ))
    }
}

fn set_network_status(state: &AppState, admin_id: &str, id: &str, status: &str) -> ApiResult<()> {
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            let changed = connection
                .execute(
                    "UPDATE private_networks SET status = ?1, updated_at = ?2 WHERE id = ?3",
                    params![status, now, id],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err("NETWORK_NOT_FOUND".into());
            }
            insert_audit(
                connection,
                "admin",
                Some(admin_id),
                if status == "active" {
                    "network.enable"
                } else {
                    "network.disable"
                },
                "network",
                id,
                "success",
                json!({}),
                &now,
            )
        })
        .map_err(map_database_error)
}

fn membership_credential(
    state: &AppState,
    membership_id: &str,
) -> ApiResult<(String, Option<String>)> {
    state
        .db
        .read(|connection| {
            connection
                .query_row(
                    "SELECT network_id, credential_id FROM network_memberships WHERE id = ?1",
                    [membership_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::bad_request("MEMBERSHIP_NOT_FOUND", "设备成员关系不存在"))
}

async fn issue_membership_credential(
    state: &AppState,
    network: &crate::models::NetworkSecret,
    membership_id: &str,
) -> ApiResult<crate::models::CredentialResult> {
    let credential_id = Uuid::new_v4().to_string();
    state
        .easytier
        .issue_credential(network, &credential_id)
        .await
        .map_err(|message| ApiError::internal(format!("签发设备凭据失败：{message}")))
        .map(|credential| {
            let _ = membership_id;
            credential
        })
}

fn insert_network(
    connection: &rusqlite::Connection,
    state: &AppState,
    name: &str,
    password: &str,
    slot: i64,
    now: &str,
) -> Result<(), String> {
    insert_network_with_id(
        connection,
        state,
        &Uuid::new_v4().to_string(),
        name,
        password,
        slot,
        now,
    )
}

fn insert_network_with_id(
    connection: &rusqlite::Connection,
    state: &AppState,
    id: &str,
    name: &str,
    password: &str,
    slot: i64,
    now: &str,
) -> Result<(), String> {
    let name = name.trim();
    let password_hash = hash_password(password)?;
    let internal_name = format!("tongnet-private-{}", compact_random(16));
    let internal_secret = random_token(32);
    let (private_key, _) = generate_x25519_keypair();
    connection
        .execute(
            r#"
            INSERT INTO private_networks
              (id, name, name_normalized, password_hash, internal_name_ciphertext,
               internal_secret_ciphertext, private_key_ciphertext, status, slot,
               created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, ?9)
            "#,
            params![
                id,
                name,
                normalize_network_name(name),
                password_hash,
                encrypt(&state.master_key, &internal_name)?,
                encrypt(&state.master_key, &internal_secret)?,
                encrypt(&state.master_key, &private_key)?,
                slot,
                now
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn insert_audit(
    connection: &rusqlite::Connection,
    actor_type: &str,
    actor_id: Option<&str>,
    action: &str,
    target_type: &str,
    target_id: &str,
    result: &str,
    metadata: Value,
    now: &str,
) -> Result<(), String> {
    connection
        .execute(
            r#"
            INSERT INTO audit_logs
              (id, actor_type, actor_id, action, target_type, target_id, result,
               ip_address, metadata_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '', ?8, ?9)
            "#,
            params![
                Uuid::new_v4().to_string(),
                actor_type,
                actor_id,
                action,
                target_type,
                target_id,
                result,
                metadata.to_string(),
                now
            ],
        )
        .map_err(|error| error.to_string())?;
    cleanup_audit(connection, now)
}

fn cleanup_audit(connection: &rusqlite::Connection, now: &str) -> Result<(), String> {
    let cutoff = (chrono::DateTime::parse_from_rfc3339(now).map_err(|error| error.to_string())?
        - Duration::days(90))
    .to_rfc3339();
    connection
        .execute("DELETE FROM audit_logs WHERE created_at < ?1", [cutoff])
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM audit_logs WHERE id IN (SELECT id FROM audit_logs ORDER BY created_at DESC LIMIT -1 OFFSET 10000)",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn audit_failure(state: &AppState, action: &str, reason: &str) -> ApiResult<()> {
    let now = Utc::now().to_rfc3339();
    state
        .db
        .write(|connection| {
            insert_audit(
                connection,
                "anonymous",
                None,
                action,
                "admin",
                "",
                "failure",
                json!({"reason": reason}),
                &now,
            )
        })
        .map_err(ApiError::internal)
}

fn request_client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("direct")
        .chars()
        .take(100)
        .collect()
}

fn enforce_login_limit(
    state: &AppState,
    client_key: &str,
    now: &chrono::DateTime<Utc>,
) -> ApiResult<()> {
    let cutoff = (*now - Duration::minutes(5)).to_rfc3339();
    let failures = state
        .db
        .write(|connection| {
            connection
                .execute(
                    "DELETE FROM admin_login_failures WHERE created_at < ?1",
                    [&cutoff],
                )
                .map_err(|error| error.to_string())?;
            connection
                .query_row(
                    "SELECT COUNT(*) FROM admin_login_failures WHERE client_key = ?1",
                    [client_key],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| error.to_string())
        })
        .map_err(ApiError::internal)?;
    if failures >= 5 {
        Err(ApiError::too_many_requests(
            "ADMIN_LOGIN_RATE_LIMITED",
            "登录失败次数过多，请 5 分钟后再试",
        ))
    } else {
        Ok(())
    }
}

fn record_login_failure(
    state: &AppState,
    client_key: &str,
    now: &chrono::DateTime<Utc>,
) -> ApiResult<()> {
    state
        .db
        .write(|connection| {
            connection
                .execute(
                    "INSERT INTO admin_login_failures (id, client_key, created_at) VALUES (?1, ?2, ?3)",
                    params![Uuid::new_v4().to_string(), client_key, now.to_rfc3339()],
                )
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .map_err(ApiError::internal)
}

fn validate_username(value: &str) -> ApiResult<()> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 40 || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "USERNAME_INVALID",
            "管理员用户名需要 1-40 个字符",
        ));
    }
    Ok(())
}

fn validate_label(value: &str, label: &str, max: usize) -> ApiResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "VALUE_INVALID",
            format!("{label}需要 1-{max} 个字符"),
        ));
    }
    Ok(value.into())
}

fn validate_public_host(value: &str) -> ApiResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.contains('/')
        || value.contains(char::is_whitespace)
        || value.contains("://")
    {
        return Err(ApiError::bad_request(
            "PUBLIC_HOST_INVALID",
            "对外地址只填写 IP 或域名，不包含协议和路径",
        ));
    }
    Ok(value.into())
}

fn validate_network_input(name: &str, password: &str) -> ApiResult<()> {
    validate_label(name, "网络名称", 64)?;
    validate_password(password)
        .map_err(|message| ApiError::bad_request("PASSWORD_TOO_WEAK", message))
}

fn validate_client_device_id(value: &str) -> ApiResult<()> {
    if value.trim().is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request("DEVICE_ID_INVALID", "设备 ID 无效"));
    }
    Ok(())
}

fn normalize_network_name(value: &str) -> String {
    value.trim().to_lowercase()
}

fn compact_random(bytes: usize) -> String {
    token_hash(&random_token(bytes))[..bytes * 2].to_string()
}

fn map_database_error(error: String) -> ApiError {
    match error.as_str() {
        "MODE_NOT_PRIVATE" => ApiError::conflict("MODE_NOT_PRIVATE", "当前不是私有节点模式"),
        "NETWORK_LIMIT_REACHED" => {
            ApiError::conflict("NETWORK_LIMIT_REACHED", "私有网络数量已达到 10 个")
        }
        "NETWORK_NOT_FOUND" => ApiError::bad_request("NETWORK_NOT_FOUND", "网络不存在"),
        "NETWORK_MUST_BE_DISABLED" => {
            ApiError::conflict("NETWORK_MUST_BE_DISABLED", "请先停用网络")
        }
        "NETWORK_HAS_ONLINE_DEVICES" => {
            ApiError::conflict("NETWORK_HAS_ONLINE_DEVICES", "网络中仍有在线设备")
        }
        "MEMBERSHIP_NOT_FOUND" => {
            ApiError::bad_request("MEMBERSHIP_NOT_FOUND", "设备成员关系不存在")
        }
        "MEMBERSHIP_MUST_BE_REVOKED" => {
            ApiError::conflict("MEMBERSHIP_MUST_BE_REVOKED", "请先撤销设备")
        }
        "MEMBERSHIP_STILL_ONLINE" => {
            ApiError::conflict("MEMBERSHIP_STILL_ONLINE", "在线设备不能删除")
        }
        _ if error.contains("private_networks.name_normalized") => {
            ApiError::conflict("NETWORK_NAME_EXISTS", "网络名称已经存在")
        }
        _ => ApiError::internal(error),
    }
}
