//! Actual isolated app + authenticated socket + real Metal. Never runs on Linux.
#![cfg(target_os = "macos")]
use anyhow::{Context, Result, bail, ensure};
use gpui_pilot::{protocol::*, transport::Client};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command as Process, Stdio},
    time::{Duration, Instant},
};

struct App(Child);
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn snap(client: &mut Client) -> Result<Snapshot> {
    Ok(client
        .call(Command::Snapshot)?
        .snapshot()
        .context("snapshot reply")?
        .clone())
}
fn act(client: &mut Client, id: &str, text: Option<&str>) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = snap(client)?;
        let reference = snapshot.by_id(id)?.reference.clone();
        let command = match text {
            Some(text) => Command::Type {
                reference,
                text: text.into(),
            },
            None => Command::Click { reference },
        };
        match client.request(command)?.result {
            Reply::Ok { output } => return Ok(output.snapshot().context("acted frame")?.clone()),
            Reply::Error { error, .. }
                if error.code == "stale_ref" && Instant::now() < deadline =>
            {
                continue;
            }
            Reply::Error { error, .. } => bail!(error),
        }
    }
}
fn wait_disabled(client: &mut Client, id: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = snap(client)?;
        // The page disables all controls while saving; the back control becomes
        // available only when acknowledgement and the refreshed state are loaded.
        if !snapshot.by_id(id)?.enabled && snapshot.by_id("tickets.back")?.enabled {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "Ticket acknowledgement timed out: {id}"
        );
        wait(
            client,
            Condition::FrameAfter {
                frame: snapshot.frame,
            },
        )?;
    }
}
fn wait(client: &mut Client, condition: Condition) -> Result<Snapshot> {
    Ok(client
        .call(Command::Wait {
            condition,
            timeout_ms: 3000,
        })?
        .snapshot()
        .context("wait frame")?
        .clone())
}
fn screenshot(client: &mut Client, name: &str, output: &std::path::Path) -> Result<()> {
    let Output::Screenshot {
        path,
        width,
        height,
        frame,
        scale,
        ..
    } = client.call(Command::Screenshot)?
    else {
        bail!("screenshot reply")
    };
    let image = image::open(&path)?.into_rgba8();
    ensure!(image.dimensions() == (width, height), "dimensions");
    ensure!(
        frame > 0 && width == (1360. * scale) as u32 && height == (828. * scale) as u32,
        "frame/viewport"
    );
    for (region, [x0, y0, x1, y1]) in [
        ("header", [150, 10, 1350, 40]),
        ("sidebar", [20, 110, 165, 390]),
        ("profile", [15, 784, 170, 816]),
        ("Evee", [1110, 56, 1340, 90]),
    ] {
        let mut bright = 0;
        for y in (y0 as f32 * scale) as u32..(y1 as f32 * scale) as u32 {
            for x in (x0 as f32 * scale) as u32..(x1 as f32 * scale) as u32 {
                if image.get_pixel(x, y).0[..3].iter().any(|c| *c > 60) {
                    bright += 1;
                }
            }
        }
        ensure!(bright > 100, "{region} missing from capture: {bright}");
    }
    image::imageops::resize(&image, 1360, 828, image::imageops::FilterType::Lanczos3)
        .save(output.join(format!("{name}.png")))?;
    Ok(())
}
fn latency(client: &mut Client, command: Command, count: usize) -> Result<serde_json::Value> {
    let mut samples = Vec::new();
    for _ in 0..count {
        let start = Instant::now();
        client.call(command.clone())?;
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    samples.sort_by(f64::total_cmp);
    Ok(
        serde_json::json!({"samples": count, "median_ms": samples[count/2], "p95_ms": samples[(count*95).div_ceil(100)-1]}),
    )
}
#[test]
fn search_tickets_create_via_driver_and_real_capture() -> Result<()> {
    fs::create_dir_all(".local")?;
    let temporary = tempfile::Builder::new().prefix("p").tempdir_in(".local")?;
    let directory = temporary.path().canonicalize()?;
    let pilot = directory.join("s");
    let output = PathBuf::from("target/pilot-acceptance");
    fs::create_dir_all(&output)?;
    let log = fs::File::create(output.join("app.log"))?;
    let mut app = App(Process::new(env!("CARGO_BIN_EXE_agentinc-os"))
        .args(["--gpui-pilot-session", pilot.to_str().unwrap()])
        .env("AGENTINC_SESSION_PATH", directory.join("session.json"))
        .env(
            "AINC_DISCOVERY_FILE",
            std::env::var_os("AINC_DISCOVERY_FILE")
                .context("run against an isolated cargo xtask dev stack")?,
        )
        .env("AINC_LEGACY_DIR", directory.join("legacy"))
        .env("AGENTINC_CODEX_HOME", directory.join("codex"))
        .env("AGENTINC_WINDOW_TITLE", "Agentinc Pilot Acceptance")
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()?);
    let manifest = pilot.join("instance.json");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !manifest.exists() {
        ensure!(
            app.0.try_wait()?.is_none(),
            "app exited: see target/pilot-acceptance/app.log"
        );
        ensure!(Instant::now() < deadline, "app startup timed out");
        std::thread::sleep(Duration::from_millis(20)); // Process readiness only; UI waits use the protocol.
    }
    let mut client = Client::connect(&manifest)?;
    client.call(Command::Hello)?;
    let initial = snap(&mut client)?;
    ensure!(initial.by_id("shell.search")?.name.as_deref() == Some("Search"));
    fs::write(
        output.join("initial.json"),
        serde_json::to_vec_pretty(&initial)?,
    )?;
    screenshot(&mut client, "initial", &output)?;
    act(&mut client, "shell.search", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "search.dialog".into(),
        },
    )?;
    // The overlay must reject a fresh ref to the obscured sidebar.
    let behind = snap(&mut client)?.by_id("nav.tickets")?.reference.clone();
    let blocked = client.request(Command::Click { reference: behind })?;
    ensure!(
        matches!(blocked.result, Reply::Error { error, .. } if error.code == "target_occluded" || error.code == "stale_ref")
    );
    act(&mut client, "search.input", Some("Tickets"))?;
    let filtered = wait(
        &mut client,
        Condition::Value {
            author_id: "search.input".into(),
            equals: "Tickets".into(),
        },
    )?;
    ensure!(
        filtered
            .nodes
            .iter()
            .filter(|n| n
                .author_id
                .as_deref()
                .is_some_and(|id| id.starts_with("search.result.")))
            .count()
            == 1
    );
    act(&mut client, "search.result.tickets", None)?;
    wait(
        &mut client,
        Condition::Absent {
            author_id: "search.dialog".into(),
        },
    )?;
    let stale = client.request(Command::Click {
        reference: filtered.by_id("search.result.tickets")?.reference.clone(),
    })?;
    ensure!(matches!(stale.result, Reply::Error { error, .. } if error.code == "stale_ref"));
    act(&mut client, "tickets.create", None)?;
    let empty = snap(&mut client)?;
    ensure!(!empty.by_id("tickets.submit")?.enabled);
    act(&mut client, "tickets.title", Some("Pilot café 👋"))?;
    wait(
        &mut client,
        Condition::Value {
            author_id: "tickets.title".into(),
            equals: "Pilot café 👋".into(),
        },
    )?;
    // A concurrent wait must not hold the mutation lane.
    let mut observer = Client::connect(&manifest)?;
    let waiting = std::thread::spawn(move || {
        observer.call(Command::Wait {
            condition: Condition::Absent {
                author_id: "tickets.title".into(),
            },
            timeout_ms: 3000,
        })
    });
    act(&mut client, "tickets.submit", None)?;
    waiting
        .join()
        .unwrap()
        .context("Ticket acknowledgement did not close the dialog")?;
    let created = snap(&mut client)?;
    let ticket = created
        .nodes
        .iter()
        .find(|node| {
            node.name.as_deref() == Some("Pilot café 👋")
                && node
                    .author_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("ticket.") && !id.ends_with(".complete"))
        })
        .context("acknowledged Ticket missing from rendered snapshot")?;
    let ticket_id = ticket.author_id.clone().unwrap();
    act(&mut client, &ticket_id, None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "tickets.status.backlog".into(),
        },
    )?;
    act(&mut client, "tickets.status.backlog", None)?;
    wait_disabled(&mut client, "tickets.status.backlog")?;
    act(&mut client, "tickets.comment", None)?;
    act(
        &mut client,
        "tickets.comment",
        Some("Pilot evidence: scoped Comments survive refresh."),
    )?;
    act(&mut client, "tickets.post", None)?;
    wait(
        &mut client,
        Condition::Value {
            author_id: "tickets.comment".into(),
            equals: "".into(),
        },
    )?;
    let detail = snap(&mut client)?;
    ensure!(
        detail
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("Pilot evidence: scoped Comments survive refresh."))
    );
    screenshot(&mut client, "ticket-comments", &output)?;
    act(&mut client, "nav.agents", None)?;
    act(&mut client, "agents.create", None)?;
    act(&mut client, "agents.name", Some("Pilot worker"))?;
    act(&mut client, "agents.instructions", None)?;
    act(
        &mut client,
        "agents.instructions",
        Some("Produce fixture evidence for the assigned Ticket."),
    )?;
    act(&mut client, "tickets.submit", None)?;
    wait(
        &mut client,
        Condition::Absent {
            author_id: "agents.name".into(),
        },
    )?;
    let agents = snap(&mut client)?;
    let agent = agents
        .nodes
        .iter()
        .rfind(|n| {
            n.name.as_deref() == Some("Pilot worker")
                && n.author_id
                    .as_deref()
                    .is_some_and(|s| s.starts_with("agent."))
        })
        .context("registered agent missing")?;
    let agent_id = agent
        .author_id
        .as_ref()
        .unwrap()
        .strip_prefix("agent.")
        .unwrap()
        .to_owned();
    screenshot(&mut client, "agents", &output)?;
    act(&mut client, "nav.tickets", None)?;
    let assign = format!("tickets.assign.{agent_id}");
    act(&mut client, &assign, None)?;
    wait_disabled(&mut client, &assign)?;
    // Backlog assignment does not invoke a provider. Reassign to the human before
    // making the Ticket actionable, so acceptance cannot spend a subscription.
    act(&mut client, "tickets.assign.owner", None)?;
    wait_disabled(&mut client, "tickets.assign.owner")?;
    act(&mut client, "tickets.status.in_progress", None)?;
    wait_disabled(&mut client, "tickets.status.in_progress")?;
    screenshot(&mut client, "ticket-assignee", &output)?;
    act(&mut client, "nav.today", None)?;
    let today_id = ticket_id.replacen("ticket.", "today.ticket.", 1);
    wait(
        &mut client,
        Condition::Present {
            author_id: today_id.clone(),
        },
    )?;
    screenshot(&mut client, "today-tickets", &output)?;
    act(&mut client, &today_id, None)?;
    act(&mut client, "tickets.status.done", None)?;
    wait_disabled(&mut client, "tickets.status.done")?;
    act(&mut client, "tickets.status.to_do", None)?;
    wait_disabled(&mut client, "tickets.status.to_do")?;
    screenshot(&mut client, "created", &output)?;
    fs::write(
        output.join("created.json"),
        serde_json::to_vec_pretty(&created)?,
    )?;
    act(&mut client, "nav.automations", None)?;
    act(&mut client, "automations.create", None)?;
    act(
        &mut client,
        "automations.name",
        Some("Pilot recurring review"),
    )?;
    act(&mut client, "automations.prompt", None)?;
    act(
        &mut client,
        "automations.prompt",
        Some("Provide scheduled fixture evidence"),
    )?;
    act(&mut client, &format!("automations.agent.{agent_id}"), None)?;
    screenshot(&mut client, "automation-create", &output)?;
    act(&mut client, "automations.save", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "automations.pause".into(),
        },
    )?;
    act(&mut client, "automations.pause", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "automations.pause".into(),
            equals: "Resume".into(),
        },
    )?;
    act(&mut client, "automations.edit", None)?;
    act(&mut client, "automations.minutes", None)?;
    client.call(Command::Press {
        key: "cmd-a".into(),
    })?;
    act(&mut client, "automations.minutes", Some("45"))?;
    act(&mut client, "automations.save", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "automations.run".into(),
        },
    )?;
    act(&mut client, "automations.run", None)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let linked = loop {
        let snapshot = snap(&mut client)?;
        if let Some(node) = snapshot.nodes.iter().find(|n| {
            n.author_id
                .as_deref()
                .is_some_and(|id| id.starts_with("automations.ticket."))
        }) {
            break node.author_id.clone().unwrap();
        }
        ensure!(
            Instant::now() < deadline,
            "Automation did not produce a linked Ticket"
        );
        wait(
            &mut client,
            Condition::FrameAfter {
                frame: snapshot.frame,
            },
        )?;
    };
    screenshot(&mut client, "automation-history", &output)?;
    act(&mut client, &linked, None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "tickets.back".into(),
        },
    )?;
    screenshot(&mut client, "automation-ticket", &output)?;
    act(&mut client, "nav.automations", None)?;
    act(&mut client, "automations.pause", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "automations.pause".into(),
            equals: "Pause".into(),
        },
    )?;
    act(&mut client, "automations.pause", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "automations.pause".into(),
            equals: "Resume".into(),
        },
    )?;
    // Typing through GPUI preserves normal selection and undo handling.
    client.call(Command::Press {
        key: "cmd-k".into(),
    })?;
    act(&mut client, "search.input", Some("temporary"))?;
    client.call(Command::Press {
        key: "cmd-a".into(),
    })?;
    act(&mut client, "search.input", Some("Tickets"))?;
    client.call(Command::Press {
        key: "cmd-z".into(),
    })?;
    wait(
        &mut client,
        Condition::Value {
            author_id: "search.input".into(),
            equals: "temporary".into(),
        },
    )?;
    client.call(Command::Press {
        key: "escape".into(),
    })?;
    let timeout = client.request(Command::Wait {
        condition: Condition::Present {
            author_id: "never-mounted".into(),
        },
        timeout_ms: 50,
    })?;
    ensure!(
        matches!(timeout.result, Reply::Error { error, snapshot: Some(_) } if error.code == "deadline_exceeded")
    );
    act(&mut client, "profile", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "updates.check".into(),
        },
    )?;
    act(&mut client, "updates.auto", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "updates.auto".into(),
            equals: "Automatic checks: Off".into(),
        },
    )?;
    act(&mut client, "updates.auto", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "updates.auto".into(),
            equals: "Automatic checks: On".into(),
        },
    )?;
    act(&mut client, "updates.interval", None)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "updates.interval".into(),
            equals: "Check weekly".into(),
        },
    )?;
    act(&mut client, "updates.interval", None)?;
    screenshot(&mut client, "update-settings", &output)?;
    let metrics = serde_json::json!({
        "snapshot": latency(&mut client, Command::Snapshot, 100)?,
        "press_and_committed_frame": latency(&mut client, Command::Press { key: "escape".into() }, 50)?,
        "retina_png": latency(&mut client, Command::Screenshot, 10)?
    });
    fs::write(
        output.join("latency.json"),
        serde_json::to_vec_pretty(&metrics)?,
    )?;
    println!(
        "Tickets, Comments, assignee, four states and Today passed via socket + GPUI dispatch. {metrics}"
    );
    client
        .call(Command::Press {
            key: "cmd-q".into(),
        })
        .ok();
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.0.try_wait()?.is_none() {
        ensure!(Instant::now() < deadline, "owned app did not quit");
        std::thread::sleep(Duration::from_millis(20));
    }
    ensure!(!manifest.exists(), "normal quit left session endpoints");
    Ok(())
}

