#![windows_subsystem = "windows"]

use std::sync::Arc;

pub mod config;
pub mod event_bus;
pub mod hotkeys;
pub mod logger;
pub mod matchers;
pub mod overlay;
pub mod shop;
pub mod tray;

use config::new_shared_config;
use hotkeys::spawn_hotkey_listener;
use overlay::draw_list::new_draw_list;

use overlay::run_overlay;
use shop::{ShopAssets, start_detection_loop};

fn main() {
    let _ = logger::init();
    println!("[main] starting Moonlighter Overlay...");

    let config = new_shared_config();
    {
        let cfg = config.lock().unwrap();
        logger::set_debug(cfg.debug_mode);
    }

    println!("[main] loading assets...");
    let assets = match ShopAssets::load() {
        Ok(a) => Arc::new(a),
        Err(e) => {
            eprintln!("[main] failed to load embedded assets: {e:#}");
            std::process::exit(1);
        }
    };
    println!("[main] assets loaded successfully.");

    let draw_list = new_draw_list();
    let (cmd_tx, cmd_rx) = event_bus::create_command_bus();

    tray::spawn_tray_icon(Arc::clone(&config), Arc::clone(&draw_list), cmd_tx.clone());

    spawn_hotkey_listener(
        Arc::clone(&config),
        Arc::clone(&draw_list),
        Arc::clone(&assets),
        cmd_tx.clone(),
    );

    let dl_detect = Arc::clone(&draw_list);
    let cfg_detect = Arc::clone(&config);
    let assets_detect = Arc::clone(&assets);
    std::thread::spawn(move || {
        start_detection_loop(dl_detect, cfg_detect, assets_detect);
    });

    run_overlay(draw_list, config, Arc::clone(&assets), cmd_rx);
}
