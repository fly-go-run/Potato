//! A Windows process boundary, not an approval engine. See README.md for scope.
//! The launch/ownership model follows openai/codex's Windows process.rs and
//! unified_exec backends (2cbbf0c9b542a36a1c3284b5e804917635b6f666).
//! LPAC is Potato's choice; Codex uses restricted tokens/dedicated accounts.
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Options {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub project: PathBuf,
    pub private: PathBuf,
    pub scratch: PathBuf,
    pub denied: Vec<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub network: bool,
}

pub fn powershell_args(command: &str) -> Vec<String> {
    use base64::Engine;
    let command = format!("$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false);\n{command}");
    let bytes: Vec<_> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
    vec![
        "-NoLogo".into(),
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-EncodedCommand".into(),
        base64::engine::general_purpose::STANDARD.encode(bytes),
    ]
}

// Pure policy/encoding logic is also tested on the development host.
#[cfg(any(windows, test))]
fn secret_name(name: &str) -> bool {
    let name = name.trim_end_matches([' ', '.']).to_ascii_lowercase();
    matches!(name.as_str(), ".ssh" | ".aws" | ".gnupg" | ".env")
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
}

#[cfg(any(windows, test))]
fn quote(arg: &str) -> String {
    // CommandLineToArgvW/CRT quoting. Shell code itself is UTF-16 Base64, never
    // interpolated into this command line or a PowerShell setup script.
    let mut result = String::from("\"");
    let mut slashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            '\\',
            if c == '"' { slashes * 2 + 1 } else { slashes },
        ));
        slashes = 0;
        result.push(c);
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}

#[cfg(windows)]
mod acl;
#[cfg(windows)]
mod lpac;
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::{probe, system_directory, Process};

#[cfg(all(test, windows))]
#[path = "tests.rs"]
mod windows_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_case_insensitive_secret_names() {
        for name in [
            ".ENV",
            ".env.production",
            ".SSH",
            "PRIVATE.PEM",
            "client.Key",
            ".env ",
            "client.pem.",
        ] {
            assert!(secret_name(name), "{name}");
        }
        for name in ["src", "environment.rs", "key.rs", ".git"] {
            assert!(!secret_name(name), "{name}");
        }
    }

    #[test]
    fn quotes_windows_arguments_without_losing_backslashes() {
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("C:\\space dir\\"), "\"C:\\space dir\\\\\"");
        assert_eq!(quote("a\\\"b"), "\"a\\\\\\\"b\"");
    }

    #[test]
    fn preserves_unicode_and_shell_metacharacters_in_encoded_command() {
        use base64::Engine;
        let command = "Write-Output '中文 \"& | $HOME'; exit 7";
        let args = powershell_args(command);
        assert_eq!(args[3], "-EncodedCommand");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&args[4])
            .unwrap();
        let utf16: Vec<_> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let decoded = String::from_utf16(&utf16).unwrap();
        assert_eq!(decoded.split_once('\n').unwrap().1, command);
    }
}
