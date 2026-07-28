// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = tong_net_desktop_lib::run_easytier_service_from_args() {
        std::process::exit(code);
    }
    tong_net_desktop_lib::run()
}
