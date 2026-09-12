use crate::{required, Error, Result, Runtime};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};

async fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    // Never let inherited Git routing select an unrelated repository/index.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "--no-pager",
            "--literal-pathspecs",
            "-c",
            "core.hooksPath=",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| Error::new(503, "Git executable is unavailable"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(20), async {
        let read = |stream: Box<dyn tokio::io::AsyncRead + Unpin + Send>| async move {
            let mut bytes = Vec::new();
            stream.take(4_000_001).read_to_end(&mut bytes).await?;
            if bytes.len() > 4_000_000 {
                return Err(Error::new(413, "Git output exceeds 4 MB"));
            }
            Ok(bytes)
        };
        let (out, err) = tokio::try_join!(read(Box::new(stdout)), read(Box::new(stderr)))?;
        let status = child.wait().await?;
        if !status.success() {
            return Err(Error::new(
                422,
                format!(
                    "Git command failed: {}",
                    String::from_utf8_lossy(&err)
                        .chars()
                        .take(1000)
                        .collect::<String>()
                ),
            ));
        }
        Ok(out)
    })
    .await;
    match outcome {
        Ok(result) => result,
        Err(_) => Err(Error::new(408, "Git command timed out")),
    }
}

fn expand_path(value: &str) -> Result<PathBuf> {
    let path = if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .ok_or_else(|| Error::new(400, "Home directory unavailable"))?;
        PathBuf::from(home).join(value.strip_prefix("~/").unwrap_or(""))
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err(Error::new(400, "An absolute directory is required"));
    }
    Ok(path)
}

impl Runtime {
    pub(crate) async fn turn_project(&self, body: &Value) -> Result<PathBuf> {
        if let Some(path) = body["request_context"]["potato.coding_project_dir"]
            .as_str()
            .or_else(|| body["metadata"]["potato.coding_project_dir"].as_str())
        {
            let target = tokio::fs::canonicalize(expand_path(path)?).await?;
            if !target.is_dir() {
                return Err(Error::new(400, "Conversation project is not a directory"));
            }
            self.check_public_path(&target).await?;
            Ok(target)
        } else {
            self.project_dir().await
        }
    }
    pub(crate) async fn workspace_dir(&self) -> Result<PathBuf> {
        let path = self.root.join("workspace");
        tokio::fs::create_dir_all(&path).await?;
        let canonical = tokio::fs::canonicalize(&path).await?;
        if canonical != tokio::fs::canonicalize(&self.root).await?.join("workspace") {
            return Err(Error::new(
                403,
                "Native workspace must not be redirected by a symbolic link",
            ));
        }
        Ok(canonical)
    }

    pub(crate) async fn check_public_path(&self, path: &Path) -> Result<()> {
        let root = tokio::fs::canonicalize(&self.root).await?;
        if path.starts_with(&root) && !path.starts_with(self.workspace_dir().await?) {
            return Err(Error::new(403, "Native runtime data is private"));
        }
        Ok(())
    }

    pub(crate) async fn project_dir(&self) -> Result<PathBuf> {
        let saved = self.db()?.get("project_dir", Value::Null)?;
        match saved.as_str() {
            Some(path) => {
                let path = tokio::fs::canonicalize(expand_path(path)?).await?;
                self.check_public_path(&path).await?;
                Ok(path)
            }
            None => self.workspace_dir().await,
        }
    }

