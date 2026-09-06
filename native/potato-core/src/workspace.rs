//! Editable workspace documents use the same API as the existing desktop UI.
//! Prompt documents remain in SQLite; memory notes use ordinary Markdown files.
use crate::{Error, Result, Runtime};
use serde_json::{json, Value};

const DEFAULT_FILES: &[&str] = &["AGENTS.md", "SOUL.md", "PROFILE.md"];
const MAX_CONTENT: usize = 128_000;

fn valid_name(name: &str, nested: bool) -> Result<()> {
    if name.len() > 240
        || !name.ends_with(".md")
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
        || name
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || (!nested && name.contains('/'))
    {
        return Err(Error::new(
            400,
            "Expected a relative Markdown document name",
        ));
    }
    Ok(())
}

fn template(name: &str, language: &str) -> &'static str {
    match (language, name) {
        ("zh", "AGENTS.md") => include_str!("../../../src/potato/agents/md_files/zh/AGENTS.md"),
        ("id", "AGENTS.md") => include_str!("../../../src/potato/agents/md_files/id/AGENTS.md"),
        ("ru", "AGENTS.md") => include_str!("../../../src/potato/agents/md_files/ru/AGENTS.md"),
        (_, "AGENTS.md") => include_str!("../../../src/potato/agents/md_files/en/AGENTS.md"),
        // Leave personal identity/preferences empty until the user supplies them.
        _ => "",
    }
}

