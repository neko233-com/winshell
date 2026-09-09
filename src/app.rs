use crate::terminal_view::{self, TerminalView};
use gpui::{prelude::*, *};
use std::time::Duration;
use winshell::{
    config::{self, Config},
    i18n::{LANGUAGES, Language},
    shell::{self, ShellProfile},
    terminal::{ACCENT, BACKGROUND, FOREGROUND, Session},
    theme::{THEMES, Theme},
};

actions!(
    winshell_app,
    [
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        ToggleSidebar,
        ToggleInspector,
        Launcher,
        Settings,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        ReloadConfig
    ]
);

pub fn run(
    config: Config,
    startup_error: Option<String>,
    smoke_report: Option<std::path::PathBuf>,
) {
    Application::new().run(move |cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-shift-t", NewTab, None),
            KeyBinding::new("ctrl-shift-w", CloseTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
            KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-i", ToggleInspector, None),
            KeyBinding::new("ctrl-shift-p", Launcher, None),
            KeyBinding::new("ctrl-,", Settings, None),
            KeyBinding::new("ctrl-=", ZoomIn, None),
            KeyBinding::new("ctrl-+", ZoomIn, None),
            KeyBinding::new("ctrl--", ZoomOut, None),
            KeyBinding::new("ctrl-0", ResetZoom, None),
            KeyBinding::new("ctrl-shift-r", ReloadConfig, None),
            KeyBinding::new("ctrl-shift-c", terminal_view::Copy, Some("Terminal")),
            KeyBinding::new("ctrl-shift-v", terminal_view::Paste, Some("Terminal")),
            KeyBinding::new("shift-insert", terminal_view::Paste, Some("Terminal")),
            KeyBinding::new("ctrl-shift-f", terminal_view::Search, Some("Terminal")),
            KeyBinding::new(
                "alt-right",
                terminal_view::AcceptSuggestion,
                Some("Terminal"),
            ),
        ]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-t", NewTab, None),
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-,", Settings, None),
            KeyBinding::new("cmd-=", ZoomIn, None),
            KeyBinding::new("cmd--", ZoomOut, None),
            KeyBinding::new("cmd-0", ResetZoom, None),
            KeyBinding::new("cmd-c", terminal_view::Copy, Some("Terminal")),
            KeyBinding::new("cmd-v", terminal_view::Paste, Some("Terminal")),
            KeyBinding::new("cmd-f", terminal_view::Search, Some("Terminal")),
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1220.), px(780.)), cx);
        let is_validation = smoke_report.is_some();
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(680.), px(430.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        if is_validation {
                            "WinShell — validation"
                        } else {
                            "WinShell"
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
                app_id: Some("winshell".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Workspace::new(config, startup_error, window, cx)),
        );
        match result {
            Ok(window) => {
                if let Some(report) = smoke_report {
                    start_ui_smoke(window.into(), report, cx);
                }
            }
            Err(error) => {
                eprintln!("Could not open WinShell: {error:#}");
                if let Some(report) = smoke_report {
                    let _ = std::fs::write(report, format!("FAIL window startup: {error:#}"));
                }
                cx.quit();
            }
        }
        cx.activate(true);
    });
}

