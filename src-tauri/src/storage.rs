use crate::models::{AppSettings, Conversation, DocumentInfo, Message, ModelInfo, SourceRef};
use crate::rag::{cosine, embed, lexical_overlap};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use uuid::Uuid;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)
            .with_context(|| format!("Could not open local database at {}", path.display()))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              pinned INTEGER NOT NULL DEFAULT 0,
              archived INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
              role TEXT NOT NULL,
              content TEXT NOT NULL,
              mode TEXT NOT NULL DEFAULT 'chat',
              created_at TEXT NOT NULL,
              sources_json TEXT NOT NULL DEFAULT '[]',
              feedback TEXT,
              saved INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS documents (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              local_path TEXT NOT NULL,
              file_type TEXT NOT NULL,
              size_bytes INTEGER NOT NULL,
              page_count INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL,
              created_at TEXT NOT NULL,
              tags_json TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE IF NOT EXISTS document_chunks (
              id TEXT PRIMARY KEY,
              document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
              page INTEGER,
              content TEXT NOT NULL,
              vector_json TEXT NOT NULL,
              ordinal INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_document ON document_chunks(document_id);
            CREATE TABLE IF NOT EXISTS models (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              local_path TEXT NOT NULL,
              parameters TEXT NOT NULL,
              quantization TEXT NOT NULL,
              context_length INTEGER NOT NULL,
              size_bytes INTEGER NOT NULL,
              required_ram_bytes INTEGER NOT NULL,
              built_in INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL DEFAULT 'unloaded',
              is_default INTEGER NOT NULL DEFAULT 0,
              download_url TEXT,
              sha256 TEXT,
              description TEXT NOT NULL DEFAULT '',
              capability_tier TEXT NOT NULL DEFAULT 'General',
              best_for TEXT NOT NULL DEFAULT 'General chat'
            );
            CREATE TABLE IF NOT EXISTS settings (
              id INTEGER PRIMARY KEY CHECK (id = 1),
              value_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit_logs (
              id TEXT PRIMARY KEY,
              action TEXT NOT NULL,
              details TEXT NOT NULL,
              created_at TEXT NOT NULL
            );
            "#,
        )?;

        for migration in [
            "ALTER TABLE models ADD COLUMN download_url TEXT",
            "ALTER TABLE models ADD COLUMN sha256 TEXT",
            "ALTER TABLE models ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE models ADD COLUMN capability_tier TEXT NOT NULL DEFAULT 'General'",
            "ALTER TABLE models ADD COLUMN best_for TEXT NOT NULL DEFAULT 'General chat'",
        ] {
            let _ = connection.execute(migration, []);
        }

        let database = Self {
            connection: Mutex::new(connection),
        };
        database.ensure_settings()?;
        Ok(database)
    }

    fn ensure_settings(&self) -> Result<()> {
        let settings = serde_json::to_string(&AppSettings::default())?;
        self.connection.lock().execute(
            "INSERT OR IGNORE INTO settings (id, value_json) VALUES (1, ?1)",
            [settings],
        )?;
        Ok(())
    }

    pub fn seed_model_catalog(
        &self,
        models_directory: &Path,
        bundled_model_path: &Path,
    ) -> Result<()> {
        std::fs::create_dir_all(models_directory)?;
        let catalog = [
            (
                "lfm2.5-230m",
                "LFM2.5-230M-fine-tunned",
                "230M",
                32768u32,
                153_406_304u64,
                536_870_912u64,
                "LFM2.5-230M-Q4_K_M.gguf",
                "LiquidAI/LFM2.5-230M-GGUF",
                "7bbd90384d3deffe4c646ec9643b212802d32d4ce417c90a1ec9282100650062",
                "Small and fast. The best first model for nearly any Windows PC.",
                "Essential",
                "Fast chat, extraction, and short summaries",
            ),
            (
                "lfm2.5-350m",
                "LFM2.5-350M",
                "350M",
                32768u32,
                229_312_224u64,
                805_306_368u64,
                "LFM2.5-350M-Q4_K_M.gguf",
                "LiquidAI/LFM2.5-350M-GGUF",
                "7e6f72643caafc9a68256686638c4d7916f2cec76d1df478d4c3ddcd95a6aed4",
                "A stronger small model with modest memory and storage needs.",
                "Everyday",
                "Summaries, writing help, and document questions",
            ),
            (
                "lfm2.5-1.2b",
                "LFM2.5-1.2B Instruct",
                "1.2B",
                32768u32,
                730_895_168u64,
                1_610_612_736u64,
                "LFM2.5-1.2B-Instruct-Q4_K_M.gguf",
                "LiquidAI/LFM2.5-1.2B-Instruct-GGUF",
                "b1b3de114215d9507409a662a501a631095a479a419584e8a2ded6304b19b4f5",
                "Better writing and instruction following for modern laptops.",
                "Strong",
                "Writing, research overviews, and longer conversations",
            ),
            (
                "lfm2.5-2.6b",
                "LFM2.5-2.6B",
                "2.6B",
                32768u32,
                1_674_455_040u64,
                3_221_225_472u64,
                "LFM2.5-2.6B-Q4_K_M.gguf",
                "LiquidAI/LFM2.5-2.6B-GGUF",
                "02a8b7e17487d326e46d68ce0ba24211e1b80a14c4cd0597fa73c1cd697f52ed",
                "Higher quality for systems with more memory. A larger download.",
                "Advanced",
                "Research, analysis, and richer writing",
            ),
            (
                "lfm2-700m",
                "LFM2-700M",
                "700M",
                32768u32,
                468_624_320u64,
                1_207_959_552u64,
                "LFM2-700M-Q4_K_M.gguf",
                "LiquidAI/LFM2-700M-GGUF",
                "684e8406dc13321452b3f6aeca432776e2a6a7e1ad6c23f7887b8fe3efbe2efa",
                "A balanced Liquid model for older and mid-range laptops.",
                "Balanced",
                "General chat and document work",
            ),
            (
                "qwen3-0.6b",
                "Qwen3-0.6B",
                "600M",
                32768u32,
                639_446_688u64,
                1_342_177_280u64,
                "Qwen3-0.6B-Q8_0.gguf",
                "Qwen/Qwen3-0.6B-GGUF",
                "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
                "A compact multilingual model with thinking and non-thinking modes.",
                "Balanced",
                "Multilingual chat, translation, and reasoning",
            ),
            (
                "qwen3.5-0.8b",
                "Qwen3.5-0.8B",
                "800M",
                32768u32,
                563_036_064u64,
                1_342_177_280u64,
                "Qwen3.5-0.8B-Q4_0.gguf",
                "ggml-org/Qwen3.5-0.8B-GGUF",
                "57d1997790d1744fba5b40a7317df71ea5e2acee28c47e78f0cce39c0703f8cf",
                "A newer compact Qwen model converted by the llama.cpp publisher.",
                "Strong",
                "Multilingual instructions and structured answers",
            ),
            (
                "lfm2.5-8b-a1b",
                "LFM2.5-8B-A1B",
                "8B / 1B active",
                32768u32,
                5_155_564_768u64,
                6_442_450_944u64,
                "LFM2.5-8B-A1B-Q4_K_M.gguf",
                "LiquidAI/LFM2.5-8B-A1B-GGUF",
                "4923ec14f06b968b74d663e5949867d2d9c3bf13a20b8be1a9f9af39989b2bb0",
                "A high-capability mixture-of-experts model for well-equipped systems.",
                "Expert",
                "Complex research, tools, analysis, and demanding writing",
            ),
        ];
        let connection = self.connection.lock();
        let previous_bundled_default: Option<String> = connection
            .query_row(
                "SELECT id FROM models WHERE built_in = 1 AND is_default = 1 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        for (
            id,
            name,
            parameters,
            context,
            size,
            ram,
            file_name,
            repository,
            sha256,
            description,
            capability,
            best_for,
        ) in catalog
        {
            let built_in = id == "lfm2.5-230m";
            let revision = match id {
                "lfm2.5-230m" => "fb5e743241d08c98626e04c13828feffae4acdfb",
                "lfm2.5-350m" => "d86ad5aad24b8bd87a7c4821439e63e7ba589bc3",
                "lfm2.5-1.2b" => "afbd8eaeab5dd94ba0b079ebfb02517d19641e38",
                "lfm2.5-2.6b" => "f4a289c8a200a5ca71005ba7abc2dad33058a450",
                "lfm2-700m" => "43e05b4efd464155b3807bde379942bb43d8ee3c",
                "qwen3-0.6b" => "23749fefcc72300e3a2ad315e1317431b06b590a",
                "qwen3.5-0.8b" => "8fea620810c4afa23dd6443f999a48574c1611a3",
                "lfm2.5-8b-a1b" => "dfd5fdcad7a1c0d31473fb4ca443b8befbacddf0",
                _ => unreachable!("catalog revisions must be pinned"),
            };
            let path = if built_in {
                bundled_model_path.to_path_buf()
            } else {
                models_directory.join(file_name)
            };
            let status = if path.exists() {
                "unloaded"
            } else {
                "not-downloaded"
            };
            let url = format!(
                "https://huggingface.co/{repository}/resolve/{revision}/{file_name}?download=true"
            );
            connection.execute(
                r#"INSERT INTO models
                   (id, name, local_path, parameters, quantization, context_length, size_bytes, required_ram_bytes, built_in, status, is_default, download_url, sha256, description, capability_tier, best_for)
                   VALUES (?1, ?2, ?3, ?4, 'Q4_K_M', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                   ON CONFLICT(id) DO UPDATE SET name=excluded.name, local_path=excluded.local_path, parameters=excluded.parameters,
                     context_length=excluded.context_length, size_bytes=excluded.size_bytes, required_ram_bytes=excluded.required_ram_bytes,
                     status=CASE WHEN models.status IN ('downloading','paused') THEN models.status ELSE excluded.status END,
                     download_url=excluded.download_url, sha256=excluded.sha256, description=excluded.description,
                     built_in=excluded.built_in, capability_tier=excluded.capability_tier, best_for=excluded.best_for"#,
                params![id, name, path.to_string_lossy(), parameters, context, size as i64, ram as i64, built_in, status, built_in, url, sha256, description, capability, best_for],
            )?;
        }
        if previous_bundled_default.as_deref() == Some("lfm2.5-1.2b") {
            connection.execute("UPDATE models SET is_default = 0", [])?;
            connection.execute(
                "UPDATE models SET is_default = 1 WHERE id = 'lfm2.5-230m'",
                [],
            )?;
        }
        Ok(())
    }

    pub fn conversations(&self) -> Result<Vec<Conversation>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT id, title, pinned, archived, created_at, updated_at FROM conversations ORDER BY pinned DESC, updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                pinned: row.get(2)?,
                archived: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn create_conversation(&self, title: Option<&str>) -> Result<Conversation> {
        let now = Utc::now().to_rfc3339();
        let conversation = Conversation {
            id: Uuid::new_v4().to_string(),
            title: title.unwrap_or("New conversation").to_string(),
            pinned: false,
            archived: false,
            created_at: now.clone(),
            updated_at: now,
        };
        self.connection.lock().execute(
            "INSERT INTO conversations (id, title, pinned, archived, created_at, updated_at) VALUES (?1, ?2, 0, 0, ?3, ?4)",
            params![conversation.id, conversation.title, conversation.created_at, conversation.updated_at],
        )?;
        self.audit("conversation.created", &conversation.id)?;
        Ok(conversation)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> Result<()> {
        let clean = title.trim();
        if clean.is_empty() {
            return Err(anyhow!("A conversation title cannot be empty."));
        }
        self.connection.lock().execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![clean, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn set_conversation_flag(&self, id: &str, flag: &str, value: bool) -> Result<()> {
        let column = match flag {
            "pinned" => "pinned",
            "archived" => "archived",
            _ => return Err(anyhow!("Unknown conversation flag.")),
        };
        self.connection.lock().execute(
            &format!("UPDATE conversations SET {column} = ?1, updated_at = ?2 WHERE id = ?3"),
            params![value, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        self.connection
            .lock()
            .execute("DELETE FROM conversations WHERE id = ?1", [id])?;
        self.audit("conversation.deleted", id)?;
        Ok(())
    }

    pub fn messages(&self, conversation_id: Option<&str>) -> Result<Vec<Message>> {
        let connection = self.connection.lock();
        let query = if conversation_id.is_some() {
            "SELECT id, conversation_id, role, content, mode, created_at, sources_json, feedback, saved FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC"
        } else {
            "SELECT id, conversation_id, role, content, mode, created_at, sources_json, feedback, saved FROM messages ORDER BY created_at ASC"
        };
        let mut statement = connection.prepare(query)?;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<Message> {
            let sources_json: String = row.get(6)?;
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                mode: row.get(4)?,
                created_at: row.get(5)?,
                sources: serde_json::from_str(&sources_json).unwrap_or_default(),
                feedback: row.get(7)?,
                saved: row.get(8)?,
            })
        };
        let rows = if let Some(id) = conversation_id {
            statement
                .query_map([id], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            statement
                .query_map([], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }

    pub fn insert_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        mode: &str,
        sources: &[SourceRef],
    ) -> Result<Message> {
        let message = Message {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            mode: mode.to_string(),
            created_at: Utc::now().to_rfc3339(),
            sources: sources.to_vec(),
            feedback: None,
            saved: false,
        };
        let connection = self.connection.lock();
        connection.execute(
            "INSERT INTO messages (id, conversation_id, role, content, mode, created_at, sources_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![message.id, message.conversation_id, message.role, message.content, message.mode, message.created_at, serde_json::to_string(sources)?],
        )?;
        connection.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![message.created_at, conversation_id],
        )?;
        if role == "user" {
            let title: Option<String> = connection
                .query_row(
                    "SELECT title FROM conversations WHERE id = ?1",
                    [conversation_id],
                    |row| row.get(0),
                )
                .optional()?;
            if title.as_deref() == Some("New conversation") {
                let generated = content
                    .split_whitespace()
                    .take(7)
                    .collect::<Vec<_>>()
                    .join(" ");
                let generated = if generated.chars().count() > 52 {
                    generated.chars().take(52).collect()
                } else {
                    generated
                };
                connection.execute(
                    "UPDATE conversations SET title = ?1 WHERE id = ?2",
                    params![generated, conversation_id],
                )?;
            }
        }
        Ok(message)
    }

    pub fn delete_message(&self, id: &str) -> Result<()> {
        self.connection
            .lock()
            .execute("DELETE FROM messages WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_message_feedback(&self, id: &str, feedback: Option<&str>) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE messages SET feedback = ?1 WHERE id = ?2",
            params![feedback, id],
        )?;
        Ok(())
    }

    pub fn set_message_saved(&self, id: &str, saved: bool) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE messages SET saved = ?1 WHERE id = ?2",
            params![saved, id],
        )?;
        Ok(())
    }

    pub fn settings(&self) -> Result<AppSettings> {
        let value: String = self.connection.lock().query_row(
            "SELECT value_json FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        let defaults = AppSettings::default();
        let mut json = serde_json::to_value(defaults)?;
        let stored: serde_json::Value = serde_json::from_str(&value).unwrap_or_default();
        if let (Some(base), Some(overlay)) = (json.as_object_mut(), stored.as_object()) {
            for (key, value) in overlay {
                base.insert(key.clone(), value.clone());
            }
        }
        Ok(serde_json::from_value(json)?)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE settings SET value_json = ?1 WHERE id = 1",
            [serde_json::to_string(settings)?],
        )?;
        self.audit("settings.updated", "Local settings changed")?;
        Ok(())
    }

    pub fn documents(&self) -> Result<Vec<DocumentInfo>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT id, name, file_type, size_bytes, page_count, status, created_at, tags_json FROM documents ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let tags: String = row.get(7)?;
            Ok(DocumentInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                file_type: row.get(2)?,
                size_bytes: row.get::<_, i64>(3)?.max(0) as u64,
                page_count: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn insert_document(
        &self,
        id: &str,
        name: &str,
        local_path: &Path,
        file_type: &str,
        size_bytes: u64,
        page_count: u32,
    ) -> Result<DocumentInfo> {
        let document = DocumentInfo {
            id: id.to_string(),
            name: name.to_string(),
            file_type: file_type.to_string(),
            size_bytes,
            page_count,
            status: "ready".into(),
            created_at: Utc::now().to_rfc3339(),
            tags: Vec::new(),
        };
        self.connection.lock().execute(
            "INSERT INTO documents (id, name, local_path, file_type, size_bytes, page_count, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![document.id, document.name, local_path.to_string_lossy(), document.file_type, document.size_bytes as i64, document.page_count, document.status, document.created_at],
        )?;
        self.audit("document.indexed", name)?;
        Ok(document)
    }

    pub fn insert_chunk(
        &self,
        document_id: &str,
        page: Option<u32>,
        content: &str,
        ordinal: usize,
    ) -> Result<()> {
        self.connection.lock().execute(
            "INSERT INTO document_chunks (id, document_id, page, content, vector_json, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![Uuid::new_v4().to_string(), document_id, page, content, serde_json::to_string(&embed(content))?, ordinal as i64],
        )?;
        Ok(())
    }

    pub fn delete_document(&self, id: &str) -> Result<()> {
        let path: Option<String> = self
            .connection
            .lock()
            .query_row(
                "SELECT local_path FROM documents WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        self.connection
            .lock()
            .execute("DELETE FROM documents WHERE id = ?1", [id])?;
        if let Some(path) = path {
            let _ = std::fs::remove_file(path);
        }
        self.audit("document.deleted", id)?;
        Ok(())
    }

    pub fn retrieve(
        &self,
        query: &str,
        document_ids: &[String],
        limit: usize,
    ) -> Result<Vec<SourceRef>> {
        let query_vector = embed(query);
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT c.document_id, d.name, c.page, c.content, c.vector_json FROM document_chunks c JOIN documents d ON d.id = c.document_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<u32>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let allowed: std::collections::HashSet<&str> =
            document_ids.iter().map(String::as_str).collect();
        let mut scored = Vec::new();
        for row in rows {
            let (document_id, document_name, page, content, vector_json) = row?;
            if !allowed.is_empty() && !allowed.contains(document_id.as_str()) {
                continue;
            }
            let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
            let semantic = cosine(&query_vector, &vector);
            let lexical = lexical_overlap(query, &content);
            let score = semantic * 0.7 + lexical * 0.3;
            if score > 0.02 {
                scored.push(SourceRef {
                    document_id,
                    document_name,
                    page,
                    excerpt: content,
                    score,
                });
            }
        }
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(limit);
        Ok(scored)
    }

    pub fn models(&self) -> Result<Vec<ModelInfo>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT id, name, local_path, parameters, quantization, context_length, size_bytes, required_ram_bytes, built_in, status, is_default, download_url, sha256, description, capability_tier, best_for FROM models ORDER BY is_default DESC, size_bytes ASC, name ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ModelInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                parameters: row.get(3)?,
                quantization: row.get(4)?,
                context_length: row.get(5)?,
                size_bytes: row.get::<_, i64>(6)?.max(0) as u64,
                required_ram_bytes: row.get::<_, i64>(7)?.max(0) as u64,
                built_in: row.get(8)?,
                status: row.get(9)?,
                is_default: row.get(10)?,
                download_url: row.get(11)?,
                sha256: row.get(12)?,
                description: row.get(13)?,
                capability_tier: row.get(14)?,
                best_for: row.get(15)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn default_model(&self) -> Result<ModelInfo> {
        let models = self.models()?;
        models
            .iter()
            .find(|model| model.is_default && Path::new(&model.path).exists())
            .cloned()
            .or_else(|| {
                models
                    .into_iter()
                    .find(|model| Path::new(&model.path).exists())
            })
            .ok_or_else(|| {
                anyhow!("Download or import a local model before starting a local chat.")
            })
    }

    pub fn insert_model(&self, model: &ModelInfo) -> Result<()> {
        self.connection.lock().execute(
            "INSERT INTO models (id, name, local_path, parameters, quantization, context_length, size_bytes, required_ram_bytes, built_in, status, is_default, download_url, sha256, description, capability_tier, best_for) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![model.id, model.name, model.path, model.parameters, model.quantization, model.context_length, model.size_bytes as i64, model.required_ram_bytes as i64, model.built_in, model.status, model.is_default, model.download_url, model.sha256, model.description, model.capability_tier, model.best_for],
        )?;
        Ok(())
    }

    pub fn set_default_model(&self, id: &str) -> Result<()> {
        let connection = self.connection.lock();
        let transaction = connection.unchecked_transaction()?;
        transaction.execute("UPDATE models SET is_default = 0", [])?;
        transaction.execute("UPDATE models SET is_default = 1 WHERE id = ?1", [id])?;
        transaction.commit()?;
        self.audit("model.default_changed", id)?;
        Ok(())
    }

    pub fn set_model_status(&self, id: &str, status: &str) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE models SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn delete_model(&self, id: &str) -> Result<()> {
        let model = self
            .models()?
            .into_iter()
            .find(|model| model.id == id)
            .ok_or_else(|| anyhow!("Model not found."))?;
        if model.built_in {
            return Err(anyhow!(
                "The built-in default model is part of Moco and cannot be removed."
            ));
        }
        let _ = std::fs::remove_file(model.path);
        if model.download_url.is_some() {
            self.connection.lock().execute(
                "UPDATE models SET status = 'not-downloaded' WHERE id = ?1",
                [id],
            )?;
        } else {
            self.connection
                .lock()
                .execute("DELETE FROM models WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    pub fn clear_data(&self, scope: &str) -> Result<()> {
        let connection = self.connection.lock();
        match scope {
            "chats" => {
                connection.execute("DELETE FROM conversations", [])?;
            }
            "documents" => {
                connection.execute("DELETE FROM documents", [])?;
            }
            "all" => {
                connection.execute("DELETE FROM conversations", [])?;
                connection.execute("DELETE FROM documents", [])?;
                connection.execute("DELETE FROM audit_logs", [])?;
                connection.execute(
                    "UPDATE settings SET value_json = ?1 WHERE id = 1",
                    [serde_json::to_string(&AppSettings::default())?],
                )?;
            }
            _ => return Err(anyhow!("Unknown data scope.")),
        }
        Ok(())
    }

    pub fn audit(&self, action: &str, details: &str) -> Result<()> {
        self.connection.lock().execute(
            "INSERT INTO audit_logs (id, action, details, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                action,
                details,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }
}
