use crate::shell::ShellProfile;
use alacritty_terminal::{
    Term,
    event::{Event, EventListener, WindowSize},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{Config as TermConfig, Osc52, TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape, Processor, Rgb},
};
use anyhow::{Context, Result};
use portable_pty::{Child, MasterPty, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};

pub const BACKGROUND: u32 = 0x101419;
pub const FOREGROUND: u32 = 0xd8dee9;
pub const ACCENT: u32 = 0x74d5bb;
const PALETTE: [u32; 16] = [
    0x26303b, 0xef7d8e, 0x91d7a3, 0xeac48b, 0x82aaff, 0xc3a6ff, 0x7dd6df, 0xd8dee9, 0x657387,
    0xff96a7, 0xb4edb5, 0xffdfa8, 0xaac5ff, 0xdcc3ff, 0xa0eef4, 0xf2f5fa,
];

pub fn indexed_color(index: usize) -> u32 {
    match index {
        0..=15 => PALETTE[index],
        16..=231 => {
            let levels = [0, 95, 135, 175, 215, 255];
            let color = index - 16;
            (levels[color / 36] << 16) | (levels[color / 6 % 6] << 8) | levels[color % 6]
        }
        232..=255 => {
            let grey = 8 + (index as u32 - 232) * 10;
            (grey << 16) | (grey << 8) | grey
        }
        257 => BACKGROUND,
        258 => ACCENT,
        259..=266 => dim(PALETTE[index - 259]),
        _ => FOREGROUND,
    }
}

fn dim(color: u32) -> u32 {
    ((color & 0xfefefe) >> 1) + ((color & 0xfcfcfc) >> 2)
}
fn rgb_number(color: Rgb) -> u32 {
    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

#[derive(Clone)]
pub struct EventProxy(mpsc::Sender<Event>);
impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

pub struct TerminalState {
    pub term: Term<EventProxy>,
    pub title: String,
    pub cwd: PathBuf,
    pub at_prompt: bool,
    pub prompt_count: u64,
    pub last_exit: Option<i32>,
    pub closed: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Glyph {
    pub row: usize,
    pub col: usize,
    pub text: String,
    pub fg: u32,
    pub bg: u32,
    pub flags: Flags,
}

pub struct Frame {
    pub cells: Vec<Glyph>,
    pub cursor: Option<(usize, usize, CursorShape)>,
    pub offset: usize,
}

impl TerminalState {
    fn new(size: Size, scrollback: usize, cwd: &Path) -> (Self, Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let config = TermConfig {
            scrolling_history: scrollback,
            osc52: Osc52::Disabled,
            ..Default::default()
        };
        (
            Self {
                term: Term::new(config, &size, EventProxy(tx)),
                title: String::new(),
                cwd: cwd.into(),
                at_prompt: false,
                prompt_count: 0,
                last_exit: None,
                closed: false,
                error: None,
            },
            rx,
        )
    }

    pub fn frame(&self) -> Frame {
        let content = self.term.renderable_content();
        let offset = content.display_offset;
        let resolve = |color: Color| match color {
            Color::Spec(rgb) => rgb_number(rgb),
            Color::Indexed(index) => content.colors[index as usize]
                .map(rgb_number)
                .unwrap_or_else(|| indexed_color(index as usize)),
            Color::Named(name) => content.colors[name as usize]
                .map(rgb_number)
                .unwrap_or_else(|| indexed_color(name as usize)),
        };
        let mut cells = Vec::new();
        for indexed in content.display_iter {
            let cell = indexed.cell;
            let row = indexed.point.line.0 + offset as i32;
            if row < 0 {
                continue;
            }
            let mut foreground = resolve(cell.fg);
            if cell.flags.contains(Flags::BOLD)
                && let Color::Named(named) = cell.fg
                && (named as usize) < 8
            {
                foreground = indexed_color(named as usize + 8);
            }
            if cell.flags.contains(Flags::DIM) {
                foreground = dim(foreground);
            }
            let mut background = resolve(cell.bg);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }
            if content
                .selection
                .is_some_and(|selection| selection.contains(indexed.point))
            {
                background = 0x344c5b;
            }
            let spacer = cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);
            let mut text = if cell.flags.contains(Flags::HIDDEN) || spacer {
                String::new()
            } else {
                cell.c.to_string()
            };
            if let Some(zero_width) = cell.zerowidth() {
                text.extend(zero_width);
            }
            if text == " "
                && background == BACKGROUND
                && !cell
                    .flags
                    .intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT)
            {
                continue;
            }
            cells.push(Glyph {
                row: row as usize,
                col: indexed.point.column.0,
                text,
                fg: foreground,
                bg: background,
                flags: cell.flags,
            });
        }
        let cursor = content.cursor;
        let row = cursor.point.line.0 + offset as i32;
        let cursor = if content.mode.contains(TermMode::SHOW_CURSOR)
            && row >= 0
            && row < self.term.screen_lines() as i32
            && cursor.shape != CursorShape::Hidden
        {
            Some((row as usize, cursor.point.column.0, cursor.shape))
        } else {
            None
        };
        Frame {
            cells,
            cursor,
            offset,
        }
    }

    /// Suggest only for the integrated Bash prompt, at the end of a single line.
    /// Reading the rendered line also handles Readline history/editing faithfully.
    pub fn input_query(&self) -> Option<String> {
        if !self.at_prompt
            || self.closed
            || self.term.mode().contains(TermMode::ALT_SCREEN)
            || self.term.grid().display_offset() != 0
        {
            return None;
        }
        let cursor = self.term.grid().cursor.point;
        let row = &self.term.grid()[cursor.line];
        if cursor.column.0 < 2 || row[Column(0)].c != '❯' || row[Column(1)].c != ' ' {
            return None;
        }
        if (cursor.column.0..self.term.columns()).any(|column| row[Column(column)].c != ' ') {
            return None;
        }
        let mut query = String::new();
        for column in 2..cursor.column.0 {
            let cell = &row[Column(column)];
            if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                query.push(cell.c);
            }
            if let Some(zero_width) = cell.zerowidth() {
                query.extend(zero_width);
            }
        }
        Some(query)
    }

    pub fn text(&self) -> String {
        self.term.bounds_to_string(
            Point::new(self.term.topmost_line(), Column(0)),
            Point::new(self.term.bottommost_line(), self.term.last_column()),
        )
    }
}

