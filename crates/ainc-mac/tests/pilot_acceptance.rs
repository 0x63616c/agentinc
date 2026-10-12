//! Actual isolated app + authenticated socket + real Metal. Never runs on Linux.
#![allow(clippy::disallowed_macros)] // test output
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
            Reply::Error { error, .. } => bail!("{id}: {error}"),
        }
    }
}
/// Choose an option in a select by its name, or the last option, and wait
/// until the trigger shows the acknowledged value.
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
    image::imageops::resize(&image, 1360, 828, image::imageops::FilterType::Lanczos3)
        .save(output.join(format!("{name}.png")))?;
    ensure!(image.dimensions() == (width, height), "dimensions");
    ensure!(
        frame > 0 && width == (1360. * scale) as u32 && height == (828. * scale) as u32,
        "frame/viewport"
    );
    let regions = if snap(client)?.nodes.iter().any(|node| node.role == "Dialog") {
        // A modal deliberately dims the shell below the normal ink threshold.
        vec![("dialog", [460, 150, 900, 680])]
    } else {
        vec![
            ("header", [150, 10, 1350, 40]),
            ("sidebar", [20, 110, 165, 390]),
            ("profile", [15, 735, 170, 812]),
        ]
    };
    for (region, [x0, y0, x1, y1]) in regions {
        let mut bright = 0;
        for y in (y0 as f32 * scale) as u32..(y1 as f32 * scale) as u32 {
            for x in (x0 as f32 * scale) as u32..(x1 as f32 * scale) as u32 {
                if image.get_pixel(x, y).0[..3].iter().any(|c| *c > 60) {
                    bright += 1;
                }
            }
        }
        ensure!(
            bright > 100,
            "{name}/{region} missing from capture: {bright}"
        );
    }
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
fn search_todos_create_via_driver_and_real_capture() -> Result<()> {
    fs::create_dir_all(".local")?;
    let temporary = tempfile::Builder::new()
        .prefix("p")
        .tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))?;
    let directory = temporary.path().canonicalize()?;
    let pilot = directory.join("s");
    let output = PathBuf::from("target/pilot-acceptance");
    fs::create_dir_all(&output)?;
    let log = fs::File::create(output.join("app.log"))?;
    let mut app = App(Process::new(env!("CARGO_BIN_EXE_AgentInc"))
        .args(["--gpui-pilot-session", pilot.to_str().unwrap()])
        // This suite verifies animated real captures, including the launch handoff.
        .arg("--gpui-pilot-visible")
        .env("AINC_SESSION_PATH", directory.join("session.json"))
        .env(
            "AINC_DISCOVERY_FILE",
            std::env::var_os("AINC_DISCOVERY_FILE")
                .context("run against an isolated cargo xtask dev stack")?,
        )
        .env("AINC_WINDOW_TITLE", "AgentInc Pilot Acceptance")
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
    wait(
        &mut client,
        Condition::Absent {
            author_id: "shell.launch".into(),
        },
    )?;
    let initial = snap(&mut client)?;
    ensure!(initial.by_id("shell.search")?.name.as_deref() == Some("Search"));
    ensure!(
        initial.by_id("sidebar.version")?.name.as_deref()
            == Some(ainc_release::identity::version().as_str())
    );
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
    let behind = snap(&mut client)?.by_id("nav.todos")?.reference.clone();
    let blocked = client.request(Command::Click { reference: behind })?;
    ensure!(
        matches!(blocked.result, Reply::Error { error, .. } if error.code == "target_occluded" || error.code == "stale_ref")
    );
    act(&mut client, "search.input", Some("Todos"))?;
    let filtered = wait(
        &mut client,
        Condition::Value {
            author_id: "search.input".into(),
            equals: "Todos".into(),
        },
    )?;
    ensure!(
        filtered
            .nodes
            .iter()
            .filter(|n| n
                .author_id
                .as_deref()
                .is_some_and(|id| id.starts_with("palette.result.pages.")))
            .count()
            == 1
    );
    act(&mut client, "palette.result.pages.page.todos", None)?;
    wait(
        &mut client,
        Condition::Absent {
            author_id: "search.dialog".into(),
        },
    )?;
    let stale = client.request(Command::Click {
        reference: filtered
            .by_id("palette.result.pages.page.todos")?
            .reference
            .clone(),
    })?;
    ensure!(matches!(stale.result, Reply::Error { error, .. } if error.code == "stale_ref"));
    act(&mut client, "todos.create", None)?;
    let empty = snap(&mut client)?;
    ensure!(!empty.by_id("todos.submit")?.enabled);
    act(&mut client, "todos.title", Some("Pilot café 👋"))?;
    wait(
        &mut client,
        Condition::Value {
            author_id: "todos.title".into(),
            equals: "Pilot café 👋".into(),
        },
    )?;
    // A concurrent wait must not hold the mutation lane.
    let mut observer = Client::connect(&manifest)?;
    let waiting = std::thread::spawn(move || {
        observer.call(Command::Wait {
            condition: Condition::Absent {
                author_id: "todos.title".into(),
            },
            timeout_ms: 3000,
        })
    });
    act(&mut client, "todos.submit", None)?;
    waiting
        .join()
        .unwrap()
        .context("Todo acknowledgement did not close the dialog")?;
    let created = snap(&mut client)?;
    let todo = created
        .nodes
        .iter()
        .find(|node| {
            node.name
                .as_deref()
                .is_some_and(|name| name.ends_with("Pilot café 👋"))
                && node
                    .author_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("todo.") && id.ends_with(".done"))
        })
        .context("acknowledged Todo missing from rendered snapshot")?;
    let todo_id = todo.author_id.clone().unwrap();
    screenshot(&mut client, "created", &output)?;
    fs::write(
        output.join("created.json"),
        serde_json::to_vec_pretty(&created)?,
    )?;
    // Completing moves it to Done; the row survives the refresh.
    act(&mut client, &todo_id, None)?;
    wait(
        &mut client,
        Condition::Absent {
            author_id: todo_id.clone(),
        },
    )?;
    act(&mut client, "todos.show-done", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: todo_id.clone(),
        },
    )?;
    screenshot(&mut client, "todo-done", &output)?;
    act(&mut client, "nav.calendar", None)?;
    act(&mut client, "calendar.create", None)?;
    act(&mut client, "calendar.title", Some("Pilot standup"))?;
    act(&mut client, "calendar.submit", None)?;
    wait(
        &mut client,
        Condition::Absent {
            author_id: "calendar.title".into(),
        },
    )?;
    let events = snap(&mut client)?;
    ensure!(
        events
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("Pilot standup")),
        "acknowledged Event missing from rendered snapshot"
    );
    screenshot(&mut client, "calendar", &output)?;
    act(&mut client, "nav.smart-home", None)?;
    act(&mut client, "home.light.0", None)?;
    screenshot(&mut client, "smart-home", &output)?;
    act(&mut client, "nav.todos", None)?;
    // A recent command also appears under Pages, with distinct native identities.
    // Rendering both used to crash the accessibility tree.
    client.call(Command::Press {
        key: "cmd-k".into(),
    })?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "search.dialog".into(),
        },
    )?;
    let recent = snap(&mut client)?;
    ensure!(
        recent.by_id("palette.result.recent.page.todos")?.reference
            != recent.by_id("palette.result.pages.page.todos")?.reference
    );
    // Typing through GPUI preserves normal selection and undo handling.
    act(&mut client, "search.input", Some("temporary"))?;
    client.call(Command::Press {
        key: "cmd-a".into(),
    })?;
    act(&mut client, "search.input", Some("Todos"))?;
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
    client.call(Command::Press {
        key: "cmd-,".into(),
    })?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "updates.check".into(),
        },
    )?;
    // Development profiles never activate Sparkle. Its live settings and installer
    // are exercised by the separate release-Mac upgrade gate.
    let settings = snap(&mut client)?;
    for id in ["updates.check", "updates.auto", "updates.download"] {
        ensure!(
            !settings.by_id(id)?.enabled,
            "development update control {id}"
        );
    }
    screenshot(&mut client, "update-settings", &output)?;
    act(&mut client, "nav.overview", None)?;
    wait(
        &mut client,
        Condition::Present {
            author_id: "overview.open.todos".into(),
        },
    )?;
    screenshot(&mut client, "overview", &output)?;
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
        "Todos, Calendar, Smart Home and Overview navigation passed via socket + GPUI dispatch. {metrics}"
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
