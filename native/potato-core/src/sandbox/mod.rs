//! Process isolation, separate from action approval. base.sbpl is adapted from
//! openai/codex 2cbbf0c9b542a36a1c3284b5e804917635b6f666 (Apache-2.0;
//! see LICENSE.codex). Potato adds restricted reads, private scratch and denies.
use crate::{Error, Result};
use serde_json::{json, Value};
#[cfg(not(windows))]
use std::path::Path;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

pub(crate) fn backend() -> &'static str {
    if cfg!(windows) {
        "windows-lpac"
    } else if cfg!(target_os = "macos") {
        "macos-seatbelt"
    } else {
        "unavailable"
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub command: String,
    pub cwd: PathBuf,
    pub project: PathBuf,
    pub private: PathBuf,
    pub scratch: PathBuf,
    pub mode: String,
    pub network: bool,
    pub unsandboxed: bool,
    pub env: BTreeMap<String, String>,
    pub denied: Vec<PathBuf>,
}

pub(crate) fn available() -> bool {
    status()["available"] == true
}

pub(crate) fn status() -> Value {
    static STATUS: OnceLock<Value> = OnceLock::new();
    STATUS.get_or_init(|| {
        #[cfg(windows)]
        return match potato_windows_sandbox::probe() {
            Ok(()) => json!({"available":true,"backend":backend(),"effective_file_mode":"read-only","reason":null}),
            Err(error) => json!({"available":false,"backend":backend(),"effective_file_mode":"read-only","reason":error.to_string()}),
        };
        #[cfg(not(windows))]
        {
        if !cfg!(target_os = "macos") {
            return json!({"available":false,"backend":"unavailable","reason":"This platform's OS sandbox is not implemented; commands need explicit unsandboxed approval."});
        }
        // Probe enforcement, not just the existence of sandbox-exec. No user code.
        let result = std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1)(deny default)(allow process-exec)(allow file-read*)(allow sysctl-read)", "--", "/usr/bin/true"])
            .env_clear().output();
        match result {
            Ok(out) if out.status.success() => json!({"available":true,"backend":"macos-seatbelt","reason":null}),
            Ok(out) => json!({"available":false,"backend":"macos-seatbelt","reason":String::from_utf8_lossy(&out.stderr).chars().take(500).collect::<String>()}),
            Err(error) => json!({"available":false,"backend":"macos-seatbelt","reason":error.to_string()}),
        }
        }
    }).clone()
}

