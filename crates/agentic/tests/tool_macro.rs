use agentic::{Tool, tool};
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

#[tokio::test]
async fn macro_derives_name_description_schema_and_call() {
    assert_eq!(add.name(), "add");
    assert_eq!(add.description(), "Adds two numbers.");
    let schema = add.schema();
    assert_eq!(schema["properties"]["a"]["type"], "integer");
    assert_eq!(schema["required"], json!(["a", "b"]));

    assert_eq!(add.call(json!({"a": 2, "b": 3})).await.unwrap(), json!(5));
    assert!(matches!(
        add.call(json!({"a": 2})).await,
        Err(agentic::ToolError::InvalidArguments(_))
    ));

    assert_eq!(no_args.description(), "explicit description wins");
    assert_eq!(no_args.call(json!({})).await.unwrap(), json!("ok"));
}
