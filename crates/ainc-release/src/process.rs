//! Signal hygiene at native process boundaries. UI dispatch threads may block
//! signals; inheriting their mask prevents Tokio from reaping exited children.
use std::{io, os::unix::process::CommandExt, process::Command};

/// Call only at process startup, before creating threads or installing handlers.
/// SIGPIPE retains Rust's ignored disposition so broken pipes return I/O errors.
pub fn reset_inherited_signals() -> io::Result<()> {
    // SAFETY: initialized local signal sets; these POSIX operations are also
    // async-signal-safe, allowing this routine in the pre-exec child.
    unsafe {
        let mut empty = std::mem::zeroed();
        if libc::sigemptyset(&mut empty) != 0 {
            return Err(io::Error::last_os_error());
        }
        let error = libc::pthread_sigmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Reset in the child, never in the spawning UI/runtime thread.
pub fn prepare_child(command: &mut Command) -> &mut Command {
    // SAFETY: the callback uses only async-signal-safe signal operations and
    // returns an OS error without acquiring locks or allocating.
    unsafe { command.pre_exec(reset_inherited_signals) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawning_clears_inherited_blocked_and_ignored_sigchld() {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args(["--exact", "process::tests::child_signal_probe", "--ignored"]);
        // SAFETY: affect only the forked child, using async-signal-safe operations.
        unsafe {
            child.pre_exec(|| {
                let mut blocked = std::mem::zeroed();
                libc::sigemptyset(&mut blocked);
                libc::sigaddset(&mut blocked, libc::SIGCHLD);
                let error = libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, std::ptr::null_mut());
                if error != 0 {
                    return Err(io::Error::from_raw_os_error(error));
                }
                let mut ignored: libc::sigaction = std::mem::zeroed();
                ignored.sa_sigaction = libc::SIG_IGN;
                libc::sigemptyset(&mut ignored.sa_mask);
                if libc::sigaction(libc::SIGCHLD, &ignored, std::ptr::null_mut()) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        assert!(prepare_child(&mut child).status().unwrap().success());
    }

    #[test]
    #[ignore = "subprocess probe launched by spawning_clears_inherited_blocked_and_ignored_sigchld"]
    fn child_signal_probe() {
        // SAFETY: querying initialized local structures, without changing signals.
        unsafe {
            let mut mask = std::mem::zeroed();
            assert_eq!(
                libc::pthread_sigmask(libc::SIG_SETMASK, std::ptr::null(), &mut mask),
                0
            );
            assert_eq!(libc::sigismember(&mask, libc::SIGCHLD), 0);
            let mut action: libc::sigaction = std::mem::zeroed();
            assert_eq!(
                libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action),
                0
            );
            assert_eq!(action.sa_sigaction, libc::SIG_DFL);
        }
    }
}
