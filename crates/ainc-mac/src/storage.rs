//! Local assistant data. Shell layout deliberately remains in session.json.
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use std::{
    fs,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub completed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub id: i64,
    pub prompt: String,
    pub response: Option<String>,
    pub error: Option<String>,
}
pub struct Store(Connection);
impl Store {
    pub fn default_path() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("AGENTINC_DATABASE_PATH") {
            return Ok(path.into());
        }
        Ok(
            PathBuf::from(std::env::var_os("HOME").context("Home directory is unavailable")?)
                .join("Library/Application Support/Agentinc OS/assistant.sqlite3"),
        )
    }
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        // Restrict newly created databases and their SQLite sidecars to this user.
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        let connection = Connection::open(path).context("Could not open assistant database")?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 1 {
            bail!("This database was created by a newer app version.");
        }
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY, title TEXT NOT NULL CHECK(length(trim(title)) > 0),
                completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1)));
            CREATE TABLE IF NOT EXISTS turns (
                id INTEGER PRIMARY KEY, prompt TEXT NOT NULL CHECK(length(trim(prompt)) > 0),
                response TEXT, error TEXT);
            PRAGMA user_version=1;",
        )?;
        Ok(Self(connection))
    }
    pub fn recover_interrupted(&self) -> Result<()> {
        self.0.execute("UPDATE turns SET error='Reply interrupted. Retry when ready.' WHERE response IS NULL AND error IS NULL", [])?;
        Ok(())
    }
    pub fn todos(&self) -> Result<Vec<Todo>> {
        Ok(self
            .0
            .prepare("SELECT id,title,completed FROM todos ORDER BY completed,id DESC")?
            .query_map([], |row| {
                Ok(Todo {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    completed: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn add_todo(&self, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 500 {
            bail!("Use a task name between 1 and 500 characters.");
        }
        self.0
            .execute("INSERT INTO todos(title) VALUES (?)", [title])?;
        Ok(())
    }
    pub fn set_completed(&self, id: i64, completed: bool) -> Result<()> {
        self.0.execute(
            "UPDATE todos SET completed=? WHERE id=?",
            params![completed, id],
        )?;
        Ok(())
    }
    pub fn delete_todo(&self, id: i64) -> Result<()> {
        self.0.execute("DELETE FROM todos WHERE id=?", [id])?;
        Ok(())
    }
    pub fn turns(&self) -> Result<Vec<Turn>> {
        Ok(self
            .0
            .prepare("SELECT id,prompt,response,error FROM turns ORDER BY id")?
            .query_map([], |row| {
                Ok(Turn {
                    id: row.get(0)?,
                    prompt: row.get(1)?,
                    response: row.get(2)?,
                    error: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn begin_turn(&self, prompt: &str) -> Result<Turn> {
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > 8000 {
            bail!("Use a message between 1 and 8,000 characters.");
        }
        self.0
            .execute("INSERT INTO turns(prompt) VALUES (?)", [prompt])?;
        Ok(Turn {
            id: self.0.last_insert_rowid(),
            prompt: prompt.into(),
            response: None,
            error: None,
        })
    }
    pub fn save_turn(&self, turn: &Turn) -> Result<()> {
        self.0.execute(
            "UPDATE turns SET response=?,error=? WHERE id=?",
            params![turn.response, turn.error, turn.id],
        )?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newer_schema_is_preserved_and_new_files_are_private() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::current_dir()?
            .join(format!("target/schema-test-{}.sqlite3", std::process::id()));
        let store = Store::open(&path)?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        store.0.execute_batch("PRAGMA user_version=2;")?;
        drop(store);
        assert!(Store::open(&path).is_err());
        let connection = Connection::open(&path)?;
        assert_eq!(
            connection.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))?,
            2
        );
        drop(connection);
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn durable_tasks_chat_and_interrupted_retry() -> Result<()> {
        let path = std::env::current_dir()?.join(format!(
            "target/assistant-test-{}.sqlite3",
            std::process::id()
        ));
        let store = Store::open(&path)?;
        store
            .0
            .execute_batch("DELETE FROM todos; DELETE FROM turns;")?;
        assert!(store.add_todo("  ").is_err());
        store.add_todo("  Plan café visit 👋  ")?;
        store.add_todo("Delete me")?;
        let tasks = store.todos()?;
        store.delete_todo(tasks[0].id)?;
        store.set_completed(tasks[1].id, true)?;
        let mut turn = store.begin_turn("Hello")?;
        turn.response = Some("Hi".into());
        store.save_turn(&turn)?;
        let interrupted = store.begin_turn("Follow-up")?;
        drop(store);
        let reopened = Store::open(&path)?;
        reopened.recover_interrupted()?;
        assert_eq!(
            reopened.todos()?,
            vec![Todo {
                id: tasks[1].id,
                title: "Plan café visit 👋".into(),
                completed: true
            }]
        );
        assert_eq!(reopened.turns()?[0], turn);
        let mut retry = reopened.turns()?[1].clone();
        assert_eq!(retry.id, interrupted.id);
        assert!(retry.error.is_some());
        retry.error = None;
        reopened.save_turn(&retry)?;
        retry.response = Some("Resumed".into());
        reopened.save_turn(&retry)?;
        assert_eq!(reopened.turns()?.len(), 2);
        assert_eq!(reopened.turns()?[1], retry);
        drop(reopened);
        fs::remove_file(path)?;
        Ok(())
    }
}
