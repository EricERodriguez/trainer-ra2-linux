//! Privileged CLI invoked via `pkexec` by the Tauri app to do the actual
//! ptrace attach/read/write against the game process. Kept as a separate,
//! minimal binary so only this part of the app ever runs elevated.
//!
//! Always exits 0 and reports failures through the JSON envelope on stdout
//! (`{"ok":false,"error":"..."}`), so the parent process can distinguish
//! "pkexec auth was cancelled" (non-zero exit, no valid stdout) from
//! "auth succeeded but the operation failed" (exit 0, ok:false).

use app_lib::cheats::{self, CheatStatus};
use app_lib::process;
use app_lib::ptrace_mem::{Attached, MemError};
use serde::Serialize;

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn ok<T: Serialize>(data: T) -> Envelope<T> {
    Envelope { ok: true, data: Some(data), error: None }
}

fn err<T: Serialize>(message: impl std::fmt::Display) -> Envelope<T> {
    Envelope { ok: false, data: None, error: Some(message.to_string()) }
}

fn print_and_exit<T: Serialize>(envelope: Envelope<T>) -> ! {
    println!("{}", serde_json::to_string(&envelope).expect("la respuesta siempre es serializable"));
    std::process::exit(0);
}

fn get_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn run_status(pid: i32) -> Result<Vec<CheatStatus>, String> {
    let process_name = process::name_for_pid(pid).ok_or_else(|| format!("no existe el proceso con pid {pid}"))?;
    let mem = Attached::new(pid).map_err(|e: MemError| e.to_string())?;
    cheats::CHEATS
        .iter()
        .map(|c| cheats::evaluate_cheat(&mem, c, &process_name).map_err(|e| e.to_string()))
        .collect()
}

fn run_apply(pid: i32, cheat_id: &str) -> Result<CheatStatus, String> {
    let process_name = process::name_for_pid(pid).ok_or_else(|| format!("no existe el proceso con pid {pid}"))?;
    let cheat = cheats::find_cheat(cheat_id).ok_or_else(|| format!("cheat desconocido: {cheat_id}"))?;
    let mem = Attached::new(pid).map_err(|e: MemError| e.to_string())?;
    cheats::apply_cheat(&mem, cheat, &process_name).map_err(|e| e.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(subcommand) = args.first() else {
        print_and_exit(err::<()>("uso: ra2-trainer-helper <status|apply> --pid <pid> [--cheat <id>]"));
    };

    let Some(pid) = get_flag(&args, "--pid").and_then(|p| p.parse::<i32>().ok()) else {
        print_and_exit(err::<()>("falta o es invalido --pid <pid>"));
    };

    match subcommand.as_str() {
        "status" => match run_status(pid) {
            Ok(statuses) => print_and_exit(ok(statuses)),
            Err(e) => print_and_exit(err::<()>(e)),
        },
        "apply" => {
            let Some(cheat_id) = get_flag(&args, "--cheat") else {
                print_and_exit(err::<()>("falta --cheat <id>"));
            };
            match run_apply(pid, &cheat_id) {
                Ok(status) => print_and_exit(ok(status)),
                Err(e) => print_and_exit(err::<()>(e)),
            }
        }
        other => print_and_exit(err::<()>(format!("subcomando desconocido: {other}"))),
    }
}
