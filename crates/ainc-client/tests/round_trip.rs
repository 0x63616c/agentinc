use ainc_client::{Client, types::TicketContract};

#[tokio::test]
async fn generated_ticket_operation_round_trips() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let pool = sqlx::PgPool::connect_lazy("postgres://unused:unused@localhost/unused").unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, ainc_daemon::router(pool))
            .await
            .unwrap();
    });
    let ticket = TicketContract {
        title: "round trip".into(),
    };
    let result = Client::new(&url)
        .ticket_contract()
        .body(ticket)
        .send()
        .await
        .unwrap();
    assert_eq!(result.title, "round trip");
    assert_eq!(
        result.headers().get("agent-inc-server").unwrap(),
        "aincd/0.1.0 (api 1)"
    );
    server.abort();
}
