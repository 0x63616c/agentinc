//! `ainc terminals attach` against a real daemon: it creates the Terminal, runs a command in
//! the daemon-owned shell, and recreates a Terminal whose saved ID the daemon no longer has.
use std::{
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
};

/// Attach to a Terminal with `extra` arguments, run one command, return what the shell printed.
async fn attach_and_run(pool: sqlx::PgPool, extra: &'static [&'static str]) -> String {
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
    let id = uuid::Uuid::new_v4().to_string();
    let output = tokio::task::spawn_blocking(move || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ainc"))
            .args(["terminals", "attach", &id])
            .args(extra)
            .env("AINC_API_URL", &url)
            .env("AINC_TOKEN_FILE", &token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        // The typed command echoes with `$((6*7))` unexpanded; only the shell's answer has 42.
        stdin.write_all(b"echo __ATTACHED_$((6*7))__\n").unwrap();
        let mut seen = Vec::new();
        let mut chunk = [0u8; 4096];
        while !String::from_utf8_lossy(&seen).contains("__ATTACHED_42__") {
            let count = stdout.read(&mut chunk).unwrap();
            assert!(
                count > 0,
                "attach ended early: {}",
                String::from_utf8_lossy(&seen)
            );
            seen.extend_from_slice(&chunk[..count]);
        }
        // Closing stdin ends the viewer; the daemon keeps the Terminal.
        drop(stdin);
        assert!(child.wait().unwrap().success());
        String::from_utf8_lossy(&seen).into_owned()
    })
    .await
    .unwrap();
    server.abort();
    output
}

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn attach_creates_the_terminal_and_runs_a_command(pool: sqlx::PgPool) {
    let seen = attach_and_run(pool, &[]).await;
    assert!(seen.contains("[Connecting to AgentInc terminal...]"));
}

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn attach_recreates_a_terminal_the_daemon_no_longer_has(pool: sqlx::PgPool) {
    attach_and_run(pool, &["--existing"]).await;
}
