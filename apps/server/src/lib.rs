pub mod api;
pub mod config;
pub mod crypto;
pub mod db;
pub mod easytier;
pub mod models;
pub mod state;

use axum::{
    http::{header, HeaderValue, Method},
    Router,
};
use state::AppState;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

pub fn app(state: AppState) -> Router {
    let index = state.config.web_dir.join("index.html");
    let static_files = ServeDir::new(&state.config.web_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index));
    let desktop_api_cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("*"))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);
    api::router(state)
        .layer(desktop_api_cors)
        .fallback_service(static_files)
}