/// Exercise the actual native window and GPUI key dispatch against real Bash.
/// This is a bounded acceptance test; its caller supplies an isolated HOME/config.
fn start_ui_smoke(window: AnyWindowHandle, report: std::path::PathBuf, cx: &mut App) {
    let original_clipboard = cx.read_from_clipboard();
    cx.spawn(async move |cx| {
        let mut step = 0usize;
        let mut input_size = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        loop {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let _ = std::fs::write(report.with_extension("progress.txt"), format!("stage {step}"));
            let outcome = window.update(cx, |root, window, cx| -> anyhow::Result<bool> {
                let root = root.downcast::<Workspace>().map_err(|_| anyhow::anyhow!("Root view changed"))?;
                let active = root.read(cx).tabs.get(root.read(cx).active).map(|tab| tab.terminal.clone());
                let key = |name: &str, window: &mut Window, cx: &mut App| -> anyhow::Result<()> {
                    let handled = window.dispatch_keystroke(Keystroke::parse(name)?, cx);
                    eprintln!("UI validation key {name}: handled={handled}");
                    anyhow::ensure!(handled, "Key {name} had no registered handler");
                    Ok(())
                };
                let text = |value: &str, window: &mut Window, cx: &mut App| -> anyhow::Result<()> {
                    for character in value.chars() {
                        let handled = window.dispatch_keystroke(Keystroke { key: character.to_string(), key_char: Some(character.to_string()), modifiers: Modifiers::default() }, cx);
                        anyhow::ensure!(handled, "Character {character:?} had no registered input handler");
                    }
                    Ok(())
                };
                let ready = active.as_ref().is_some_and(|entity| {
                    let terminal = entity.read(cx);
                    terminal.input_painted.get() && terminal.session.state.lock().unwrap().at_prompt
                });
                if let Some(error) = root.read(cx).error.as_ref() { anyhow::bail!("Workspace error: {error}"); }
                if let Some(active) = &active {
                    let terminal = active.read(cx);
                    anyhow::ensure!(terminal.error.is_none(), "Terminal error: {:?}", terminal.error);
                    let state = terminal.session.state.lock().unwrap();
                    let _ = std::fs::write(report.with_extension("terminal.txt"), state.text());
                    let _ = std::fs::write(report.with_extension("status.txt"), format!("stage={step} painted={} focused={} prompt={} tabs={} active={}", terminal.input_painted.get(), terminal.focus.is_focused(window), state.at_prompt, root.read(cx).tabs.len(), root.read(cx).active));
                    anyhow::ensure!(!state.text().contains("syntax error"), "Shell initialization produced a syntax error");
                }
                match step {
                    0 if ready => { input_size = active.as_ref().map(|view| view.read(cx).session.size); text("git st", window, cx)?; step += 1; }
                    1 if active.as_ref().is_some_and(|view| !view.read(cx).hints.is_empty()) => {
                        anyhow::ensure!(active.as_ref().map(|view| view.read(cx).session.size) == input_size, "Suggestion panel resized the PTY during input");
                        key("alt-right", window, cx)?; step += 1;
                    }
                    2 if active.as_ref().is_some_and(|view| view.read(cx).session.state.lock().unwrap().input_query().as_deref() == Some("git status")) => {
                        anyhow::ensure!(active.as_ref().map(|view| view.read(cx).session.size) == input_size, "Accepting a suggestion resized the PTY");
                        key("ctrl-u", window, cx)?;
                        text("printf 'UI_%s\\n' 'PASS'", window, cx)?;
                        key("enter", window, cx)?; step += 1;
                    }
                    3 if ready && active.as_ref().is_some_and(|view| view.read(cx).session.state.lock().unwrap().text().contains("UI_PASS")) => {
                        key("ctrl-shift-f", window, cx)?; step += 1;
                    }
                    4 if active.as_ref().is_some_and(|view| view.read(cx).search.is_some()) => { text("UI_PASS", window, cx)?; step += 1; }
                    5 if active.as_ref().is_some_and(|view| view.read(cx).search_count > 0) => { key("ctrl-shift-c", window, cx)?; step += 1; }
                    6 => {
                        anyhow::ensure!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref() == Some("UI_PASS"), "Copy did not contain the search selection");
                        key("escape", window, cx)?; key("ctrl-shift-t", window, cx)?; step += 1;
                    }
                    7 if root.read(cx).tabs.len() == 2 && ready => { key("ctrl-tab", window, cx)?; step += 1; }
                    8 if root.read(cx).active == 0 => { key("ctrl-shift-b", window, cx)?; step += 1; }
                    9 if !root.read(cx).config.sidebar => { key("ctrl-,", window, cx)?; step += 1; }
                    10 if root.read(cx).settings => { key("escape", window, cx)?; key("ctrl-=", window, cx)?; step += 1; }
                    11 if root.read(cx).config.font_size == 15. => { key("ctrl-shift-w", window, cx)?; step += 1; }
                    12 if root.read(cx).tabs.len() == 1 => { key("ctrl-shift-w", window, cx)?; step += 1; }
                    13 if root.read(cx).tabs.is_empty() => { key("ctrl-shift-t", window, cx)?; step += 1; }
                    14 if ready => {
                        text("printf 'LANG_%s\\n' '中文 日本語 한국어 café'", window, cx)?;
                        key("enter", window, cx)?; step += 1;
                    }
                    15 if ready && active.as_ref().is_some_and(|view| view.read(cx).session.state.lock().unwrap().text().contains("LANG_中文 日本語 한국어 café")) => {
                        key("ctrl-,", window, cx)?; step += 1;
                    }
                    16 if root.read(cx).settings => {
                        key("f6", window, cx)?; key("f6", window, cx)?; key("f6", window, cx)?;
                        key("f7", window, cx)?; step += 1;
                    }
                    17 => {
                        anyhow::ensure!(root.read(cx).config.theme == "light", "Theme keyboard switching failed");
                        anyhow::ensure!(root.read(cx).config.language == "en", "Language switching failed");
                        let terminal = active.as_ref().unwrap().read(cx);
                        anyhow::ensure!(terminal.theme.background == 0xf8fafc && terminal.session.state.lock().unwrap().theme.background == 0xf8fafc, "Terminal palette did not follow theme");
                        let saved = Config::load()?;
                        anyhow::ensure!(saved.theme == "light" && saved.language == "en", "Appearance was not persisted");
                        key("f7", window, cx)?; step += 1;
                    }
                    18 => {
                        anyhow::ensure!(root.read(cx).config.language == "zh-CN", "Chinese selection failed");
                        anyhow::ensure!(active.as_ref().unwrap().read(cx).language == Language::Chinese, "Terminal UI language did not update");
                        key("escape", window, cx)?; step += 1;
                    }
                    19 if ready => {
                        text("printf 'IME_%s\\n' '", window, cx)?;
                        active.as_ref().unwrap().update(cx, |terminal, cx| {
                            terminal.replace_and_mark_text_in_range(None, "输入🙂", None, window, cx);
                        });
                        step += 1;
                    }
                    20 => {
                        active.as_ref().unwrap().update(cx, |terminal, cx| -> anyhow::Result<()> {
                            anyhow::ensure!(terminal.marked_text_range(window, cx) == Some(0..4), "IME marked range must use UTF-16 units");
                            anyhow::ensure!(!terminal.session.state.lock().unwrap().input_query().unwrap_or_default().contains("输入"), "IME preedit leaked into shell input");
                            terminal.replace_text_in_range(None, "输入🙂", window, cx);
                            Ok(())
                        })?;
                        text("'", window, cx)?; key("enter", window, cx)?; step += 1;
                    }
                    21 if ready && active.as_ref().is_some_and(|view| view.read(cx).session.state.lock().unwrap().text().contains("IME_输入🙂")) => {
                        text("vim -Nu NONE -n --noplugin -i NONE ui-buffer.txt", window, cx)?;
                        key("enter", window, cx)?; step += 1;
                    }
                    22 if active.as_ref().is_some_and(|view| view.read(cx).session.state.lock().unwrap().term.mode().contains(alacritty_terminal::term::TermMode::ALT_SCREEN)) => {
                        text("iWinShell 编辑验证", window, cx)?;
                        key("escape", window, cx)?;
                        text(":wq", window, cx)?; key("enter", window, cx)?; step += 1;
                    }
                    23 if ready => {
                        let cwd = root.read(cx).config.cwd();
                        anyhow::ensure!(std::fs::read_to_string(cwd.join("ui-buffer.txt"))?.trim() == "WinShell 编辑验证", "Full-screen editor did not save UTF-8 input");
                        text("sleep 30", window, cx)?; key("enter", window, cx)?; step += 1;
                    }
                    24 if !ready => { key("ctrl-c", window, cx)?; step += 1; }
                    25 if ready => return Ok(true),
                    _ => {},
                }
                anyhow::ensure!(std::time::Instant::now() < deadline, "UI acceptance test timed out at stage {step}");
                Ok(false)
            }).and_then(|result| result);
            let result = match outcome {
                Ok(false) => continue,
                Ok(true) => "PASS native GPUI window: keyboard input, suggestions, acceptance, Bash execution, search, copy, tabs, focus, sidebar, settings, zoom, close/reopen, Chinese/Japanese/Korean/Latin text, language switching, theme palette, persisted settings, IME composition/commit/UTF-16 ranges, Vim alternate screen and UTF-8 file editing, Ctrl+C interrupts".to_owned(),
                Err(error) => format!("FAIL stage {step}: {error:#}"),
            };
            let _ = std::fs::write(&report, result);
            let _ = window.update(cx, |root, _, cx| {
                if let Ok(root) = root.downcast::<Workspace>() { root.update(cx, |workspace, _| workspace.tabs.clear()); }
                cx.write_to_clipboard(original_clipboard.unwrap_or_else(|| ClipboardItem::new_string(String::new())));
                cx.quit();
            });
            break;
        }
    }).detach();
}

