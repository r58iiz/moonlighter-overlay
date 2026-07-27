use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_F1, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_F10,
    VK_F11, VK_F12,
};

use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

use crate::config::SharedConfig;
use crate::overlay::draw_list::{OverlayMode, SearchResult, SharedDrawList};
use crate::shop::assets::ShopAssets;

use crate::config::TitleMatcher;

pub fn is_target_foreground_matcher(matcher: &TitleMatcher) -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return false;
        }
        let title = String::from_utf16_lossy(&buf[..len as usize]);
        matcher.is_match(&title)
    }
}

pub fn is_target_foreground(target_title: &str) -> bool {
    let matcher = TitleMatcher::new(target_title);
    is_target_foreground_matcher(&matcher)
}

fn parse_hotkey_vk(name: &str) -> i32 {
    let s = name.trim().to_uppercase();
    if s.is_empty() || s == "NONE" || s == "0" || s == "OFF" {
        return -1;
    }
    match s.as_str() {
        "F1" => VK_F1.0 as i32,
        "F2" => VK_F2.0 as i32,
        "F3" => VK_F3.0 as i32,
        "F4" => VK_F4.0 as i32,
        "F5" => VK_F5.0 as i32,
        "F6" => VK_F6.0 as i32,
        "F7" => VK_F7.0 as i32,
        "F8" => VK_F8.0 as i32,
        "F9" => VK_F9.0 as i32,
        "F10" => VK_F10.0 as i32,
        "F11" => VK_F11.0 as i32,
        "F12" => VK_F12.0 as i32,
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "ALT" => 0x12,
        "CTRL" | "CONTROL" => 0x11,
        "SHIFT" => 0x10,
        _ => {
            if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphanumeric() {
                    return ch as i32;
                }
            }
            -1
        }
    }
}

fn check_hotkey_trigger(
    target_vk: i32,
    leader_vk: i32,
    now: Instant,
    last_trigger: &mut Instant,
    debounce_dur: Duration,
) -> bool {
    if target_vk <= 0 {
        return false;
    }

    let is_pressed = unsafe { (GetAsyncKeyState(target_vk) as u16 & 0x8000) != 0 };
    if !is_pressed {
        return false;
    }

    let is_f_key = (0x70..=0x7B).contains(&target_vk);
    if !is_f_key && leader_vk > 0 {
        let is_leader_pressed = unsafe { (GetAsyncKeyState(leader_vk) as u16 & 0x8000) != 0 };
        if !is_leader_pressed {
            return false;
        }
    }

    if now.duration_since(*last_trigger) >= debounce_dur {
        *last_trigger = now;
        true
    } else {
        false
    }
}

fn key_just_pressed(vk: i32, prev_keys: &mut [bool; 256]) -> bool {
    let idx = vk as usize;
    if idx >= 256 {
        return false;
    }
    let pressed = unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
    let was_pressed = prev_keys[idx];
    prev_keys[idx] = pressed;
    pressed && !was_pressed
}

use crate::event_bus::AppCommand;

use std::sync::mpsc::Sender;

