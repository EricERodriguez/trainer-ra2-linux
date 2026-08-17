pub mod cheats;
pub mod instant_build;
pub mod process;
pub mod ptrace_mem;

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(serde::Serialize)]
struct CheatMeta {
    id: String,
    name: String,
    description: String,
    hotkey: String,
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

/// PID of the game process the UI currently has selected, kept in sync via
/// `set_active_pid` so global-shortcut handlers (which don't go through the
/// frontend) know which process to act on.
#[derive(Default)]
struct ActivePid(Mutex<Option<i32>>);

/// Mirrors instant-build's on/off state so the hotkey handler knows which
/// way to flip it without a round trip to the frontend.
#[derive(Default)]
struct InstantBuildEnabled(Mutex<bool>);

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
        .map(|c| CheatMeta {
            id: c.id.to_string(),
            name: c.name.to_string(),
            description: c.description.to_string(),
            hotkey: c.hotkey.to_string(),
        })
        .collect()
}

#[tauri::command]
fn refresh_status(pid: i32, helper: tauri::State<HelperState>) -> Result<Vec<cheats::CheatStatus>, String> {
    let request = serde_json::json!({ "cmd": "status", "pid": pid });
    let value = send_request(&helper, &request)?;
    unwrap_helper_result(value)
}

fn do_apply_cheat(pid: i32, cheat_id: &str, helper: &HelperState) -> Result<cheats::CheatStatus, String> {
    let request = serde_json::json!({ "cmd": "apply", "pid": pid, "cheat_id": cheat_id });
    let value = send_request(helper, &request)?;
    unwrap_helper_result(value)
}

#[tauri::command]
fn apply_cheat(
    pid: i32,
    cheat_id: String,
    helper: tauri::State<HelperState>,
) -> Result<cheats::CheatStatus, String> {
    do_apply_cheat(pid, &cheat_id, &helper)
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct InstantBuildStatus {
    enabled: bool,
}

fn do_toggle_instant_build(pid: i32, enabled: bool, helper: &HelperState) -> Result<InstantBuildStatus, String> {
    let request = serde_json::json!({ "cmd": "toggle_instant_build", "pid": pid, "enabled": enabled });
    let value = send_request(helper, &request)?;
    unwrap_helper_result(value)
}

#[tauri::command]
fn toggle_instant_build(
    pid: i32,
    enabled: bool,
    helper: tauri::State<HelperState>,
    instant_build_enabled: tauri::State<InstantBuildEnabled>,
) -> Result<InstantBuildStatus, String> {
    let status = do_toggle_instant_build(pid, enabled, &helper)?;
    *instant_build_enabled.0.lock().expect("el mutex no deberia envenenarse") = status.enabled;
    Ok(status)
}

/// Records which process the UI has selected (or `None` while none is
/// detected), and resets the instant-build flag to match the frontend's own
/// reset-on-process-change behavior, so a hotkey pressed right after
/// switching processes doesn't act on the previous one.
#[tauri::command]
fn set_active_pid(
    pid: Option<i32>,
    active_pid: tauri::State<ActivePid>,
    instant_build_enabled: tauri::State<InstantBuildEnabled>,
) {
    *active_pid.0.lock().expect("el mutex no deberia envenenarse") = pid;
    *instant_build_enabled.0.lock().expect("el mutex no deberia envenenarse") = false;
}

#[tauri::command]
fn instant_build_hotkey() -> &'static str {
    cheats::INSTANT_BUILD_HOTKEY
}

/// Common lookup used by every hotkey handler: bails out with a
/// `hotkey-error` event if no game process is currently selected.
fn active_pid_or_notify(app: &tauri::AppHandle) -> Option<i32> {
    let pid = *app.state::<ActivePid>().0.lock().expect("el mutex no deberia envenenarse");
    if pid.is_none() {
        let _ = app.emit("hotkey-error", "No hay ningun proceso del juego detectado todavia.".to_string());
    }
    pid
}

fn handle_cheat_hotkey(app: &tauri::AppHandle, cheat_id: &str) {
    let Some(pid) = active_pid_or_notify(app) else { return };
    let helper = app.state::<HelperState>();
    match do_apply_cheat(pid, cheat_id, &helper) {
        Ok(status) => {
            let _ = app.emit("cheat-status-changed", status);
        }
        Err(e) => {
            let _ = app.emit("hotkey-error", e);
        }
    }
}

fn handle_instant_build_hotkey(app: &tauri::AppHandle) {
    let Some(pid) = active_pid_or_notify(app) else { return };
    let instant_build_enabled = app.state::<InstantBuildEnabled>();
    let next = !*instant_build_enabled.0.lock().expect("el mutex no deberia envenenarse");
    let helper = app.state::<HelperState>();
    match do_toggle_instant_build(pid, next, &helper) {
        Ok(status) => {
            *instant_build_enabled.0.lock().expect("el mutex no deberia envenenarse") = status.enabled;
            let _ = app.emit("instant-build-changed", status);
        }
        Err(e) => {
            let _ = app.emit("hotkey-error", e);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(HelperState::default())
        .manage(ActivePid::default())
        .manage(InstantBuildEnabled::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let gs = app.handle().global_shortcut();
            for cheat in cheats::CHEATS {
                let cheat_id = cheat.id.to_string();
                gs.on_shortcut(cheat.hotkey, move |app_handle, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        handle_cheat_hotkey(app_handle, &cheat_id);
                    }
                })?;
            }
            gs.on_shortcut(cheats::INSTANT_BUILD_HOTKEY, |app_handle, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    handle_instant_build_hotkey(app_handle);
                }
            })?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_process,
            resolve_pid,
            get_cheats,
            refresh_status,
            apply_cheat,
            toggle_instant_build,
            set_active_pid,
            instant_build_hotkey,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        // Make sure the elevated helper doesn't outlive the app. Dropping
        // its stdin (rather than an immediate kill) lets `serve()`'s loop
        // see EOF and run its own cleanup first: if instant-build is
        // active, that's what restores the breakpoint byte it swapped into
        // the game process before detaching. A raw kill() would skip that
        // and leave the game with a permanent trap instruction in its code.
        if let tauri::RunEvent::Exit = event {
            let state = app_handle.state::<HelperState>();
            let taken = state.0.lock().expect("el mutex del helper no deberia envenenarse").take();
            if let Some(mut handle) = taken {
                drop(handle.stdin);
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    match handle.child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) if std::time::Instant::now() < deadline => {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        _ => {
                            let _ = handle.child.kill();
                            break;
                        }
                    }
                }
            }
        }
    });
}
