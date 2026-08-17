//! Privileged process invoked via `pkexec` by the Tauri app to do the actual
//! ptrace attach/read/write against the game process. Kept as a separate,
//! minimal binary so only this part of the app ever runs elevated.
//!
//! `serve` mode (used by the app) stays resident and handles one JSON
//! request per stdin line for as long as the app keeps it running, so
//! pkexec only has to authenticate once per app session instead of once per
//! button click. `status`/`apply` one-shot subcommands are kept too, for
//! manual debugging from a terminal.
//!
//! Every response is a JSON envelope on stdout
//! (`{"ok":true,"data":...}` / `{"ok":false,"error":"..."}`), never a
//! non-zero exit for an operation failure — only pkexec itself exits
//! non-zero (or closes the pipe) when auth is cancelled/denied, which is how
//! the parent tells "auth failed" apart from "auth ok, operation failed".

use app_lib::cheats::{self, CheatStatus};
use app_lib::instant_build::{self, InstantBuildHandle};
use app_lib::process;
use app_lib::ptrace_mem::{Attached, MemError};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

#[derive(Serialize)]
struct InstantBuildStatus {
    enabled: bool,
}

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

fn envelope_json<T: Serialize>(envelope: Envelope<T>) -> String {
    serde_json::to_string(&envelope).expect("la respuesta siempre es serializable")
}

fn print_and_exit<T: Serialize>(envelope: Envelope<T>) -> ! {
    println!("{}", envelope_json(envelope));
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

fn run_toggle(pid: i32, cheat_id: &str) -> Result<CheatStatus, String> {
    let process_name = process::name_for_pid(pid).ok_or_else(|| format!("no existe el proceso con pid {pid}"))?;
    let cheat = cheats::find_cheat(cheat_id).ok_or_else(|| format!("cheat desconocido: {cheat_id}"))?;
    let mem = Attached::new(pid).map_err(|e: MemError| e.to_string())?;
    cheats::toggle_cheat(&mem, cheat, &process_name).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Status { pid: i32 },
    Toggle { pid: i32, cheat_id: String },
    ToggleInstantBuild { pid: i32, enabled: bool },
}

const INSTANT_BUILD_BUSY: &str =
    "instant-build esta activo; desactivalo para usar otras acciones sobre este proceso";

/// Handles one request line, never panics/exits: any failure (bad JSON,
/// unknown pid, ptrace error, ...) becomes an `ok:false` envelope so the
/// `serve` loop keeps running for the next request.
///
/// `instant_build` is threaded through (rather than global state) because
/// its background thread owns the only ptrace attach allowed on the
/// tracee at a time — status/apply must not race it.
fn handle_line(line: &str, instant_build_state: &mut Option<InstantBuildHandle>) -> String {
    let request: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return envelope_json(err::<()>(format!("pedido invalido: {e}"))),
    };
    match request {
        Request::Status { pid } => {
            if instant_build_active_for(instant_build_state, pid) {
                return envelope_json(err::<()>(INSTANT_BUILD_BUSY));
            }
            match run_status(pid) {
                Ok(statuses) => envelope_json(ok(statuses)),
                Err(e) => envelope_json(err::<()>(e)),
            }
        }
        Request::Toggle { pid, cheat_id } => {
            if instant_build_active_for(instant_build_state, pid) {
                return envelope_json(err::<()>(INSTANT_BUILD_BUSY));
            }
            match run_toggle(pid, &cheat_id) {
                Ok(status) => envelope_json(ok(status)),
                Err(e) => envelope_json(err::<()>(e)),
            }
        }
        Request::ToggleInstantBuild { pid, enabled } => {
            match run_toggle_instant_build(instant_build_state, pid, enabled) {
                Ok(status) => envelope_json(ok(status)),
                Err(e) => envelope_json(err::<()>(e)),
            }
        }
    }
}

fn instant_build_active_for(state: &Option<InstantBuildHandle>, pid: i32) -> bool {
    state.as_ref().is_some_and(|h| h.pid() == pid)
}

fn run_toggle_instant_build(
    state: &mut Option<InstantBuildHandle>,
    pid: i32,
    enabled: bool,
) -> Result<InstantBuildStatus, String> {
    // A stale handle for a different (presumably closed) game process
    // shouldn't block toggling this one on.
    if let Some(existing) = state {
        if existing.pid() != pid {
            if let Some(stale) = state.take() {
                stale.stop();
            }
        }
    }

    match (state.is_some(), enabled) {
        (true, true) | (false, false) => Ok(InstantBuildStatus { enabled: state.is_some() }),
        (false, true) => {
            let handle = instant_build::start(pid)?;
            *state = Some(handle);
            Ok(InstantBuildStatus { enabled: true })
        }
        (true, false) => {
            if let Some(handle) = state.take() {
                handle.stop();
            }
            Ok(InstantBuildStatus { enabled: false })
        }
    }
}

fn serve() {
    let mut instant_build_state: Option<InstantBuildHandle> = None;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_line(&line, &mut instant_build_state);
        if writeln!(out, "{response}").is_err() || out.flush().is_err() {
            break;
        }
    }
    if let Some(handle) = instant_build_state.take() {
        handle.stop();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(subcommand) = args.first() else {
        print_and_exit(err::<()>("uso: ra2-trainer-helper <serve|status|toggle> [--pid <pid>] [--cheat <id>]"));
    };

    if subcommand == "serve" {
        return serve();
    }

    let Some(pid) = get_flag(&args, "--pid").and_then(|p| p.parse::<i32>().ok()) else {
        print_and_exit(err::<()>("falta o es invalido --pid <pid>"));
    };

    match subcommand.as_str() {
        "status" => match run_status(pid) {
            Ok(statuses) => print_and_exit(ok(statuses)),
            Err(e) => print_and_exit(err::<()>(e)),
        },
        "toggle" => {
            let Some(cheat_id) = get_flag(&args, "--cheat") else {
                print_and_exit(err::<()>("falta --cheat <id>"));
            };
            match run_toggle(pid, &cheat_id) {
                Ok(status) => print_and_exit(ok(status)),
                Err(e) => print_and_exit(err::<()>(e)),
            }
        }
        other => print_and_exit(err::<()>(format!("subcomando desconocido: {other}"))),
    }
}
