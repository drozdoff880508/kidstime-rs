#![cfg(windows)]

#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    },
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        LockWorkStation,
    },
};

pub struct ActiveWindowInfo {
    pub window: String,
    pub process: String,
}

#[cfg(windows)]
pub fn lock_screen() {
    unsafe {
        let _ = LockWorkStation();
    }
}

#[cfg(not(windows))]
pub fn lock_screen() {
    eprintln!("[lock_screen] Not supported on this platform");
}

#[cfg(windows)]
pub fn get_active_window_info() -> ActiveWindowInfo {
    let mut result = ActiveWindowInfo {
        window: "N/A".to_string(),
        process: "N/A".to_string(),
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return result;
        }

        // Window title
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            GetWindowTextW(hwnd, &mut buf);
            if let Some(s) = String::from_utf16(&buf) {
                result.window = s.trim_end_matches('\0').to_string();
            }
        }

        // Process name via PID + ToolHelp
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid > 0 {
            result.process = get_process_name_by_pid(pid);
        }
    }

    result
}

#[cfg(not(windows))]
pub fn get_active_window_info() -> ActiveWindowInfo {
    ActiveWindowInfo {
        window: "N/A".to_string(),
        process: "N/A".to_string(),
    }
}

#[cfg(windows)]
fn get_process_name_by_pid(target_pid: u32) -> String {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return "Unknown".to_string(),
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut found = String::new();

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID == target_pid {
                    if let Some(s) = String::from_utf16(
                        &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                    ) {
                        found = s;
                    }
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        if found.is_empty() { "Unknown".to_string() } else { found }
    }
}
