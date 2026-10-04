//! The release pipeline, ported from the Python scripts that used to live in
//! `scripts/release/`. A faithful port: same flags, same outputs, same exit codes.
//! Each module is one script; `run` maps the xtask subcommand onto it.
use anyhow::{Result, anyhow};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio, exit},
    thread,
    time::Duration,
};

pub mod bump;
pub mod ci_gate;
pub mod distribute;
pub mod file_server;
pub mod http;
pub mod measure_build;
mod notes;
pub mod pending;
#[cfg(test)]
mod pipeline_config;
pub mod prepare;
pub mod proc;
pub mod runtime_smoke;
pub mod sparkle;
pub mod terminal_smoke;
pub mod upgrade_gate;

/// The subcommands this module serves, for the xtask usage string.
pub const NAMES: &str = "bump|release-next-patch|release-package-version|release-version|release-version-less|release|release-ci-gate|release-distribute|release-measure-build|release-runtime-smoke|release-terminal-smoke|release-upgrade-gate|release-pending|release-sparkle-deltas|release-sparkle-sign";

pub fn handles(operation: &str) -> bool {
    NAMES.split('|').any(|name| name == operation)
}

/// Run one release subcommand. Like the scripts did, a failure prints its message and exits 1.
pub fn run(operation: &str, args: Vec<String>, root: &Path) -> Result<()> {
    let result = match operation {
        "release" => prepare::cli(root, &args),
        "bump" => bump::bump(root, &args),
        "release-version" => bump::version_cli(root, &args),
        "release-package-version" => bump::package_version_cli(root, &args),
        "release-next-patch" => bump::next_patch_cli(&args),
        "release-version-less" => bump::version_less_cli(&args),
        "release-ci-gate" => ci_gate::cli(&args),
        "release-distribute" => distribute::cli(&args),
        "release-sparkle-deltas" => sparkle::deltas_cli(root, &args),
        "release-sparkle-sign" => sparkle::sign_cli(&args),
        "release-measure-build" => measure_build::cli(&args),
        "release-runtime-smoke" => runtime_smoke::cli(&args),
        "release-terminal-smoke" => terminal_smoke::cli(&args),
        "release-upgrade-gate" => upgrade_gate::cli(&args),
        "release-pending" => pending::cli(&args),
        other => Err(anyhow!("unknown release command {other}")),
    };
    if let Err(error) = result {
        eprintln!("{error:#}");
        exit(1);
    }
    Ok(())
}

/// The shell-quoted-ish command line used in error messages, like Python's CalledProcessError.
pub fn command_line(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

fn failed(line: &str, status: std::process::ExitStatus) -> anyhow::Error {
    anyhow!(
        "Command '{line}' returned non-zero exit status {}.",
        status.code().unwrap_or(-1)
    )
}

/// `subprocess.check_output(args, text=True).strip()`: stderr is inherited.
/// A [repeatable] command is retried.
pub fn output(program: &str, args: &[&str]) -> Result<String> {
    let run = || {
        let result = crate::spawn::command(program)
            .args(args)
            .stderr(Stdio::inherit())
            .output()?;
        if !result.status.success() {
            return Err(failed(&command_line(program, args), result.status));
        }
        Ok(String::from_utf8(result.stdout)?.trim().to_string())
    };
    if repeatable(program, args) {
        retry(run, thread::sleep)
    } else {
        run()
    }
}

/// `subprocess.run(command, check=True)`. A [repeatable] command is retried.
pub fn checked(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut run = || {
        let status = command.status()?;
        if !status.success() {
            let line = format!("{command:?}");
            return Err(failed(&line, status));
        }
        Ok(())
    };
    if repeatable(&program, &args) {
        retry(run, thread::sleep)
    } else {
        run()
    }
}

/// GitHub's API and release downloads fail transiently (an HTTP 500 on an asset, a connect
/// timeout), and one such failure used to drop a whole release run. A `gh` call that only
/// reads, or overwrites with `--clobber`, is safe to repeat; creating a release or writing
/// through `gh api` is not. Downloads through `curl` retry with its own `--retry`.
fn repeatable(program: &str, args: &[&str]) -> bool {
    if program != "gh" {
        return false;
    }
    match args {
        ["api", rest @ ..] => !rest.iter().any(|arg| {
            matches!(
                *arg,
                "-X" | "--method" | "-f" | "-F" | "--field" | "--raw-field" | "--input"
            )
        }),
        ["release", "download" | "view", ..] => true,
        ["release", "upload", ..] => args.contains(&"--clobber"),
        _ => false,
    }
}

/// Up to three attempts, waiting 5s and then 20s between them.
fn retry<T>(mut attempt: impl FnMut() -> Result<T>, mut wait: impl FnMut(Duration)) -> Result<T> {
    let mut delay = Duration::from_secs(5);
    for _ in 1..3 {
        match attempt() {
            Ok(value) => return Ok(value),
            Err(error) => {
                eprintln!("{error:#}; retrying in {}s", delay.as_secs());
                wait(delay);
                delay *= 4;
            }
        }
    }
    attempt()
}

/// Python truthiness of a JSON value, for `if identity.get(...)`.
pub fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(map)) => !map.is_empty(),
    }
}