impl Plan {
    pub fn new(
        command: String,
        cwd: PathBuf,
        project: PathBuf,
        private: PathBuf,
        mode: String,
        scratch: PathBuf,
    ) -> Self {
        let mut env = BTreeMap::new();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut path =
            String::from("/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin");
        if let Some(home) = &home {
            path.push_str(&format!(":{}/.cargo/bin", home.display()));
            env.insert(
                "RUSTUP_HOME".into(),
                home.join(".rustup").display().to_string(),
            );
        }
        env.insert("PATH".into(), path);
        env.insert("LANG".into(), "en_US.UTF-8".into());
        env.insert("HOME".into(), scratch.display().to_string());
        env.insert("TMPDIR".into(), scratch.display().to_string());
        env.insert("TMP".into(), scratch.display().to_string());
        env.insert("TEMP".into(), scratch.display().to_string());
        env.insert(
            "CARGO_HOME".into(),
            scratch.join("cargo").display().to_string(),
        );
        env.insert(
            "XDG_CACHE_HOME".into(),
            scratch.join("cache").display().to_string(),
        );
        // Required by Windows process creation, never copy arbitrary parent env.
        for key in ["SystemRoot", "WINDIR", "COMSPEC", "PATHEXT"] {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.into(), value);
            }
        }
        #[cfg(windows)]
        if let Ok(system) = potato_windows_sandbox::system_directory() {
            let root = system.parent().unwrap_or(&system).display().to_string();
            env.insert("SystemRoot".into(), root.clone());
            env.insert("WINDIR".into(), root.clone());
            env.insert(
                "COMSPEC".into(),
                system.join("cmd.exe").display().to_string(),
            );
            env.insert("PATHEXT".into(), ".COM;.EXE;.BAT;.CMD".into());
            env.remove("RUSTUP_HOME");
            env.insert(
                "PATH".into(),
                format!("{root}\\System32;{root};{root}\\System32\\WindowsPowerShell\\v1.0"),
            );
            env.insert("USERPROFILE".into(), scratch.display().to_string());
        }
        Self {
            denied: Vec::new(),
            command,
            cwd,
            project,
            private,
            scratch,
            unsandboxed: mode == "danger-full-access",
            mode,
            network: false,
            env,
        }
    }

    pub fn context(&self) -> Value {
        let scope_env: BTreeMap<_, _> = self
            .env
            .iter()
            .map(|(k, v)| {
                (
                    k,
                    v.replace(&self.scratch.display().to_string(), "<job-scratch>"),
                )
            })
            .collect();
        let effective_mode = if self.unsandboxed {
            "danger-full-access"
        } else if cfg!(windows) {
            "read-only"
        } else {
            &self.mode
        };
        json!({"scope_digest":crate::reviewer_cache::hash(&json!([backend(),effective_mode,self.command,self.cwd,self.project,self.private,self.mode,self.network,self.unsandboxed,scope_env,self.denied])),"backend":if self.unsandboxed {"none"} else {backend()},
            "effective_file_mode":effective_mode,
            "platform_scope":if cfg!(windows) && !self.unsandboxed {"LPAC OS baseline, registryRead, lpacInstrumentation and lpacCom; individually granted project reads; scratch/profile writes; project shell writes require reviewed host execution or file tools"} else {"project and private scratch"},
            "network_scope":if cfg!(windows) && !self.unsandboxed && self.network {"internetClient capability; no LAN or loopback exemption"} else {"default"},
            "file_mode":self.mode,"denied_paths":self.denied,"project":self.project,"cwd":self.cwd,"scratch":self.scratch,
            "network":if self.network || self.unsandboxed {"enabled"} else {"disabled"},
            "unsandboxed":self.unsandboxed,"environment":"explicit; credentials and agent sockets excluded",
            "plan_digest":crate::reviewer_cache::hash(&json!([backend(),effective_mode,self.command,self.cwd,self.project,self.private,self.scratch,self.mode,self.network,self.unsandboxed,self.env,self.denied]))})
    }

    pub fn launch(&self) -> Result<(String, Vec<String>)> {
        #[cfg(windows)]
        let shell = (
            potato_windows_sandbox::system_directory()?
                .join("WindowsPowerShell\\v1.0\\powershell.exe")
                .display()
                .to_string(),
            potato_windows_sandbox::powershell_args(&self.command),
        );
        #[cfg(not(windows))]
        let shell = ("/bin/sh".into(), vec!["-c".into(), self.command.clone()]);
        if self.unsandboxed {
            return Ok(shell);
        }
        #[cfg(windows)]
        return Err(Error::new(
            500,
            "Windows sandbox must use the native LPAC process launcher",
        ));
        #[cfg(not(windows))]
        self.launch_seatbelt(shell)
    }

    #[cfg(not(windows))]
    fn launch_seatbelt(&self, shell: (String, Vec<String>)) -> Result<(String, Vec<String>)> {
        if !available() {
            return Err(Error::new(
                503,
                format!("OS sandbox unavailable: {}", status()["reason"]),
            ));
        }
        let mut profile = Profile::default();
        profile.policy.push_str(include_str!("base.sbpl"));
        for path in [
            "/System",
            "/usr",
            "/bin",
            "/sbin",
            "/Library/Developer",
            "/Library/Apple/usr/libexec/oah/libRosettaRuntime",
            "/Applications/Xcode.app",
            "/opt/homebrew",
        ] {
            profile.allow_read(Path::new(path))?;
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for path in [home.join(".cargo/bin"), home.join(".rustup")] {
                profile.allow_read(&path)?;
            }
        }
        for path in [
            "/private/etc",
            "/private/var/db/timezone",
            "/dev/null",
            "/dev/zero",
            "/dev/random",
            "/dev/urandom",
            "/dev/fd",
        ] {
            profile.allow_read(Path::new(path))?;
        }
        profile.policy.push_str("\n(allow file-read* file-test-existence (literal \"/\"))\n(allow file-read-metadata)\n(allow file-map-executable)\n");
        profile.allow_read(&self.project)?;
        profile.allow_read(&self.scratch)?;
        profile.allow_write(&self.scratch)?;
        if self.mode == "workspace-write" {
            profile.allow_write(&self.project)?;
        }
        profile.protect_ancestors(&self.project)?;
        profile.protect_ancestors(&self.scratch)?;
        // Private runtime can contain the selected project, but never grant its siblings.
        let private = profile.param(&self.private)?;
        let exception = if self.project.starts_with(&self.private) {
            let project = profile.param(&self.project)?;
            format!("(require-not (subpath (param \"{project}\")))")
        } else {
            String::new()
        };
        profile.policy.push_str(&format!("\n(deny file-read* file-write* (require-all (subpath (param \"{private}\")) {exception}))\n"));
        for denied in &self.denied {
            let key = profile.param(denied)?;
            profile.policy.push_str(&format!(
                "\n(deny file-read* file-write* (subpath (param \"{key}\")))\n"
            ));
            profile.protect_ancestors(denied)?;
        }
        // Global name-based carveouts survive renames of ordinary parent directories.
        profile.policy.push_str("\n(deny file-link)\n");
        for (operations, pattern) in [
            (
                "file-read* file-write*",
                "(^|/)([.]ssh|[.]aws|[.]gnupg|[.]env([.][^/]*)?)(/|$)",
            ),
            ("file-read* file-write*", "[.](pem|key)$"),
            (
                "file-write*",
                "(^|/)([.]git|[.]agents|[.]codex|AGENTS[.]md|SOUL[.]md|PROFILE[.]md|SKILL[.]md|policy[.]yaml)(/|$)",
            ),
        ] {
            // Seatbelt's regex engine does not implement PCRE (?i). Spell out
            // ASCII case pairs so protection also works on case-insensitive APFS.
            let pattern: String = pattern
                .chars()
                .map(|c| {
                    if c.is_ascii_alphabetic() {
                        format!("[{}{}]", c.to_ascii_lowercase(), c.to_ascii_uppercase())
                    } else {
                        c.to_string()
                    }
                })
                .collect();
            profile
                .policy
                .push_str(&format!("\n(deny {operations} (regex #\"{pattern}\"))\n"));
        }
        let metadata = profile.param(&self.project.join(".potato"))?;
        profile.policy.push_str(&format!(
            "\n(deny file-write* (subpath (param \"{metadata}\")))\n"
        ));
        // A pre-existing hardlink is another pathname for the same data. Deny
        // aliases and protect their ancestors; new links are denied above.
        profile.protect_hardlinks(&self.project)?;
        if self.network {
            profile.policy.push_str(include_str!("network.sbpl"));
            profile
                .policy
                .push_str("\n(allow network-outbound (remote ip \"*:*\"))\n");
        }
        let mut args = vec!["-p".into(), profile.policy];
        args.extend(profile.params);
        args.push("--".into());
        args.push(shell.0);
        args.extend(shell.1);
        Ok(("/usr/bin/sandbox-exec".into(), args))
    }
}

