#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod terminal_view;

fn main() {
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
    if !args.is_empty() {
        if let Err(error) = cli(&args, &config) {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    app::run(config, error);
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
        other => anyhow::bail!("Unknown argument: {other}. Use --help."),
    }
    Ok(())
}
