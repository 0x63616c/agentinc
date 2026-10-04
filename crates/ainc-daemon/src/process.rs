//! Unix process groups and pseudo-terminals: the one place the daemon spawns a child into its
//! own group, signals a whole group, opens a pty and makes a shell its session leader. Callers
//! (the coding sandbox, Terminals, the Codex child, the local runtime) hold a [`Group`] and
//! never touch libc for any of it.
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
    process::Command as StdCommand,
};

/// A child's process group, named by its leader's pid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group(i32);
impl Group {
    /// The group a child leads after [`lead_group`] or [`Pty::spawn`]; `None` without a pid.
    pub fn of(pid: Option<u32>) -> Option<Self> {
        i32::try_from(pid?).ok().map(Self)
    }
    fn signal(self, signal: i32) {
        // SAFETY: killpg takes no pointers; the id came from a child this daemon spawned.
        unsafe { libc::killpg(self.0, signal) };
    }
    /// SIGKILL every member.
    pub fn kill(self) {
        self.signal(libc::SIGKILL);
    }
    /// SIGHUP every member, as a closed terminal does.
    pub fn hangup(self) {
        self.signal(libc::SIGHUP);
    }
    /// SIGKILL the group when the guard drops, whichever way its owner exits.
    pub fn kill_on_drop(self) -> KillOnDrop {
        KillOnDrop(self)
    }
}

/// Kills its group on drop.
pub struct KillOnDrop(Group);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        self.0.kill();
    }
}

/// Make the child lead a new process group, so its helpers can be signalled with it.
pub fn lead_group(command: &mut StdCommand) {
    std::os::unix::process::CommandExt::process_group(command, 0);
}

/// Signal one process the daemon owns (not its group).
fn signal_child(pid: u32, signal: i32) {
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: the pid belongs to a child this daemon spawned and has not yet reaped.
        unsafe { libc::kill(pid, signal) };
    }
}

/// A pseudo-terminal pair. [`Pty::spawn`] gives the slave to a shell and returns the master.
pub struct Pty {
    master: File,
    slave: File,
}
impl Pty {
    pub fn open(rows: u16, cols: u16) -> anyhow::Result<Self> {
        let (mut master, mut slave) = (-1, -1);
        let mut size = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // macOS takes the size by mutable pointer, Linux by const pointer.
        #[cfg(target_os = "macos")]
        let size_ptr = &mut size as *mut libc::winsize;
        #[cfg(not(target_os = "macos"))]
        let size_ptr = &size as *const libc::winsize;
        // SAFETY: every pointer is valid for the call; the fds are wrapped right below.
        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                size_ptr,
            )
        };
        anyhow::ensure!(opened == 0, "openpty: {}", std::io::Error::last_os_error());
        // SAFETY: openpty returned two fresh descriptors this function now owns.
        let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        Ok(Self { master, slave })
    }

    /// Spawn `command` as a session leader with the slave as its controlling terminal and
    /// stdio. Returns the child, its group, and the master side.
    pub fn spawn(
        self,
        command: &mut tokio::process::Command,
    ) -> std::io::Result<(tokio::process::Child, Group, File)> {
        let slave_fd = self.slave.as_raw_fd();
        // SAFETY: only async-signal-safe libc calls run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::ioctl(slave_fd, libc::TIOCSCTTY as libc::c_ulong, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                for fd in 0..=2 {
                    if libc::dup2(slave_fd, fd) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if slave_fd > 2 {
                    libc::close(slave_fd);
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        let group = Group::of(child.id()).ok_or_else(|| std::io::Error::other("no pid"))?;
        Ok((child, group, self.master))
    }
}

/// Tell a pty and the shell group on it the terminal is now `rows` by `cols`.
pub fn resize(master: &File, group: Group, rows: u16, cols: u16) {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `size` is a valid winsize for the duration of the call.
    unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ as libc::c_ulong, &size) };
    group.signal(libc::SIGWINCH);
}

/// Ask a child to stop: SIGTERM, or SIGINT where that means a fast shutdown (Postgres).
#[derive(Clone, Copy)]
pub enum Stop {
    Terminate,
    Interrupt,
}
impl Stop {
    pub fn send(self, pid: u32) {
        signal_child(
            pid,
            match self {
                Self::Terminate => libc::SIGTERM,
                Self::Interrupt => libc::SIGINT,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::process::Stdio;

    /// A helper the child started shares its group, so killing the group ends it too: the pipe
    /// both hold reaches EOF only when the last holder is gone.
    #[test]
    fn killing_a_group_takes_its_helpers_with_it() {
        let mut command = StdCommand::new("sh");
        command
            .args(["-c", "sleep 600 & wait"])
            .stdout(Stdio::piped());
        lead_group(&mut command);
        let mut child = command.spawn().unwrap();
        let mut output = child.stdout.take().unwrap();
        let group = Group::of(Some(child.id())).unwrap();
        group.kill();
        let mut rest = Vec::new();
        output.read_to_end(&mut rest).unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn a_pty_shell_leads_its_own_group_and_hangs_up_with_it() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let pty = Pty::open(24, 80).unwrap();
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "sleep 600"]);
            let (mut child, group, master) = pty.spawn(&mut command).unwrap();
            assert_eq!(Group::of(child.id()), Some(group));
            group.hangup();
            let status = child.wait().await.unwrap();
            assert!(!status.success());
            drop(master);
        });
    }
}