#[derive(Default)]
#[cfg(not(windows))]
struct Profile {
    policy: String,
    params: Vec<String>,
}
#[cfg(not(windows))]
impl Profile {
    fn param(&mut self, path: &Path) -> Result<String> {
        let path = path
            .to_str()
            .ok_or_else(|| Error::new(400, "Sandbox paths must be UTF-8"))?;
        if !Path::new(path).is_absolute() || path.contains('\0') {
            return Err(Error::new(400, "Invalid sandbox path"));
        }
        let key = format!("ROOT_{}", self.params.len());
        self.params.push(format!("-D{key}={path}"));
        Ok(key)
    }
    fn allow_read(&mut self, path: &Path) -> Result<()> {
        let key = self.param(path)?;
        self.policy.push_str(&format!(
            "\n(allow file-read* (subpath (param \"{key}\")))\n"
        ));
        Ok(())
    }
    fn allow_write(&mut self, path: &Path) -> Result<()> {
        let key = self.param(path)?;
        self.policy.push_str(&format!(
            "\n(allow file-write* (subpath (param \"{key}\")))\n"
        ));
        Ok(())
    }
    fn protect_ancestors(&mut self, path: &Path) -> Result<()> {
        for path in path.ancestors() {
            let key = self.param(path)?;
            self.policy.push_str(&format!(
                "\n(deny file-write-unlink (literal (param \"{key}\")))\n"
            ));
        }
        Ok(())
    }
    fn protect_hardlinks(&mut self, project: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mut pending = vec![project.to_owned()];
            let mut count = 0;
            while let Some(directory) = pending.pop() {
                for item in std::fs::read_dir(&directory)? {
                    let item = item?;
                    count += 1;
                    if count > 100_000 {
                        return Err(Error::new(
                            503,
                            "Sandbox hardlink inspection exceeded 100000 entries; use a smaller project or request reviewed unsandboxed execution",
                        ));
                    }
                    let path = item.path();
                    let m = std::fs::symlink_metadata(&path)?;
                    if m.is_file() && m.nlink() > 1 {
                        let key = self.param(&path)?;
                        self.policy.push_str(&format!(
                            "\n(deny file-read* file-write* (literal (param \"{key}\")))\n"
                        ));
                        self.protect_ancestors(&directory)?;
                    } else if m.is_dir() {
                        pending.push(path);
                    }
                }
            }
        }
        #[cfg(not(unix))]
        let _ = project;
        Ok(())
    }
}

