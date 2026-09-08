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

pub fn discover(config: &Config) -> Vec<ShellProfile> {
    let mut profiles = Vec::new();
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
    if let Some(dir) = &executable_dir {
        candidates.push((dir.join("runtime/git/bin/bash.exe"), true));
        // Development builds can use the runtime downloaded into the project root.
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
    if let Some((program, bundled)) = candidates.into_iter().find(|(path, _)| path.is_file()) {
        profiles.push(ShellProfile {
            id: "bash".into(),
            name: "Bash".into(),
            program,
            args: vec![],
            bundled,
            env: config.env.clone(),
        });
    }
    if let Some(program) = find_on_path("pwsh.exe") {
        profiles.push(ShellProfile {
            id: "pwsh".into(),
            name: "PowerShell 7".into(),
            program,
            args: vec!["-NoLogo".into()],
            bundled: false,
            env: config.env.clone(),
        });
    }
    if let Some(root) = std::env::var_os("SystemRoot") {
        let system = PathBuf::from(root).join("System32");
        let powershell = system.join("WindowsPowerShell/v1.0/powershell.exe");
        if powershell.is_file() {
            profiles.push(ShellProfile {
                id: "powershell".into(),
                name: "PowerShell".into(),
                program: powershell,
                args: vec!["-NoLogo".into()],
                bundled: false,
                env: config.env.clone(),
            });
        }
        profiles.push(ShellProfile {
            id: "cmd".into(),
            name: "Command Prompt".into(),
            program: system.join("cmd.exe"),
            args: vec![],
            bundled: false,
            env: config.env.clone(),
        });
    }
    for shell in &config.shells {
        let mut env = config.env.clone();
        env.extend(shell.env.clone());
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
            std::fs::write(&rcfile, include_str!("../assets/shell-integration.bash"))
                .context("Could not install Bash integration")?;
            command.args(["--noprofile", "--rcfile"]);
            command.arg(rcfile.to_string_lossy().replace('\\', "/"));
            command.arg("-i");
            command.env("CHERE_INVOKING", "1");
            command.env("MSYSTEM", "MINGW64");
        } else {
            command.args(&self.args);
        }
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "winshell");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        // CommandBuilder inherits this process's complete environment. Overrides
        // apply only to the child; they never mutate the machine or other tabs.
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
