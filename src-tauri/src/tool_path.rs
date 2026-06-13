//! Enriched executable lookup for GUI apps with a minimal macOS PATH.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use crate::models::DetectedProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSource {
    Process,
    LoginShell,
    Inherited,
    Common,
}

impl PathSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::LoginShell => "login-shell",
            Self::Inherited => "PATH",
            Self::Common => "common",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedExecutable {
    pub path: PathBuf,
    pub source: PathSource,
}

pub fn find_executable(name: &str, processes: &[DetectedProcess]) -> Option<ResolvedExecutable> {
    for (directory, source) in search_directories(processes) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(ResolvedExecutable {
                path: candidate,
                source,
            });
        }
    }
    None
}

pub fn search_directories(processes: &[DetectedProcess]) -> Vec<(PathBuf, PathSource)> {
    let mut directories = Vec::new();
    let mut seen = Vec::new();

    for directory in process_derived_directories(processes) {
        push_directory(&mut directories, &mut seen, directory, PathSource::Process);
    }
    for directory in login_shell_path_directories() {
        push_directory(&mut directories, &mut seen, directory, PathSource::LoginShell);
    }
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            push_directory(&mut directories, &mut seen, directory, PathSource::Inherited);
        }
    }
    for directory in common_macos_directories() {
        push_directory(&mut directories, &mut seen, directory, PathSource::Common);
    }

    directories
}

fn push_directory(
    directories: &mut Vec<(PathBuf, PathSource)>,
    seen: &mut Vec<PathBuf>,
    directory: PathBuf,
    source: PathSource,
) {
    if !directory.is_dir() {
        return;
    }
    if seen.iter().any(|existing| existing == &directory) {
        return;
    }
    seen.push(directory.clone());
    directories.push((directory, source));
}

fn process_derived_directories(processes: &[DetectedProcess]) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for process in processes {
        if let Some(executable) = process.executable.as_deref() {
            if let Some(parent) = Path::new(executable).parent() {
                directories.push(parent.to_path_buf());
            }
        }
        if let Some(command) = process.command.as_deref() {
            if let Some(parent) = Path::new(command).parent() {
                directories.push(parent.to_path_buf());
            }
        }
    }
    directories
}

fn login_shell_path_directories() -> Vec<PathBuf> {
    static LOGIN_SHELL_PATH: OnceLock<Vec<PathBuf>> = OnceLock::new();
    LOGIN_SHELL_PATH
        .get_or_init(|| {
            Command::new("/bin/zsh")
                .args(["-l", "-c", "printf %s \"$PATH\""])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|text| env::split_paths(text.trim()).collect())
                })
                .unwrap_or_default()
        })
        .clone()
}

fn common_macos_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];

    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        directories.push(home.join(".cargo/bin"));
        directories.push(home.join(".local/bin"));
        directories.push(home.join(".codex/bin"));
        directories.push(home.join(".nvm/current/bin"));
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for entry in entries.flatten() {
                directories.push(entry.path().join("bin"));
            }
        }
        if let Ok(entries) = std::fs::read_dir(home.join(".fnm/node-versions")) {
            for entry in entries.flatten() {
                directories.push(entry.path().join("installation/bin"));
            }
        }
    }

    directories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_directories_include_common_paths() {
        let directories = search_directories(&[]);
        assert!(directories
            .iter()
            .any(|(path, _)| path == Path::new("/usr/bin")));
    }

    #[test]
    fn process_paths_precede_inherited_path() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck-tool-path-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let executable = directory.join("node");
        std::fs::write(&executable, b"").expect("temp executable");
        let processes = vec![DetectedProcess {
            id: "process:1".to_owned(),
            pid: 1,
            name: "node".to_owned(),
            executable: Some(executable.to_string_lossy().into_owned()),
            command: None,
            category: "runtime".to_owned(),
        }];
        let directories = search_directories(&processes);
        assert_eq!(
            directories.first().map(|(path, _)| path),
            Some(&directory)
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}