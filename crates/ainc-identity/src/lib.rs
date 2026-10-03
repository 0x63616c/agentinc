//! Product release identity and wire compatibility: the version, build channel, request
//! headers and the client/server compatibility check. Every AgentInc binary links this; only
//! the app and the updater link `ainc-release` on top of it.
#[cfg(unix)]
pub mod process;
use semver::Version;
use serde::{Deserialize, Serialize};

pub const VERSION: &str = match option_env!("AINC_UPGRADE_TEST_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
pub mod identity {
    use std::path::PathBuf;

    pub const PRODUCTION: bool = cfg!(ainc_production);
    /// The release commit. Development builds are not stamped with one.
    pub const COMMIT: &str = match option_env!("AINC_COMMIT") {
        Some(commit) => commit,
        None => "unknown",
    };
    pub const BUNDLE_ID: &str = if PRODUCTION {
        "co.worldwidewebb.agentinc"
    } else {
        "co.worldwidewebb.agentinc.dev"
    };

    pub fn version() -> String {
        if PRODUCTION {
            super::VERSION.into()
        } else {
            format!("{}-dev", super::VERSION)
        }
    }

    pub fn support_dir() -> PathBuf {
        let folder = if PRODUCTION {
            "Agentinc OS"
        } else {
            "AgentInc Development"
        };
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Application Support")
            .join(folder)
    }

    /// Where the installed daemon publishes its API URL for the app and CLI.
    pub fn discovery_file() -> PathBuf {
        support_dir().join("daemon/api-url")
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn compiled_channel_selects_distinct_identity_and_profile() {
            let expected_version = super::super::VERSION;
            let (version, bundle, profile) = if cfg!(ainc_production) {
                (
                    expected_version.to_string(),
                    "co.worldwidewebb.agentinc",
                    "Agentinc OS",
                )
            } else {
                (
                    format!("{expected_version}-dev"),
                    "co.worldwidewebb.agentinc.dev",
                    "AgentInc Development",
                )
            };
            assert_eq!(super::PRODUCTION, cfg!(ainc_production));
            assert_eq!(super::version(), version);
            assert_eq!(super::BUNDLE_ID, bundle);
            assert!(super::support_dir().ends_with(profile));
            assert_eq!(
                super::discovery_file(),
                super::support_dir().join("daemon/api-url")
            );
        }
    }
}
pub const API: u32 = 1;
pub const MIN_CLIENT: &str = "0.1.0";
pub const BUILD: &str = match option_env!("AINC_BUILD_ID") {
    Some(value) => value,
    None => "development",
};
/// The product repository, for help and feedback links.
pub const REPOSITORY: &str = "https://github.com/0x63616c/agentinc";

/// Request header carrying [`client_header`].
pub const CLIENT_HEADER: &str = "agent-inc-client";
/// Response header carrying [`server_header`].
pub const SERVER_HEADER: &str = "agent-inc-server";
/// The log line `aincd` prints once it is serving. Launchers wait for it; sharing the
/// constant means a reworded log line cannot leave them waiting forever.
pub const DAEMON_READY: &str = "daemon ready";

pub fn client_header() -> String {
    format!("mac/{VERSION} (build {BUILD}; api {API})")
}
pub fn server_header() -> String {
    format!("aincd/{VERSION} (build {BUILD}; api {API})")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityError {
    UpgradeRequired,
    ServerTooOld,
    InvalidClient,
}
impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Update to continue")
    }
}
impl std::error::Error for CompatibilityError {}

pub fn check_client(header: &str, minimum: &str, api: u32) -> Result<(), CompatibilityError> {
    let parse = || -> Option<(Version, u32)> {
        let (identity, details) = header.split_once(" (")?;
        let (_, version) = identity.split_once('/')?;
        let api = details
            .strip_suffix(')')?
            .rsplit_once("api ")?
            .1
            .parse()
            .ok()?;
        Some((Version::parse(version).ok()?, api))
    };
    let (version, client_api) = parse().ok_or(CompatibilityError::InvalidClient)?;
    let minimum = Version::parse(minimum).map_err(|_| CompatibilityError::InvalidClient)?;
    if client_api > api {
        Err(CompatibilityError::ServerTooOld)
    } else if client_api < api || version < minimum {
        Err(CompatibilityError::UpgradeRequired)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_is_reciprocal() {
        assert_eq!(check_client(&client_header(), MIN_CLIENT, API), Ok(()));
        assert_eq!(
            check_client("mac/0.0.1 (api 1)", MIN_CLIENT, API),
            Err(CompatibilityError::UpgradeRequired)
        );
        assert_eq!(
            check_client("mac/0.1.0 (api 2)", MIN_CLIENT, API),
            Err(CompatibilityError::ServerTooOld)
        );
        assert_eq!(
            check_client("", MIN_CLIENT, API),
            Err(CompatibilityError::InvalidClient)
        );
    }
}
