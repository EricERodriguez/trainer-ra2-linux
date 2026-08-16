//! "Instant build" cheat: instead of a static byte patch, this intercepts
//! (via a real INT3 breakpoint, the same mechanism any debugger uses) the
//! single instruction in gamemd.exe/game.exe that writes a production
//! timer's progress value, and forces that value to "complete" (0x36) every
//! time it fires. No new code is ever injected into the target process —
//! only one byte (the breakpoint) is ever swapped in and back out.
//!
//! Verified live (via gdb) against Red Alert 2 v1.006 game.exe:
//! `0x4b9367: mov [esi+0x24], edx` is the exact instruction that stores a
//! FactoryClass::Production timer's new "Value" into itself; 0x36 (54) is
//! the value the surrounding code already treats as "done" (matches the
//! documented 54-step build cycle). This applies to every producer that
//! uses this code path, including the AI's.
//!
//! Runs for as long as `start()`'s returned handle is alive; `stop()` (or
//! dropping the owning Option and calling it) cleanly restores the original
//! byte and detaches before returning.

use nix::sys::ptrace;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::Pid;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;

const HOOK_ADDR: u64 = 0x4b9367;
const COMPLETE_VALUE: u64 = 0x36;
const BREAKPOINT_BYTE: [u8; 1] = [0xCC];

pub struct InstantBuildHandle {
    pid: i32,
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InstantBuildHandle {
    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// Signals the background thread to clean up (restore the original
    /// byte, detach) and blocks until it has done so.
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        // The thread is almost certainly blocked in waitpid(); a SIGSTOP to
        // the tracee is a harmless way (we're its tracer) to wake it up so
        // it notices the flag instead of waiting for the next real trap.
        let _ = kill(Pid::from_raw(self.pid), Signal::SIGSTOP);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start(pid: i32) -> Result<InstantBuildHandle, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag = stop_flag.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let thread = std::thread::Builder::new()
        .name("instant-build".into())
        .spawn(move || run_loop(pid, flag, ready_tx))
        .map_err(|e| format!("no se pudo iniciar el hilo de instant-build: {e}"))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(InstantBuildHandle { pid, stop_flag, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err("el hilo de instant-build termino antes de confirmar el enganche".to_string())
        }
    }
}

fn run_loop(pid: i32, stop_flag: Arc<AtomicBool>, ready: mpsc::Sender<Result<(), String>>) {
    let pid_t = Pid::from_raw(pid);

    if let Err(e) = ptrace::attach(pid_t) {
        let _ = ready.send(Err(format!("no se pudo hacer ptrace attach: {e}")));
        return;
    }
    if let Err(e) = waitpid(pid_t, None) {
        let _ = ready.send(Err(format!("el proceso no se detuvo tras el attach: {e}")));
        return;
    }

    let mem = match OpenOptions::new().read(true).write(true).open(format!("/proc/{pid}/mem")) {
        Ok(f) => f,
        Err(e) => {
            let _ = ptrace::detach(pid_t, None);
            let _ = ready.send(Err(format!("no se pudo abrir /proc/{pid}/mem: {e}")));
            return;
        }
    };

    let mut original_byte = [0u8; 1];
    if let Err(e) = mem.read_exact_at(&mut original_byte, HOOK_ADDR) {
        let _ = ptrace::detach(pid_t, None);
        let _ = ready.send(Err(format!("no se pudo leer el punto de enganche: {e}")));
        return;
    }
    if let Err(e) = mem.write_all_at(&BREAKPOINT_BYTE, HOOK_ADDR) {
        let _ = ptrace::detach(pid_t, None);
        let _ = ready.send(Err(format!("no se pudo instalar el breakpoint: {e}")));
        return;
    }
    if let Err(e) = ptrace::cont(pid_t, None) {
        let _ = mem.write_all_at(&original_byte, HOOK_ADDR);
        let _ = ptrace::detach(pid_t, None);
        let _ = ready.send(Err(format!("no se pudo reanudar el proceso: {e}")));
        return;
    }

    let _ = ready.send(Ok(()));

    loop {
        let status = match waitpid(pid_t, None) {
            Ok(s) => s,
            Err(_) => break,
        };

        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        match status {
            WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                if !handle_trap(pid_t, &mem, &original_byte) {
                    break;
                }
            }
            WaitStatus::Exited(..) | WaitStatus::Signaled(..) => break,
            _ => {
                // Some other stop (commonly our own forced SIGSTOP waking us
                // up to re-check stop_flag, already handled above): just let
                // the tracee carry on.
                let _ = ptrace::cont(pid_t, None);
            }
        }
    }

    let _ = mem.write_all_at(&original_byte, HOOK_ADDR);
    let _ = ptrace::detach(pid_t, None);
}

/// Handles one SIGTRAP. Returns false if the loop should give up (a ptrace
/// call failed, meaning the process is probably gone).
fn handle_trap(pid: Pid, mem: &std::fs::File, original_byte: &[u8; 1]) -> bool {
    let mut regs = match ptrace::getregs(pid) {
        Ok(r) => r,
        Err(_) => return false,
    };

    // INT3 leaves rip one byte past the breakpoint. Anything else stopping
    // us with SIGTRAP isn't our breakpoint; just let it continue.
    if regs.rip != HOOK_ADDR + 1 {
        return ptrace::cont(pid, None).is_ok();
    }

    regs.rip = HOOK_ADDR;
    regs.rdx = COMPLETE_VALUE;
    if ptrace::setregs(pid, regs).is_err() {
        return false;
    }

    // Swap the real instruction back in, execute exactly that one
    // instruction (now storing our forced edx), then re-arm the breakpoint.
    if mem.write_all_at(original_byte, HOOK_ADDR).is_err() {
        return false;
    }
    if ptrace::step(pid, None).is_err() {
        return false;
    }
    match waitpid(pid, None) {
        Ok(WaitStatus::Exited(..)) | Ok(WaitStatus::Signaled(..)) | Err(_) => return false,
        Ok(_) => {}
    }
    if mem.write_all_at(&BREAKPOINT_BYTE, HOOK_ADDR).is_err() {
        return false;
    }
    ptrace::cont(pid, None).is_ok()
}
