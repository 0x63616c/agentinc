//! Signalling the children the daemon owns: the one place it touches libc for it.

/// Ask a child to stop: SIGINT, which means a fast shutdown to Postgres.
#[derive(Clone, Copy)]
pub enum Stop {
    Interrupt,
}
impl Stop {
    pub fn send(self, pid: u32) {
        if let Ok(pid) = i32::try_from(pid) {
            let signal = match self {
                Self::Interrupt => libc::SIGINT,
            };
            // SAFETY: the pid belongs to a child this daemon spawned and has not yet reaped.
            unsafe { libc::kill(pid, signal) };
        }
    }
}
