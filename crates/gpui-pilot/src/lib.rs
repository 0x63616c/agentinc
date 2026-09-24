//! Local, opt-in GPUI automation protocol, client and host.
#[cfg(feature = "host")]
pub mod host;
pub mod protocol;
pub mod transport;
