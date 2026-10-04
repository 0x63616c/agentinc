//! Tickets read as `T-12` in the CLI's text output; `--json` and the commands keep numbers.
use std::{fs, process::Command};

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn text_output_names_tickets_by_key_and_json_keeps_the_number(pool: sqlx::PgPool) {
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
    let (text, json) = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str]| {
            let output = Command::new(env!("CARGO_BIN_EXE_ainc"))
                .args(args)
                .env("AINC_API_URL", &url)
                .env("AINC_TOKEN_FILE", &token)
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap()
        };
        run(&["tickets", "create", "--title", "Keyed"]);
        (
            run(&["tickets", "list"]),
            run(&["--json", "tickets", "list"]),
        )
    })
    .await
    .unwrap();
    assert!(text.contains("key=T-"), "{text}");
    assert!(!json.contains("\"key\""), "{json}");
    assert!(json.contains("\"id\""));
    server.abort();
}
