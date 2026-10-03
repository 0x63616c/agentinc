//! A child process with a timed wait, terminate and kill: what `subprocess.Popen` offered.
use anyhow::{Result, bail};
use std::{
    io,
    process::{Command, ExitStatus},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::Duration,
};

pub struct ManagedChild {
    pid: i32,
    exited: Receiver<io::Result<ExitStatus>>,
    status: Option<ExitStatus>,
}

impl ManagedChild {
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        let mut child = command.spawn()?;
        // The command still holds any pipe ends given to the child; release them.
        drop(command);
        let pid = child.id() as i32;
        let (sender, exited) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(child.wait());
        });
        Ok(Self {
            pid,
            exited,
            status: None,
        })
    }

    pub fn id(&self) -> i32 {
        self.pid
    }

    /// `Popen.poll()`: the exit status if the child has finished.
    pub fn poll(&mut self) -> Option<ExitStatus> {
        if self.status.is_none()
            && let Ok(Ok(status)) = self.exited.try_recv()
        {
            self.status = Some(status);
        }
        self.status
    }

    /// `Popen.wait(timeout=...)`: errors when the child is still running afterwards.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        match self.exited.recv_timeout(timeout) {
            Ok(status) => {
                let status = status?;
                self.status = Some(status);
                Ok(status)
            }
            Err(RecvTimeoutError::Timeout) => {
                bail!(
                    "process {} timed out after {} seconds",
                    self.pid,
                    timeout.as_secs()
                )
            }
            Err(RecvTimeoutError::Disconnected) => bail!("process waiter exited"),
        }
    }

    fn signal(&mut self, signal: i32) {
        if self.poll().is_none() {
            unsafe { libc::kill(self.pid, signal) };
        }
    }

    pub fn terminate(&mut self) {
        self.signal(libc::SIGTERM);
    }

    pub fn kill(&mut self) {
        self.signal(libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_exit_status_and_timeouts() {
        let mut quick = ManagedChild::spawn(Command::new("true")).unwrap();
        assert!(
            quick
                .wait_timeout(Duration::from_secs(30))
                .unwrap()
                .success()
        );
        assert!(quick.poll().is_some());
        let mut hung = ManagedChild::spawn({
            let mut command = Command::new("sh");
            command
                .args(["-c", "read line"])
                .stdin(std::process::Stdio::piped());
            command
        })
        .unwrap();
        assert!(hung.poll().is_none());
        assert!(hung.wait_timeout(Duration::from_millis(1)).is_err());
        hung.terminate();
        assert!(
            !hung
                .wait_timeout(Duration::from_secs(30))
                .unwrap()
                .success()
        );
    }
}
