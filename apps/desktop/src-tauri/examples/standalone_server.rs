use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(7879);
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = std::env::temp_dir().join("tong-net-browser-smoke");
    let web_root = manifest.join("../dist");
    println!("http://127.0.0.1:{port}/?token=smoke-token#/web");
    tong_net_desktop_lib::run_standalone(port, data_dir, web_root, "smoke-token".into())
        .await
        .unwrap();
}
