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
    let (config, error) = match winshell::config::Config::load() {
        Ok(config) => (config, None),
        Err(error) => (
            winshell::config::Config::default(),
            Some(format!("{error:#}")),
        ),
    };
    if !args.is_empty() && args[0] != "--cwd" && args[0] != "--shell" {
        if let Some(error) = &error {
            eprintln!("{error}");
            std::process::exit(1);
        }
        if let Err(error) = cli(&args, &config) {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    let mut config = config;
    for pair in args.chunks(2) {
        let result = (|| -> anyhow::Result<()> {
            let value = pair
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("{} requires a value", pair[0]))?;
            match pair[0].as_str() {
                "--cwd" => {
                    let path = std::path::PathBuf::from(value);
                    anyhow::ensure!(
                        path.is_dir(),
                        "Directory does not exist: {}",
                        path.display()
                    );
                    config.working_directory = Some(path);
                }
                "--shell" => {
                    anyhow::ensure!(
                        winshell::shell::discover(&config)
                            .iter()
                            .any(|profile| profile.id == *value),
                        "Shell profile not found: {value}"
                    );
                    config.default_shell = value.clone();
                }
                other => anyhow::bail!("Unknown argument: {other}"),
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
    app::run(config, error, None);
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
            "WinShell — a native multi-tab Windows terminal\n\n  winshell                 Open the desktop application\n  winshell --doctor        Check available shells and configuration\n  winshell --smoke-test ID  Test a real PTY session (bash, powershell, cmd)\n  winshell --version       Print the version\n\nConfiguration: {}",
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
