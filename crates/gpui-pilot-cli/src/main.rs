use anyhow::{Context, Result, bail, ensure};
use gpui_pilot::{
    protocol::{Command, Condition, Reply},
    transport::Client,
};
use std::{io::Read, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("gpui-pilot: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(flag) = args.next() else {
        bail!(
            "usage: gpui-pilot --instance /absolute/instance.json [--json] hello|snapshot|click REF|press KEY|type REF --stdin|wait CONDITION_JSON [TIMEOUT_MS]|screenshot"
        )
    };
    ensure!(flag == "--instance", "--instance is required");
    let path = args.next().context("missing instance path")?;
    let mut op = args.next().context("missing command")?;
    let json = op == "--json";
    if json {
        op = args.next().context("missing command")?;
    }
    let command = match op.as_str() {
        "hello" => Command::Hello,
        "snapshot" => Command::Snapshot,
        "click" => Command::Click {
            reference: args.next().context("missing reference")?,
        },
        "press" => Command::Press {
            key: args.next().context("missing key")?,
        },
        "type" => {
            let reference = args.next().context("missing reference")?;
            ensure!(
                args.next().as_deref() == Some("--stdin"),
                "type requires --stdin"
            );
            let mut text = String::new();
            std::io::stdin()
                .take((gpui_pilot::protocol::MAX_TEXT + 1) as u64)
                .read_to_string(&mut text)?;
            Command::Type { reference, text }
        }
        "wait" => Command::Wait {
            condition: serde_json::from_str::<Condition>(
                &args.next().context("missing condition JSON")?,
            )?,
            timeout_ms: args.next().map(|s| s.parse()).transpose()?.unwrap_or(3000),
        },
        "screenshot" => Command::Screenshot,
        _ => bail!("unknown command {op}"),
    };
    ensure!(args.next().is_none(), "unexpected arguments");
    let response = Client::connect(Path::new(&path))?.request(command)?;
    if json {
        println!("{}", serde_json::to_string(&response)?);
    }
    match response.result {
        Reply::Error { error, .. } => bail!(error),
        Reply::Ok { output } if !json => {
            if let Some(snapshot) = output.snapshot() {
                println!(
                    "{} — {} — frame {}",
                    snapshot.title, snapshot.window, snapshot.frame
                );
                for node in &snapshot.nodes {
                    println!(
                        "{} {} {:?}{}{}",
                        node.reference,
                        node.role,
                        node.name.as_deref().unwrap_or(""),
                        node.author_id
                            .as_ref()
                            .map(|id| format!(" [{id}]"))
                            .unwrap_or_default(),
                        node.value
                            .as_ref()
                            .map(|v| format!(" = {v:?}"))
                            .unwrap_or_default()
                    );
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        }
        _ => {}
    }
    Ok(())
}