impl Runtime {
    pub(crate) fn export_workspace(&self) -> Result<Value> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use std::io::{Cursor, Write};
        let working = self.documents(false)?;
        let memory = self.documents(true)?;
        let db = self.db()?;
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut total = 0;
        let mut write = |name: &str, content: &[u8]| -> Result<()> {
            total += content.len();
            if total > 100_000_000 {
                return Err(Error::new(413, "Workspace export exceeds 100 MB"));
            }
            archive
                .start_file(name, options)
                .map_err(|_| Error::new(500, "Cannot create backup entry"))?;
            archive.write_all(content)?;
            Ok(())
        };
        for (prefix, documents) in [("workspace/", working), ("memory/", memory)] {
            for (name, doc) in documents.as_object().unwrap() {
                write(
                    &format!("{prefix}{name}"),
                    doc["content"].as_str().unwrap_or("").as_bytes(),
                )?;
            }
        }
        let mut chats = Vec::new();
        for spec in db.chats()? {
            let messages = db.history(crate::required(&spec, "id")?, false)?;
            chats.push(json!({"spec":spec,"messages":messages}));
        }
        write(
            "history.json",
            serde_json::to_string_pretty(
                &json!({"format":"potato-native-history-v1","chats":chats}),
            )?
            .as_bytes(),
        )?;
        write(
            "skills.json",
            serde_json::to_string_pretty(&db.get("skills", json!({}))?)?.as_bytes(),
        )?;
        write(
            "scheduled-tasks.json",
            serde_json::to_string_pretty(&db.get("cron_jobs", json!({}))?)?.as_bytes(),
        )?;
        write("README.txt",b"Potato workspace export v1\nIncludes saved workspace documents and user-wide memory notes, conversation history, skills and scheduled task definitions. Project-local memory travels with its project and is not included.\nDoes not contain service credentials, files in external projects, or the native encryption key.\nImport history.json using the existing history import control; other entries are portable records, not an automatic full restore.\n")?;
        let bytes = archive
            .finish()
            .map_err(|_| Error::new(500, "Cannot finish workspace export"))?
            .into_inner();
        Ok(
            json!({"native_binary":STANDARD.encode(bytes),"mime":"application/zip","filename":"potato-workspace.zip"}),
        )
    }
    fn documents(&self, memory: bool) -> Result<Value> {
        if memory {
            let root = self.memory_root()?;
            let mut documents = json!({});
            let mut bytes = 0;
            for note in crate::memory::list_notes(&root)? {
                let name = crate::required(&note, "path")?;
                let content = crate::memory::read_note(&root, name)?;
                bytes += content.len();
                if bytes > 100_000_000 {
                    return Err(Error::new(413, "Memory export exceeds 100 MB"));
                }
                documents[name] = json!({"content":content,"created_time":note["created_time"],"modified_time":note["modified_time"]});
            }
            return Ok(documents);
        }
        let db = self.db()?;
        let key = "workspace_documents";
        let mut documents = db.get(key, Value::Null)?;
        if documents.is_null() {
            documents = json!({});
            if !memory {
                let language = db.get("language", json!("zh"))?;
                for name in DEFAULT_FILES {
                    documents[*name] = json!({"content":template(name, language.as_str().unwrap_or("zh")),
                        "created_time":chrono::Utc::now().timestamp(),"modified_time":chrono::Utc::now().timestamp()});
                }
            }
            db.put(key, &documents)?;
        }
        Ok(documents)
    }

    pub(crate) fn document_request(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Option<Value>> {
        if path == "/api/workspace/memory-location" && method == "GET" {
            return Ok(Some(
                json!({"path":self.memory_root()?,"format":"Markdown files","source_of_truth":"filesystem","project_relative_path":".potato/memory"}),
            ));
        }
        if path == "/api/workspace/system-prompt-files" {
            return Ok(Some(match method {
                "GET" => self
                    .db()?
                    .get("system_prompt_files", json!(DEFAULT_FILES))?,
                "PUT" => {
                    let files = body
                        .as_array()
                        .ok_or_else(|| Error::new(400, "Expected document names"))?;
                    if files.len() > 16 {
                        return Err(Error::new(400, "At most 16 prompt documents"));
                    }
                    let documents = self.documents(false)?;
                    let mut seen = std::collections::HashSet::new();
                    for file in files {
                        let name = file
                            .as_str()
                            .ok_or_else(|| Error::new(400, "Expected a document name"))?;
                        valid_name(name, false)?;
                        if documents.get(name).is_none() {
                            return Err(Error::new(404, "Prompt document not found"));
                        }
                        if !seen.insert(name) {
                            return Err(Error::new(400, "Duplicate prompt document"));
                        }
                    }
                    self.db()?.put("system_prompt_files", body)?;
                    body.clone()
                }
                _ => return Err(Error::new(405, "Method not allowed")),
            }));
        }
        let (memory, name) = if path == "/api/workspace/files" {
            (false, "")
        } else if path == "/api/workspace/memory" {
            (true, "")
        } else if let Some(name) = path.strip_prefix("/api/workspace/files/") {
            (false, name)
        } else if let Some(name) = path.strip_prefix("/api/workspace/memory/") {
            (true, name)
        } else {
            return Ok(None);
        };
        if memory {
            return self.memory_document_request(method, name, body).map(Some);
        }
        if !name.is_empty() {
            valid_name(name, memory)?;
        }
        let mut documents = self.documents(memory)?;
        let result = match (method, name.is_empty()) {
            ("GET", true) => json!(documents.as_object().unwrap().iter().map(|(name, doc)| {
                json!({"filename":name,"path":name,"size":doc["content"].as_str().unwrap_or("").len(),
                    "created_time":doc["created_time"],"modified_time":doc["modified_time"]})
            }).collect::<Vec<_>>()),
            ("GET", false) => {
                let doc = documents.get(name).ok_or_else(|| Error::new(404,"Document not found"))?;
                json!({"content":doc["content"]})
            },
            ("PUT", false) => {
                let content = body["content"].as_str().ok_or_else(|| Error::new(400,"Expected text content"))?;
                if content.len() > MAX_CONTENT { return Err(Error::new(413,"Document exceeds 128 KB")); }
                if let Some(expected)=body.get("expected_content"){if documents.get(name).map(|d|&d["content"]).unwrap_or(&Value::Null)!=expected{return Err(Error::new(409,"Document changed since it was opened; reload before saving"));}}
                let now = chrono::Utc::now().timestamp();
                let created = documents[name]["created_time"].as_i64().unwrap_or(now);
                documents[name] = json!({"content":content,"created_time":created,"modified_time":now});
                self.db()?.put(if memory {"memory_documents"} else {"workspace_documents"}, &documents)?;
                json!({"written":true})
            },
            ("DELETE",false)=>{
                if documents.as_object_mut().unwrap().remove(name).is_none(){return Err(Error::new(404,"Document not found"));}
                let mut db=self.db()?;let mut values=vec![(if memory{"memory_documents"}else{"workspace_documents"}.into(),documents.clone())];
                if !memory {let mut files=db.get("system_prompt_files",json!(DEFAULT_FILES))?;if let Some(files)=files.as_array_mut(){files.retain(|v|v!=name);}values.push(("system_prompt_files".into(),files));}
                db.put_batch(&values)?;json!({"deleted":true})
            },
            _ => return Err(Error::new(405,"Method not allowed")),
        };
        Ok(Some(result))
    }

    pub(crate) fn system_prompt(&self) -> Result<String> {
        let documents = self.documents(false)?;
        let files = self
            .db()?
            .get("system_prompt_files", json!(DEFAULT_FILES))?;
        let mut prompt = String::from("You are Potato, the user's desktop assistant. Use only the tools supplied in this request. Never claim that a reminder, background job, file change or message was completed without a successful tool result.\n");
        for name in files
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let content = documents[name]["content"].as_str().unwrap_or("");
            // Cron is available, but autonomous heartbeat polling is not enabled.
            let content = without_heartbeat(content);
            if prompt.len() + content.len() > MAX_CONTENT {
                return Err(Error::new(413, "Combined system prompt exceeds 128 KB"));
            }
            prompt.push_str(&format!("\n# {name}\n{content}\n"));
        }
        Ok(prompt)
    }
}

fn without_heartbeat(content: &str) -> String {
    let mut result = String::new();
    let mut remaining = content;
    while let Some((before, after)) = remaining.split_once("<!-- heartbeat:start -->") {
        result.push_str(before);
        match after.split_once("<!-- heartbeat:end -->") {
            Some((_, rest)) => remaining = rest,
            None => return result,
        }
    }
    result.push_str(remaining);
    result
}
