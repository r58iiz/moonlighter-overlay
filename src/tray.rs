use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, GWLP_USERDATA,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, IDI_APPLICATION, InsertMenuItemW, LoadIconW,
    MENUITEMINFOW, MFS_CHECKED, MFS_UNCHECKED, MFT_SEPARATOR, MFT_STRING, MIIM_FTYPE, MIIM_ID,
    MIIM_STATE, MIIM_STRING, PostMessageW, RegisterClassW, SetForegroundWindow, SetWindowLongPtrW,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
    WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

use crate::config::SharedConfig;
use crate::event_bus::AppCommand;
use crate::overlay::draw_list::{OverlayMode, SharedDrawList};
use std::sync::mpsc::Sender;

const WM_TRAYICON: u32 = WM_USER + 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TrayMenuItem {
    Pause = 1001,
    Search = 1002,
    Marking = 1003,
    NgPlus = 1004,
    Redetect = 1005,
    Exit = 1006,
    Debug = 1007,
    Dump = 1008,
}

impl TryFrom<u32> for TrayMenuItem {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1001 => Ok(TrayMenuItem::Pause),
            1002 => Ok(TrayMenuItem::Search),
            1003 => Ok(TrayMenuItem::Marking),
            1004 => Ok(TrayMenuItem::NgPlus),
            1005 => Ok(TrayMenuItem::Redetect),
            1006 => Ok(TrayMenuItem::Exit),
            1007 => Ok(TrayMenuItem::Debug),
            1008 => Ok(TrayMenuItem::Dump),
            _ => Err(()),
        }
    }
}

impl TrayMenuItem {
    pub fn to_app_command(self) -> AppCommand {
        match self {
            TrayMenuItem::Pause => AppCommand::ToggleOverlayMode(OverlayMode::Paused),
            TrayMenuItem::Search => AppCommand::ToggleOverlayMode(OverlayMode::Search),
            TrayMenuItem::Marking => AppCommand::ToggleOverlayMode(OverlayMode::Marking),
            TrayMenuItem::Debug => AppCommand::ToggleDebugMode,
            TrayMenuItem::NgPlus => AppCommand::ToggleNgPlusMode,
            TrayMenuItem::Redetect => AppCommand::RequestRedetect,
            TrayMenuItem::Dump => AppCommand::DumpTemplates,
            TrayMenuItem::Exit => AppCommand::Exit,
        }
    }
}

struct TrayState {
    config: SharedConfig,
    draw_list: SharedDrawList,
    cmd_tx: Sender<AppCommand>,
}

