//! A real daemon, real Postgres and a protocol fixture with an explicit gate.
//! The test never launches a real provider or reads a real user's files.
use ainc_daemon::product::{Acknowledgement, Command, CommandRequest, Snapshot};
use sqlx::{ConnectOptions, PgPool, postgres::PgListener};
use std::{os::unix::fs::PermissionsExt, process::Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[sqlx::test]
async fn daemon_finishes_reply_after_http_client_exits(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let gate = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = dir.path().join("codex-fixture");
    std::fs::write(&fixture,r#"#!/usr/bin/env python3
import json,os,socket,sys
for line in sys.stdin:
    request=json.loads(line)
    method=request.get('method')
    if method=='initialized': continue
    result={}
    if method=='account/read': result={'account':{'type':'chatgpt','email':'fixture@example.test','planType':'fixture'}}
    if method=='thread/start': result={'thread':{'id':'fixture-thread'}}
    print(json.dumps({'id':request['id'],'result':result}),flush=True)
    if method=='turn/start':
        with socket.create_connection(('127.0.0.1',int(os.environ['FIXTURE_GATE']))) as gate:
            gate.recv(1)
        print(json.dumps({'method':'item/completed','params':{'item':{'type':'agentMessage','text':'Completed after client close'}}}),flush=True)
        print(json.dumps({'method':'turn/completed','params':{'turn':{'status':'completed'}}}),flush=True)
"#).unwrap();
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut daemon = tokio::process::Command::new(env!("CARGO_BIN_EXE_aincd"))
        .env(
            "DATABASE_URL",
            pool.connect_options().to_url_lossy().as_str(),
        )
        .env("AINC_DISCOVERY_FILE", dir.path().join("api-url"))
        .env("AINC_LEGACY_DIR", dir.path().join("empty-legacy"))
        .env("AGENTINC_CODEX_HOME", dir.path().join("codex-home"))
        .env("AGENTINC_CODEX_PATH", fixture)
        .env(
            "FIXTURE_GATE",
            gate.local_addr().unwrap().port().to_string(),
        )
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = tokio::io::BufReader::new(daemon.stdout.take().unwrap()).lines();
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
    let url = std::fs::read_to_string(dir.path().join("api-url")).unwrap();
    let token = std::fs::read_to_string(dir.path().join("owner-token")).unwrap();
    let client = reqwest::Client::new();
    let request = |command| CommandRequest {
        command,
        operation_id: uuid::Uuid::new_v4().to_string(),
    };
    let conversation: Acknowledgement = client
        .post(format!("{}/v1/commands", url.trim()))
        .bearer_auth(&token)
        .json(&request(Command::CreateConversation))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let accepted: Acknowledgement = client
        .post(format!("{}/v1/commands", url.trim()))
        .bearer_auth(&token)
        .json(&request(Command::Send {
            conversation_id: conversation.result_id.unwrap(),
            prompt: "Do not depend on this client".into(),
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let (mut gate, _) = gate.accept().await.unwrap();
    let mut results = PgListener::connect_with(&pool).await.unwrap();
    results.listen("agentinc_results").await.unwrap();
    drop(client); // The client disappears while Codex is explicitly gated.
    gate.write_all(b"x").await.unwrap();
    let event = results.recv().await.unwrap();
    assert_eq!(event.payload(), accepted.result_id.unwrap().to_string());
    let reopened = reqwest::Client::new();
    let snapshot: Snapshot = reopened
        .get(format!("{}/v1/state", url.trim()))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(snapshot.turns[0].state, "completed");
    assert_eq!(
        snapshot.turns[0].response.as_deref(),
        Some("Completed after client close")
    );
    daemon.kill().await.unwrap();
    daemon.wait().await.unwrap();
    assert_eq!(
        ainc_daemon::product::snapshot(&pool).await.unwrap().turns[0].state,
        "completed"
    );
}
