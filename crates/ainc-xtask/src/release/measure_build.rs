//! Run a native build and record sampled CPU/RSS for its process tree.
use super::output;
use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::Write,
    process::exit,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Summed RSS (KiB) and CPU (%) of `pid` and all its descendants in `ps -A -o pid=,ppid=,rss=,%cpu=`.
pub fn tree_usage(ps: &str, pid: i32) -> (u64, f64) {
    let mut rows: HashMap<i32, (i32, u64, f64)> = HashMap::new();
    for line in ps.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if let [child, parent, rss, cpu] = fields[..]
            && let (Ok(child), Ok(parent), Ok(rss), Ok(cpu)) =
                (child.parse(), parent.parse(), rss.parse(), cpu.parse())
        {
            rows.insert(child, (parent, rss, cpu));
        }
    }
    let mut family = std::collections::HashSet::from([pid]);
    loop {
        let children: Vec<i32> = rows
            .iter()
            .filter(|(_, (parent, _, _))| family.contains(parent))
            .map(|(child, _)| *child)
            .collect();
        if children.iter().all(|child| family.contains(child)) {
            break;
        }
        family.extend(children);
    }
    family
        .iter()
        .filter_map(|child| rows.get(child))
        .fold((0, 0.0), |(rss, cpu), row| (rss + row.1, cpu + row.2))
}

fn usage(pid: i32) -> Result<(u64, f64)> {
    Ok(tree_usage(
        &output("ps", &["-A", "-o", "pid=,ppid=,rss=,%cpu="])?,
        pid,
    ))
}

fn nodename() -> String {
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    unsafe { libc::uname(&mut name) };
    let bytes: Vec<u8> = name
        .nodename
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn cli(args: &[String]) -> Result<()> {
    let Some((program, rest)) = args.split_first() else {
        bail!("usage: cargo xtask release-measure-build COMMAND [ARG ...]");
    };
    let started = Instant::now();
    let mut child = crate::spawn::command(program).args(rest).spawn()?;
    let pid = child.id() as i32;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(child.wait());
    });
    let (mut peak_rss, mut peak_cpu) = (0, 0.0_f64);
    let status = loop {
        let (rss, cpu) = usage(pid)?;
        peak_rss = peak_rss.max(rss);
        peak_cpu = peak_cpu.max(cpu);
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(status) => break status?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => bail!("build waiter exited"),
        }
    };
    let summary = format!(
        "Native build on {}: {:.0}s; peak sampled process-tree RSS {:.0} MiB; peak sampled CPU {:.0}% (100% = one core).",
        nodename(),
        started.elapsed().as_secs_f64(),
        peak_rss as f64 / 1024.0,
        peak_cpu
    );
    println!("{summary}");
    if let Some(path) = env::var_os("GITHUB_STEP_SUMMARY").filter(|p| !p.is_empty()) {
        let mut file = OpenOptions::new().append(true).create(true).open(path)?;
        writeln!(file, "{summary}")?;
    }
    use std::os::unix::process::ExitStatusExt;
    exit(
        status
            .code()
            .unwrap_or_else(|| -status.signal().unwrap_or(1)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_the_whole_process_tree() {
        let ps = " 10 1 100 1.5\n 11 10 200 2.0\n 12 11 300 3.0\n 13 1 999 9.0\n bad line\n";
        assert_eq!(tree_usage(ps, 10), (600, 6.5));
        assert_eq!(tree_usage(ps, 13), (999, 9.0));
    }
}
