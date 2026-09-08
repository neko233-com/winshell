//! Real ConPTY validation, independent of a GPU or desktop session.
use crate::{config::Config, shell, terminal::Session};
use anyhow::{Context, Result};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn run(config: &Config, id: &str) -> Result<()> {
    let mut profile = shell::discover(config)
        .into_iter()
        .find(|profile| profile.id == id)
        .context("Requested shell is not installed")?;
    let path = std::env::temp_dir().join(format!("winshell-smoke-{}-{}", std::process::id(), id));
    // create_dir prevents accidentally reusing or deleting an existing directory.
    std::fs::create_dir(&path)?;
    let scratch = Scratch(path);
    profile
        .env
        .insert("WINSHELL_SMOKE_INHERITED".into(), "global value".into());
    if id == "bash" {
        profile
            .env
            .insert("HOME".into(), scratch.0.to_string_lossy().into_owned());
        profile.env.insert("HISTFILE".into(), "/dev/null".into());
    }
    let (script_name, script) = match id {
        "bash" => (
            "probe.bash",
            r#"[[ "$WINSHELL_SMOKE_INHERITED" == 'global value' ]] || exit 11
export WINSHELL_SMOKE_LOCAL='hello world'
[[ "$WINSHELL_SMOKE_LOCAL" == 'hello world' ]] || exit 12
winshell_probe() { printf '%s' 'function-ok'; }
[[ "$(winshell_probe)" == 'function-ok' ]] || exit 13
alias winshell_alias='printf alias-ok'
[[ "$(winshell_alias)" == 'alias-ok' ]] || exit 14
printf 'alpha\nbeta\n' | grep beta > 'file with spaces.txt'
[[ "$(cat 'file with spaces.txt')" == beta ]] || exit 15
git --version >/dev/null || exit 16
printf '\e[38;2;116;213;187m中文 UTF-8\e[0m\n'
printf 'WINSHELL_%s\n' SMOKE_PASS
"#,
        ),
        "pwsh" | "powershell" => (
            "probe.ps1",
            r#"$ErrorActionPreference = 'Stop'
if ($env:WINSHELL_SMOKE_INHERITED -ne 'global value') { throw 'inherited environment' }
$env:WINSHELL_SMOKE_LOCAL = 'hello world'
function Test-WinShell { 'function-ok' }
Set-Alias winshell_alias Test-WinShell
if ((winshell_alias) -ne 'function-ok') { throw 'function/alias' }
'alpha', 'beta' | Where-Object { $_ -eq 'beta' } | Set-Content 'file with spaces.txt'
if ((Get-Content 'file with spaces.txt') -ne 'beta') { throw 'pipeline/redirection' }
if ($env:WINSHELL_SMOKE_LOCAL -ne 'hello world') { throw 'local environment' }
Write-Output ('WINSHELL_' + 'SMOKE_PASS')
"#,
        ),
        "cmd" => (
            "probe.cmd",
            "@echo off\r\nif not \"%WINSHELL_SMOKE_INHERITED%\"==\"global value\" exit /b 11\r\nset \"WINSHELL_SMOKE_LOCAL=hello world\"\r\nif not \"%WINSHELL_SMOKE_LOCAL%\"==\"hello world\" exit /b 12\r\necho beta| findstr beta > \"file with spaces.txt\"\r\nfindstr beta \"file with spaces.txt\" >nul || exit /b 13\r\necho WINSHELL_SMOKE_PASS\r\n",
        ),
        _ => anyhow::bail!("Smoke tests cover bash, pwsh, powershell, and cmd"),
    };
    let script_path = scratch.0.join(script_name);
    std::fs::write(&script_path, script)?;
    let mut session = Session::spawn(profile.clone(), &scratch.0, 1000)?;
    session.resize(30, 110)?;
    if id == "bash" {
        wait(&session, |state| state.at_prompt)?;
    }
    let path = script_path.to_string_lossy().replace('\\', "/");
    let command = match id {
        "bash" => format!("source '{}'\r", path.replace('\'', "'\\''")),
        "pwsh" | "powershell" => format!(
            "& ([scriptblock]::Create([IO.File]::ReadAllText('{}')))\r",
            path.replace('\'', "''")
        ),
        _ => format!("call \"{}\"\r", script_path.display()),
    };
    session.write(command.into_bytes())?;
    wait(&session, |state| {
        state.text().contains("WINSHELL_SMOKE_PASS")
    })?;
    anyhow::ensure!(
        scratch.0.join("file with spaces.txt").is_file(),
        "Redirection did not create the file"
    );
    session.resize(40, 140)?;
    anyhow::ensure!(
        session.size.rows == 40 && session.size.cols == 140,
        "Resize did not propagate"
    );
    // The second tab must inherit the baseline, not the first tab's exported values.
    let other = Session::spawn(profile, Path::new(&scratch.0), 100)?;
    if id == "bash" {
        wait(&other, |state| state.at_prompt)?;
    }
    let isolation = match id {
        "bash" => "[[ -z ${WINSHELL_SMOKE_LOCAL+x} ]] && printf 'WINSHELL_%s\\n' ISOLATED\r",
        "pwsh" | "powershell" => {
            "if (-not $env:WINSHELL_SMOKE_LOCAL) { Write-Output ('WINSHELL_' + 'ISOLATED') }\r"
        }
        _ => {
            "if not defined WINSHELL_SMOKE_LOCAL set WINSHELL_SMOKE_RESULT=ISOLATED\recho WINSHELL_%WINSHELL_SMOKE_RESULT%\r"
        }
    };
    other.write(isolation.as_bytes())?;
    wait(&other, |state| state.text().contains("WINSHELL_ISOLATED"))?;
    drop(other);
    session.write(b"exit\r".to_vec())?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        session.poll_exit();
        if session.state.lock().unwrap().closed {
            break;
        }
        anyhow::ensure!(Instant::now() < deadline, "Shell did not exit cleanly");
        thread::sleep(Duration::from_millis(30));
    }
    drop(session);
    println!(
        "PASS {id}: real ConPTY, inherited/exported variables, pipeline, redirection, script, independent sessions, resize, exit"
    );
    Ok(())
}

fn wait(
    session: &Session,
    predicate: impl Fn(&crate::terminal::TerminalState) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        let state = session.state.lock().unwrap();
        if predicate(&state) {
            return Ok(());
        }
        anyhow::ensure!(
            !state.closed,
            "Shell exited before the expected result: {:?}",
            state.error
        );
        anyhow::ensure!(
            Instant::now() < deadline,
            "PTY timed out waiting for test output: {}",
            state.text()
        );
        drop(state);
        thread::sleep(Duration::from_millis(30));
    }
}
