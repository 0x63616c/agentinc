//! Real ain cd processes, Postgres, SDK service and loopback Responses fixtures.
//! No user's profile, repository or subscription is touched.
use ainc_daemon::product::{Acknowledgement, Command, CommandRequest};
use axum::{Json, Router, extract::State, routing::post};
use serde_json::{Value, json};
use sqlx::{ConnectOptions, PgPool, postgres::PgListener};
use std::{
    os::unix::fs::PermissionsExt,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::io::{AsyncBufReadExt, BufReader};
use turnkeel::testing::Server;

struct Fixture {
    calls: AtomicUsize,
    requests: Mutex<Vec<Value>>,
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}
struct Stack {
    dir: tempfile::TempDir,
    server: Server,
    url: String,
    fixture: Arc<Fixture>,
    http: tokio::task::JoinHandle<()>,
}
impl Stack {
    async fn new() -> (
        Self,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("codex-fixture");
        std::fs::write(&script,r#"#!/usr/bin/env python3
import json,sys
for line in sys.stdin:
    request=json.loads(line)
    method=request.get('method')
    if method=='initialized': continue
    result={}
    if method=='account/read': result={'account':{'type':'chatgpt','email':'fixture@example.test','planType':'fixture'}}
    if method=='model/list': result={'data':[{'model':'fixture','displayName':'Fixture','isDefault':True}],'nextCursor':None}
    print(json.dumps({'id':request['id'],'result':result}),flush=True)
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let home = dir.path().join("codex-home");
        std::fs::create_dir(&home).unwrap();
        std::fs::write(home.join("auth.json"),json!({"tokens":{"access_token":"fixture-access-token","account_id":"fixture-account"}}).to_string()).unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let fixture = Arc::new(Fixture {
            calls: AtomicUsize::new(0),
            requests: Mutex::new(vec![]),
            entered: Mutex::new(Some(entered_tx)),
            release: tokio::sync::Mutex::new(Some(release_rx)),
        });
        let app=Router::new().route("/responses",post(|State(fixture):State<Arc<Fixture>>,Json(body):Json<Value>|async move {
            fixture.requests.lock().unwrap().push(body);
            if fixture.calls.fetch_add(1,Ordering::SeqCst)==0 {
                if let Some(entered)=fixture.entered.lock().unwrap().take() {let _=entered.send(());}
                let release=fixture.release.lock().await.take();if let Some(release)=release {let _=release.await;}
            }
            format!("data: {}\n\n",json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Durable fixture reply"}]}]}}))
        })).with_state(fixture.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/responses", listener.local_addr().unwrap());
        let http = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            Self {
                dir,
                server: Server::start().await.unwrap(),
                url,
                fixture,
                http,
            },
            entered_rx,
            release_tx,
        )
    }
    async fn daemon(&self, pool: &PgPool) -> tokio::process::Child {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_aincd"))
            .env(
                "DATABASE_URL",
                pool.connect_options().to_url_lossy().as_str(),
            )
            .env("AINC_DISCOVERY_FILE", self.dir.path().join("api-url"))
            .env("AINC_LEGACY_DIR", self.dir.path().join("empty-legacy"))
            .env(
                "AINC_RUNTIME_CONFIG",
                serde_json::to_string(&self.server.config()).unwrap(),
            )
            .env("AGENTINC_CODEX_HOME", self.dir.path().join("codex-home"))
            .env("AGENTINC_CODEX_PATH", self.dir.path().join("codex-fixture"))
            .env("AINC_TEST_RESPONSES_URL", &self.url)
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        loop {
            let line = lines
                .next_line()
                .await
                .unwrap()
                .expect("daemon exited before ready");
            if line.contains("AgentInc daemon ready") {
                break;
            }
        }
        // Drain logs so a long-lived worker cannot block on a full stdout pipe.
        tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
        child
    }
    fn client(&self) -> (reqwest::Client, String) {
        let token = std::fs::read_to_string(self.dir.path().join("owner-token")).unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", token.trim()).parse().unwrap(),
        );
        (
            reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .unwrap(),
            std::fs::read_to_string(self.dir.path().join("api-url"))
                .unwrap()
                .trim()
                .into(),
        )
    }
    async fn shutdown(self) {
        self.http.abort();
        self.server.shutdown().await.unwrap();
    }
}
async fn command(client: &reqwest::Client, url: &str, command: Command) -> Acknowledgement {
    client
        .post(format!("{url}/v1/commands"))
        .json(&CommandRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            command,
        })
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[sqlx::test]
async fn daemon_finishes_reply_after_http_client_exits(pool: PgPool) {
    let (stack, entered, release) = Stack::new().await;
    let mut daemon = stack.daemon(&pool).await;
    let (client, url) = stack.client();
    let conversation = command(&client, &url, Command::CreateConversation)
        .await
        .result_id
        .unwrap();
    let mut results = PgListener::connect_with(&pool).await.unwrap();
    results.listen("agentinc_results").await.unwrap();
    let accepted = command(
        &client,
        &url,
        Command::Send {
            conversation_id: conversation,
            prompt: "Keep working after client close".into(),
        },
    )
    .await;
    entered.await.unwrap();
    drop(client);
    release.send(()).unwrap();
    assert_eq!(
        results.recv().await.unwrap().payload(),
        accepted.result_id.unwrap().to_string()
    );
    let response: String = sqlx::query_scalar("SELECT response FROM turns WHERE id=$1")
        .bind(accepted.result_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(response, "Durable fixture reply");
    daemon.kill().await.unwrap();
    daemon.wait().await.unwrap();
    let mut replacement = stack.daemon(&pool).await;
    let (client, url) = stack.client();
    command(
        &client,
        &url,
        Command::Send {
            conversation_id: conversation,
            prompt: "Retain the prior dialogue".into(),
        },
    )
    .await;
    results.recv().await.unwrap();
    let requests = stack.fixture.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1]["input"].as_array().unwrap().len(), 3);
    assert_eq!(
        requests[1]["input"][1]["content"][0]["text"],
        "Durable fixture reply"
    );
    replacement.kill().await.unwrap();
    replacement.wait().await.unwrap();
    stack.shutdown().await;
}

#[sqlx::test]
async fn killed_daemon_recovers_accepted_conversation_in_a_new_process(pool: PgPool) {
    let (stack, entered, release) = Stack::new().await;
    let mut daemon = stack.daemon(&pool).await;
    let (client, url) = stack.client();
    let conversation = command(&client, &url, Command::CreateConversation)
        .await
        .result_id
        .unwrap();
    let mut results = PgListener::connect_with(&pool).await.unwrap();
    results.listen("agentinc_results").await.unwrap();
    let accepted = command(
        &client,
        &url,
        Command::Send {
            conversation_id: conversation,
            prompt: "Survive process death".into(),
        },
    )
    .await;
    entered.await.unwrap();
    daemon.kill().await.unwrap();
    daemon.wait().await.unwrap();
    let _ = release.send(());
    let mut replacement = stack.daemon(&pool).await;
    assert_eq!(
        results.recv().await.unwrap().payload(),
        accepted.result_id.unwrap().to_string()
    );
    let row: (String, String) = sqlx::query_as("SELECT state,response FROM turns WHERE id=$1")
        .bind(accepted.result_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row, ("completed".into(), "Durable fixture reply".into()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM turns")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(stack.fixture.calls.load(Ordering::SeqCst), 2); // only the interrupted model activity retried
    replacement.kill().await.unwrap();
    replacement.wait().await.unwrap();
    stack.shutdown().await;
}

#[sqlx::test]
async fn killed_ticket_worker_reconciles_dispatch_and_projects_one_result(pool: PgPool) {
    use ainc_daemon::tickets::{
        AssigneeKind, TicketCommand, TicketCommandRequest, TicketReceipt, TicketSnapshot,
        TicketStatus,
    };
    let (stack, entered, release) = Stack::new().await;
    let mut daemon = stack.daemon(&pool).await;
    let (client, url) = stack.client();
    async fn apply(client: &reqwest::Client, url: &str, command: TicketCommand) -> TicketReceipt {
        client
            .post(format!("{url}/v1/tickets/commands"))
            .json(&TicketCommandRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                command,
            })
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    apply(
        &client,
        &url,
        TicketCommand::RegisterAgent {
            name: "Recovery fixture".into(),
            instructions: "Return fixture evidence".into(),
            model: "fixture".into(),
        },
    )
    .await;
    let state: TicketSnapshot = client
        .get(format!("{url}/v1/tickets"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let agent = state
        .assignees
        .iter()
        .find(|a| a.kind == AssigneeKind::Agent)
        .unwrap();
    let id = apply(
        &client,
        &url,
        TicketCommand::Create {
            title: "Survive worker death".into(),
        },
    )
    .await
    .result_id
    .unwrap();
    let mut results = PgListener::connect_with(&pool).await.unwrap();
    results.listen("agentinc_results").await.unwrap();
    apply(
        &client,
        &url,
        TicketCommand::Assign {
            id,
            revision: 0,
            assignee_kind: AssigneeKind::Agent,
            assignee_id: agent.id.clone(),
        },
    )
    .await;
    entered.await.unwrap();
    daemon.kill().await.unwrap();
    daemon.wait().await.unwrap();
    let run: String = sqlx::query_scalar("SELECT run_id FROM ticket_runs WHERE ticket_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    // Recreate the crash gap between SDK dispatch and its database acknowledgement.
    sqlx::query("UPDATE dispatch_outbox SET dispatched=false WHERE run_id=$1")
        .bind(&run)
        .execute(&pool)
        .await
        .unwrap();
    let _ = release.send(());
    let mut replacement = stack.daemon(&pool).await;
    assert_eq!(results.recv().await.unwrap().payload(), run);
    let (client, url) = stack.client();
    let state: TicketSnapshot = client
        .get(format!("{url}/v1/tickets"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(state.tickets[0].status, TicketStatus::Done);
    assert_eq!(state.comments.len(), 1);
    assert_eq!(state.comments[0].body, "Durable fixture reply");
    assert_eq!(state.runs.len(), 1);
    assert_eq!(stack.fixture.calls.load(Ordering::SeqCst), 2);
    replacement.kill().await.unwrap();
    replacement.wait().await.unwrap();
    stack
        .server
        .replay(&turnkeel::RunId::new(run))
        .await
        .unwrap();
    stack.shutdown().await;
}
