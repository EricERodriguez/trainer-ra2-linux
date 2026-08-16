use serde::Serialize;
use std::fs;

const KNOWN_PROCESS_NAMES: &[&str] = &["game.exe", "gamemd.exe"];

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: i32,
    pub name: String,
}

/// Scans /proc for a running game.exe or gamemd.exe, the same way the
/// original scripts' `ps -A -o pid -o comm=` + regex did. Read-only, no
/// ptrace involved.
pub fn detect() -> Option<ProcessInfo> {
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        if KNOWN_PROCESS_NAMES.contains(&comm) {
            return Some(ProcessInfo { pid, name: comm.to_string() });
        }
    }
    None
}

/// Looks up the process name (comm) for a manually-entered PID, so the UI
/// can validate/label it the same way as an auto-detected one.
pub fn name_for_pid(pid: i32) -> Option<String> {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim().to_string())
}
