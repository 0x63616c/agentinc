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
#[derive(Clone, Debug)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub updated: String,
    pub updated_at: i64,
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
        if version > 2 {
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
",
        )?;
        connection.execute_batch("PRAGMA foreign_keys=ON;")?;
        if version < 2 {
            connection.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE conversations (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, updated_at INTEGER NOT NULL DEFAULT (unixepoch()));
                INSERT INTO conversations(id,title) SELECT 1,'Previous conversation' WHERE EXISTS(SELECT 1 FROM turns);
                ALTER TABLE turns ADD COLUMN conversation_id INTEGER REFERENCES conversations(id) ON DELETE CASCADE;
                UPDATE turns SET conversation_id=1;
                CREATE INDEX turns_conversation ON turns(conversation_id,id);
                CREATE TABLE assistant_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                PRAGMA user_version=2;
                COMMIT;")?;
        }
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
    pub fn turns(&self, conversation: i64) -> Result<Vec<Turn>> {
        Ok(self
            .0
            .prepare(
                "SELECT id,prompt,response,error FROM turns WHERE conversation_id=? ORDER BY id",
            )?
            .query_map([conversation], |row| {
                Ok(Turn {
                    id: row.get(0)?,
                    prompt: row.get(1)?,
                    response: row.get(2)?,
                    error: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn begin_turn(&self, conversation: i64, prompt: &str) -> Result<Turn> {
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > 8000 {
            bail!("Use a message between 1 and 8,000 characters.");
        }
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO turns(prompt,conversation_id) VALUES (?,?)",
            params![prompt, conversation],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute("UPDATE conversations SET updated_at=unixepoch(),title=CASE WHEN title='New conversation' THEN ? ELSE title END WHERE id=?", params![prompt.chars().take(60).collect::<String>(),conversation])?;
        tx.commit()?;
        Ok(Turn {
            id,
            prompt: prompt.into(),
            response: None,
            error: None,
        })
    }
    pub fn conversations(&self) -> Result<Vec<Conversation>> {
        Ok(self.0.prepare("SELECT c.id,c.title,COALESCE((SELECT COALESCE(response,prompt) FROM turns WHERE conversation_id=c.id ORDER BY id DESC LIMIT 1),''),strftime('%Y-%m-%d %H:%M',c.updated_at,'unixepoch','localtime'),c.updated_at FROM conversations c ORDER BY updated_at DESC,id DESC")?
            .query_map([], |row| Ok(Conversation{id:row.get(0)?,title:row.get(1)?,snippet:row.get(2)?,updated:row.get::<_,Option<String>>(3)?.unwrap_or_default(),updated_at:row.get(4)?}))?.collect::<rusqlite::Result<_>>()?)
    }
    pub fn new_conversation(&self) -> Result<i64> {
        self.0.execute(
            "INSERT INTO conversations(title) VALUES ('New conversation')",
            [],
        )?;
        Ok(self.0.last_insert_rowid())
    }
    pub fn rename_conversation(&self, id: i64, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 120 {
            bail!("Use a title between 1 and 120 characters.");
        }
        self.0.execute(
            "UPDATE conversations SET title=? WHERE id=?",
            params![title, id],
        )?;
        Ok(())
    }
    pub fn delete_conversation(&self, id: i64) -> Result<()> {
        self.0
            .execute("DELETE FROM conversations WHERE id=?", [id])?;
        Ok(())
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .query_row(
                "SELECT value FROM assistant_settings WHERE key=?",
                [key],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.0.execute("INSERT INTO assistant_settings(key,value) VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn save_turn(&self, turn: &Turn) -> Result<()> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "UPDATE turns SET response=?,error=? WHERE id=?",
            params![turn.response, turn.error, turn.id],
        )?;
        if turn.response.is_some() {
            tx.execute("UPDATE conversations SET updated_at=unixepoch() WHERE id=(SELECT conversation_id FROM turns WHERE id=?)",[turn.id])?;
        }
        tx.commit()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_legacy_turns_and_conversations_are_isolated() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("migration.sqlite3");
        let legacy = Connection::open(&path)?;
        legacy.execute_batch("CREATE TABLE turns(id INTEGER PRIMARY KEY,prompt TEXT NOT NULL,response TEXT,error TEXT); INSERT INTO turns VALUES(1,'Legacy question','Legacy reply',NULL); PRAGMA user_version=1;")?;
        drop(legacy);
        let db = Store::open(&path)?;
        assert_eq!(db.conversations()?.len(), 1);
        assert_eq!(db.turns(1)?[0].response.as_deref(), Some("Legacy reply"));
        let next = db.new_conversation()?;
        db.begin_turn(next, "New chat")?;
        db.rename_conversation(next, "Renamed 👋")?;
        assert!(db.rename_conversation(next, " ").is_err());
        assert_eq!(db.conversations()?[0].title, "Renamed 👋");
        assert_eq!(db.turns(next)?.len(), 1);
        db.delete_conversation(next)?;
        assert!(db.turns(next)?.is_empty());
        assert_eq!(db.turns(1)?.len(), 1);
        db.set_setting("model", "example")?;
        drop(db);
        let db = Store::open(&path)?;
        assert_eq!(db.conversations()?.len(), 1);
        assert_eq!(db.setting("model")?.as_deref(), Some("example"));
        drop(db);
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn newer_schema_is_preserved_and_new_files_are_private() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("schema.sqlite3");
        let store = Store::open(&path)?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        store.0.execute_batch("PRAGMA user_version=3;")?;
        drop(store);
        assert!(Store::open(&path).is_err());
        let connection = Connection::open(&path)?;
        assert_eq!(
            connection.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))?,
            3
        );
        drop(connection);
        fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn durable_tasks_chat_and_interrupted_retry() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("assistant.sqlite3");
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
        let conversation = store.new_conversation()?;
        let mut turn = store.begin_turn(conversation, "Hello")?;
        turn.response = Some("Hi".into());
        store.save_turn(&turn)?;
        let interrupted = store.begin_turn(conversation, "Follow-up")?;
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
        assert_eq!(reopened.turns(conversation)?[0], turn);
        let mut retry = reopened.turns(conversation)?[1].clone();
        assert_eq!(retry.id, interrupted.id);
        assert!(retry.error.is_some());
        retry.error = None;
        reopened.save_turn(&retry)?;
        retry.response = Some("Resumed".into());
        reopened.save_turn(&retry)?;
        assert_eq!(reopened.turns(conversation)?.len(), 2);
        assert_eq!(reopened.turns(conversation)?[1], retry);
        drop(reopened);
        fs::remove_file(path)?;
        Ok(())
    }
}
