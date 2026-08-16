pub mod cheats;
pub mod process;
pub mod ptrace_mem;

use std::path::PathBuf;

#[derive(serde::Serialize)]
struct CheatMeta {
    id: String,
    name: String,
    description: String,
}

fn helper_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no se pudo resolver el directorio del ejecutable actual".to_string())?;
    let name = if cfg!(windows) { "ra2-trainer-helper.exe" } else { "ra2-trainer-helper" };
    let candidate = dir.join(name);
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(format!(
            "no se encontro el binario helper en {} (¿se compilo `cargo build` completo?)",
            candidate.display()
        ))
    }
}

/// Runs the privileged helper via pkexec and parses its `{"ok": bool, ...}`
/// stdout envelope. A non-zero pkexec exit means the auth dialog was
/// cancelled/denied, not that the underlying operation failed.
fn run_helper(args: &[&str]) -> Result<serde_json::Value, String> {
    let helper = helper_path()?;
    let output = std::process::Command::new("pkexec")
        .arg(helper)
        .args(args)
        .output()
        .map_err(|e| format!("no se pudo ejecutar pkexec: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "autenticacion cancelada o no autorizada (pkexec salio con codigo {:?})",
            output.status.code()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| format!("respuesta invalida del helper: {e} (salida: {stdout})"))
}

fn unwrap_helper_result<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if ok {
        let data = value.get("data").cloned().unwrap_or(serde_json::Value::Null);
        serde_json::from_value(data).map_err(|e| format!("respuesta invalida del helper: {e}"))
    } else {
        let msg = value.get("error").and_then(|v| v.as_str()).unwrap_or("error desconocido del helper");
        Err(msg.to_string())
    }
}

#[tauri::command]
fn detect_process() -> Option<process::ProcessInfo> {
    process::detect()
}

#[tauri::command]
fn resolve_pid(pid: i32) -> Option<process::ProcessInfo> {
    process::name_for_pid(pid).map(|name| process::ProcessInfo { pid, name })
}

#[tauri::command]
fn get_cheats() -> Vec<CheatMeta> {
    cheats::CHEATS
        .iter()
        .map(|c| CheatMeta { id: c.id.to_string(), name: c.name.to_string(), description: c.description.to_string() })
        .collect()
}

#[tauri::command]
fn refresh_status(pid: i32) -> Result<Vec<cheats::CheatStatus>, String> {
    let pid_str = pid.to_string();
    let value = run_helper(&["status", "--pid", &pid_str])?;
    unwrap_helper_result(value)
}

#[tauri::command]
fn apply_cheat(pid: i32, cheat_id: String) -> Result<cheats::CheatStatus, String> {
    let pid_str = pid.to_string();
    let value = run_helper(&["apply", "--pid", &pid_str, "--cheat", &cheat_id])?;
    unwrap_helper_result(value)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_process,
            resolve_pid,
            get_cheats,
            refresh_status,
            apply_cheat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
