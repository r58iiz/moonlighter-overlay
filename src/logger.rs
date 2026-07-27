use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle};

pub const LOG_FILE_PATH: &str = "moonlighter_overlay.log";

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_debug() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
#[allow(non_snake_case)]
struct Systemtime {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}

#[repr(C)]
#[allow(non_snake_case)]
struct SECURITY_ATTRIBUTES {
    nLength: u32,
    lpSecurityDescriptor: *mut std::ffi::c_void,
    bInheritHandle: i32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(lpSystemTime: *mut Systemtime);
    fn CreatePipe(
        hReadPipe: *mut *mut std::ffi::c_void,
        hWritePipe: *mut *mut std::ffi::c_void,
        lpPipeAttributes: *const SECURITY_ATTRIBUTES,
        nSize: u32,
    ) -> i32;
}

/// YYYY-MM-DD HH:MM:SS:MS
pub fn get_timestamp() -> String {
    let mut st = Systemtime::default();
    unsafe {
        GetLocalTime(&mut st);
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}:{:03}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    )
}

pub fn init() -> anyhow::Result<()> {
    let mut read_raw: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut write_raw: *mut std::ffi::c_void = std::ptr::null_mut();

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };

    unsafe {
        if CreatePipe(&mut read_raw, &mut write_raw, &sa, 0) == 0 {
            anyhow::bail!("failed to create pipe for logging");
        }

        let write_handle = HANDLE(write_raw);
        let _ = SetStdHandle(STD_OUTPUT_HANDLE, write_handle);
        let _ = SetStdHandle(STD_ERROR_HANDLE, write_handle);
    }

    let read_file = unsafe { std::fs::File::from_raw_handle(read_raw) };

    std::thread::spawn(move || {
        let mut log_file: Option<std::fs::File> = None;

        let reader = BufReader::new(read_file);
        for content in reader.lines().map_while(Result::ok) {
            if !is_debug() {
                continue;
            }
            if content.trim().is_empty() {
                continue;
            }

            if log_file.is_none() {
                match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(LOG_FILE_PATH)
                {
                    Ok(f) => log_file = Some(f),
                    Err(e) => {
                        eprintln!("[logger] failed to open log file: {e}");
                        continue;
                    }
                }
            }

            if let Some(ref mut f) = log_file {
                let ts = get_timestamp();
                let log_entry = format!("[{ts}] {content}\n");
                let _ = f.write_all(log_entry.as_bytes());
                let _ = f.flush();
            }
        }
    });

    println!("[logger] initialized file logging with timestamps to {LOG_FILE_PATH}");
    Ok(())
}

pub fn log(msg: &str) {
    println!("{msg}");
}