struct Tab {
    id: usize,
    terminal: Entity<TerminalView>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Appearance,
    Terminal,
    About,
}

pub struct Workspace {
    config: Config,
    profiles: Vec<ShellProfile>,
    tabs: Vec<Tab>,
    active: usize,
    next_id: usize,
    focus: FocusHandle,
    launcher: bool,
    settings: bool,
    settings_page: SettingsPage,
    launcher_index: usize,
    error: Option<String>,
    metadata: String,
}

impl Workspace {
    fn new(
        config: Config,
        startup_error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let profiles = shell::discover(&config);
        let mut workspace = Self {
            config,
            profiles,
            tabs: vec![],
            active: 0,
            next_id: 1,
            focus: cx.focus_handle(),
            launcher: false,
            settings: false,
            settings_page: SettingsPage::Appearance,
            launcher_index: 0,
            error: startup_error,
            metadata: String::new(),
        };
        workspace.new_tab(&NewTab, window, cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                if this
                    .update(cx, |workspace, cx| {
                        let mut metadata = String::new();
                        for (index, tab) in workspace.tabs.iter().enumerate() {
                            tab.terminal.update(cx, |terminal, cx| {
                                terminal.tick(index == workspace.active, cx);
                                let state = terminal.session.state.lock().unwrap();
                                metadata.push_str(&format!(
                                    "{}|{}|{:?}|{};",
                                    state.cwd.display(),
                                    state.closed,
                                    state.last_exit,
                                    state.title
                                ));
                            });
                        }
                        if workspace.metadata != metadata {
                            workspace.metadata = metadata;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        workspace
    }

    fn open_profile(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.profiles.get(index).cloned() else {
            self.error = Some("No shell is available. Add a profile to config.toml or run scripts/setup-runtime.ps1.".into());
            return;
        };
        let cwd = self
            .tabs
            .get(self.active)
            .map(|tab| {
                tab.terminal
                    .read(cx)
                    .session
                    .state
                    .lock()
                    .unwrap()
                    .cwd
                    .clone()
            })
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| self.config.cwd());
        match Session::spawn(profile, &cwd, self.config.scrollback) {
            Ok(session) => {
                let terminal = cx.new(|cx| TerminalView::new(session, &self.config, window, cx));
                window.focus(&terminal.read(cx).focus);
                self.tabs.push(Tab {
                    id: self.next_id,
                    terminal,
                });
                self.next_id += 1;
                self.active = self.tabs.len() - 1;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        self.launcher = false;
        self.settings = false;
        cx.notify();
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        let index = self
            .profiles
            .iter()
            .position(|profile| profile.id == self.config.default_shell)
            .unwrap_or(0);
        self.open_profile(index, window, cx);
    }
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(index) {
            self.active = index;
            window.focus(&tab.terminal.read(cx).focus);
            cx.notify();
        }
    }
    fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active = self.active.saturating_sub(1);
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        if self.tabs.is_empty() {
            window.focus(&self.focus);
        } else {
            self.activate(self.active, window, cx);
        }
        cx.notify();
    }
    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close(self.active, window, cx);
    }
    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            self.activate((self.active + 1) % self.tabs.len(), window, cx);
        }
    }
    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tabs.is_empty() {
            self.activate(
                (self.active + self.tabs.len() - 1) % self.tabs.len(),
                window,
                cx,
            );
        }
    }
    fn sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.config.sidebar = !self.config.sidebar;
        self.save();
        cx.notify();
    }
    fn inspector(&mut self, _: &ToggleInspector, _: &mut Window, cx: &mut Context<Self>) {
        self.config.inspector = !self.config.inspector;
        self.save();
        cx.notify();
    }
    fn save(&mut self) {
        if let Err(error) = self.config.save() {
            self.error = Some(error.to_string());
        }
    }
    fn launcher(&mut self, _: &Launcher, window: &mut Window, cx: &mut Context<Self>) {
        // Single-shell product: launcher just opens another Bash tab.
        self.new_tab(&NewTab, window, cx);
    }
    fn settings(&mut self, _: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = !self.settings;
        self.launcher = false;
        if self.settings {
            self.settings_page = SettingsPage::Appearance;
            window.focus(&self.focus);
        } else {
            self.activate(self.active, window, cx);
        }
        cx.notify();
    }
    fn zoom(&mut self, amount: f32, cx: &mut Context<Self>) {
        self.config.font_size = (self.config.font_size + amount).clamp(10., 32.);
        for tab in &self.tabs {
            tab.terminal.update(cx, |view, cx| {
                view.font_size = self.config.font_size;
                cx.notify();
            });
        }
        self.save();
        cx.notify();
    }
    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(1., cx);
    }
    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(-1., cx);
    }
    fn reset_font_size(&mut self, cx: &mut Context<Self>) {
        let delta = Config::default().font_size - self.config.font_size;
        self.zoom(delta, cx);
    }
    fn reset_zoom(&mut self, _: &ResetZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.reset_font_size(cx);
    }
    fn reload(&mut self, _: &ReloadConfig, _: &mut Window, cx: &mut Context<Self>) {
        match Config::load() {
            Ok(config) => {
                self.config = config;
                self.profiles = shell::discover(&self.config);
                self.apply_appearance(cx);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        cx.notify();
    }
    fn apply_appearance(&mut self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            tab.terminal.update(cx, |view, cx| {
                view.apply_config(&self.config);
                cx.notify();
            });
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.launcher || self.settings {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.launcher = false;
                    self.settings = false;
                    self.activate(self.active, window, cx);
                }
                "down" if self.launcher && !self.profiles.is_empty() => {
                    self.launcher_index = (self.launcher_index + 1) % self.profiles.len()
                }
                "up" if self.launcher && !self.profiles.is_empty() => {
                    self.launcher_index =
                        (self.launcher_index + self.profiles.len() - 1) % self.profiles.len()
                }
                "enter" if self.launcher => self.open_profile(self.launcher_index, window, cx),
                "f6" if self.settings => {
                    let index = THEMES
                        .iter()
                        .position(|(id, _)| *id == self.config.theme)
                        .unwrap_or(0);
                    self.config.theme = THEMES[(index + 1) % THEMES.len()].0.into();
                    self.apply_appearance(cx);
                    self.save();
                }
                "f7" if self.settings => {
                    let index = LANGUAGES
                        .iter()
                        .position(|(id, _)| *id == self.config.language)
                        .unwrap_or(0);
                    self.config.language = LANGUAGES[(index + 1) % LANGUAGES.len()].0.into();
                    self.apply_appearance(cx);
                    self.save();
                }
                _ => {}
            }
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn button(&self, id: &'static str, text: impl Into<SharedString>) -> Stateful<Div> {
        let theme = Theme::resolve(&self.config);
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(theme.rgb(0xa8b3c4))
            .hover(|style| {
                style
                    .bg(theme.rgb(0x293340))
                    .text_color(theme.rgb(0xf1f5fb))
            })
            .child(text.into())
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::resolve(&self.config);
        let language = Language::resolve(&self.config.language);
        div()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.rgb(0x161c24))
            .border_r_1()
            .border_color(theme.rgb(0x29313c))
            .p_2()
            .gap_1()
            .child(
                div()
                    .px_2()
                    .py_2()
                    .text_size(px(10.))
                    .text_color(theme.rgb(0x6b7a90))
                    .child(language.t("Workspace")),
            )
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.rgb(0xdce5f0))
                    .child(
                        self.config
                            .cwd()
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Local".into()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .text_size(px(11.))
                    .text_color(theme.rgb(0x7a8a9e))
                    .child(language.t("Sessions"))
                    .child(self.tabs.len().to_string()),
            )
            .child(
                div()
                    .id("session-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let terminal = tab.terminal.read(cx);
                        let closed = terminal.session.state.lock().unwrap().closed;
                        div()
                            .id(("session", tab.id))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .text_size(px(12.))
                            .bg(theme.rgb(if index == self.active {
                                0x233631
                            } else {
                                0x161c24
                            }))
                            .hover(|style| style.bg(theme.rgb(0x25312f)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.rgb(if closed { 0x657387 } else { ACCENT }))
                                    .child("›"),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(theme.rgb(0xc5d0de))
                                    .child(terminal.label()),
                            )
                            .on_click(cx.listener(move |workspace, _, window, cx| {
                                workspace.activate(index, window, cx)
                            }))
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(theme.rgb(0x29313c))
                    .text_size(px(10.))
                    .text_color(theme.rgb(0x5d6c80))
                    .child(language.t("One workspace. Every shell.")),
            )
            .child(
                self.button(
                    "settings-side",
                    format!("⚙  {}", language.t("Settings")),
                )
                .on_click(
                    cx.listener(|workspace, _, window, cx| {
                        workspace.settings(&Settings, window, cx)
                    }),
                ),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::resolve(&self.config);
        let language = Language::resolve(&self.config.language);
        let page = self.settings_page;
        let pages = [
            (
                SettingsPage::Appearance,
                "Appearance",
                language.t("Appearance"),
            ),
            (SettingsPage::Terminal, "Terminal", language.t("Terminal")),
            (SettingsPage::About, "About", language.t("About")),
        ];
        div()
            .id("settings-panel")
            .w(px(720.))
            .h(px(480.))
            .rounded_lg()
            .overflow_hidden()
            .bg(theme.rgb(0x1b232e))
            .border_1()
            .border_color(theme.rgb(0x3a4656))
            .shadow_lg()
            .flex()
            .flex_row()
            .child(
                div()
                    .id("settings-nav")
                    .w(px(168.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .bg(theme.rgb(0x161c24))
                    .border_r_1()
                    .border_color(theme.rgb(0x29313c))
                    .child(
                        div()
                            .px_2()
                            .py_2()
                            .mb_2()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.rgb(0xdce5f0))
                            .child(language.t("Settings")),
                    )
                    .children(pages.iter().map(|(id, key, label)| {
                        let page_id = *id;
                        let active = page == page_id;
                        div()
                            .id(*key)
                            .px_2()
                            .py_2()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(theme.rgb(if active {
                                0xf1f5fb
                            } else {
                                0x9aa8bc
                            }))
                            .bg(theme.rgb(if active {
                                0x2a413b
                            } else {
                                0x161c24
                            }))
                            .hover(|style| style.bg(theme.rgb(0x222c38)))
                            .child(*label)
                            .on_click(cx.listener(move |workspace, _, _, cx| {
                                workspace.settings_page = page_id;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p_6()
                    .gap_4()
                    .overflow_y_scroll()
                    .when(page == SettingsPage::Appearance, |view| {
                        view.child(
                            div()
                                .text_sm()
                                .text_color(theme.rgb(0x8e9cb0))
                                .child(language.t("Settings help")),
                        )
                        .child(Self::setting_row(
                            language.t("Language").into(),
                            div().flex().flex_wrap().gap_1().children(
                                LANGUAGES.iter().enumerate().map(|(index, &(id, label))| {
                                    div()
                                        .id(("language", index))
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_size(px(12.))
                                        .bg(gpui::rgb(if self.config.language == id {
                                            theme.selected
                                        } else {
                                            theme.raised
                                        }))
                                        .text_color(gpui::rgb(if self.config.language == id {
                                            theme.accent
                                        } else {
                                            theme.foreground
                                        }))
                                        .child(language.t(label))
                                        .on_click(cx.listener(move |workspace, _, _, cx| {
                                            workspace.config.language = id.into();
                                            workspace.apply_appearance(cx);
                                            workspace.save();
                                        }))
                                }),
                            ),
                            theme,
                        ))
                        .child(Self::setting_row(
                            language.t("Theme").into(),
                            div().flex().flex_wrap().gap_2().children(
                                THEMES.iter().enumerate().map(|(index, &(id, label))| {
                                    let sample = Theme::resolve(&Config {
                                        theme: id.into(),
                                        ..self.config.clone()
                                    });
                                    div()
                                        .id(("theme", index))
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .text_size(px(12.))
                                        .border_1()
                                        .border_color(gpui::rgb(if self.config.theme == id {
                                            theme.accent
                                        } else {
                                            theme.border
                                        }))
                                        .child(
                                            div()
                                                .size(px(10.))
                                                .rounded_full()
                                                .bg(gpui::rgb(sample.background))
                                                .border_1()
                                                .border_color(gpui::rgb(sample.accent)),
                                        )
                                        .child(language.t(label))
                                        .on_click(cx.listener(move |workspace, _, _, cx| {
                                            workspace.config.theme = id.into();
                                            workspace.apply_appearance(cx);
                                            workspace.save();
                                        }))
                                }),
                            ),
                            theme,
                        ))
                    })
                    .when(page == SettingsPage::Terminal, |view| {
                        view.child(Self::setting_row(
                            format!(
                                "{} · {} · {} px",
                                language.t("Terminal font"),
                                self.config.font_family,
                                self.config.font_size
                            )
                            .into(),
                            div().flex().gap_2()
                                .child(self.button("font-minus", "−").on_click(
                                    cx.listener(|workspace, _, _, cx| workspace.zoom(-1., cx)),
                                ))
                                .child(self.button("font-plus", "+").on_click(
                                    cx.listener(|workspace, _, _, cx| workspace.zoom(1., cx)),
                                ))
                                .child(
                                    self.button("font-reset", language.t("Reset default"))
                                        .on_click(cx.listener(|workspace, _, _, cx| {
                                            workspace.reset_font_size(cx);
                                        })),
                                ),
                            theme,
                        ))
                        .child(Self::setting_row(
                            format!(
                                "{} · {}",
                                language.t("Command suggestions"),
                                language.t(if self.config.suggestions {
                                    "On"
                                } else {
                                    "Off"
                                }),
                            )
                            .into(),
                            div().child(
                                self.button("toggle-hints", language.t(if self.config.suggestions {
                                    "On"
                                } else {
                                    "Off"
                                }))
                                .on_click(cx.listener(|workspace, _, _, cx| {
                                    workspace.config.suggestions = !workspace.config.suggestions;
                                    for tab in &workspace.tabs {
                                        tab.terminal.update(cx, |view, cx| {
                                            view.suggestions_enabled = workspace.config.suggestions;
                                            view.hints.clear();
                                            cx.notify();
                                        });
                                    }
                                    workspace.save();
                                    cx.notify();
                                })),
                            ),
                            theme,
                        ))
                        .child(Self::setting_row(
                            format!(
                                "{} · {}",
                                language.t("History inspector"),
                                language.t(if self.config.inspector {
                                    "On"
                                } else {
                                    "Off"
                                }),
                            )
                            .into(),
                            div().child(
                                self.button("toggle-inspector", language.t(if self.config.inspector {
                                    "On"
                                } else {
                                    "Off"
                                }))
                                .on_click(cx.listener(|workspace, _, window, cx| {
                                    workspace.inspector(&ToggleInspector, window, cx)
                                })),
                            ),
                            theme,
                        ))
                    })
                    .when(page == SettingsPage::About, |view| {
                        view.child(
                            div()
                                .text_sm()
                                .text_color(theme.rgb(0x8e9cb0))
                                .child(format!(
                                    "{}: {} + {} {}",
                                    language.t("Environment"),
                                    language.t("Inherited"),
                                    self.config.env.len(),
                                    language.t("Global overrides")
                                )),
                        )
                        .child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(theme.rgb(BACKGROUND))
                                .text_size(px(11.))
                                .text_color(theme.rgb(0x96b7ab))
                                .child(
                                    config::directory()
                                        .join("config.toml")
                                        .display()
                                        .to_string(),
                                ),
                        )
                        .child(div().flex().gap_2().child(
                            self.button("edit-config", language.t("Edit config.toml")).on_click(
                                cx.listener(|workspace, _, _, cx| {
                                    if !config::directory().join("config.toml").exists() {
                                        workspace.save();
                                    }
                                    let mut editor = std::process::Command::new(if cfg!(windows) {
                                        "notepad.exe"
                                    } else {
                                        "/usr/bin/open"
                                    });
                                    if cfg!(target_os = "macos") {
                                        editor.arg("-t");
                                    }
                                    if let Err(error) = editor
                                        .arg(config::directory().join("config.toml"))
                                        .spawn()
                                    {
                                        workspace.error = Some(error.to_string());
                                    }
                                    cx.notify();
                                }),
                            ),
                        )).child(
                            self.button("reload-config", language.t("Reload configuration"))
                                .on_click(cx.listener(|workspace, _, window, cx| {
                                    workspace.reload(&ReloadConfig, window, cx)
                                })),
                        )
                        .child(div().text_2xl().mt_4().child(concat!(
                            "WinShell ",
                            env!("CARGO_PKG_VERSION")
                        )))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.rgb(0x7a8a9e))
                                .child(language.t("Settings shortcuts")),
                        )
                    }),
            )
    }

    fn setting_row(
        label: SharedString,
        control: impl IntoElement,
        theme: Theme,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pb_4()
            .border_b_1()
            .border_color(theme.rgb(0x2a3340))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.rgb(0xdce5f0))
                    .child(label),
            )
            .child(control.into_element())
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::resolve(&self.config);
        let language = Language::resolve(&self.config.language);
        let active = self.tabs.get(self.active).map(|tab| tab.terminal.clone());
        let status = active.as_ref().map(|terminal| {
            let terminal = terminal.read(cx);
            let state = terminal.session.state.lock().unwrap();
            (
                state.cwd.display().to_string(),
                if state.closed {
                    language.t("Exited")
                } else if state.at_prompt {
                    language.t("Ready")
                } else {
                    language.t("Running")
                },
                terminal.session.profile.name.clone(),
                terminal.session.profile.bundled,
                state.last_exit,
            )
        });
        let history = active.as_ref().map(|terminal| {
            terminal.read(cx).history_entries(80)
        });
        div().relative().size_full().flex().flex_col().bg(theme.rgb(BACKGROUND)).text_color(theme.rgb(FOREGROUND)).font_family(if cfg!(target_os = "macos") { ".AppleSystemUIFont" } else { "Segoe UI" }).text_size(px(13.))
            .track_focus(&self.focus).key_context("WinShell")
            .on_action(cx.listener(Self::new_tab)).on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab)).on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::sidebar)).on_action(cx.listener(Self::inspector)).on_action(cx.listener(Self::launcher)).on_action(cx.listener(Self::settings))
            .on_action(cx.listener(Self::zoom_in)).on_action(cx.listener(Self::zoom_out)).on_action(cx.listener(Self::reset_zoom)).on_action(cx.listener(Self::reload))
            .on_key_down(cx.listener(Self::key_down))
            .child(div().h(px(40.)).flex_shrink_0().px_3().flex().items_center().justify_between().bg(theme.rgb(0x19202a)).border_b_1().border_color(theme.rgb(0x29313c))
                .child(div().flex().items_center().gap_2()
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(theme.rgb(0xdce5f0)).child("WinShell"))
                    .child(div().text_size(px(10.)).px_1().py_1().rounded_sm().bg(theme.rgb(0x2a3340)).text_color(theme.rgb(0x7a8a9e)).child(concat!("v", env!("CARGO_PKG_VERSION")))))
                .child(div().flex().gap_1()
                    .child(self.button("sidebar", language.t("Sidebar")).on_click(cx.listener(|workspace, _, window, cx| workspace.sidebar(&ToggleSidebar, window, cx))))
                    .child(self.button("inspector", language.t("History")).on_click(cx.listener(|workspace, _, window, cx| workspace.inspector(&ToggleInspector, window, cx))))
                    .child(self.button("launcher", language.t("New terminal")).on_click(cx.listener(|workspace, _, window, cx| workspace.launcher(&Launcher, window, cx))))))
            .child(div().flex().flex_1().min_h_0()
                .when(self.config.sidebar, |view| view.child(self.render_sidebar(cx)))
                .child(div().flex().flex_col().flex_1().min_w_0().min_h_0()
                    .child(div().h(px(36.)).flex_shrink_0().flex().items_center().bg(theme.rgb(0x131921)).border_b_1().border_color(theme.rgb(0x29313c))
                        .child(div().id("tabbar").flex().flex_1().min_w_0().h_full().overflow_x_scroll()
                            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                                div().id(("tab", tab.id)).h_full().w(px(180.)).flex_shrink_0().px_3().flex().items_center().gap_2().cursor_pointer()
                                    .border_b_2().border_color(theme.rgb(if index == self.active { ACCENT } else { 0x131921 }))
                                    .bg(theme.rgb(if index == self.active { BACKGROUND } else { 0x131921 }))
                                    .child(div().text_size(px(11.)).text_color(theme.rgb(if index == self.active { ACCENT } else { 0x6b7a90 })).child("›"))
                                    .child(div().flex_1().truncate().text_size(px(12.)).text_color(theme.rgb(if index == self.active { 0xdce5f0 } else { 0x8a97ab })).child(tab.terminal.read(cx).label()))
                                    .child(div().id(("close", tab.id)).px_1().rounded_sm().hover(|style| style.bg(theme.rgb(0x3f3038))).text_color(theme.rgb(0x6b7a90)).child("×")
                                        .on_click(cx.listener(move |workspace, _, window, cx| { cx.stop_propagation(); workspace.close(index, window, cx); })))
                                    .on_click(cx.listener(move |workspace, _, window, cx| workspace.activate(index, window, cx)))
                            })))
                        .child(self.button("new-tab", "+").on_click(cx.listener(|workspace, _, window, cx| workspace.new_tab(&NewTab, window, cx)))))
                    .child(div().flex().flex_1().min_h_0()
                        .when_some(active, |view, terminal| view.child(terminal))
                        .when(self.tabs.is_empty(), |view| view.child(div().size_full().flex().flex_col().items_center().justify_center().gap_4()
                            .child(div().text_2xl().text_color(theme.rgb(ACCENT)).child(language.t("Your next command starts here.")))
                            .child(div().text_color(theme.rgb(0x6b7a90)).child(language.t("Empty help")))
                            .child(self.button("start", format!("+ {}", language.t("New terminal"))).on_click(cx.listener(|workspace, _, window, cx| workspace.launcher(&Launcher, window, cx))))))
                        .when(self.config.inspector && history.is_some(), |view| {
                            let entries = history.clone().unwrap_or_default();
                            view.child(
                                div()
                                    .id("inspector")
                                    .w(px(260.))
                                    .h_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .bg(theme.rgb(0x161c24))
                                    .border_l_1()
                                    .border_color(theme.rgb(0x29313c))
                                    .child(
                                        div()
                                            .px_3()
                                            .py_2()
                                            .text_size(px(10.))
                                            .text_color(theme.rgb(0x6b7a90))
                                            .border_b_1()
                                            .border_color(theme.rgb(0x29313c))
                                            .child(language.t("History")),
                                    )
                                    .child(
                                        div()
                                            .id("history-list")
                                            .flex()
                                            .flex_col()
                                            .py_1()
                                            .overflow_y_scroll()
                                            .children(entries.iter().enumerate().rev().map(|(index, command)| {
                                                let (head, rest) = match command.split_once(char::is_whitespace) {
                                                    Some((h, r)) => (h.to_string(), r.to_string()),
                                                    None => (command.clone(), String::new()),
                                                };
                                                div()
                                                    .id(("history", index))
                                                    .px_3()
                                                    .py_1()
                                                    .text_size(px(11.))
                                                    .font_family(self.config.font_family.clone())
                                                    .flex()
                                                    .gap_1()
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(theme.rgb(0x222c38)))
                                                    .child(div().text_color(theme.rgb(ACCENT)).child(head))
                                                    .when(!rest.is_empty(), |view| {
                                                        view.child(div().min_w_0().truncate().text_color(theme.rgb(0x9aa8bc)).child(rest))
                                                    })
                                                    .on_click(cx.listener({
                                                        let command = command.clone();
                                                        move |workspace, _, window, cx| {
                                                            let Some(tab) = workspace.tabs.get(workspace.active) else {
                                                                return;
                                                            };
                                                            let text = command.clone();
                                                            tab.terminal.update(cx, |terminal, _| {
                                                                terminal.paste_command_for_inspector(&text);
                                                            });
                                                            workspace.activate(workspace.active, window, cx);
                                                        }
                                                    }))
                                            })),
                                    ),
                            )
                        })
                    ))
            )
            .when_some(self.error.clone(), |view, error| view.child(div().px_3().py_2().flex().justify_between().items_center().bg(theme.rgb(0x3d2830)).text_color(theme.rgb(0xffb5c0)).text_size(px(12.))
                .child(div().truncate().child(error)).child(self.button("dismiss", language.t("Dismiss")).on_click(cx.listener(|workspace, _, _, cx| { workspace.error = None; cx.notify(); })))))
            .child(div().h(px(24.)).flex_shrink_0().px_3().flex().items_center().justify_between().border_t_1().border_color(theme.rgb(0x29313c)).bg(theme.rgb(0x161d25)).text_size(px(10.)).text_color(theme.rgb(0x6b7a90))
                .child(div().flex().items_center().gap_3().min_w_0()
                    .child(div().text_color(theme.rgb(ACCENT)).child(status.as_ref().map(|s| format!("● {}", s.1)).unwrap_or_else(|| "● WinShell".into())))
                    .child(div().truncate().child(status.as_ref().map(|s| s.0.clone()).unwrap_or_default())))
                .child(status.as_ref().map(|s| format!("{}{}  ·  {}  ·  UTF-8  ·  {} px", s.2, if s.3 { format!(" / {}", language.t("Bundled")) } else { String::new() }, if cfg!(windows) { "ConPTY" } else { "PTY" }, self.config.font_size)).unwrap_or_default()))
            .when(self.launcher || self.settings, |view| view.child(
                div().absolute().inset_0().flex().items_center().justify_center().bg(rgba(0x080b11bb))
                    .when(self.launcher, |view| view.child(
                        div().w(px(420.)).p_4().rounded_lg().bg(theme.rgb(0x1b232e)).border_1().border_color(theme.rgb(0x3a4656)).shadow_lg().flex().flex_col().gap_1()
                            .child(div().px_2().py_2().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme.rgb(0xdce5f0)).child(language.t("Open a new terminal")))
                            .children(self.profiles.iter().enumerate().map(|(index, profile)| {
                                div().id(("launch", index)).px_3().py_2().rounded_sm().flex().flex_col().cursor_pointer().text_size(px(12.))
                                    .bg(theme.rgb(if index == self.launcher_index { 0x2a413b } else { 0x1b232e })).hover(|style| style.bg(theme.rgb(0x2a413b)))
                                    .child(profile.name.clone()).child(div().text_size(px(10.)).text_color(theme.rgb(0x7a8a9e)).truncate().child(profile.program.display().to_string()))
                                    .on_click(cx.listener(move |workspace, _, window, cx| workspace.open_profile(index, window, cx)))
                            }))
                            .child(div().px_2().pt_2().text_size(px(10.)).text_color(theme.rgb(0x5d6c80)).child(language.t("Launcher shortcuts")))
                    ))
                    .when(self.settings, |view| view.child(self.render_settings(cx)))
            ))
    }
}
