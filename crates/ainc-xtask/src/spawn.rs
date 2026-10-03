//! Child processes xtask launches. xtask itself runs under `cargo run`, so its environment
//! carries `CARGO_MANIFEST_DIR`, `CARGO_MANIFEST_LINKS` and `CARGO_PKG_*` for the xtask
//! package. A nested cargo passes them on to build scripts, and crates such as ring declare
//! `rerun-if-env-changed` on them, so every xtask-launched cargo rebuilt half the workspace.
//! Every command goes through here so none of them inherits those variables.
use std::{ffi::OsStr, process::Command};

/// Package-scoped variables cargo sets for the process it runs. `CARGO`, `CARGO_HOME` and
/// `CARGO_TARGET_DIR` are deliberately kept: they are configuration, not package identity.
fn is_package_var(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    key.starts_with("CARGO_PKG_") || key == "CARGO_MANIFEST_DIR" || key == "CARGO_MANIFEST_LINKS"
}

/// `Command::new(program)` without the `CARGO_*` package variables of xtask's own build.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for (key, _) in std::env::vars_os() {
        if is_package_var(&key) {
            command.env_remove(key);
        }
    }
    command
}

/// A `cargo` invocation that will not disturb the fingerprints of the workspace it builds.
pub fn cargo() -> Command {
    command("cargo")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_removes_package_vars_from_the_child_env() {
        // SAFETY: tests in this binary run single-threaded with respect to this variable;
        // nothing else reads or writes it.
        unsafe { std::env::set_var("CARGO_PKG_XTASK_SPAWN_TEST", "1") };
        let command = cargo();
        let envs = command.get_envs().collect::<Vec<_>>();
        let removed = |name: &str| envs.iter().any(|(k, v)| *k == name && v.is_none());
        assert!(removed("CARGO_PKG_XTASK_SPAWN_TEST"));
        assert!(
            envs.iter()
                .all(|(key, value)| value.is_none() && is_package_var(key)),
            "only package variables are touched: {envs:?}"
        );
        if std::env::var_os("CARGO_MANIFEST_DIR").is_some() {
            assert!(removed("CARGO_MANIFEST_DIR"));
        }
    }
}
