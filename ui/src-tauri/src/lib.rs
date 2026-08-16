pub mod cheats;
pub mod process;
pub mod ptrace_mem;

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use tauri::Manager;

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

/// A long-lived `ra2-trainer-helper serve` child, started once (elevated via
/// pkexec) and reused for every status/apply request via a JSON-lines
/// stdin/stdout protocol. This is what keeps pkexec's auth prompt to once
/// per app session instead of once per button click.
struct HelperHandle {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Default)]
struct HelperState(Mutex<Option<HelperHandle>>);

fn spawn_helper() -> Result<HelperHandle, String> {
    let helper = helper_path()?;
    let mut child = Command::new("pkexec")
        .arg(helper)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar pkexec: {e}"))?;
    let stdin = child.stdin.take().expect("stdin fue configurado como piped");
    let stdout = child.stdout.take().expect("stdout fue configurado como piped");
    Ok(HelperHandle { child, stdin, stdout: BufReader::new(stdout) })
}

/// Sends one JSON request line to the persistent helper and reads back one
/// JSON response line, (re)spawning the helper first if it isn't running
/// yet. If the helper's pipe is dead (auth was cancelled/denied, or it
/// crashed), the stale handle is dropped so the *next* call starts a fresh
/// one (and pkexec prompts again) instead of failing forever.
fn send_request(state: &HelperState, request: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut guard = state.0.lock().expect("el mutex del helper no deberia envenenarse");

    if guard.is_none() {
        *guard = Some(spawn_helper()?);
    }
    let handle = guard.as_mut().expect("se acaba de asignar arriba");

    let line = serde_json::to_string(request).map_err(|e| e.to_string())?;
    if writeln!(handle.stdin, "{line}").is_err() || handle.stdin.flush().is_err() {
        *guard = None;
        return Err("el proceso helper no responde; se reiniciara en el proximo intento (puede pedir autenticacion de nuevo)".to_string());
    }

    let mut response_line = String::new();
    match handle.stdout.read_line(&mut response_line) {
        Ok(0) => {
            // EOF: pkexec was cancelled/denied, or the helper crashed.
            let code = handle.child.try_wait().ok().flatten().and_then(|s| s.code());
            *guard = None;
            Err(format!(
                "la autenticacion fue cancelada o el proceso helper se cerro (codigo {code:?}). Volve a intentar la accion."
            ))
        }
        Ok(_) => serde_json::from_str(&response_line)
            .map_err(|e| format!("respuesta invalida del helper: {e} (linea: {response_line:?})")),
        Err(e) => {
            *guard = None;
            Err(format!("error leyendo la respuesta del helper: {e}"))
        }
    }
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
fn refresh_status(pid: i32, helper: tauri::State<HelperState>) -> Result<Vec<cheats::CheatStatus>, String> {
    let request = serde_json::json!({ "cmd": "status", "pid": pid });
    let value = send_request(&helper, &request)?;
    unwrap_helper_result(value)
}

#[tauri::command]
fn apply_cheat(
    pid: i32,
    cheat_id: String,
    helper: tauri::State<HelperState>,
) -> Result<cheats::CheatStatus, String> {
    let request = serde_json::json!({ "cmd": "apply", "pid": pid, "cheat_id": cheat_id });
    let value = send_request(&helper, &request)?;
    unwrap_helper_result(value)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(HelperState::default())
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        // Make sure the elevated helper doesn't outlive the app.
        if let tauri::RunEvent::Exit = event {
            let state = app_handle.state::<HelperState>();
            let taken = state.0.lock().expect("el mutex del helper no deberia envenenarse").take();
            if let Some(mut handle) = taken {
                let _ = handle.child.kill();
            }
        }
    });
}
