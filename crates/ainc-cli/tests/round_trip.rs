use std::{fs, process::Command};

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn cli_reads_and_commands_against_real_daemon(pool: sqlx::PgPool) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            ainc_daemon::product_router(
                ainc_daemon::product::Product::new(pool, "fixture".into()).unwrap(),
            ),
        )
        .await
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let token = dir.path().join("owner-token");
    fs::write(&token, "fixture\n").unwrap();
    let (help, status, before, create, after) = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str]| {
            Command::new(env!("CARGO_BIN_EXE_ainc"))
                .args(args)
                .env("AINC_API_URL", &url)
                .env("AINC_TOKEN_FILE", &token)
                .output()
                .unwrap()
        };
        (
            run(&["--help"]),
            run(&["status"]),
            run(&["tickets", "list"]),
            run(&["tickets", "create", "--title", "CLI round trip"]),
            run(&["tickets", "list"]),
        )
    })
    .await
    .unwrap();
    assert!(String::from_utf8_lossy(&after.stdout).contains("CLI round trip"));
    for (command, output) in [
        ("ainc --help", help),
        ("ainc status", status),
        ("ainc tickets list", before),
        ("ainc tickets create --title 'CLI round trip'", create),
        ("ainc tickets list", after),
    ] {
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("$ {command}\n{}", String::from_utf8_lossy(&output.stdout));
    }
    server.abort();
}