/// Streaming OSC observer. It never removes bytes from the terminal parser and
/// handles sequences split across arbitrary PTY read boundaries.
#[derive(Default)]
struct Integration {
    escape: bool,
    osc: Option<Vec<u8>>,
}
impl Integration {
    fn process(&mut self, bytes: &[u8], state: &mut TerminalState) {
        for &byte in bytes {
            if let Some(buffer) = &mut self.osc {
                if byte == 7 || (self.escape && byte == b'\\') {
                    if self.escape && byte == b'\\' {
                        buffer.pop();
                    }
                    let payload = String::from_utf8_lossy(buffer).into_owned();
                    self.osc = None;
                    self.escape = false;
                    self.dispatch(&payload, state);
                } else if buffer.len() >= 8192 {
                    self.osc = None;
                    self.escape = false;
                } else {
                    buffer.push(byte);
                    self.escape = byte == 27;
                }
            } else if self.escape && byte == b']' {
                self.osc = Some(Vec::new());
                self.escape = false;
            } else {
                self.escape = byte == 27;
            }
        }
    }
    fn dispatch(&self, payload: &str, state: &mut TerminalState) {
        if payload.starts_with("133;A") {
            state.at_prompt = false;
            state.prompt_count += 1;
            state.last_exit = payload
                .strip_prefix("133;A;")
                .and_then(|code| code.parse().ok());
        } else if payload == "133;B" {
            state.at_prompt = true;
        } else if payload == "133;C" {
            state.at_prompt = false;
        } else if let Some(path) = payload.strip_prefix("7;file://localhost/")
            && !path.chars().any(char::is_control)
        {
            state.cwd = PathBuf::from(path);
        }
    }
}

