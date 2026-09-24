use futures::StreamExt;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use turnkeel::testing::{Script, ScriptedModel, Server, text};
use turnkeel::{Agent, Error, Event, ModelResponse, RunId, Runtime, RuntimeConfig, SessionId};

fn agent() -> Agent {
    Agent::builder("recovery-v1")
        .model(
            ScriptedModel::new()
                .on_user("first", text("one"))
                .on_user("second", text("two")),
        )
        .build()
}

// Child entry point in this same executable; ordinary test runs do nothing here.
#[tokio::test]
async fn recovery_worker() -> anyhow::Result<()> {
    let Ok(config) = std::env::var("TURNKEEL_RECOVERY_CONFIG") else {
        return Ok(());
    };
    let config: RuntimeConfig = serde_json::from_str(&config)?;
    let agent = agent();
    let runtime = Runtime::configured(config, std::slice::from_ref(&agent)).await?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    println!("READY");
    while let Some(line) = lines.next_line().await? {
        let session = if line == "start" {
            runtime.session(&agent).await?
        } else {
            runtime.session_by_id(&agent, SessionId::new(line))
        };
        println!("SESSION {}", session.id());
        let prompt = lines.next_line().await?.unwrap();
        if prompt != "observe" {
            session.send(prompt).await?;
        }
        let mut events = session.events();
        let mut ended = 0;
        let expected = lines.next_line().await?.unwrap().parse::<usize>()?;
        while let Some(event) = events.next().await {
            if event? == Event::TurnEnded {
                ended += 1;
                if ended == expected {
                    break;
                }
            }
        }
        println!("COMPLETED {ended}");
    }
    runtime.shutdown().await?;
    Ok(())
}

async fn child(
    config: &RuntimeConfig,
) -> anyhow::Result<(
    tokio::process::Child,
    tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
)> {
    let mut process = tokio::process::Command::new(std::env::current_exe()?)
        .args(["--exact", "recovery_worker", "--nocapture"])
        .env("TURNKEEL_RECOVERY_CONFIG", serde_json::to_string(config)?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut lines = BufReader::new(process.stdout.take().unwrap()).lines();
    until(&mut lines, "READY").await?;
    Ok((process, lines))
}

async fn until(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    prefix: &str,
) -> anyhow::Result<String> {
    while let Some(line) = lines.next_line().await? {
        if line.starts_with(prefix) {
            return Ok(line);
        }
    }
    anyhow::bail!("worker exited before {prefix}")
}

#[tokio::test]
async fn session_survives_killed_process_with_recorded_history() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let (mut first, mut lines) = child(&server.config()).await?;
    first
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"start\nfirst\n1\n")
        .await?;
    let id = until(&mut lines, "SESSION ")
        .await?
        .trim_start_matches("SESSION ")
        .to_owned();
    until(&mut lines, "COMPLETED 1").await?;
    first.kill().await?;
    first.wait().await?;
    // Submit while no worker in the target group is alive. The replacement must
    // replay the first turn and pick up this accepted message.
    let mut control_config = server.config();
    control_config.worker_group.push_str("-control");
    let control = Runtime::configured(control_config, &[]).await?;
    control
        .session_by_id(&agent(), SessionId::new(&id))
        .send("second")
        .await?;
    let (mut second, mut lines) = child(&server.config()).await?;
    second
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("{id}\nobserve\n2\n").as_bytes())
        .await?;
    until(&mut lines, "COMPLETED 2").await?;
    drop(second.stdin.take());
    assert!(second.wait().await?.success());
    control.shutdown().await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn start_deduplicates_before_and_after_completion_and_replays() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let script = Script::new();
    let agent = Agent::builder("dedup-v1").model(script.model()).build();
    let runtime = Runtime::configured(server.config(), std::slice::from_ref(&agent)).await?;
    let id = RunId::new("receipt-1");
    let first = runtime.start_with_id(id.clone(), &agent, "hello").await?;
    let call = script.next_model_call().await;
    let duplicate = runtime.start_with_id(id.clone(), &agent, "hello").await?;
    call.reply(ModelResponse::text("once"));
    assert_eq!(first.result().await?, "once");
    assert_eq!(duplicate.result().await?, "once");
    let completed = runtime.start_with_id(id.clone(), &agent, "hello").await?;
    assert_eq!(completed.result().await?, "once");
    runtime.shutdown().await?;
    server.replay(&id).await?;
    let replacement = Runtime::configured(server.config(), &[agent]).await?;
    assert_eq!(replacement.run_by_id(id).result().await?, "once");
    replacement.shutdown().await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn cancellation_stops_a_gated_run() -> anyhow::Result<()> {
    let script = Script::new();
    let agent = Agent::builder("cancel-v1").model(script.model()).build();
    let runtime = Runtime::test().await?;
    let run = runtime.start(&agent, "wait").await?;
    let mut call = script.next_model_call().await;
    run.cancel().await?;
    assert!(matches!(run.result().await, Err(Error::Cancelled)));
    call.cancelled().await;
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn run_events_arrive_before_completion_and_replay_afterward() -> anyhow::Result<()> {
    let script = Script::new();
    let agent = Agent::builder("events-v1").model(script.model()).build();
    let runtime = Runtime::test().await?;
    let run = runtime.start(&agent, "hello").await?;
    let call = script.next_model_call().await;
    let mut events = run.events();
    assert_eq!(
        events.next().await.unwrap()?,
        Event::Message(turnkeel::Message::user("hello"))
    );
    call.reply(ModelResponse::text("world"));
    let mut rest = Vec::new();
    while let Some(event) = events.next().await {
        rest.push(event?);
    }
    assert_eq!(rest.len(), 2);
    assert_eq!(rest.last(), Some(&Event::TurnEnded));
    let mut replay = run.events();
    let mut count = 0;
    while let Some(event) = replay.next().await {
        event?;
        count += 1;
    }
    assert_eq!(count, 3);
    runtime.shutdown().await?;
    Ok(())
}

#[derive(Clone)]
struct ReceiptTool {
    directory: std::path::PathBuf,
    gate: bool,
}
impl turnkeel::Tool for ReceiptTool {
    fn name(&self) -> &str {
        "receipt"
    }
    fn description(&self) -> &str {
        "Commit a durable fixture effect once."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object"})
    }
    fn call(
        &self,
        ctx: turnkeel::ToolCtx,
        _: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, Result<serde_json::Value, turnkeel::ToolError>> {
        let path = self.directory.join("effect");
        let gate = self.gate;
        let key = ctx.idempotency_key().to_owned();
        Box::pin(async move {
            use std::io::Write;
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(key.as_bytes()).unwrap();
                    file.sync_all().unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    assert_eq!(std::fs::read_to_string(&path).unwrap(), key);
                }
                Err(error) => panic!("fixture effect: {error}"),
            }
            if gate {
                println!("EFFECT_COMMITTED");
                std::future::pending::<()>().await;
            }
            Ok(serde_json::json!("one effect"))
        })
    }
}

