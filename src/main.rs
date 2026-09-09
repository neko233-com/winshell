#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod terminal_view;

fn main() {
    std::panic::set_hook(Box::new(|panic| {
        let directory = winshell::config::directory();
        let _ = std::fs::create_dir_all(&directory);
        let _ = std::fs::write(
            directory.join("crash.log"),
            format!(
                "WinShell {}\n{panic}\n{}",
                env!("CARGO_PKG_VERSION"),
                std::backtrace::Backtrace::force_capture()
            ),
        );
        eprintln!("{panic}");
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        attach_parent_console();
    }
    match run(args) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
}

fn run(args: Vec<String>) -> anyhow::Result<()> {
    let (config, error) = match winshell::config::Config::load() {
        Ok(config) => (config, None),
        Err(error) => (
            winshell::config::Config::default(),
            Some(format!("{error:#}")),
        ),
    };
    let mut config = config;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        let arg = args[i].clone();
        if let Some(value) = arg.strip_prefix("--cwd=") {
            let path = std::path::PathBuf::from(value);
            anyhow::ensure!(path.is_dir(), "Directory does not exist: {}", path.display());
            config.working_directory = Some(path);
            i += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--shell=") {
            anyhow::ensure!(
                winshell::shell::discover(&config)
                    .iter()
                    .any(|profile| profile.id == *value),
                "Shell profile not found: {value}"
            );
            config.default_shell = value.to_string();
            i += 1;
            continue;
        }
        match arg.as_str() {
            "--cwd" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--cwd requires a value"))?;
                let path = std::path::PathBuf::from(value);
                anyhow::ensure!(path.is_dir(), "Directory does not exist: {}", path.display());
                config.working_directory = Some(path);
                i += 2;
            }
            "--shell" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--shell requires a value"))?
                    .clone();
                anyhow::ensure!(
                    winshell::shell::discover(&config)
                        .iter()
                        .any(|profile| profile.id == value),
                    "Shell profile not found: {value}"
                );
                config.default_shell = value;
                i += 2;
            }
            other if other.starts_with('-') => {
                rest.push(other.to_string());
                i += 1;
            }
            other => {
                let path = std::path::PathBuf::from(other);
                if path.is_dir() {
                    config.working_directory = Some(path);
                } else {
                    rest.push(other.to_string());
                }
                i += 1;
            }
        }
    }
    if !rest.is_empty() {
        if let Some(error) = &error {
            anyhow::bail!("{error}");
        }
        cli(&rest, &config)?;
        return Ok(());
    }
    app::run(config, error, None);
    Ok(())
}

#[cfg(windows)]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    // SAFETY: AttachConsole has no pointer arguments. ATTACH_PARENT_PROCESS
    // attaches CLI invocations to an existing console; GUI launches skip it.
    unsafe {
        AttachConsole(u32::MAX);
    }
}
#[cfg(not(windows))]
fn attach_parent_console() {}

fn cli(args: &[String], config: &winshell::config::Config) -> anyhow::Result<()> {
    match args[0].as_str() {
        "--version" | "-V" => println!("winshell {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" => println!(
            "WinShell — a native multi-tab Windows terminal\n\n  winshell                 Open the desktop application\n  winshell --cwd DIR       Open in a directory\n  winshell --doctor        Check available shells and configuration\n  winshell --smoke-test ID  Test a real PTY session (bash, powershell, cmd)\n  winshell --version       Print the version\n\nConfiguration: {}",
            winshell::config::directory().join("config.toml").display()
        ),
        "--doctor" => {
            println!(
                "WinShell {} | {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::ARCH
            );
            println!(
                "Configuration: {}",
                winshell::config::directory().join("config.toml").display()
            );
            println!(
                "Environment: inherited + {} global overrides",
                config.env.len()
            );
            let profiles = winshell::shell::discover(config);
            for profile in &profiles {
                println!(
                    "{}: {}{}",
                    profile.id,
                    profile.program.display(),
                    if profile.bundled { " [bundled]" } else { "" }
                );
            }
            anyhow::ensure!(!profiles.is_empty(), "No shells found");
        }
        "--smoke-test" => {
            let id = args.get(1).map(String::as_str).unwrap_or("bash");
            winshell::smoke::run(config, id)?;
        }
        "--ui-smoke-test" => {
            let report = std::path::PathBuf::from(
                args.get(1)
                    .ok_or_else(|| anyhow::anyhow!("--ui-smoke-test requires a report path"))?,
            );
            let scratch = std::env::temp_dir().join(format!("winshell-ui-{}", std::process::id()));
            std::fs::create_dir(&scratch)?;
            winshell::config::set_directory(scratch.join("settings"))?;
            let mut config = winshell::config::Config {
                working_directory: Some(scratch.clone()),
                // Acceptance script is Bash-specific; keep the desktop default free.
                default_shell: "bash".into(),
                ..Default::default()
            };
            config
                .env
                .insert("HOME".into(), scratch.to_string_lossy().into_owned());
            config.env.insert("HISTFILE".into(), "/dev/null".into());
            app::run(config, None, Some(report.clone()));
            let _ = std::fs::remove_dir_all(scratch);
            let result = std::fs::read_to_string(report)?;
            anyhow::ensure!(result.starts_with("PASS"), "{result}");
            println!("{result}");
        }
        other => anyhow::bail!("Unknown argument: {other}. Use --help."),
    }
    Ok(())
}