fn to_wstring(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn format_hotkey_label(leader: &str, key: &str) -> String {
    let leader = leader.trim().to_uppercase();
    let key = key.trim().to_uppercase();
    if leader.is_empty() {
        key
    } else {
        format!("{leader}+{key}")
    }
}

pub fn spawn_tray_icon(
    config: SharedConfig,
    draw_list: SharedDrawList,
    cmd_tx: Sender<AppCommand>,
) {
    std::thread::spawn(move || unsafe {
        let class_name = to_wstring("MoonlighterOverlayTrayClass");
        let window_title = to_wstring("Moonlighter Tray");

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE(std::ptr::null_mut()),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        RegisterClassW(&wnd_class);

        let state = Box::new(TrayState {
            config,
            draw_list,
            cmd_tx,
        });

        let state_ptr = Box::into_raw(state);

        let hwnd_res = CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            Some(state_ptr as *const _),
        );

        let hwnd = match hwnd_res {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[tray] failed to create tray window: {e}");
                let _ = Box::from_raw(state_ptr);
                return;
            }
        };

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        let hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = hicon;

        let tip = "Moonlighter Overlay";
        let tip_u16 = to_wstring(tip);
        let len = tip_u16.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip_u16[..len]);

        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        println!("[tray] system tray icon created");

        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        let _ = Box::from_raw(state_ptr);
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;

        match msg {
            WM_TRAYICON => {
                let event = lparam.0 as u32;
                if (event == WM_RBUTTONUP || event == WM_LBUTTONUP || event == WM_CONTEXTMENU)
                    && !ptr.is_null()
                {
                    show_context_menu(hwnd, &*ptr);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let command_id = (wparam.0 & 0xFFFF) as u32;
                if !ptr.is_null() {
                    handle_menu_command(&*ptr, command_id);
                }
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn show_context_menu(hwnd: HWND, state: &TrayState) {
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return;
        }

        let hmenu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };

        let (
            mode,
            is_debug,
            is_ng,
            leader,
            mark_hk,
            debug_hk,
            detect_hk,
            exit_hk,
            ng_hk,
            dump_hk,
            search_hk,
            pause_hk,
        ) = {
            let dl = state.draw_list.lock().unwrap();
            let cfg = state.config.lock().unwrap();
            (
                dl.mode,
                cfg.debug_mode,
                cfg.ng_plus_mode,
                cfg.leader_key.clone(),
                cfg.mark_region_hotkey.clone(),
                cfg.toggle_debug_hotkey.clone(),
                cfg.detect_window_hotkey.clone(),
                cfg.exit_app_hotkey.clone(),
                cfg.toggle_ng_plus_hotkey.clone(),
                cfg.dump_templates_hotkey.clone(),
                cfg.manual_search_hotkey.clone(),
                cfg.pause_overlay_hotkey.clone(),
            )
        };

        let is_paused = mode == OverlayMode::Paused;
        let is_search = mode == OverlayMode::Search;
        let is_marking = mode == OverlayMode::Marking;

        let hk_search = format_hotkey_label(&leader, &search_hk);
        let hk_marking = format_hotkey_label(&leader, &mark_hk);
        let hk_debug = format_hotkey_label(&leader, &debug_hk);
        let hk_ng = format_hotkey_label(&leader, &ng_hk);
        let hk_redetect = format_hotkey_label(&leader, &detect_hk);
        let hk_dump = format_hotkey_label(&leader, &dump_hk);
        let hk_exit = format_hotkey_label(&leader, &exit_hk);
        let hk_pause = format_hotkey_label(&leader, &pause_hk);

        let pause_label = if is_paused {
            to_wstring(&format!("Resume Overlay ({hk_pause})"))
        } else {
            to_wstring(&format!("Pause Overlay ({hk_pause})"))
        };

        let search_label = to_wstring(&format!("Search Items ({hk_search})"));
        let marking_label = to_wstring(&format!("Mark Slots ({hk_marking})"));
        let debug_label = if is_debug {
            to_wstring(&format!("Debug Mode [ON] ({hk_debug})"))
        } else {
            to_wstring(&format!("Debug Mode [OFF] ({hk_debug})"))
        };
        let ng_label = if is_ng {
            to_wstring(&format!("NG+ Mode [ON] ({hk_ng})"))
        } else {
            to_wstring(&format!("NG+ Mode [OFF] ({hk_ng})"))
        };
        let redetect_label = to_wstring(&format!("Redetect Window ({hk_redetect})"));
        let dump_label = to_wstring(&format!("Dump Asset Templates ({hk_dump})"));
        let exit_label = to_wstring(&format!("Exit Application ({hk_exit})"));

        add_menu_item(hmenu, TrayMenuItem::Pause, &pause_label, is_paused);
        add_menu_item(hmenu, TrayMenuItem::Search, &search_label, is_search);
        add_menu_item(hmenu, TrayMenuItem::Marking, &marking_label, is_marking);
        add_menu_item(hmenu, TrayMenuItem::Debug, &debug_label, is_debug);
        add_menu_item(hmenu, TrayMenuItem::NgPlus, &ng_label, is_ng);
        add_menu_item(hmenu, TrayMenuItem::Redetect, &redetect_label, false);
        add_menu_item(hmenu, TrayMenuItem::Dump, &dump_label, false);
        add_separator(hmenu);
        add_menu_item(hmenu, TrayMenuItem::Exit, &exit_label, false);

        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(hmenu, TPM_RIGHTBUTTON, pt.x, pt.y, None, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(hmenu);
    }
}

unsafe fn add_menu_item(
    hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    item: TrayMenuItem,
    label: &[u16],
    checked: bool,
) {
    let id = item as u32;
    let state_flags = if checked { MFS_CHECKED } else { MFS_UNCHECKED };
    let menu_item = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_ID | MIIM_STRING | MIIM_STATE | MIIM_FTYPE,
        fType: MFT_STRING,
        fState: state_flags,
        wID: id,
        dwTypeData: windows::core::PWSTR(label.as_ptr() as *mut _),
        cch: (label.len() - 1) as u32,
        ..Default::default()
    };
    unsafe {
        let _ = InsertMenuItemW(hmenu, id, false, &menu_item);
    }
}

unsafe fn add_separator(hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU) {
    let item = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE,
        fType: MFT_SEPARATOR,
        ..Default::default()
    };
    unsafe {
        let _ = InsertMenuItemW(hmenu, 9999, false, &item);
    }
}

fn handle_menu_command(state: &TrayState, id: u32) {
    if let Ok(item) = TrayMenuItem::try_from(id) {
        let cmd = item.to_app_command();
        let _ = state.cmd_tx.send(cmd);
    }
}