    pub(crate) async fn project_request(
        &self,
        method: &str,
        path: &str,
        body: &Value,
        url: &reqwest::Url,
    ) -> Result<Option<Value>> {
        let query = |key: &str| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        };
        let prefix = "/api/workspace/coding-project";
        if let Some(suffix) = path.strip_prefix(prefix) {
            let workspace = self.workspace_dir().await?;
            let result = match (method, suffix) {
                ("GET", "") => {
                    let current = self.project_dir().await?;
                    json!({"path":current,"name":current.file_name().unwrap_or_default().to_string_lossy(),"is_workspace_default":current==workspace,"workspace_dir":workspace,"exists":current.is_dir()})
                }
                ("PUT", "") => {
                    let current = if body["path"].is_null() {
                        workspace.clone()
                    } else {
                        tokio::fs::canonicalize(expand_path(required(body, "path")?)?).await?
                    };
                    if !current.is_dir() {
                        return Err(Error::new(400, "Project must be a directory"));
                    }
                    self.check_public_path(&current).await?;
                    self.db()?.put(
                        "project_dir",
                        &if current == workspace {
                            Value::Null
                        } else {
                            json!(current)
                        },
                    )?;
                    json!({"path":current,"name":current.file_name().unwrap_or_default().to_string_lossy(),"is_workspace_default":current==workspace})
                }
                ("POST", "/create") => {
                    let name = required(body, "name")?.trim();
                    if name.len() > 100
                        || name.starts_with('.')
                        || name.ends_with(['.', ' '])
                        || name.contains(['/', '\\', ':'])
                        || name.chars().any(char::is_control)
                    {
                        return Err(Error::new(400, "Invalid project name"));
                    }
                    let base = workspace.join("coding_projects");
                    tokio::fs::create_dir_all(&base).await?;
                    let target = base.join(name);
                    tokio::fs::create_dir(&target).await.map_err(|e| {
                        if e.kind() == std::io::ErrorKind::AlreadyExists {
                            Error::new(409, "Project already exists")
                        } else {
                            e.into()
                        }
                    })?;
                    if let Err(error) = git(&target, &["init", "--template="]).await {
                        let _ = tokio::fs::remove_dir(&target).await;
                        return Err(error);
                    }
                    self.db()?.put("project_dir", &json!(target))?;
                    json!({"path":target,"name":name})
                }
                ("GET", "/list") => {
                    let base = workspace.join("coding_projects");
                    tokio::fs::create_dir_all(&base).await?;
                    let current = self.project_dir().await?;
                    let mut dir = tokio::fs::read_dir(&base).await?;
                    let mut entries = Vec::new();
                    while let Some(entry) = dir.next_entry().await? {
                        if entry.file_type().await?.is_dir() {
                            entries.push(json!({"path":entry.path(),"name":entry.file_name().to_string_lossy(),"is_git":entry.path().join(".git").exists(),"is_active":entry.path()==current}));
                        }
                    }
                    entries.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_owned());
                    json!(entries)
                }
                ("GET", "/browse-dirs") => {
                    let requested = query("path");
                    #[cfg(windows)]
                    if requested == "/" || requested == "\\" {
                        let dirs: Vec<_> = (b'A'..=b'Z')
                            .filter_map(|drive| {
                                let path = format!("{}:\\", drive as char);
                                Path::new(&path).is_dir().then(
                                    || json!({"name":format!("{}:",drive as char),"path":path}),
                                )
                            })
                            .collect();
                        return Ok(Some(
                            json!({"current":"/","parent":null,"dirs":dirs,"selectable":false}),
                        ));
                    }
                    let current = tokio::fs::canonicalize(expand_path(if requested.is_empty() {
                        "~"
                    } else {
                        &requested
                    })?)
                    .await?;
                    self.check_public_path(&current).await?;
                    let mut dir = tokio::fs::read_dir(&current).await?;
                    let mut entries = Vec::new();
                    while let Some(entry) = dir.next_entry().await? {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if query("show_hidden") != "true" && name.starts_with('.') {
                            continue;
                        }
                        if entry.file_type().await?.is_dir()
                            && self.check_public_path(&entry.path()).await.is_ok()
                        {
                            entries.push(json!({"name":name,"path":entry.path()}));
                        }
                        if entries.len() >= 2000 {
                            break;
                        }
                    }
                    entries.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_owned());
                    let parent = current
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .or_else(|| cfg!(windows).then(|| "/".into()));
                    json!({"current":current,"parent":parent,"dirs":entries})
                }
                _ => return Err(Error::new(501, "Project operation is not implemented yet")),
            };
            return Ok(Some(result));
        }
        if path == "/api/workspace/git/status" && method == "GET" {
            let root = self.project_dir().await?;
            let raw = git(
                &root,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
            .await?;
            let mut entries = raw.split(|b| *b == 0).filter(|b| !b.is_empty());
            let mut changes = Vec::new();
            while let Some(entry) = entries.next() {
                if entry.len() < 4 {
                    continue;
                }
                let name = String::from_utf8_lossy(&entry[3..]);
                let x = entry[0] as char;
                let y = entry[1] as char;
                if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
                    entries.next();
                }
                if x == '?' {
                    changes.push(json!({"path":name,"status":"?","staged":false}));
                    continue;
                }
                if x != ' ' {
                    changes.push(json!({"path":name,"status":x.to_string(),"staged":true}));
                }
                if y != ' ' {
                    changes.push(json!({"path":name,"status":y.to_string(),"staged":false}));
                }
            }
            let branch = git(&root, &["symbolic-ref", "--short", "HEAD"])
                .await
                .unwrap_or_else(|_| b"HEAD".to_vec());
            let counts = git(
                &root,
                &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
            )
            .await
            .unwrap_or_default();
            let counts = String::from_utf8_lossy(&counts);
            let mut counts = counts
                .split_whitespace()
                .filter_map(|n| n.parse::<u64>().ok());
            return Ok(Some(
                json!({"branch":String::from_utf8_lossy(&branch).trim(),"changes":changes,"ahead":counts.next().unwrap_or(0),"behind":counts.next().unwrap_or(0)}),
            ));
        }
        if path == "/api/workspace/git/diff" && method == "GET" {
            let root = self.project_dir().await?;
            let name = query("path");
            let relative = Path::new(&name);
            if name.is_empty()
                || relative.is_absolute()
                || relative
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(Error::new(400, "Expected a relative project file"));
            }
            let diff = if query("untracked") == "true" {
                let target = tokio::fs::canonicalize(root.join(relative)).await?;
                if !target.starts_with(&root) {
                    return Err(Error::new(403, "File is outside project"));
                }
                self.check_public_path(&target).await?;
                let file = tokio::fs::File::open(target).await?;
                if !file.metadata().await?.is_file() {
                    return Err(Error::new(400, "Expected a regular file"));
                }
                let mut bytes = Vec::new();
                file.take(1_000_001).read_to_end(&mut bytes).await?;
                if bytes.len() > 1_000_000 {
                    return Err(Error::new(413, "File exceeds preview limit"));
                }
                let content = String::from_utf8(bytes)
                    .map_err(|_| Error::new(400, "Binary file cannot be previewed"))?;
                format!(
                    "--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{} @@\n{}",
                    content.lines().count(),
                    content
                        .lines()
                        .map(|l| format!("+{l}\n"))
                        .collect::<String>()
                )
            } else {
                let mut args = vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color"];
                if query("staged") == "true" {
                    args.push("--cached");
                }
                args.extend(["--", &name]);
                String::from_utf8_lossy(&git(&root, &args).await?).to_string()
            };
            return Ok(Some(json!({"diff":diff})));
        }
        Ok(None)
    }
}
