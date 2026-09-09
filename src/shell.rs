use crate::config::{self, Config};
use anyhow::{Context, Result};
use portable_pty::CommandBuilder;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ShellProfile {
    pub id: String,
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub bundled: bool,
    pub env: BTreeMap<String, String>,
}

/// Bash only. Custom profiles in config.toml are optional extras.
pub fn discover(config: &Config) -> Vec<ShellProfile> {
    let mut profiles = Vec::new();
    if let Some((program, bundled)) = find_bash(config) {
        profiles.push(ShellProfile {
            id: "bash".into(),
            name: "Bash".into(),
            program,
            args: vec![],
            bundled,
            env: config.env.clone(),
        });
    }
    for shell in &config.shells {
        let mut env = config.env.clone();
        for (key, value) in &shell.env {
            env.retain(|existing, _| {
                if cfg!(windows) {
                    !existing.eq_ignore_ascii_case(key)
                } else {
                    existing != key
                }
            });
            env.insert(key.clone(), value.clone());
        }
        profiles.retain(|profile| profile.id != shell.id);
        profiles.push(ShellProfile {
            id: shell.id.clone(),
            name: shell.name.clone(),
            program: shell.program.clone(),
            args: shell.args.clone(),
            bundled: false,
            env,
        });
    }
    profiles
}

fn find_bash(config: &Config) -> Option<(PathBuf, bool)> {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let mut candidates = Vec::new();
    if let Some(path) = &config.bash_path {
        candidates.push((path.clone(), false));
    }
    if let Some(path) = std::env::var_os("WINSHELL_BASH") {
        candidates.push((path.into(), false));
    }
    #[cfg(unix)]
    for path in ["/opt/homebrew/bin/bash", "/usr/local/bin/bash", "/bin/bash"] {
        candidates.push((PathBuf::from(path), false));
    }
    if let Some(dir) = &executable_dir {
        candidates.push((dir.join("runtime/git/bin/bash.exe"), true));
        if let Some(root) = dir.parent().and_then(Path::parent) {
            candidates.push((root.join("runtime/git/bin/bash.exe"), true));
        }
    }
    for variable in [
        "ProgramW6432",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
    ] {
        if let Some(dir) = std::env::var_os(variable) {
            candidates.push((PathBuf::from(&dir).join("Git/bin/bash.exe"), false));
            candidates.push((PathBuf::from(dir).join("Programs/Git/bin/bash.exe"), false));
        }
    }
    if let Some(path) =
        find_on_path("git.exe").and_then(|p| p.parent()?.parent().map(|p| p.join("bin/bash.exe")))
    {
        candidates.push((path, false));
    }
    candidates.into_iter().find(|(path, _)| path.is_file())
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

impl ShellProfile {
    pub fn command(&self, cwd: &Path) -> Result<CommandBuilder> {
        let mut command = CommandBuilder::new(&self.program);
        if self.id == "bash" && self.args.is_empty() {
            let directory = config::directory();
            std::fs::create_dir_all(&directory)?;
            let rcfile = directory.join("bash-integration-v1.bash");
            // Never edit the user's own profile or shell history.
            let content = include_str!("../assets/shell-integration.bash");
            if std::fs::read_to_string(&rcfile).ok().as_deref() != Some(content) {
                let temporary =
                    directory.join(format!("bash-integration-{}.tmp", std::process::id()));
                std::fs::write(&temporary, content)
                    .context("Could not install Bash integration")?;
                std::fs::rename(temporary, &rcfile).context("Could not update Bash integration")?;
            }
            command.args(["--noprofile", "--rcfile"]);
            command.arg(rcfile.to_string_lossy().replace('\\', "/"));
            command.arg("-i");
            #[cfg(windows)]
            {
                command.env("CHERE_INVOKING", "1");
                command.env("MSYSTEM", "MINGW64");
            }
        } else {
            command.args(&self.args);
        }
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "winshell");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        for (name, value) in &self.env {
            anyhow::ensure!(
                !name.is_empty() && !name.contains(['=', '\0']) && !value.contains('\0'),
                "Invalid environment variable name or value"
            );
            command.env(name, value);
        }
        Ok(command)
    }
}
