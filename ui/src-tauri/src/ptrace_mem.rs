use nix::sys::ptrace;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use std::fmt;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;

#[derive(Debug)]
pub enum MemError {
    Attach(nix::errno::Errno),
    Wait(nix::errno::Errno),
    Io(std::io::Error),
    NoMatchingVariant,
}

impl fmt::Display for MemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemError::Attach(e) => write!(f, "no se pudo hacer ptrace attach al proceso: {e}"),
            MemError::Wait(e) => write!(f, "el proceso no se detuvo tras el attach: {e}"),
            MemError::Io(e) => write!(f, "error leyendo/escribiendo memoria del proceso: {e}"),
            MemError::NoMatchingVariant => {
                write!(f, "ninguna version conocida coincide con la memoria actual del proceso")
            }
        }
    }
}

impl std::error::Error for MemError {}

impl From<std::io::Error> for MemError {
    fn from(e: std::io::Error) -> Self {
        MemError::Io(e)
    }
}

/// RAII handle for a ptrace-attached process. Attaching stops the tracee;
/// dropping this handle always detaches (resuming it), even on error paths.
pub struct Attached {
    pid: Pid,
    mem: std::fs::File,
}

impl Attached {
    pub fn new(pid: i32) -> Result<Self, MemError> {
        let pid = Pid::from_raw(pid);
        ptrace::attach(pid).map_err(MemError::Attach)?;
        waitpid(pid, None).map_err(MemError::Wait)?;
        let mem = OpenOptions::new().read(true).write(true).open(format!("/proc/{}/mem", pid.as_raw()))?;
        Ok(Self { pid, mem })
    }

    pub fn read(&self, address: u64, len: usize) -> Result<Vec<u8>, MemError> {
        let mut buf = vec![0u8; len];
        self.mem.read_exact_at(&mut buf, address)?;
        Ok(buf)
    }

    pub fn write(&self, address: u64, data: &[u8]) -> Result<(), MemError> {
        self.mem.write_all_at(data, address)?;
        Ok(())
    }
}

impl Drop for Attached {
    fn drop(&mut self) {
        let _ = ptrace::detach(self.pid, None);
    }
}
