//! The app installs its tracing subscriber at launch. Installing it must not panic: a
//! second `log` bridge once made every build exit before opening a window.
#[test]
fn installing_the_subscriber_does_not_panic() {
    ainc_mac::init_tracing();
    tracing::info!("subscriber installed");
    log::warn!("log records reach the same subscriber");
}
