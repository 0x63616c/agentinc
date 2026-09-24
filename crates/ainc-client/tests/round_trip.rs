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

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn generated_product_commands_and_nullable_state_round_trip(pool: sqlx::PgPool) {
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
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer fixture".parse().unwrap());
    let client = Client::new_with_client(
        &url,
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap(),
    );
    let ack = client
        .product_command()
        .body(ainc_client::types::CommandRequest {
            operation_id: "797a8c57-7931-4884-88cd-49c85ab224e0".into(),
            command: ainc_client::types::Command::CreateTodo {
                title: "generated client".into(),
            },
        })
        .send()
        .await
        .unwrap();
    let state = client.product_state().send().await.unwrap();
    assert_eq!(state.todos[0].id, ack.result_id.unwrap());
    assert_eq!(state.settings.model, None);
    assert_eq!(state.settings.selected_conversation, None);
    let accepted = client
        .tickets_command()
        .body(ainc_client::types::TicketCommandRequest {
            operation_id: "9cdc4782-ef0c-478a-8c91-176dc31aa3ed".into(),
            command: ainc_client::types::TicketCommand::AddComment {
                ticket_id: ack.result_id.unwrap(),
                body: "Generated command evidence".into(),
            },
        })
        .send()
        .await
        .unwrap();
    let tickets = client.tickets_state().send().await.unwrap();
    assert_eq!(
        tickets.tickets[0].status,
        ainc_client::types::TicketStatus::ToDo
    );
    assert_eq!(tickets.comments[0].id, accepted.result_id.unwrap());
    assert_eq!(tickets.comments[0].body, "Generated command evidence");
    server.abort();
}

#[sqlx::test(migrations = "../ainc-daemon/migrations")]
async fn generated_automation_rule_pause_and_run_now_round_trip(pool: sqlx::PgPool) {
    use ainc_client::types::*;
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
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer fixture".parse().unwrap());
    let client = Client::new_with_client(
        &url,
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap(),
    );
    client
        .tickets_command()
        .body(TicketCommandRequest {
            operation_id: "00000000-0000-4000-8000-000000000001".into(),
            command: TicketCommand::RegisterAgent {
                name: "Fixture".into(),
                instructions: "No live model".into(),
                model: "fixture".into(),
            },
        })
        .send()
        .await
        .unwrap();
    let agent = client
        .tickets_state()
        .send()
        .await
        .unwrap()
        .assignees
        .iter()
        .find(|a| a.kind == AssigneeKind::Agent)
        .unwrap()
        .id
        .clone();
    let rule = client
        .automations_command()
        .body(AutomationRequest {
            operation_id: "00000000-0000-4000-8000-000000000002".into(),
            command: AutomationCommand::Save {
                id: None,
                revision: None,
                name: "Generated rule".into(),
                proposal: TicketProposal {
                    title: "Generated Ticket".into(),
                    agent_id: agent,
                },
                every_minutes: 30,
            },
        })
        .send()
        .await
        .unwrap()
        .result_id
        .clone();
    client
        .automations_command()
        .body(AutomationRequest {
            operation_id: "00000000-0000-4000-8000-000000000003".into(),
            command: AutomationCommand::Pause {
                id: rule.clone(),
                revision: 0,
                paused: true,
            },
        })
        .send()
        .await
        .unwrap();
    let request = AutomationRequest {
        operation_id: "00000000-0000-4000-8000-000000000004".into(),
        command: AutomationCommand::RunNow {
            id: rule,
            revision: 1,
        },
    };
    let first = client
        .automations_command()
        .body(request.clone())
        .send()
        .await
        .unwrap()
        .result_id
        .clone();
    let second = client
        .automations_command()
        .body(request)
        .send()
        .await
        .unwrap()
        .result_id
        .clone();
    assert_eq!(first, second);
    let state = client.automations_state().send().await.unwrap();
    assert!(state.rules[0].paused);
    assert_eq!(state.occurrences.len(), 1);
    assert_eq!(state.occurrences[0].state, "waiting_for_worker");
    server.abort();
}
