//! Repeat-safe, one-way legacy import. Only the daemon opens SQLite and only
//! read-only; SQLx owns the destination. A read transaction pins a WAL snapshot.
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use sqlx::PgPool;
use std::path::Path;

type LegacyTurn = (i64, i64, String, Option<String>, Option<String>);

struct Legacy {
    conversations: Vec<(i64, String, i64)>,
    turns: Vec<LegacyTurn>,
    todos: Vec<(i64, String, bool)>,
    settings: Vec<(String, String)>,
    session: Option<serde_json::Value>,
}

fn read(directory: &Path) -> Result<Legacy> {
    let mut data = Legacy {
        conversations: vec![],
        turns: vec![],
        todos: vec![],
        settings: vec![],
        session: None,
    };
    let session_path = directory.join("session.json");
    if session_path.exists() {
        data.session = Some(
            serde_json::from_slice(&std::fs::read(session_path)?)
                .context("legacy session JSON is invalid; source preserved")?,
        );
    }
    let path = directory.join("assistant.sqlite3");
    if !path.exists() {
        return Ok(data);
    }
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let tx = db.transaction()?;
    let version: u32 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > 2 {
        bail!("legacy database is newer than supported schema 2; source preserved");
    }
    let has_table = |name: &str| -> rusqlite::Result<bool> {
        tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
            [name],
            |r| r.get(0),
        )
    };
    if has_table("todos")? {
        data.todos = tx
            .prepare("SELECT id,title,completed FROM todos ORDER BY id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
    }
    if version == 2 {
        data.conversations = tx
            .prepare("SELECT id,title,updated_at FROM conversations ORDER BY id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        data.settings = tx
            .prepare("SELECT key,value FROM assistant_settings")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
    }
    if has_table("turns")? {
        let query = if version == 2 {
            "SELECT id,conversation_id,prompt,response,error FROM turns ORDER BY id"
        } else {
            "SELECT id,1,prompt,response,error FROM turns ORDER BY id"
        };
        data.turns = tx
            .prepare(query)?
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        if version < 2 && !data.turns.is_empty() {
            data.conversations
                .push((1, "Previous conversation".into(), 0));
        }
    }
    tx.commit()?;
    Ok(data)
}

/// Run before accepting commands. A completed receipt prevents a second import,
/// including after the user deletes imported records. Conflicts fail atomically.
pub async fn import(pool: &PgPool, directory: &Path) -> Result<bool> {
    if !directory.exists() {
        return Ok(false);
    }
    if !directory.join("assistant.sqlite3").exists() && !directory.join("session.json").exists() {
        return Ok(false);
    }
    let directory = directory.canonicalize()?;
    let source = directory.to_string_lossy().into_owned();
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('agentinc-legacy-import',0))")
        .execute(&mut *tx)
        .await?;
    let imported: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM legacy_imports WHERE source=$1)")
            .bind(&source)
            .fetch_one(&mut *tx)
            .await?;
    if imported {
        return Ok(false);
    }
    // Import is first-start only: refusing a populated destination is safer than
    // remapping or overwriting existing IDs behind the user's back.
    let populated: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversations UNION ALL SELECT id FROM todos)",
    )
    .fetch_one(&mut *tx)
    .await?;
    if populated {
        bail!(
            "legacy import requires an empty product database; no source or destination records changed"
        );
    }
    let legacy = tokio::task::spawn_blocking(move || read(&directory)).await??;
    for (id, title, updated) in legacy.conversations {
        sqlx::query("INSERT INTO conversations(id,title,updated_at) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(title)
            .bind(updated)
            .execute(&mut *tx)
            .await?;
    }
    for (id, conversation, prompt, response, error) in legacy.turns {
        let (state, error) = if response.is_some() && error.is_none() {
            ("completed", None)
        } else {
            (
                "failed",
                Some(error.unwrap_or_else(|| {
                    "Reply interrupted before import. Retry when ready.".into()
                })),
            )
        };
        sqlx::query("INSERT INTO turns(id,conversation_id,prompt,response,error,state) VALUES ($1,$2,$3,$4,$5,$6)").bind(id).bind(conversation).bind(prompt).bind(response).bind(error).bind(state).execute(&mut *tx).await?;
    }
    for (id, title, completed) in legacy.todos {
        sqlx::query("INSERT INTO todos(id,title,completed) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(title)
            .bind(completed)
            .execute(&mut *tx)
            .await?;
    }
    for (key, value) in legacy.settings {
        sqlx::query("INSERT INTO assistant_settings(key,value) VALUES ($1,$2) ON CONFLICT(workspace_id,key) DO NOTHING").bind(key).bind(value).execute(&mut *tx).await?;
    }
    for table in ["conversations", "turns", "todos"] {
        sqlx::query(&format!("SELECT setval(pg_get_serial_sequence('{table}','id'), COALESCE((SELECT max(id) FROM {table}),0)+1,false)")).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO legacy_imports(source,session) VALUES ($1,$2)")
        .bind(source)
        .bind(legacy.session)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
