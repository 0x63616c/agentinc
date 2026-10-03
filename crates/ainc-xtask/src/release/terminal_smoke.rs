//! Check the signed bundle's real terminal attach process through a PTY.
use super::{parse_args, proc::ManagedChild, resolve};
use anyhow::{Result, bail};
use std::{
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn tail(output: &[u8]) -> String {
    format!(
        "{:?}",
        String::from_utf8_lossy(&output[output.len().saturating_sub(500)..])
    )
}

fn read_until(
    master: &OwnedFd,
    process: &mut ManagedChild,
    predicate: impl Fn(&[u8]) -> bool,
    description: &str,
) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !predicate(&output) {
        if let Some(status) = process.poll() {
            bail!(
                "{description}: attach exited {}: {}",
                status.code().unwrap_or(-1),
                tail(&output)
            );
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("{description}: {}", tail(&output));
        }
        let mut poll = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe {
            libc::poll(
                &mut poll,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        };
        if ready > 0 {
            let mut chunk = [0u8; 8192];
            let n =
                unsafe { libc::read(master.as_raw_fd(), chunk.as_mut_ptr().cast(), chunk.len()) };
            if n < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            output.extend_from_slice(&chunk[..n as usize]);
        }
    }
    Ok(output)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    let (mut total, mut start) = (0, 0);
    while let Some(at) = find(&haystack[start..], needle) {
        total += 1;
        start += at + needle.len();
    }
    total
}

fn open_pty() -> Result<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave) = (0, 0);
    let status = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    let size = libc::winsize {
        ws_row: 42,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCSWINSZ, &size) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((master, slave))
}

fn check_pane(app: &Path, discovery: &Path, existing: bool) -> Result<()> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (master, slave) = open_pty()?;
    let mut command = Command::new(app.join("Contents/MacOS/aincd"));
    command.args(["--terminal-attach", &session_id]);
    if existing {
        command.arg("--existing");
    }
    command
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?))
        .env("AINC_DISCOVERY_FILE", discovery);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut process = ManagedChild::spawn(command)?;
    drop(slave);
    let result = (|| -> Result<()> {
        // The shell's first prompt has no newline. This catches a buffered
        // stdout writer that leaves a pane showing "Connecting" indefinitely.
        read_until(
            &master,
            &mut process,
            |data| find(data, b"\x1bc").is_some_and(|at| data.len() > at + 2),
            "initial terminal output",
        )?;
        let marker = format!("__AINC_TERMINAL_{}__", session_id.replace('-', ""));
        let typed = format!("printf '{marker}\\n'\n");
        if unsafe { libc::write(master.as_raw_fd(), typed.as_ptr().cast(), typed.len()) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let needle = marker.as_bytes();
        let result = read_until(
            &master,
            &mut process,
            |data| count(data, needle) >= 2,
            "terminal command output",
        )?;
        if count(&result, needle) < 2 {
            bail!("terminal command did not run");
        }
        println!(
            "PASS terminal {}: {session_id}",
            if existing { "restored" } else { "new" }
        );
        Ok(())
    })();
    process.terminate();
    let waited = process.wait_timeout(Duration::from_secs(10));
    drop(master);
    result?;
    waited.map(|_| ())
}

pub fn check(app: &Path, profile: &Path) -> Result<()> {
    let discovery = resolve(&profile.join("daemon/api-url"));
    if !discovery.is_file() {
        bail!("terminal daemon discovery missing: {}", discovery.display());
    }
    check_pane(app, &discovery, false)?;
    // A saved pane from the old daemon has an ID but no live PTY after update.
    check_pane(app, &discovery, true)
}

pub fn cli(args: &[String]) -> Result<()> {
    let opts = parse_args("release-terminal-smoke", args, &[], &["--app", "--profile"]);
    check(
        &resolve(Path::new(opts.required("--app"))),
        &resolve(Path::new(opts.required("--profile"))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_non_overlapping_markers_and_finds_the_reset_sequence() {
        assert_eq!(count(b"aXbXXc", b"X"), 3);
        assert_eq!(count(b"__M____M__", b"__M__"), 2);
        assert_eq!(find(b"abc\x1bcdef", b"\x1bc"), Some(3));
    }

    #[test]
    fn a_pty_child_echoes_what_the_shell_prints() {
        let (master, slave) = open_pty().unwrap();
        let mut command = Command::new("sh");
        command
            .args([
                "-c",
                "printf '\\033cprompt'; read line; echo \"$line\" \"$line\"",
            ])
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        let mut process = ManagedChild::spawn(command).unwrap();
        drop(slave);
        read_until(
            &master,
            &mut process,
            |d| find(d, b"\x1bc").is_some_and(|at| d.len() > at + 2),
            "first",
        )
        .unwrap();
        let typed = b"MARK\n";
        unsafe { libc::write(master.as_raw_fd(), typed.as_ptr().cast(), typed.len()) };
        let out = read_until(&master, &mut process, |d| count(d, b"MARK") >= 3, "echo").unwrap();
        assert!(count(&out, b"MARK") >= 3);
        process.terminate();
    }
}