pub(crate) fn likely_denied(output: &Value) -> bool {
    if output["exit_code"] == 0 {
        return false;
    }
    // Windows may report startup isolation failures without any output.
    // Normalize both signed and unsigned JSON representations of NTSTATUS.
    if output["exit_code"]
        .as_i64()
        .is_some_and(|code| matches!(code as u32, 0xc0000022 | 0xc0000142 | 0xc0000135))
    {
        return true;
    }
    let text = format!("{} {}", output["stdout"], output["stderr"]).to_lowercase();
    [
        "operation not permitted",
        "permission denied",
        "access is denied",
        "access to the path",
        "unauthorizedaccessexception",
        "拒绝访问",
        "read-only file system",
        "sandbox_apply",
        "rosetta error: failed to open",
        "could not resolve host",
        "couldn't connect",
        "could not connect",
        "network is unreachable",
        "an attempt was made to access a socket",
        "forbidden by its access permissions",
        "unable to connect to the remote server",
        "无法连接到远程服务器",
        "remote name could not be resolved",
        "no such host is known",
        "socketexception",
        "远程名称",
    ]
    .iter()
    .any(|word| text.contains(word))
}

pub(crate) fn network_denial(output: &Value) -> bool {
    let text = format!("{} {}", output["stdout"], output["stderr"]).to_lowercase();
    [
        "could not resolve",
        "couldn't connect",
        "could not connect",
        "network",
        "connect:",
        "socket:",
        "socketexception",
        "access a socket",
        "remote server",
        "远程服务器",
        "remote name",
        "no such host",
        "远程名称",
    ]
    .iter()
    .any(|word| text.contains(word))
}

/// Complex shell programs need the agent to inspect partial effects and prepare
/// the remaining action. This is a replay guard, never an approval allowlist.
pub(crate) fn needs_diagnosis(command: &str) -> bool {
    [";", "&&", "||", "|", "\n", "`", "$(", ">", " &"]
        .iter()
        .any(|part| command.contains(part))
}

#[cfg(test)]
mod tests;
