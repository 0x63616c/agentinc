use agentinc::{Tool, ToolCtx, tool};
use serde_json::json;

/// Adds two numbers.
#[tool]
async fn add(a: i64, b: i64) -> anyhow::Result<i64> {
    Ok(a + b)
}

#[tool(description = "explicit description wins")]
async fn no_args() -> anyhow::Result<&'static str> {
    Ok("ok")
}

/// Sends an email. Never safe to repeat.
#[tool(idempotent = false)]
async fn send_email(to: String) -> anyhow::Result<String> {
    Ok(format!("sent to {to}"))
}

/// Echoes the idempotency key it was given.
#[tool]
async fn key(ctx: &ToolCtx, label: String) -> anyhow::Result<String> {
    Ok(format!("{label}:{}", ctx.idempotency_key()))
}

fn ctx() -> ToolCtx {
    ToolCtx::new("run-1/t0/c0")
}

#[tokio::test]
async fn macro_derives_name_description_schema_and_call() {
    assert_eq!(add.name(), "add");
    assert_eq!(add.description(), "Adds two numbers.");
    assert!(add.idempotent());
    let schema = add.schema();
    assert_eq!(schema["properties"]["a"]["type"], "integer");
    assert_eq!(schema["required"], json!(["a", "b"]));

    assert_eq!(
        add.call(ctx(), json!({"a": 2, "b": 3})).await.unwrap(),
        json!(5)
    );
    assert!(matches!(
        add.call(ctx(), json!({"a": 2})).await,
        Err(agentinc::ToolError::InvalidArguments(_))
    ));

    assert_eq!(no_args.description(), "explicit description wins");
    assert_eq!(no_args.call(ctx(), json!({})).await.unwrap(), json!("ok"));
}

#[tokio::test]
async fn idempotent_flag_and_ctx_param() {
    assert!(!send_email.idempotent());
    assert_eq!(
        send_email.description(),
        "Sends an email. Never safe to repeat."
    );

    // ctx is not part of the schema
    assert_eq!(key.schema()["required"], json!(["label"]));
    assert_eq!(
        key.call(ctx(), json!({"label": "x"})).await.unwrap(),
        json!("x:run-1/t0/c0")
    );
}