pub fn spawn_hotkey_listener(
    config: SharedConfig,
    draw_list: SharedDrawList,
    assets: Arc<ShopAssets>,
    cmd_tx: Sender<AppCommand>,
) {
    std::thread::spawn(move || {
        let mut last_mark_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_debug_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_detect_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_exit_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_ng_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_dump_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_search_trigger = Instant::now() - Duration::from_secs(10);
        let mut last_pause_trigger = Instant::now() - Duration::from_secs(10);
        let mut prev_keys = [false; 256];
        let mut cached_title = String::new();
        let mut cached_matcher = TitleMatcher::new("");

        loop {
            let target_title = {
                let cfg = config.lock().unwrap();
                cfg.target_window_title.clone()
            };

            if cached_title != target_title {
                cached_matcher = TitleMatcher::new(&target_title);
                cached_title = target_title;
            }

            if !is_target_foreground_matcher(&cached_matcher) {
                prev_keys = [false; 256];
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            let (
                leader_vk,
                mark_vk,
                debug_vk,
                detect_vk,
                exit_vk,
                ng_vk,
                dump_vk,
                search_vk,
                pause_vk,
                debounce_ms,
            ) = {
                let cfg = config.lock().unwrap();
                (
                    parse_hotkey_vk(&cfg.leader_key),
                    parse_hotkey_vk(&cfg.mark_region_hotkey),
                    parse_hotkey_vk(&cfg.toggle_debug_hotkey),
                    parse_hotkey_vk(&cfg.detect_window_hotkey),
                    parse_hotkey_vk(&cfg.exit_app_hotkey),
                    parse_hotkey_vk(&cfg.toggle_ng_plus_hotkey),
                    parse_hotkey_vk(&cfg.dump_templates_hotkey),
                    parse_hotkey_vk(&cfg.manual_search_hotkey),
                    parse_hotkey_vk(&cfg.pause_overlay_hotkey),
                    cfg.debounce_ms,
                )
            };

            let debounce_dur = Duration::from_millis(debounce_ms);
            let now = Instant::now();

            if check_hotkey_trigger(
                pause_vk,
                leader_vk,
                now,
                &mut last_pause_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::ToggleOverlayMode(OverlayMode::Paused));
            }

            if check_hotkey_trigger(
                mark_vk,
                leader_vk,
                now,
                &mut last_mark_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::ToggleOverlayMode(OverlayMode::Marking));
            }

            if check_hotkey_trigger(
                debug_vk,
                leader_vk,
                now,
                &mut last_debug_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::ToggleDebugMode);
            }

            if check_hotkey_trigger(
                detect_vk,
                leader_vk,
                now,
                &mut last_detect_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::RequestRedetect);
            }

            if check_hotkey_trigger(
                exit_vk,
                leader_vk,
                now,
                &mut last_exit_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::Exit);
            }

            if check_hotkey_trigger(ng_vk, leader_vk, now, &mut last_ng_trigger, debounce_dur) {
                let _ = cmd_tx.send(AppCommand::ToggleNgPlusMode);
            }

            if check_hotkey_trigger(
                dump_vk,
                leader_vk,
                now,
                &mut last_dump_trigger,
                debounce_dur,
            ) {
                let _ = cmd_tx.send(AppCommand::DumpTemplates);
            }

            if check_hotkey_trigger(
                search_vk,
                leader_vk,
                now,
                &mut last_search_trigger,
                debounce_dur,
            ) {
                for vk in 0..256 {
                    prev_keys[vk as usize] = unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
                }
                let _ = cmd_tx.send(AppCommand::ToggleOverlayMode(OverlayMode::Search));
            }

            let is_search = {
                let dl = draw_list.lock().unwrap();
                dl.mode == OverlayMode::Search
            };

            if is_search {
                handle_search_input(&draw_list, &assets, &config, &mut prev_keys);
            } else {
                prev_keys = [false; 256];
            }

            std::thread::sleep(Duration::from_millis(30));
        }
    });
}

fn handle_search_input(
    draw_list: &SharedDrawList,
    assets: &ShopAssets,
    config: &SharedConfig,
    prev_keys: &mut [bool; 256],
) {
    let mut chars_to_add = String::new();
    let mut backspace = false;
    let mut close = false;

    if key_just_pressed(0x1B, prev_keys) {
        close = true;
    }

    if key_just_pressed(0x08, prev_keys) {
        backspace = true;
    }

    if key_just_pressed(0x20, prev_keys) {
        chars_to_add.push(' ');
    }

    for vk in 0x41i32..=0x5A {
        if key_just_pressed(vk, prev_keys) {
            chars_to_add.push((vk as u8 - 0x41 + b'a') as char);
        }
    }

    for vk in 0x30i32..=0x39 {
        if key_just_pressed(vk, prev_keys) {
            chars_to_add.push(vk as u8 as char);
        }
    }

    if close {
        let mut dl = draw_list.lock().unwrap();
        dl.search_query.clear();
        dl.search_results.clear();
        dl.mode = OverlayMode::Passive;
        return;
    }

    if backspace || !chars_to_add.is_empty() {
        let is_ng_plus = config.lock().unwrap().ng_plus_mode;
        let mut dl = draw_list.lock().unwrap();
        if backspace {
            dl.search_query.pop();
        }
        dl.search_query.push_str(&chars_to_add);
        let query = dl.search_query.clone();
        dl.search_results = assets
            .price_table
            .search(&query, 20)
            .into_iter()
            .map(|entry| SearchResult {
                name: entry.name.clone(),
                prices: entry.prices.clone(),
                ng_plus_prices: entry.ng_plus_prices.clone(),
            })
            .collect();
        dl.is_ng_plus = is_ng_plus;
    }
}
