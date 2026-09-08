use crate::terminal_view::{self, TerminalView};
use gpui::{prelude::*, *};
use std::time::Duration;
use winshell::{
    config::{self, Config},
    shell::{self, ShellProfile},
    terminal::{ACCENT, BACKGROUND, FOREGROUND, Session},
};

actions!(
    winshell_app,
    [
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        ToggleSidebar,
        Launcher,
        Settings,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        ReloadConfig
    ]
);

pub fn run(config: Config, startup_error: Option<String>) {
    Application::new().run(move |cx| {
        cx.bind_keys([
            KeyBinding::new("ctrl-shift-t", NewTab, None),
            KeyBinding::new("ctrl-shift-w", CloseTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
            KeyBinding::new("ctrl-shift-b", ToggleSidebar, None),
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
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1220.), px(780.)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(680.), px(430.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WinShell".into()),
                    ..Default::default()
                }),
                app_id: Some("winshell".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Workspace::new(config, startup_error, window, cx)),
        );
        if let Err(error) = result {
            eprintln!("Could not open WinShell: {error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}

struct Tab {
    id: usize,
    terminal: Entity<TerminalView>,
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
    fn save(&mut self) {
        if let Err(error) = self.config.save() {
            self.error = Some(error.to_string());
        }
    }
    fn launcher(&mut self, _: &Launcher, window: &mut Window, cx: &mut Context<Self>) {
        self.launcher = !self.launcher;
        self.settings = false;
        self.launcher_index = 0;
        if self.launcher {
            window.focus(&self.focus);
        } else {
            self.activate(self.active, window, cx);
        }
        cx.notify();
    }
    fn settings(&mut self, _: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = !self.settings;
        self.launcher = false;
        if self.settings {
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
    fn reset_zoom(&mut self, _: &ResetZoom, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom(15. - self.config.font_size, cx);
    }
    fn reload(&mut self, _: &ReloadConfig, _: &mut Window, cx: &mut Context<Self>) {
        match Config::load() {
            Ok(config) => {
                self.config = config;
                self.profiles = shell::discover(&self.config);
                for tab in &self.tabs {
                    tab.terminal.update(cx, |view, cx| {
                        view.font_size = self.config.font_size;
                        view.font_family = self.config.font_family.clone();
                        view.suggestions_enabled = self.config.suggestions;
                        if !view.suggestions_enabled {
                            view.hints.clear();
                        }
                        cx.notify();
                    });
                }
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{error:#}")),
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
                _ => {}
            }
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn button(id: &'static str, text: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(0xa8b3c4))
            .hover(|style| style.bg(rgb(0x293340)).text_color(rgb(0xf1f5fb)))
            .child(text.into())
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(208.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0x161c24))
            .border_r_1()
            .border_color(rgb(0x29313c))
            .p_3()
            .gap_2()
            .child(
                div()
                    .px_2()
                    .pt_4()
                    .pb_2()
                    .text_size(px(10.))
                    .text_color(rgb(0x748095))
                    .child("WORKSPACE"),
            )
            .child(
                div()
                    .px_2()
                    .pb_3()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xdce5f0))
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
                    .px_2()
                    .py_2()
                    .flex()
                    .justify_between()
                    .text_size(px(11.))
                    .text_color(rgb(0x8896aa))
                    .child("SESSIONS")
                    .child(self.tabs.len().to_string()),
            )
            .child(
                div()
                    .id("session-list")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(280.))
                    .overflow_y_scroll()
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let terminal = tab.terminal.read(cx);
                        let closed = terminal.session.state.lock().unwrap().closed;
                        div()
                            .id(("session", tab.id))
                            .p_2()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .bg(rgb(if index == self.active {
                                0x233631
                            } else {
                                0x161c24
                            }))
                            .hover(|style| style.bg(rgb(0x25312f)))
                            .child(
                                div()
                                    .text_color(rgb(if closed { 0x657387 } else { ACCENT }))
                                    .child("›_"),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .truncate()
                                    .child(terminal.label()),
                            )
                            .on_click(cx.listener(move |workspace, _, window, cx| {
                                workspace.activate(index, window, cx)
                            }))
                    })),
            )
            .child(
                div()
                    .px_2()
                    .pt_5()
                    .pb_2()
                    .text_size(px(10.))
                    .text_color(rgb(0x748095))
                    .child("SHELLS"),
            )
            .children(self.profiles.iter().enumerate().map(|(index, profile)| {
                div()
                    .id(("profile", index))
                    .px_2()
                    .py_2()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(rgb(0xa8b3c4))
                    .hover(|style| style.bg(rgb(0x222c38)))
                    .child(profile.name.clone())
                    .child(div().text_color(rgb(0x657387)).child("+"))
                    .on_click(cx.listener(move |workspace, _, window, cx| {
                        workspace.open_profile(index, window, cx)
                    }))
            }))
            .child(div().flex_1())
            .child(
                div()
                    .px_2()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(0x29313c))
                    .text_size(px(11.))
                    .text_color(rgb(0x748095))
                    .child("One workspace. Every shell."),
            )
            .child(
                Self::button("settings-side", "Settings                 Ctrl + ,").on_click(
                    cx.listener(|workspace, _, window, cx| {
                        workspace.settings(&Settings, window, cx)
                    }),
                ),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(520.))
            .p_6()
            .rounded_xl()
            .bg(rgb(0x1b232e))
            .border_1()
            .border_color(rgb(0x3a4656))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Make it yours"),
            )
            .child(div().text_sm().text_color(rgb(0x8e9cb0)).child(
                "Appearance updates immediately. Shell and environment changes apply to new tabs.",
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(format!("Terminal font · {} px", self.config.font_size))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(Self::button("font-minus", "−").on_click(
                                cx.listener(|workspace, _, _, cx| workspace.zoom(-1., cx)),
                            ))
                            .child(Self::button("font-plus", "+").on_click(
                                cx.listener(|workspace, _, _, cx| workspace.zoom(1., cx)),
                            )),
                    ),
            )
            .child(
                Self::button(
                    "toggle-hints",
                    format!(
                        "Command suggestions    {}",
                        if self.config.suggestions { "On" } else { "Off" }
                    ),
                )
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
            )
            .child(div().text_sm().text_color(rgb(0x8e9cb0)).child(format!(
                "Environment: inherited + {} global overrides",
                self.config.env.len()
            )))
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(BACKGROUND))
                    .text_size(px(11.))
                    .text_color(rgb(0x96b7ab))
                    .child(
                        config::directory()
                            .join("config.toml")
                            .display()
                            .to_string(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Self::button("edit-config", "Edit config.toml").on_click(cx.listener(
                            |workspace, _, _, cx| {
                                if !config::directory().join("config.toml").exists() {
                                    workspace.save();
                                }
                                if let Err(error) = std::process::Command::new("notepad.exe")
                                    .arg(config::directory().join("config.toml"))
                                    .spawn()
                                {
                                    workspace.error = Some(error.to_string());
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Self::button("reload-config", "Reload configuration").on_click(
                            cx.listener(|workspace, _, window, cx| {
                                workspace.reload(&ReloadConfig, window, cx)
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x748095))
                    .child("Ctrl + Shift + R reloads · Esc closes settings"),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.tabs.get(self.active).map(|tab| tab.terminal.clone());
        let status = active.as_ref().map(|terminal| {
            let terminal = terminal.read(cx);
            let state = terminal.session.state.lock().unwrap();
            (
                state.cwd.display().to_string(),
                if state.closed {
                    "Exited"
                } else if state.at_prompt {
                    "Ready"
                } else {
                    "Running"
                },
                terminal.session.profile.name.clone(),
                terminal.session.profile.bundled,
                state.last_exit,
            )
        });
        div().relative().size_full().flex().flex_col().bg(rgb(BACKGROUND)).text_color(rgb(FOREGROUND)).font_family("Segoe UI").text_size(px(13.))
            .track_focus(&self.focus).key_context("WinShell")
            .on_action(cx.listener(Self::new_tab)).on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab)).on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::sidebar)).on_action(cx.listener(Self::launcher)).on_action(cx.listener(Self::settings))
            .on_action(cx.listener(Self::zoom_in)).on_action(cx.listener(Self::zoom_out)).on_action(cx.listener(Self::reset_zoom)).on_action(cx.listener(Self::reload))
            .on_key_down(cx.listener(Self::key_down))
            .child(div().h(px(54.)).flex_shrink_0().px_4().flex().items_center().justify_between().bg(rgb(0x19202a)).border_b_1().border_color(rgb(0x29313c))
                .child(div().flex().items_center().gap_3()
                    .child(div().px_2().py_1().rounded_md().bg(rgb(ACCENT)).text_color(rgb(0x102720)).font_weight(FontWeight::BOLD).font_family("Consolas").child("›_"))
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child("winshell"))
                    .child(div().text_size(px(10.)).px_2().py_1().rounded_sm().bg(rgb(0x2a3340)).text_color(rgb(0x8f9db1)).child("PREVIEW 0.1")))
                .child(div().flex().gap_2()
                    .child(Self::button("sidebar", "Sidebar").on_click(cx.listener(|workspace, _, window, cx| workspace.sidebar(&ToggleSidebar, window, cx))))
                    .child(Self::button("launcher", "New terminal   Ctrl + Shift + P").on_click(cx.listener(|workspace, _, window, cx| workspace.launcher(&Launcher, window, cx))))))
            .child(div().flex().flex_1().min_h_0()
                .when(self.config.sidebar, |view| view.child(self.render_sidebar(cx)))
                .child(div().flex().flex_col().flex_1().min_w_0().min_h_0()
                    .child(div().h(px(45.)).flex_shrink_0().flex().items_center().bg(rgb(0x131921)).border_b_1().border_color(rgb(0x29313c))
                        .child(div().id("tabbar").flex().flex_1().min_w_0().h_full().overflow_x_scroll()
                            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                                div().id(("tab", tab.id)).h_full().w(px(206.)).flex_shrink_0().px_3().flex().items_center().gap_2().cursor_pointer()
                                    .border_b_2().border_color(rgb(if index == self.active { ACCENT } else { 0x131921 }))
                                    .bg(rgb(if index == self.active { BACKGROUND } else { 0x131921 }))
                                    .child(div().text_color(rgb(ACCENT)).child("›_"))
                                    .child(div().flex_1().truncate().text_size(px(12.)).child(tab.terminal.read(cx).label()))
                                    .child(div().id(("close", tab.id)).px_1().rounded_sm().hover(|style| style.bg(rgb(0x3f3038))).text_color(rgb(0x7d899b)).child("×")
                                        .on_click(cx.listener(move |workspace, _, window, cx| { cx.stop_propagation(); workspace.close(index, window, cx); })))
                                    .on_click(cx.listener(move |workspace, _, window, cx| workspace.activate(index, window, cx)))
                            })))
                        .child(Self::button("new-tab", "+").on_click(cx.listener(|workspace, _, window, cx| workspace.new_tab(&NewTab, window, cx)))))
                    .when_some(active, |view, terminal| view.child(terminal))
                    .when(self.tabs.is_empty(), |view| view.child(div().size_full().flex().flex_col().items_center().justify_center().gap_4()
                        .child(div().text_3xl().text_color(rgb(ACCENT)).child("Your next command starts here."))
                        .child(div().text_color(rgb(0x8290a5)).child("Open Bash, PowerShell, or your own shell in a new tab."))
                        .child(Self::button("start", "+ Open a terminal").on_click(cx.listener(|workspace, _, window, cx| workspace.launcher(&Launcher, window, cx))))))))
            .when_some(self.error.clone(), |view, error| view.child(div().px_4().py_2().flex().justify_between().bg(rgb(0x3d2830)).text_color(rgb(0xffb5c0))
                .child(error).child(Self::button("dismiss", "Dismiss").on_click(cx.listener(|workspace, _, _, cx| { workspace.error = None; cx.notify(); })))))
            .child(div().h(px(30.)).flex_shrink_0().px_4().flex().items_center().justify_between().border_t_1().border_color(rgb(0x29313c)).bg(rgb(0x161d25)).text_size(px(10.)).text_color(rgb(0x8190a4))
                .child(div().flex().items_center().gap_3().min_w_0()
                    .child(div().text_color(rgb(ACCENT)).child(status.as_ref().map(|s| format!("● {}", s.1)).unwrap_or_else(|| "● WinShell".into())))
                    .child(div().truncate().child(status.as_ref().map(|s| s.0.clone()).unwrap_or_default())))
                .child(status.as_ref().map(|s| format!("{}{}  ·  ConPTY  ·  UTF-8  ·  {} px", s.2, if s.3 { " / bundled" } else { "" }, self.config.font_size)).unwrap_or_default()))
            .when(self.launcher || self.settings, |view| view.child(
                div().absolute().inset_0().flex().items_start().justify_center().pt(px(100.)).bg(rgba(0x080b11bb))
                    .when(self.launcher, |view| view.child(
                        div().w(px(500.)).p_4().rounded_xl().bg(rgb(0x1b232e)).border_1().border_color(rgb(0x3a4656)).shadow_lg().flex().flex_col().gap_2()
                            .child(div().px_2().py_3().text_lg().font_weight(FontWeight::SEMIBOLD).child("Open a new terminal"))
                            .children(self.profiles.iter().enumerate().map(|(index, profile)| {
                                div().id(("launch", index)).p_3().rounded_md().flex().flex_col().gap_1().cursor_pointer()
                                    .bg(rgb(if index == self.launcher_index { 0x2a413b } else { 0x1b232e })).hover(|style| style.bg(rgb(0x2a413b)))
                                    .child(profile.name.clone()).child(div().text_xs().text_color(rgb(0x849d94)).truncate().child(profile.program.display().to_string()))
                                    .on_click(cx.listener(move |workspace, _, window, cx| workspace.open_profile(index, window, cx)))
                            }))
                            .child(div().px_2().pt_3().text_xs().text_color(rgb(0x748095)).child("↑ ↓ to choose · Enter to open · Esc to close"))
                    ))
                    .when(self.settings, |view| view.child(self.render_settings(cx)))
            ))
    }
}