pub struct Session {
    pub state: Arc<Mutex<TerminalState>>,
    pub dirty: Arc<AtomicBool>,
    pub profile: ShellProfile,
    pub size: Size,
    writer: Option<SyncSender<Vec<u8>>>,
    master: Option<Box<dyn MasterPty + Send>>,
    child: Box<dyn Child + Send + Sync>,
}

impl Session {
    pub fn spawn(profile: ShellProfile, cwd: &Path, scrollback: usize) -> Result<Self> {
        let size = Size {
            rows: 28,
            cols: 100,
        };
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Could not create Windows ConPTY")?;
        let command = profile.command(cwd)?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .with_context(|| format!("Could not launch {}", profile.program.display()))?;
        drop(pair.slave);
        let resources = (|| -> Result<_> {
            Ok((pair.master.try_clone_reader()?, pair.master.take_writer()?))
        })();
        let (mut reader, mut writer) = match resources {
            Ok(resources) => resources,
            Err(error) => {
                let _ = child.kill();
                return Err(error);
            }
        };
        let (state, events) = TerminalState::new(size, scrollback, cwd);
        let state = Arc::new(Mutex::new(state));
        let dirty = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(128);
        let writer_state = state.clone();
        let writer_dirty = dirty.clone();
        thread::spawn(move || {
            while let Ok(bytes) = rx.recv() {
                if let Err(error) = writer.write_all(&bytes).and_then(|_| writer.flush()) {
                    let mut state = writer_state.lock().unwrap_or_else(|p| p.into_inner());
                    state.error = Some(format!("PTY input: {error}"));
                    writer_dirty.store(true, Ordering::Release);
                    break;
                }
            }
        });
        let read_state = state.clone();
        let read_dirty = dirty.clone();
        let response_tx = tx.clone();
        thread::spawn(move || {
            let mut parser: Processor = Processor::new();
            let mut integration = Integration::default();
            let mut bytes = [0u8; 32 * 1024];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(length) => {
                        let mut state = read_state.lock().unwrap_or_else(|p| p.into_inner());
                        parser.advance(&mut state.term, &bytes[..length]);
                        integration.process(&bytes[..length], &mut state);
                        for event in events.try_iter() {
                            let response = match event {
                                Event::Title(title) => {
                                    state.title = title
                                        .chars()
                                        .filter(|c| !c.is_control())
                                        .take(160)
                                        .collect();
                                    None
                                }
                                Event::ResetTitle => {
                                    state.title.clear();
                                    None
                                }
                                Event::PtyWrite(text) => Some(text),
                                Event::ColorRequest(index, format) => {
                                    let rgb = state.term.colors()[index]
                                        .map(rgb_number)
                                        .unwrap_or_else(|| indexed_color(index));
                                    Some(format(Rgb {
                                        r: (rgb >> 16) as u8,
                                        g: (rgb >> 8) as u8,
                                        b: rgb as u8,
                                    }))
                                }
                                Event::TextAreaSizeRequest(format) => Some(format(WindowSize {
                                    num_lines: state.term.screen_lines() as u16,
                                    num_cols: state.term.columns() as u16,
                                    cell_width: 9,
                                    cell_height: 22,
                                })),
                                _ => None,
                            };
                            if let Some(response) = response {
                                let _ = response_tx.try_send(response.into_bytes());
                            }
                        }
                        read_dirty.store(true, Ordering::Release);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        read_state.lock().unwrap_or_else(|p| p.into_inner()).error =
                            Some(format!("PTY output: {error}"));
                        break;
                    }
                }
            }
            read_state.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
            read_dirty.store(true, Ordering::Release);
        });
        Ok(Self {
            state,
            dirty,
            profile,
            size,
            writer: Some(tx),
            master: Some(pair.master),
            child,
        })
    }

    pub fn write(&self, bytes: impl Into<Vec<u8>>) -> Result<()> {
        self.writer
            .as_ref()
            .context("Session is closed")?
            .try_send(bytes.into())
            .context("Terminal input queue is unavailable")?;
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.term.scroll_display(Scroll::Bottom);
        state.term.selection = None;
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let size = Size {
            rows: rows.clamp(2, 500),
            cols: cols.clamp(2, 1000),
        };
        if self.size == size {
            return Ok(());
        }
        // Resize the model before releasing ConPTY so new output uses new bounds.
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        self.master
            .as_ref()
            .context("Session is closed")?
            .resize(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        state.term.resize(size);
        self.size = size;
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }

    pub fn poll_exit(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if !state.closed {
                state.closed = true;
                self.dirty.store(true, Ordering::Release);
            }
        }
    }

    pub fn scroll(&self, lines: i32) {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .term
            .scroll_display(Scroll::Delta(lines));
        self.dirty.store(true, Ordering::Release);
    }

    pub fn select(&self, row: usize, col: usize, start: bool, semantic: bool, right_half: bool) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let offset = state.term.grid().display_offset();
        let point = Point::new(
            Line(row.min(self.size.rows as usize - 1) as i32 - offset as i32),
            Column(col.min(self.size.cols as usize - 1)),
        );
        let side = if right_half { Side::Right } else { Side::Left };
        if start {
            state.term.selection = Some(Selection::new(
                if semantic {
                    SelectionType::Semantic
                } else {
                    SelectionType::Simple
                },
                point,
                side,
            ));
        } else if let Some(selection) = state.term.selection.as_mut() {
            selection.update(point, side);
        }
        self.dirty.store(true, Ordering::Release);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        // Closing ConPTY terminates attached clients. Keep its reader draining
        // until EOF to avoid the Windows ClosePseudoConsole deadlock.
        self.master.take();
        self.writer.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::NamedColor;
    #[test]
    fn ansi_truecolor_unicode_and_alternate_screen() {
        let (mut state, _) = TerminalState::new(Size { rows: 5, cols: 30 }, 100, Path::new("C:/"));
        let mut parser: Processor = Processor::new();
        parser.advance(
            &mut state.term,
            "normal \x1b[38;2;1;2;3m中文\x1b[0m".as_bytes(),
        );
        assert!(state.text().contains("中文"));
        assert!(
            state
                .frame()
                .cells
                .iter()
                .any(|cell| cell.text == "中" && cell.fg == 0x010203)
        );
        parser.advance(&mut state.term, b"\x1b[?1049h\x1b[Hfull screen");
        assert!(state.term.mode().contains(TermMode::ALT_SCREEN));
        parser.advance(&mut state.term, b"\x1b[?1049l");
        assert!(state.text().contains("normal"));
    }
    #[test]
    fn fragmented_shell_markers_and_prompt_gating() {
        let (mut state, _) = TerminalState::new(Size { rows: 5, cols: 30 }, 100, Path::new("C:/"));
        let mut integration = Integration::default();
        for chunk in
            b"\x1b]7;file://localhost/C:/work\x1b\\\x1b]133;A;0\x07\x1b]133;B\x07".chunks(1)
        {
            integration.process(chunk, &mut state);
        }
        assert_eq!(state.cwd, PathBuf::from("C:/work"));
        assert!(state.at_prompt);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut state.term, "❯ git st".as_bytes());
        assert_eq!(state.input_query().as_deref(), Some("git st"));
        integration.process(b"\x1b]133;C\x07", &mut state);
        assert!(state.input_query().is_none());
    }
    #[test]
    fn scrollback_and_resize_keep_content() {
        let (mut state, _) = TerminalState::new(Size { rows: 3, cols: 30 }, 100, Path::new("C:/"));
        let mut parser: Processor = Processor::new();
        parser.advance(&mut state.term, b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        state.term.scroll_display(Scroll::Top);
        assert!(state.frame().offset > 0);
        state.term.resize(Size { rows: 6, cols: 40 });
        assert!(state.text().contains("one"));
        assert!(state.text().contains("five"));
    }
    #[test]
    fn indexed_palette_is_xterm_compatible() {
        assert_eq!(indexed_color(16), 0);
        assert_eq!(indexed_color(231), 0xffffff);
        assert_eq!(indexed_color(232), 0x080808);
        assert_eq!(indexed_color(NamedColor::Background as usize), BACKGROUND);
    }
}
