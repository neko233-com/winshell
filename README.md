# WinShell

A modern, native, multi-tab Windows terminal built with **Rust + GPUI**.

WinShell aims to replace the Git Bash terminal window while keeping real Bash,
Git, and Unix utilities. The portable edition includes the official Git for
Windows runtime, so Git Bash does not need to be installed separately.

**Status: 0.1 preview.** This is a working terminal implementation, still under
compatibility testing. It is not yet a claim of complete Git Bash/Windows
Terminal feature parity.

## What it does

- Independent ConPTY sessions in tabs, a shell launcher, and a session sidebar.
- GPU-rendered native GPUI interface, dark theme, font zoom, and settings.
- Alacritty terminal engine: ANSI/VT sequences, 16/256/truecolor, scrollback,
  alternate screens, terminal responses, and wide-character cells.
- Actual Bash, PowerShell 7, Windows PowerShell, and CMD, plus configurable
  executables such as WSL and Nushell.
- Shell-native commands, aliases, functions, scripts, pipes, redirection,
  command substitution, and environment variables. WinShell passes input to the
  shell; it does not implement a partial command interpreter.
- Bash prompt integration, command descriptions, recent-history suggestions,
  and native Readline completion. Accept a suggestion with **Alt + Right**;
  it inserts text without executing it. **Tab** remains Bash completion.
- Mouse selection, double-click word selection, clipboard copy/paste, bracketed
  paste, scrollback search, and platform text input with IME composition.
- Mouse reporting for terminal applications. Hold **Shift** to select text
  instead of reporting mouse events to the application.
- Global and per-shell environment overrides, applied to new sessions.

## Build and run

Windows x64, Windows 10 1809 or newer (ConPTY), a GPUI-compatible graphics driver,
Rust stable, and Visual Studio C++ Build Tools with a Windows SDK are required.
Use Windows 11 for development and compatibility testing.

```powershell
cargo run
```

During development, WinShell discovers an existing Git for Windows installation.
To run independently of an installed Git Bash:

```powershell
.\scripts\setup-runtime.ps1
cargo run
```

The runtime download is pinned to Git for Windows 2.55.0.5 and checked against
its SHA256 before extraction. The runtime is stored under `runtime/git`, ignored
by Git, and never modifies your system Git installation.

Build a portable ZIP, including Bash, Git, Unix tools, and upstream notices:

```powershell
.\scripts\package.ps1
```

The package is written to `dist/`. Extract the **entire** ZIP, then open
`winshell.exe`. Keep its `runtime` folder beside the executable. Moving only the
EXE produces a thin terminal that needs a separately available shell.

For an executable-only package, use `scripts/package.ps1 -WithoutRuntime`.
CI builds and uploads the portable package as a GitHub Actions artifact.

## Shells and variables

Settings live in `%APPDATA%\winshell\config\config.toml`; `--doctor` prints the
resolved path. Start with [`config.example.toml`](config.example.toml), or open
Settings with **Ctrl + ,** and select **Edit config.toml**.

```toml
default_shell = "bash"
font_family = "Cascadia Mono"
font_size = 15.0

[env]
MY_PROJECT_HOME = 'D:\Code'
EDITOR = "vim"

[[shells]]
id = "wsl-ubuntu"
name = "Ubuntu (WSL)"
program = 'C:\Windows\System32\wsl.exe'
args = ["--distribution", "Ubuntu", "--cd", "~"]

[shells.env]
WSLENV = "MY_PROJECT_HOME/p"
```

Press **Ctrl + Shift + R** to reload. Appearance changes apply immediately;
shell and environment changes apply to newly opened tabs. Argument and
environment strings in TOML are literal; use complete paths.

Environment precedence is **inherited WinShell process environment → terminal
defaults → global `[env]` → `[shells.env]` → shell startup files**. Shell exports
are session-local and do not modify Windows registry variables or other tabs.
Restart WinShell to inherit changes made to Windows environment settings after
WinShell started. WSL has its own Linux environment; use `WSLENV` to bridge
selected variables.