#[tokio::test]
async fn effect_worker() -> anyhow::Result<()> {
    let Ok(config) = std::env::var("TURNKEEL_EFFECT_CONFIG") else {
        return Ok(());
    };
    let config = serde_json::from_str(&config)?;
    let gate = std::env::var("TURNKEEL_EFFECT_GATE")? == "yes";
    let agent = Agent::builder("effect-v1")
        .model(
            ScriptedModel::new()
                .on_user(
                    "go",
                    turnkeel::testing::tool_call("receipt", serde_json::json!({})),
                )
                .on_tool_result("receipt", text("finished")),
        )
        .tool(ReceiptTool {
            directory: std::env::var("TURNKEEL_EFFECT_DIR")?.into(),
            gate,
        })
        .build();
    let runtime = Runtime::configured(config, std::slice::from_ref(&agent)).await?;
    let run = runtime
        .start_with_id(RunId::new("crash-effect"), &agent, "go")
        .await?;
    assert_eq!(run.result().await?, "finished");
    println!("RECOVERED");
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn killed_worker_recovers_unacknowledged_effect_without_repeating_it() -> anyhow::Result<()> {
    let server = Server::start().await?;
    let directory = std::env::temp_dir().join(format!("turnkeel-effect-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory)?;
    let spawn = |gate: &str| -> anyhow::Result<_> {
        Ok(tokio::process::Command::new(std::env::current_exe()?)
            .args(["--exact", "effect_worker", "--nocapture"])
            .env(
                "TURNKEEL_EFFECT_CONFIG",
                serde_json::to_string(&server.config())?,
            )
            .env("TURNKEEL_EFFECT_DIR", &directory)
            .env("TURNKEEL_EFFECT_GATE", gate)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?)
    };
    let mut first = spawn("yes")?;
    let mut lines = BufReader::new(first.stdout.take().unwrap()).lines();
    until(&mut lines, "EFFECT_COMMITTED").await?;
    let committed = std::fs::read(directory.join("effect"))?;
    first.kill().await?;
    first.wait().await?;
    let mut replacement = spawn("no")?;
    let mut lines = BufReader::new(replacement.stdout.take().unwrap()).lines();
    until(&mut lines, "RECOVERED").await?;
    assert!(replacement.wait().await?.success());
    assert_eq!(std::fs::read(directory.join("effect"))?, committed);
    assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
    server.replay(&RunId::new("crash-effect")).await?;
    server.shutdown().await?;
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
