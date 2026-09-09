use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Point as TermPoint, Side},
    selection::{Selection, SelectionType},
    term::{TermMode, cell::Flags},
    vte::ansi::CursorShape,
};
use gpui::{prelude::*, *};
use std::{
    ops::Range,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use winshell::{
    config::Config,
    i18n::Language,
    input,
    suggestions::{Suggestion, Suggestions},
    terminal::{ACCENT, Frame, Session},
    theme::Theme,
};

actions!(terminal_view, [Copy, Paste, Search, AcceptSuggestion]);

pub struct TerminalView {
    pub session: Session,
    pub focus: FocusHandle,
    pub font_size: f32,
    pub language: Language,
    pub theme: Theme,
    pub font_family: String,
    pub suggestions_enabled: bool,
    pub error: Option<String>,
    pub search: Option<String>,
    pub search_count: usize,
    pub search_index: usize,
    pub hints: Vec<Suggestion>,
    pub hint_query: String,
    pub input_painted: std::cell::Cell<bool>,
    history: Suggestions,
    prompt_count: u64,
    bounds: Bounds<Pixels>,
    cell_width: f32,
    line_height: f32,
    selecting: bool,
    composition: String,
    last_blink: Instant,
    cursor_visible: bool,
    scroll_remainder: f32,
}

impl TerminalView {
    pub fn new(
        session: Session,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            session,
            focus: cx.focus_handle(),
            font_size: config.font_size,
            language: Language::resolve(&config.language),
            theme: Theme::resolve(config),
            font_family: config.font_family.clone(),
            suggestions_enabled: config.suggestions,
            error: None,
            search: None,
            search_count: 0,
            search_index: 0,
            hints: vec![],
            hint_query: String::new(),
            input_painted: std::cell::Cell::new(false),
            history: Suggestions::default(),
            prompt_count: 0,
            bounds: Bounds::default(),
            cell_width: 9.,
            line_height: 22.,
            selecting: false,
            composition: String::new(),
            last_blink: Instant::now(),
            cursor_visible: true,
            scroll_remainder: 0.,
        };
        view.apply_config(config);
        view.load_history();
        cx.on_focus(&view.focus, window, |view, _, cx| {
            view.focus_report(true);
            cx.notify();
        })
        .detach();
        cx.on_blur(&view.focus, window, |view, _, cx| {
            view.focus_report(false);
            cx.notify();
        })
        .detach();
        view
    }

    pub fn apply_config(&mut self, config: &Config) {
        self.font_size = config.font_size;
        self.font_family = config.font_family.clone();
        self.language = Language::resolve(&config.language);
        self.theme = Theme::resolve(config);
        self.session.state.lock().unwrap().theme = self.theme;
        self.suggestions_enabled = config.suggestions;
        if !config.suggestions {
            self.hints.clear();
        }
        self.session.dirty.store(true, Ordering::Release);
    }

    fn terminal_font(&self) -> Font {
        let mut result = font(self.font_family.clone());
        result.fallbacks = Some(FontFallbacks::from_fonts(vec![
            "JetBrains Mono".into(),
            "Cascadia Mono".into(),
            "Consolas".into(),
            "Microsoft YaHei UI".into(),
            "Microsoft JhengHei UI".into(),
            "Yu Gothic UI".into(),
            "Malgun Gothic".into(),
            "Segoe UI Emoji".into(),
            "Segoe UI Symbol".into(),
        ]));
        result
    }

    pub fn history_entries(&self, limit: usize) -> Vec<String> {
        self.history.recent(limit).to_vec()
    }

    pub fn paste_command_for_inspector(&mut self, text: &str) {
        let bracketed = self
            .session
            .state
            .lock()
            .unwrap()
            .term
            .mode()
            .contains(TermMode::BRACKETED_PASTE);
        self.send(input::paste(text, bracketed));
        self.cursor_visible = true;
        self.last_blink = Instant::now();
    }

    fn focus_report(&mut self, focused: bool) {
        let report = self
            .session
            .state
            .lock()
            .unwrap()
            .term
            .mode()
            .contains(TermMode::FOCUS_IN_OUT);
        if report {
            self.send(if focused {
                b"\x1b[I".to_vec()
            } else {
                b"\x1b[O".to_vec()
            });
        }
    }

    fn load_history(&mut self) {
        self.history = Suggestions::default();
        if let Some(path) = self.session.profile.env.get("HISTFILE") {
            self.history.load_history(std::path::Path::new(path));
            return;
        }
        let home = self
            .session
            .profile
            .env
            .get("HOME")
            .map(std::ffi::OsString::from)
            .or_else(|| std::env::var_os("HOME"))
            .map(std::path::PathBuf::from)
            .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()));
        if let Some(home) = home {
            self.history.load_history(&home.join(".bash_history"));
        }
    }

    pub fn tick(&mut self, active: bool, cx: &mut Context<Self>) -> bool {
        self.session.poll_exit();
        let changed = self.session.dirty.swap(false, Ordering::AcqRel);
        if changed {
            let (count, query) = {
                let state = self.session.state.lock().unwrap();
                (state.prompt_count, state.input_query())
            };
            if self.prompt_count != count {
                self.prompt_count = count;
                self.load_history();
            }
            self.hint_query = query.unwrap_or_default();
            self.hints = if self.suggestions_enabled {
                self.history.matching(&self.hint_query)
            } else {
                vec![]
            };
            self.cursor_visible = true;
            self.last_blink = Instant::now();
            if active {
                cx.notify();
            }
        } else if active && self.last_blink.elapsed() >= Duration::from_millis(550) {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
            cx.notify();
        }
        changed
    }

    pub fn label(&self) -> String {
        let state = self.session.state.lock().unwrap();
        let cwd = state
            .cwd
            .file_name()
            .unwrap_or(state.cwd.as_os_str())
            .to_string_lossy();
        format!("{} · {}", self.session.profile.name, cwd)
    }

    fn send(&mut self, bytes: Vec<u8>) {
        if let Err(error) = self.session.write(bytes) {
            self.error = Some(error.to_string());
        }
        self.cursor_visible = true;
        self.last_blink = Instant::now();
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .session
            .state
            .lock()
            .unwrap()
            .term
            .selection_to_string();
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            if self.search.is_some() {
                self.replace_text_in_range(None, &text, window, cx);
            } else {
                let bracketed = self
                    .session
                    .state
                    .lock()
                    .unwrap()
                    .term
                    .mode()
                    .contains(TermMode::BRACKETED_PASTE);
                self.send(input::paste(&text, bracketed));
            }
        }
        cx.notify();
    }

    fn search(&mut self, _: &Search, _: &mut Window, cx: &mut Context<Self>) {
        self.search = Some(String::new());
        self.search_count = 0;
        self.search_index = 0;
        self.composition.clear();
        cx.notify();
    }

    fn find(&mut self, next: bool) {
        let Some(query) = self.search.as_ref().filter(|query| !query.is_empty()) else {
            self.search_count = 0;
            return;
        };
        let mut state = self.session.state.lock().unwrap();
        let mut matches = Vec::new();
        for line in state.term.topmost_line().0..=state.term.bottommost_line().0 {
            let line = alacritty_terminal::index::Line(line);
            let mut text = String::new();
            let mut columns = Vec::new();
            for column in 0..state.term.columns() {
                let cell = &state.term.grid()[line][Column(column)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                columns.extend(std::iter::repeat_n(column, cell.c.len_utf8()));
                text.push(cell.c);
                if let Some(chars) = cell.zerowidth() {
                    for ch in chars {
                        columns.extend(std::iter::repeat_n(column, ch.len_utf8()));
                        text.push(*ch);
                    }
                }
            }
            for (index, found) in text.match_indices(query) {
                matches.push((
                    TermPoint::new(line, Column(columns[index])),
                    TermPoint::new(line, Column(columns[index + found.len() - 1])),
                ));
            }
        }
        self.search_count = matches.len();
        if matches.is_empty() {
            state.term.selection = None;
            return;
        }
        self.search_index = if next {
            (self.search_index + 1) % matches.len()
        } else {
            matches.len() - 1
        };
        let (start, end) = matches[self.search_index];
        state.term.scroll_to_point(start);
        let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
        selection.update(end, Side::Right);
        state.term.selection = Some(selection);
    }

    fn accept_hint(&mut self, index: usize, cx: &mut Context<Self>) {
        let query = self.session.state.lock().unwrap().input_query();
        if let (Some(query), Some(hint)) = (query, self.hints.get(index)) {
            // Revalidate against the live grid; a stale suggestion must never
            // replace a changed line or get sent to a foreground program.
            if !query.is_empty() && query == self.hint_query && hint.command.starts_with(&query) {
                let suffix = hint.command.as_bytes()[query.len()..].to_vec();
                self.send(suffix);
                self.hints.clear();
            }
        }
        cx.notify();
    }

    fn accept_suggestion(&mut self, _: &AcceptSuggestion, _: &mut Window, cx: &mut Context<Self>) {
        if self.search.is_none() && !self.hints.is_empty() {
            self.accept_hint(0, cx);
        } else {
            self.send(b"\x1b[1;3C".to_vec());
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let modifiers = key.modifiers;
        if self.search.is_some() {
            match key.key.as_str() {
                "escape" => {
                    self.search = None;
                    self.composition.clear();
                }
                "enter" => self.find(true),
                "backspace" => {
                    self.search.as_mut().unwrap().pop();
                    self.find(false);
                }
                "a" if modifiers.control => {
                    self.search = Some(String::new());
                    self.find(false);
                }
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if key.key == "right" && modifiers.alt && !self.hints.is_empty() {
            self.accept_hint(0, cx);
            cx.stop_propagation();
            return;
        }
        if modifiers.shift
            && !modifiers.control
            && matches!(key.key.as_str(), "pageup" | "pagedown")
        {
            self.session.scroll(if key.key == "pageup" {
                self.session.size.rows as i32 - 1
            } else {
                1 - self.session.size.rows as i32
            });
            cx.stop_propagation();
            cx.notify();
            return;
        }
        // AltGr (Ctrl+Alt) produces text through the platform input handler.
        if modifiers.control && modifiers.alt && key.key_char.is_some() {
            return;
        }
        let mode = *self.session.state.lock().unwrap().term.mode();
        if let Some(bytes) = input::encode_key(
            &key.key,
            modifiers.control,
            modifiers.alt,
            modifiers.shift,
            mode.contains(TermMode::APP_CURSOR),
        ) {
            if key.key == "enter" || (modifiers.control && matches!(key.key.as_str(), "c" | "d")) {
                self.session.state.lock().unwrap().at_prompt = false;
                self.hints.clear();
            }
            self.send(bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn position(&self, position: Point<Pixels>) -> (usize, usize, bool) {
        let x = f32::from(position.x - self.bounds.left()).max(0.) / self.cell_width;
        let y = f32::from(position.y - self.bounds.top()).max(0.) / self.line_height;
        (y as usize, x as usize, x.fract() >= 0.5)
    }

    fn mouse_report(
        &mut self,
        position: Point<Pixels>,
        button: u8,
        release: bool,
        modifiers: Modifiers,
    ) -> bool {
        let mode = *self.session.state.lock().unwrap().term.mode();
        if modifiers.shift || !mode.intersects(TermMode::MOUSE_MODE) {
            return false;
        }
        let (row, col, _) = self.position(position);
        let code = button + 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control);
        let row = row.min(self.session.size.rows as usize - 1) + 1;
        let col = col.min(self.session.size.cols as usize - 1) + 1;
        if mode.contains(TermMode::SGR_MOUSE) {
            self.send(
                format!(
                    "\x1b[<{code};{col};{row}{}",
                    if release { 'm' } else { 'M' }
                )
                .into_bytes(),
            );
        } else if row < 224 && col < 224 {
            self.send(vec![
                27,
                b'[',
                b'M',
                32 + if release { 3 } else { code },
                32 + col as u8,
                32 + row as u8,
            ]);
        }
        true
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let button = match event.button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            _ => return,
        };
        if self.mouse_report(event.position, button, false, event.modifiers) {
            return;
        }
        if event.button == MouseButton::Left {
            let (row, col, half) = self.position(event.position);
            self.session
                .select(row, col, true, event.click_count == 2, half);
            self.selecting = true;
        } else if event.button == MouseButton::Right {
            self.paste(&Paste, window, cx);
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let mode = *self.session.state.lock().unwrap().term.mode();
        let button = match event.pressed_button {
            Some(MouseButton::Left) => 0,
            Some(MouseButton::Middle) => 1,
            Some(MouseButton::Right) => 2,
            _ => 3,
        };
        if (mode.contains(TermMode::MOUSE_MOTION)
            || (mode.contains(TermMode::MOUSE_DRAG) && button != 3))
            && self.mouse_report(event.position, button + 32, false, event.modifiers)
        {
            return;
        }
        if self.selecting && event.dragging() {
            let (row, col, half) = self.position(event.position);
            self.session.select(row, col, false, false, half);
            cx.notify();
        }
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let button = match event.button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            _ => return,
        };
        self.mouse_report(event.position, button, true, event.modifiers);
        self.selecting = false;
        cx.notify();
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_remainder +=
            f32::from(event.delta.pixel_delta(px(self.line_height)).y) / self.line_height;
        let lines = self.scroll_remainder.trunc() as i32;
        self.scroll_remainder -= lines as f32;
        if lines == 0 {
            return;
        }
        let mode = *self.session.state.lock().unwrap().term.mode();
        if mode.intersects(TermMode::MOUSE_MODE) && !event.modifiers.shift {
            for _ in 0..lines.abs().min(30) {
                self.mouse_report(
                    event.position,
                    if lines > 0 { 64 } else { 65 },
                    false,
                    event.modifiers,
                );
            }
        } else if mode.contains(TermMode::ALT_SCREEN) && !event.modifiers.shift {
            let arrow = input::encode_key(
                if lines > 0 { "up" } else { "down" },
                false,
                false,
                false,
                mode.contains(TermMode::APP_CURSOR),
            )
            .unwrap();
            self.send(arrow.repeat(lines.unsigned_abs().min(30) as usize));
        } else {
            self.session.scroll(lines);
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let language = self.language;
        let state = self.session.state.lock().unwrap();
        let closed = state.closed;
        let error = self.error.clone().or(state.error.clone());
        let cursor_y = state.term.grid().cursor.point.line.0.max(0) as f32 * self.line_height;
        let suggestion_height = 40. + self.hints.len() as f32 * 40.;
        let suggestion_top = if cursor_y + self.line_height + suggestion_height
            < f32::from(self.session.size.rows) * self.line_height
        {
            16. + cursor_y + self.line_height
        } else {
            (16. + cursor_y - suggestion_height).max(16.)
        };
        drop(state);
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .key_context("Terminal")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::search))
            .on_action(cx.listener(Self::accept_suggestion))
            .on_key_down(cx.listener(Self::key_down))
            .when(self.search.is_some(), |view| {
                view.child(
                    div()
                        .h(px(42.))
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_3()
                        .bg(theme.rgb(0x202832))
                        .child(
                            div()
                                .text_color(theme.rgb(ACCENT))
                                .child(language.t("Find")),
                        )
                        .child(div().flex_1().child(format!(
                            "{}{}│",
                            self.search.as_deref().unwrap_or_default(),
                            self.composition
                        )))
                        .child(
                            div()
                                .text_color(theme.rgb(0x8c98aa))
                                .text_xs()
                                .child(format!(
                                    "{} {} · {}",
                                    self.search_count,
                                    language.t("matches"),
                                    language.t("Find shortcuts")
                                )),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .overflow_hidden()
                    .cursor(CursorStyle::IBeam)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
                    .on_mouse_move(cx.listener(Self::mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
                    .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
                    .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
                    .on_scroll_wheel(cx.listener(Self::wheel))
                    .child(TerminalCanvas { view: cx.entity() }),
            )
            .when(self.search.is_none() && !self.hints.is_empty(), |view| {
                view.child(
                    div()
                        // Suggestions must never resize the PTY while Readline
                        // is receiving input. A flowing panel caused SIGWINCH
                        // redraws to race with accepted suffixes on ConPTY.
                        .absolute()
                        .occlude()
                        .left_4()
                        .right_4()
                        .top(px(suggestion_top))
                        .shadow_lg()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.rgb(0x30413f))
                        .bg(theme.rgb(0x192322))
                        .overflow_hidden()
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(theme.rgb(0x8baca3))
                                .child(language.t("Suggestions"))
                                .child(language.t("Suggestion shortcuts")),
                        )
                        .children(self.hints.iter().enumerate().map(|(index, hint)| {
                            div()
                                .id(("hint", index))
                                .px_3()
                                .py_2()
                                .flex()
                                .justify_between()
                                .gap_4()
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.rgb(0x263834)))
                                .child(
                                    div()
                                        .font_family(self.font_family.clone())
                                        .text_color(theme.rgb(0xbfe9dc))
                                        .child(hint.command.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.rgb(0x8ba49e))
                                        .child(language.t(&hint.detail).to_owned()),
                                )
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.accept_hint(index, cx);
                                    window.focus(&view.focus);
                                }))
                        })),
                )
            })
            .when(closed, |view| {
                view.child(
                    div()
                        .px_4()
                        .py_3()
                        .bg(theme.rgb(0x282329))
                        .text_color(theme.rgb(0xeac48b))
                        .child(language.t("Session ended")),
                )
            })
            .when_some(error, |view, error| {
                view.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_color(theme.rgb(0xef7d8e))
                        .child(error),
                )
            })
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let utf16: Vec<_> = self.composition.encode_utf16().collect();
        let range = range.start.min(utf16.len())..range.end.min(utf16.len());
        *adjusted = Some(range.clone());
        Some(String::from_utf16_lossy(&utf16[range]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.composition.encode_utf16().count();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition.clear();
        if let Some(query) = &mut self.search {
            query.extend(text.chars().filter(|ch| !ch.is_control()));
            self.find(false);
        } else {
            self.send(text.as_bytes().to_vec());
        }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition = text.into();
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let cursor = self.session.state.lock().unwrap().term.grid().cursor.point;
        Some(Bounds::new(
            point(
                bounds.left() + px(cursor.column.0 as f32 * self.cell_width),
                bounds.top() + px(cursor.line.0 as f32 * self.line_height),
            ),
            size(px(self.cell_width), px(self.line_height)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

struct TerminalCanvas {
    view: Entity<TerminalView>,
}
struct CanvasState {
    frame: Frame,
    cell_width: Pixels,
    line_height: Pixels,
}
impl IntoElement for TerminalCanvas {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TerminalCanvas {
    type RequestLayoutState = ();
    type PrepaintState = CanvasState;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> CanvasState {
        self.view.update(cx, |view, _| {
            let base_font = view.terminal_font();
            let run = TextRun {
                len: 1,
                font: base_font,
                color: rgb(view.theme.foreground).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let cell_width = window
                .text_system()
                .shape_line("M".into(), px(view.font_size), &[run], None)
                .width
                .max(px(1.));
            let line_height = px((view.font_size * 1.5).ceil());
            view.cell_width = cell_width.into();
            view.line_height = line_height.into();
            view.bounds = bounds;
            if let Err(error) = view.session.resize(
                (bounds.size.height / line_height).floor() as u16,
                (bounds.size.width / cell_width).floor() as u16,
            ) {
                view.error = Some(error.to_string());
            }
            let frame = view.session.state.lock().unwrap().frame();
            CanvasState {
                frame,
                cell_width,
                line_height,
            }
        })
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut CanvasState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let view = self.view.read(cx);
        let focus = view.focus.clone();
        let font_size = px(view.font_size);
        let base_font = view.terminal_font();
        let composition = view.composition.clone();
        let theme = view.theme;
        let searching = view.search.is_some();
        let show_cursor = view.cursor_visible && focus.is_focused(window) && !searching;
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        // The platform handler is committed with this frame. An automated key
        // dispatch must not precede the first paint after focus/tab changes.
        view.input_painted.set(focus.is_focused(window));
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for glyph in &state.frame.cells {
                let origin = point(
                    bounds.left() + state.cell_width * glyph.col as f32,
                    bounds.top() + state.line_height * glyph.row as f32,
                );
                let width = state.cell_width
                    * if glyph.flags.contains(Flags::WIDE_CHAR) {
                        2.
                    } else {
                        1.
                    };
                if glyph.bg != theme.background {
                    window.paint_quad(fill(
                        Bounds::new(origin, size(width, state.line_height)),
                        rgb(glyph.bg),
                    ));
                }
                if glyph.text.is_empty() {
                    continue;
                }
                let mut cell_font = base_font.clone();
                if glyph.flags.contains(Flags::BOLD) {
                    cell_font.weight = FontWeight::BOLD;
                }
                if glyph.flags.contains(Flags::ITALIC) {
                    cell_font.style = FontStyle::Italic;
                }
                let run = TextRun {
                    len: glyph.text.len(),
                    font: cell_font,
                    color: rgb(glyph.fg).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line = window.text_system().shape_line(
                    glyph.text.clone().into(),
                    font_size,
                    &[run],
                    None,
                );
                let _ = line.paint(origin, state.line_height, window, cx);
                if glyph.flags.intersects(Flags::ALL_UNDERLINES) {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(origin.x, origin.y + state.line_height - px(2.)),
                            size(width, px(1.)),
                        ),
                        rgb(glyph.fg),
                    ));
                }
                if glyph.flags.contains(Flags::STRIKEOUT) {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(origin.x, origin.y + state.line_height / 2.),
                            size(width, px(1.)),
                        ),
                        rgb(glyph.fg),
                    ));
                }
            }
            if let Some((row, col, shape)) = state.frame.cursor {
                let origin = point(
                    bounds.left() + state.cell_width * col as f32,
                    bounds.top() + state.line_height * row as f32,
                );
                if show_cursor {
                    let cursor_bounds = match shape {
                        CursorShape::Underline => Bounds::new(
                            point(origin.x, origin.y + state.line_height - px(2.)),
                            size(state.cell_width, px(2.)),
                        ),
                        CursorShape::Block => {
                            Bounds::new(origin, size(state.cell_width, state.line_height))
                        }
                        _ => Bounds::new(origin, size(px(2.), state.line_height)),
                    };
                    window.paint_quad(fill(
                        cursor_bounds,
                        rgba(if shape == CursorShape::Block {
                            (theme.accent << 8) | 0x70
                        } else {
                            (theme.accent << 8) | 0xff
                        }),
                    ));
                }
                if !composition.is_empty() && !searching {
                    let run = TextRun {
                        len: composition.len(),
                        font: base_font,
                        color: rgb(theme.accent).into(),
                        background_color: Some(rgb(theme.background).into()),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: None,
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let line = window.text_system().shape_line(
                        composition.into(),
                        font_size,
                        &[run],
                        None,
                    );
                    let _ = line.paint(origin, state.line_height, window, cx);
                }
            }
        });
    }
}