#[test]
fn today_read_failure_is_unavailable_not_empty() -> Result<()> {
    fs::create_dir_all(".local")?;
    let temporary = tempfile::Builder::new().prefix("u").tempdir_in(".local")?;
    let directory = temporary.path().canonicalize()?;
    let pilot = directory.join("s");
    let output = PathBuf::from("target/pilot-acceptance");
    fs::create_dir_all(&output)?;
    fs::write(
        directory.join("owner-token"),
        "isolated-unavailable-fixture",
    )?;
    let mut app = App(Process::new(env!("CARGO_BIN_EXE_agentinc-os"))
        .args(["--gpui-pilot-session", pilot.to_str().unwrap()])
        .env("AGENTINC_SESSION_PATH", directory.join("session.json"))
        .env("AINC_DISCOVERY_FILE", directory.join("api-url"))
        .env("AINC_TOKEN_FILE", directory.join("owner-token"))
        .env("AINC_DAEMON_URL", "http://127.0.0.1:1")
        .env("AINC_LEGACY_DIR", directory.join("legacy"))
        .env("AGENTINC_CODEX_HOME", directory.join("codex"))
        .env("AGENTINC_WINDOW_TITLE", "Agentinc Unavailable Acceptance")
        .stdout(Stdio::null())
        .stderr(fs::File::create(output.join("unavailable.log"))?)
        .spawn()?);
    let manifest = pilot.join("instance.json");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !manifest.exists() {
        ensure!(app.0.try_wait()?.is_none(), "unavailable fixture exited");
        ensure!(Instant::now() < deadline, "startup timeout");
        std::thread::yield_now();
    }
    let mut client = Client::connect(&manifest)?;
    wait(
        &mut client,
        Condition::Name {
            author_id: "today.tickets.summary".into(),
            equals: "Tickets unavailable".into(),
        },
    )?;
    let snapshot = snap(&mut client)?;
    ensure!(
        !snapshot
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("All caught up"))
    );
    screenshot(&mut client, "today-unavailable", &output)?;
    Ok(())
}