/// SHA-256 of every regular file under `bundle`, keyed by its path relative to the bundle.
/// Symlinks to files count as files (their target is hashed); symlinks to directories are skipped.
pub fn inventory(bundle: &Path) -> Result<Map<String, Value>> {
    fn walk(dir: &Path, bundle: &Path, files: &mut Map<String, Value>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let linked = fs::symlink_metadata(&path)?.file_type().is_symlink();
            if metadata.is_dir() {
                if !linked {
                    walk(&path, bundle, files)?;
                }
            } else if metadata.is_file() {
                let digest = Sha256::digest(fs::read(&path)?);
                let relative = path.strip_prefix(bundle)?.to_string_lossy().into_owned();
                files.insert(relative, Value::String(hex(&digest)));
            }
        }
        Ok(())
    }
    let mut files = Map::new();
    walk(bundle, bundle, &mut files)?;
    Ok(files)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Unpack a `.tar.gz` like `tarfile.extractall(filter='data')`: nothing escapes `destination`.
pub fn extract_tar_gz(archive: &Path, destination: &Path) -> Result<()> {
    let file = fs::File::open(archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    tar.set_preserve_permissions(true);
    tar.unpack(destination)?;
    Ok(())
}

/// Write a `.tar.gz`; each entry is `(source, name inside the archive)`. Symlinks are stored as
/// symlinks, like `tarfile.add`.
pub fn create_tar_gz(archive: &Path, level: u32, entries: &[(&Path, &str)]) -> Result<()> {
    let file = fs::File::create(archive)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::new(level));
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    for (source, name) in entries {
        if fs::symlink_metadata(source)?.is_dir() {
            builder.append_dir_all(name, source)?;
        } else {
            builder.append_path_with_name(source, name)?;
        }
    }
    builder.into_inner()?.finish()?.flush()?;
    Ok(())
}

