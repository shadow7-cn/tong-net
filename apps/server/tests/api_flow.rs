use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tong_net_server::{
    app, config::ServerConfig, crypto::load_or_create_master_key, db::Database,
    easytier::EasyTierSupervisor, state::AppState,
};
use tower::ServiceExt;

struct TestApp {
    _directory: TempDir,
    router: axum::Router,
}

impl TestApp {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let config = ServerConfig {
            web_port: 17280,
            easytier_port: 11010,
            data_dir: directory.path().to_path_buf(),
            web_dir: directory.path().to_path_buf(),
            easytier_core: "missing".into(),
            easytier_cli: "missing".into(),
            internal_easytier_host: "127.0.0.1".into(),
            easytier_disabled: true,
        };
        config.ensure_directories().unwrap();
        std::fs::write(config.web_dir.join("index.html"), "admin").unwrap();
        let database = Arc::new(Database::open(&config.database_path()).unwrap());
        let master_key =
            load_or_create_master_key(&config.data_dir.join("keys/master.key")).unwrap();
        let state = AppState {
            easytier: EasyTierSupervisor::new(config.clone()),
            config,
            db: database,
            master_key,
        };
        Self {
            _directory: directory,
            router: app(state),
        }
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        cookie: Option<&str>,
        bearer: Option<&str>,
    ) -> Response {
        let mut builder = Request::builder().method(method).uri(path);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.router
            .clone()
            .oneshot(
                builder
                    .body(Body::from(
                        body.map_or_else(String::new, |value| value.to_string()),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
}

async fn json_body(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn initialize_and_login(app: &TestApp) -> String {
    let setup = app
        .request(
            Method::POST,
            "/api/v1/setup",
            Some(json!({
                "adminUsername": "admin",
                "adminPassword": "admin-pass-123",
                "siteName": "测试站点",
                "publicHost": "vpn.example.com",
                "mode": "private",
                "networkName": "家庭网络",
                "networkPassword": "network-pass-123"
            })),
            None,
            None,
        )
        .await;
    assert_eq!(setup.status(), StatusCode::OK);

    let login = app
        .request(
            Method::POST,
            "/api/v1/admin/login",
            Some(json!({
                "username": "admin",
                "password": "admin-pass-123"
            })),
            None,
            None,
        )
        .await;
    assert_eq!(login.status(), StatusCode::OK);
    login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn setup_is_single_use_and_exposes_public_info() {
    let app = TestApp::new();
    let cookie = initialize_and_login(&app).await;

    let info = app
        .request(Method::GET, "/api/v1/info", None, None, None)
        .await;
    assert_eq!(info.status(), StatusCode::OK);
    let info = json_body(info).await;
    assert_eq!(info["initialized"], true);
    assert_eq!(info["mode"], "private");
    assert_eq!(info["publicHost"], "vpn.example.com");
    assert_eq!(info["easytierPort"], 11010);

    let networks = app
        .request(
            Method::GET,
            "/api/v1/admin/networks",
            None,
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(networks.status(), StatusCode::OK);
    assert_eq!(json_body(networks).await.as_array().unwrap().len(), 1);

    let setup_again = app
        .request(
            Method::POST,
            "/api/v1/setup",
            Some(json!({
                "adminUsername": "other",
                "adminPassword": "other-pass-123",
                "siteName": "其他",
                "publicHost": "other.example.com",
                "mode": "public"
            })),
            None,
            None,
        )
        .await;
    assert_eq!(setup_again.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn admin_login_is_rate_limited_after_five_failures() {
    let app = TestApp::new();
    initialize_and_login(&app).await;

    for _ in 0..5 {
        let response = app
            .request(
                Method::POST,
                "/api/v1/admin/login",
                Some(json!({
                    "username": "admin",
                    "password": "wrong-password"
                })),
                None,
                None,
            )
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let limited = app
        .request(
            Method::POST,
            "/api/v1/admin/login",
            Some(json!({
                "username": "admin",
                "password": "admin-pass-123"
            })),
            None,
            None,
        )
        .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_body(limited).await["code"], "ADMIN_LOGIN_RATE_LIMITED");
}

#[tokio::test]
async fn private_device_can_connect_heartbeat_and_disconnect() {
    let app = TestApp::new();
    initialize_and_login(&app).await;

    let connect = app
        .request(
            Method::POST,
            "/api/v1/private/connect",
            Some(json!({
                "networkName": "家庭网络",
                "networkPassword": "network-pass-123",
                "clientDeviceId": "desktop-stable-id",
                "deviceName": "书房电脑",
                "platform": "macos",
                "clientVersion": "0.2.0"
            })),
            None,
            None,
        )
        .await;
    assert_eq!(connect.status(), StatusCode::OK);
    let connect = json_body(connect).await;
    let token = connect["sessionToken"].as_str().unwrap();
    assert_eq!(
        connect["network"]["peers"][0],
        "tcp://vpn.example.com:11010"
    );
    assert!(connect["network"]["credential"]
        .as_str()
        .unwrap()
        .contains("test-credential"));

    let heartbeat = app
        .request(
            Method::POST,
            "/api/v1/private/heartbeat",
            Some(json!({
                "virtualIp": "10.10.10.2",
                "protocol": "tcp",
                "latencyMs": 21,
                "rxBytes": 100,
                "txBytes": 200
            })),
            None,
            Some(token),
        )
        .await;
    assert_eq!(heartbeat.status(), StatusCode::OK);

    let disconnect = app
        .request(
            Method::POST,
            "/api/v1/private/disconnect",
            Some(json!({})),
            None,
            Some(token),
        )
        .await;
    assert_eq!(disconnect.status(), StatusCode::OK);

    let expired = app
        .request(
            Method::POST,
            "/api/v1/private/heartbeat",
            Some(json!({})),
            None,
            Some(token),
        )
        .await;
    assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoked_membership_cannot_rejoin_until_deleted() {
    let app = TestApp::new();
    let cookie = initialize_and_login(&app).await;
    let connect_payload = json!({
        "networkName": "家庭网络",
        "networkPassword": "network-pass-123",
        "clientDeviceId": "revoked-device",
        "deviceName": "客厅电脑",
        "platform": "windows",
        "clientVersion": "0.2.0"
    });
    let connect = app
        .request(
            Method::POST,
            "/api/v1/private/connect",
            Some(connect_payload.clone()),
            None,
            None,
        )
        .await;
    assert_eq!(connect.status(), StatusCode::OK);
    let membership_id = json_body(connect).await["membershipId"]
        .as_str()
        .unwrap()
        .to_string();

    let revoke = app
        .request(
            Method::POST,
            &format!("/api/v1/admin/memberships/{membership_id}/revoke"),
            Some(json!({})),
            Some(&cookie),
            None,
        )
        .await;
    assert_eq!(revoke.status(), StatusCode::OK);

    let reconnect = app
        .request(
            Method::POST,
            "/api/v1/private/connect",
            Some(connect_payload),
            None,
            None,
        )
        .await;
    assert_eq!(reconnect.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(reconnect).await["code"], "DEVICE_REVOKED");
}
