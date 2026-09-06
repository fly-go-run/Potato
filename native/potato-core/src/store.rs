use crate::{Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path};

pub struct Store {
    db: Connection,
    cipher: ChaCha20Poly1305,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        }
        let key_path = root.join("master.key");
        if !key_path.exists() {
            // Never replace a lost encryption key on an existing installation.
            if root.join("potato.sqlite3").exists() {
                return Err(Error::new(500, "Native storage encryption key is missing"));
            }
            let key = ChaCha20Poly1305::generate_key(&mut OsRng);
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&key_path)?;
            file.write_all(&key)?;
            file.sync_all()?;
        }
        let key = fs::read(key_path)?;
        let cipher = ChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| Error::new(500, "Invalid native storage encryption key"))?;
        let db = Connection::open(root.join("potato.sqlite3"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 2 {
            return Err(Error::new(
                409,
                "Native database requires a newer Potato version",
            ));
        }
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY, session_id TEXT UNIQUE NOT NULL, spec TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS messages (
                seq INTEGER PRIMARY KEY AUTOINCREMENT, chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
                frame TEXT NOT NULL, wire TEXT);
            CREATE INDEX IF NOT EXISTS messages_chat ON messages(chat_id, seq);
            CREATE TABLE IF NOT EXISTS questions (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, value TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS questions_session ON questions(session_id);
            CREATE INDEX IF NOT EXISTS questions_status ON questions(json_extract(value,'$.status'));
            UPDATE questions SET value=json_set(value,'$.status','skipped','$.answer',json_object('selected',json_array(),'text','')) WHERE json_extract(value,'$.status')='pending';
            PRAGMA user_version=2;")?;
        Ok(Self { db, cipher })
    }

    pub fn get(&self, key: &str, fallback: Value) -> Result<Value> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?;
        raw.map(|r| serde_json::from_str(&r).map_err(Into::into))
            .unwrap_or(Ok(fallback))
    }

    pub fn put(&self, key: &str, value: &Value) -> Result<()> {
        self.db.execute(
            "INSERT INTO settings VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value.to_string()],
        )?;
        Ok(())
    }

    pub fn put_batch(&mut self, values: &[(String, Value)]) -> Result<()> {
        let transaction = self.db.transaction()?;
        for (key, value) in values {
            transaction.execute("INSERT INTO settings VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value.to_string()])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn questions(&self, session: &str) -> Result<Vec<Value>> {
        let mut stmt = self
            .db
            .prepare("SELECT value FROM questions WHERE session_id=? ORDER BY rowid")?;
        let rows = stmt.query_map([session], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn question(&self, id: &str) -> Result<Value> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT value FROM questions WHERE id=?", [id], |r| r.get(0))
            .optional()?;
        Ok(serde_json::from_str(
            &raw.ok_or_else(|| Error::new(404, "Question not found"))?,
        )?)
    }
    pub fn save_question(&self, value: &Value) -> Result<()> {
        self.db.execute("INSERT INTO questions VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value",
            params![value["request_id"].as_str(),value["session_id"].as_str(),value.to_string()])?;
        Ok(())
    }

    pub fn seal(&self, value: &str) -> Result<String> {
        if value.is_empty() {
            return Ok(String::new());
        }
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let encrypted = self
            .cipher
            .encrypt(&nonce, value.as_bytes())
            .map_err(|_| Error::new(500, "Could not encrypt credential"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend(encrypted);
        Ok(STANDARD.encode(bytes))
    }

    pub fn unseal(&self, value: &str) -> Result<String> {
        if value.is_empty() {
            return Ok(String::new());
        }
        let bytes = STANDARD
            .decode(value)
            .map_err(|_| Error::new(500, "Invalid encrypted credential"))?;
        if bytes.len() < 28 {
            return Err(Error::new(500, "Invalid encrypted credential"));
        }
        let clear = self
            .cipher
            .decrypt(bytes[..12].into(), &bytes[12..])
            .map_err(|_| Error::new(500, "Could not decrypt credential"))?;
        String::from_utf8(clear).map_err(|_| Error::new(500, "Invalid credential encoding"))
    }

    pub fn chats(&self) -> Result<Vec<Value>> {
        let mut stmt = self
            .db
            .prepare("SELECT spec FROM chats ORDER BY rowid DESC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }

    pub fn chat(&self, id: &str) -> Result<Value> {
        let raw: Option<String> = self
            .db
            .query_row("SELECT spec FROM chats WHERE id=?", [id], |r| r.get(0))
            .optional()?;
        serde_json::from_str(&raw.ok_or_else(|| Error::new(404, "Chat not found"))?)
            .map_err(Into::into)
    }

    pub fn ensure_chat(&self, session: &str, title: &str) -> Result<Value> {
        let existing: Option<String> = self
            .db
            .query_row(
                "SELECT spec FROM chats WHERE session_id=?",
                [session],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(raw) = existing {
            return Ok(serde_json::from_str(&raw)?);
        }
        let now = chrono::Utc::now().to_rfc3339();
        let spec = json!({"id":uuid::Uuid::new_v4().to_string(), "session_id":session,
            "name":title.chars().take(60).collect::<String>(), "user_id":"default", "channel":"console",
            "created_at":now, "updated_at":now, "status":"idle", "pinned":false, "archived":false});
        self.save_chat(&spec)?;
        Ok(spec)
    }

    pub fn save_chat(&self, spec: &Value) -> Result<()> {
        self.db.execute(
            "INSERT INTO chats VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET spec=excluded.spec",
            params![
                spec["id"].as_str(),
                spec["session_id"].as_str(),
                spec.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, id: &str) -> Result<bool> {
        Ok(self.db.execute("DELETE FROM chats WHERE id=?", [id])? > 0)
    }

    pub fn append(&mut self, id: &str, frame: &Value, wire: Option<&Value>) -> Result<()> {
        let mut spec = self.chat(id)?;
        spec["updated_at"] = json!(chrono::Utc::now().to_rfc3339());
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT INTO messages(chat_id,frame,wire) VALUES (?,?,?)",
            params![id, frame.to_string(), wire.map(Value::to_string)],
        )?;
        tx.execute(
            "UPDATE chats SET spec=? WHERE id=?",
            params![spec.to_string(), id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn history(&self, id: &str, wire: bool) -> Result<Vec<Value>> {
        self.chat(id)?;
        let sql = if wire {
            "SELECT wire FROM messages WHERE chat_id=? AND wire IS NOT NULL ORDER BY seq"
        } else {
            "SELECT frame FROM messages WHERE chat_id=? ORDER BY seq"
        };
        let mut stmt = self.db.prepare(sql)?;
        let rows = stmt.query_map([id], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }

    pub fn import_history(&mut self, bundle: &Value) -> Result<Value> {
        let chats = crate::migration::validate(bundle)?;
        let tx = self.db.transaction()?;
        let mut imported = 0;
        for chat in chats {
            let mut spec = chat["spec"].clone();
            spec["status"] = json!("idle");
            // Existing native conversations always win, including a repeat import.
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO chats VALUES (?,?,?)",
                params![
                    spec["id"].as_str(),
                    spec["session_id"].as_str(),
                    spec.to_string()
                ],
            )?;
            if inserted == 0 {
                continue;
            }
            for frame in chat["messages"].as_array().unwrap() {
                tx.execute(
                    "INSERT INTO messages(chat_id,frame,wire) VALUES (?,?,?)",
                    params![
                        spec["id"].as_str(),
                        frame.to_string(),
                        crate::migration::wire(frame).map(|v| v.to_string())
                    ],
                )?;
            }
            imported += 1;
        }
        tx.commit()?;
        Ok(json!({"imported":imported,"skipped":chats.len()-imported}))
    }
}