/// `Path.resolve()`: canonical when it exists, otherwise made absolute.
pub fn resolve(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

/// argparse stand-in. Usage errors exit 2 like argparse does.
pub struct Opts {
    prog: String,
    flags: HashSet<String>,
    values: HashMap<String, Vec<String>>,
    pub positional: Vec<String>,
}

pub fn usage_error(prog: &str, message: &str) -> ! {
    eprintln!("{prog}: error: {message}");
    exit(2);
}

/// Parse `--flag`, `--option value`, `--option=value` and positionals.
pub fn parse_args(prog: &str, args: &[String], flags: &[&str], values: &[&str]) -> Opts {
    let mut opts = Opts {
        prog: prog.to_string(),
        flags: HashSet::new(),
        values: HashMap::new(),
        positional: Vec::new(),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        if flags.contains(&name) && inline.is_none() {
            opts.flags.insert(name.to_string());
        } else if values.contains(&name) {
            let Some(value) = inline.or_else(|| iter.next().cloned()) else {
                usage_error(prog, &format!("argument {name}: expected one argument"));
            };
            opts.values.entry(name.to_string()).or_default().push(value);
        } else if arg.starts_with("--") {
            usage_error(prog, &format!("unrecognized arguments: {arg}"));
        } else {
            opts.positional.push(arg.clone());
        }
    }
    opts
}

impl Opts {
    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    pub fn one(&self, name: &str) -> Option<&str> {
        self.values
            .get(name)
            .and_then(|all| all.last())
            .map(String::as_str)
    }

    pub fn many(&self, name: &str) -> &[String] {
        self.values.get(name).map_or(&[], Vec::as_slice)
    }

    /// A required option: exit 2 naming it, like argparse.
    pub fn required(&self, name: &str) -> &str {
        self.one(name).unwrap_or_else(|| {
            usage_error(
                &self.prog,
                &format!("the following arguments are required: {name}"),
            )
        })
    }
}

/// Stdout of a command as raw bytes, for the one place Python read bytes (`openssl pkey`).
pub fn output_bytes(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let result = crate::spawn::command(program)
        .args(args)
        .stderr(Stdio::inherit())
        .output()?;
    if !result.status.success() {
        return Err(failed(&command_line(program, args), result.status));
    }
    Ok(result.stdout)
}

pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| anyhow!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn only_reading_or_overwriting_gh_calls_repeat() {
        assert!(repeatable(
            "gh",
            &["api", "repos/o/r/releases?per_page=100"]
        ));
        assert!(repeatable(
            "gh",
            &["release", "download", "v1", "--clobber"]
        ));
        assert!(repeatable(
            "gh",
            &["release", "view", "v1", "--json", "isDraft"]
        ));
        assert!(repeatable(
            "gh",
            &["release", "upload", "v1", "a", "--clobber"]
        ));
        assert!(!repeatable("gh", &["release", "upload", "v1", "a"]));
        assert!(!repeatable("gh", &["release", "create", "v1", "--draft"]));
        assert!(!repeatable(
            "gh",
            &["api", "-X", "DELETE", "repos/o/r/releases/1"]
        ));
        assert!(!repeatable("rcodesign", &["notary-submit"]));
    }

    #[test]
    fn retry_backs_off_then_returns_the_first_success() {
        let mut calls = 0;
        let mut waits = Vec::new();
        let result = retry(
            || {
                calls += 1;
                if calls < 3 {
                    Err(anyhow!("HTTP 500"))
                } else {
                    Ok(calls)
                }
            },
            |delay| waits.push(delay.as_secs()),
        );
        assert_eq!(result.unwrap(), 3);
        assert_eq!(waits, [5, 20]);
    }

    #[test]
    fn retry_gives_up_after_three_attempts_with_the_last_error() {
        let mut calls = 0;
        let result: Result<()> = retry(
            || {
                calls += 1;
                Err(anyhow!("attempt {calls}"))
            },
            |_| {},
        );
        assert_eq!(result.unwrap_err().to_string(), "attempt 3");
        assert_eq!(calls, 3);
    }

    #[test]
    fn archives_keep_symlinks_and_modes_and_overwrite_on_extract() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let bundle = root.join("AgentInc.app");
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        let tool = bundle.join("Contents/MacOS/AgentInc");
        fs::write(&tool, b"binary").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(
            "AgentInc",
            bundle
                .join("Contents/MacOS")
                .join(prepare::LEGACY_EXECUTABLE),
        )
        .unwrap();
        let archive = root.join("bundle.tar.gz");
        create_tar_gz(&archive, 1, &[(&bundle, "AgentInc.app")]).unwrap();
        let out = root.join("out");
        fs::create_dir(&out).unwrap();
        extract_tar_gz(&archive, &out).unwrap();
        // A second extraction over the first (a rerun in the same directory) must succeed.
        extract_tar_gz(&archive, &out).unwrap();
        let link = out
            .join("AgentInc.app/Contents/MacOS")
            .join(prepare::LEGACY_EXECUTABLE);
        assert_eq!(fs::read_link(&link).unwrap(), Path::new("AgentInc"));
        let mode = fs::metadata(out.join("AgentInc.app/Contents/MacOS/AgentInc"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(
            inventory(&bundle).unwrap(),
            inventory(&out.join("AgentInc.app")).unwrap()
        );
    }
}
