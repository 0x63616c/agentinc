use ainc_client::Client;

#[tokio::test]
async fn generated_version_operation_round_trips() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let pool = sqlx::PgPool::connect_lazy("postgres://unused:unused@localhost/unused").unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            ainc_daemon::app(ainc_daemon::product::Product::new(pool, "fixture".into()).unwrap()),
        )
        .await
        .unwrap();
    });
    ainc_client::install_tls_provider();
    let result = Client::new(&url).version_show().send().await.unwrap();
    assert_eq!(result.version, ainc_identity::VERSION);
    assert_eq!(
        result.headers().get(ainc_identity::SERVER_HEADER).unwrap(),
        ainc_identity::server_header().as_str()
    );
    server.abort();
}

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn generated_todo_and_calendar_commands_round_trip(pool: sqlx::PgPool) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            ainc_daemon::app(ainc_daemon::product::Product::new(pool, "fixture".into()).unwrap()),
        )
        .await
        .unwrap();
    });
    ainc_client::install_tls_provider();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer fixture".parse().unwrap());
    let client = Client::new_with_client(
        &url,
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap(),
    );
    let todo = client
        .todos_command()
        .body(ainc_client::types::TodoRequest {
            operation_id: "0d3d9c3e-5a4d-4f1a-9c7e-2b6a8d1f0e11".into(),
            command: ainc_client::types::TodoCommand::Add {
                title: "Water the plants".into(),
            },
        })
        .send()
        .await
        .unwrap();
    client
        .todos_command()
        .body(ainc_client::types::TodoRequest {
            operation_id: "6f0a2c1b-3d4e-4a5f-8b6c-7d8e9f0a1b2c".into(),
            command: ainc_client::types::TodoCommand::Complete {
                id: todo.result_id,
                completed: true,
            },
        })
        .send()
        .await
        .unwrap();
    let todos = client.todos_state().send().await.unwrap();
    assert_eq!(todos.todos.len(), 1);
    assert!(todos.todos[0].completed);

    client
        .calendar_command()
        .body(ainc_client::types::CalendarRequest {
            operation_id: "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d".into(),
            command: ainc_client::types::CalendarCommand::Create {
                title: "Dentist".into(),
                starts_at: 1_700_000_000,
                ends_at: 1_700_003_600,
            },
        })
        .send()
        .await
        .unwrap();
    let calendar = client.calendar_state().send().await.unwrap();
    assert_eq!(calendar.events[0].title, "Dentist");
    assert_eq!(
        calendar.events[0].ends_at - calendar.events[0].starts_at,
        3_600
    );
    server.abort();
}
