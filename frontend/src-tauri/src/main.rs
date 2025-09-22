#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use log;
use env_logger;

fn main() {
    std::env::set_var("RUST_LOG", "info,ort::logging=warn");
    env_logger::init();
    log::info!("Starting application...");
    app_lib::run();
}
