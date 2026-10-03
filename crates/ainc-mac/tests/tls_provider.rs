//! A reqwest client must build with the ring TLS provider that every binary installs at
//! startup. If the provider ever disappears from the dependency graph, this fails here
//! rather than at the first HTTPS request in production.
#[test]
fn reqwest_client_builds_with_the_ring_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    reqwest::Client::builder()
        .build()
        .expect("reqwest client with the installed TLS provider");
}