| Operation | Bash | PowerShell | CMD |
| --- | --- | --- | --- |
| Set exported variable | `export PROJECT=demo` | `$env:PROJECT='demo'` | `set PROJECT=demo` |
| Read variable | `echo "$PROJECT"` | `$env:PROJECT` | `echo %PROJECT%` |
| List files | `ls -lah` | `Get-ChildItem` | `dir` |
| Run a script | `bash script.sh` | `& .\script.ps1` | `call script.cmd` |

Shell syntax is preserved. PowerShell syntax must be run in PowerShell, and Bash
syntax in Bash. Commands still require their executable, package, network,
permissions, or WSL distribution to exist.

The integrated Bash loads `/etc/profile` and your Bash login profile (falling
back to `.bashrc`), then adds a WinShell prompt and Readline bindings. It never
edits your dotfiles. History is written by Bash with `history -a`, honoring your
`HISTFILE` and `HISTCONTROL`. WinShell itself does not log typed commands or
upload history. Leading-space commands are excluded from suggestions.

To use your exact Bash prompt/startup behavior without WinShell integration,
configure a custom shell with arguments `['--login', '-i']` and a different id.
Native completion still works; WinShell's prompt-dependent suggestions and
current-directory tracking will be unavailable for that profile.

## Shortcuts

| Keys | Action |
| --- | --- |
| Ctrl + Shift + T / W | New / close tab |
| Ctrl + Tab / Ctrl + Shift + Tab | Next / previous tab |
| Ctrl + Shift + P | Open shell launcher |
| Ctrl + Shift + B | Toggle sidebar |
| Ctrl + Shift + C / V | Copy selection / paste |
| Shift + Insert / right-click | Paste |
| Ctrl + Shift + F | Find in scrollback; Enter next, Esc close |
| Shift + Page Up / Page Down | Scroll by a page |
| Ctrl + plus / minus / 0 | Font size up / down / reset |
| Ctrl + , | Settings |
| Ctrl + Shift + R | Reload configuration |
| Alt + Right | Insert the first suggestion |
| Ctrl + C | Send interrupt to the shell/program |

Closing a tab terminates its shell and attached terminal processes. An exited
tab retains its output until closed.

## Validation

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo run -- --doctor
cargo run -- --smoke-test bash
cargo run -- --smoke-test powershell
cargo run -- --smoke-test cmd
```

The smoke tests spawn real ConPTY sessions in a temporary directory and exercise
inherited variables, exports, functions/aliases where supported, pipelines,
file redirection, scripts, session isolation, resizing, and process exit. Bash
smoke tests use an isolated HOME and do not append to personal shell history.

## Current limits

- Windows x64 is the supported build target. WSL/custom profiles are configurable
  but require their own installed runtime and compatibility testing.
- Suggestion overlays and live working-directory tracking currently target the
  integrated Bash prompt. Suggestions deliberately skip wrapped/multiline input
  and foreground applications. Shell-native completion works in every profile.
- Search is case-sensitive and matches within each terminal row.
- Common xterm keyboard and mouse protocols are implemented; Kitty keyboard,
  sixel/inline images, split panes, session restoration, clickable hyperlinks,
  and a complete accessibility tree are not implemented yet.
- IME, graphics-driver behavior, and full-screen applications need manual
  validation across Windows versions. Automated terminal tests are not a
  replacement for that validation.
- Builds are unsigned. Machines enforcing application-control signing policies
  may block local binaries; a trusted code-signing/release process is needed for
  those environments.

## Architecture

`GPUI window → TerminalView → Session → ConPTY → real shell`

PTY output is parsed by `alacritty_terminal` on a reader thread. The UI paints
the resulting cell grid at fixed cell positions. Input is sent through a bounded
writer queue. Idle tabs do not repaint; output invalidates the active view.
The terminal responds to device, color, and size queries. OSC 52 clipboard
access from programs is disabled; explicit user copy/paste is supported.

WinShell code is licensed under Apache-2.0. Bundled programs retain their
respective licenses; see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

