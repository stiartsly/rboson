use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
};

struct RawModeGuard {
    orig_termios: Option<libc::termios>,
}

impl RawModeGuard {
    fn new() -> Self {
        unsafe {
            let mut orig: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut orig) == 0 {
                let mut raw = orig;
                raw.c_iflag &= !(libc::BRKINT | libc::INPCK | libc::ISTRIP | libc::IXON);
                raw.c_oflag &= !libc::OPOST;
                raw.c_cflag |= libc::CS8;
                raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN);
                raw.c_cc[libc::VMIN] = 1;
                raw.c_cc[libc::VTIME] = 0;
                if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) == 0 {
                    return Self {
                        orig_termios: Some(orig),
                    };
                }
            }
            Self { orig_termios: None }
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Some(orig) = self.orig_termios.take() {
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct ShellUi {
    state: Arc<Mutex<UiState>>,
    raw_guard: Arc<Mutex<Option<RawModeGuard>>>,
}

struct UiState {
    layout: Layout,
    logs: VecDeque<String>,
    results: VecDeque<String>,
    input_buffer: String,
    cursor_pos: usize,
    history: Vec<String>,
    history_idx: Option<usize>,
    saved_input: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Layout {
    rows: u16,
    cols: u16,
    top_height: u16,
    left_width: u16,
    right_width: u16,
    input_height: u16,
}

#[derive(Clone, Copy)]
enum Pane {
    Log,
    Result,
}

impl ShellUi {
    pub(crate) fn new() -> Self {
        let layout = Layout::current();
        Self {
            state: Arc::new(Mutex::new(UiState {
                layout,
                logs: VecDeque::new(),
                results: VecDeque::new(),
                input_buffer: String::new(),
                cursor_pos: 0,
                history: Vec::new(),
                history_idx: None,
                saved_input: String::new(),
            })),
            raw_guard: Arc::new(Mutex::new(Some(RawModeGuard::new()))),
        }
    }

    pub(crate) fn draw(&self) {
        let state = self.state.lock().unwrap();
        print_ansi(&format!("{}{}", enter_alt_screen(), state.render_full()));
    }

    #[allow(unused)]
    pub(crate) fn redraw_frame(&self) {
        let state = self.state.lock().unwrap();
        print_ansi(&format!(
            "{}{}{}",
            save_cursor(),
            state.render_frame(false),
            restore_cursor()
        ));
    }

    pub(crate) fn finish(&self) {
        *self.raw_guard.lock().unwrap() = None;
        print_ansi(leave_alt_screen());
    }

    pub(crate) fn log(&self, line: impl Into<String>) {
        let line = colorize_log_line(&line.into());
        self.push(Pane::Log, line);
    }

    pub(crate) fn result(&self, line: impl Into<String>) {
        self.push(Pane::Result, line.into());
    }

    fn push(&self, pane: Pane, text: String) {
        let mut rendered = String::new();
        {
            let mut state = self.state.lock().unwrap();
            state.push_text(pane, &text);
            rendered.push_str(save_cursor());
            rendered.push_str(&state.render_pane(pane));
            rendered.push_str(restore_cursor());
        }

        print_ansi(&rendered);
    }

    pub(crate) async fn read_line(&self) -> io::Result<Option<String>> {
        use tokio::io::AsyncReadExt;

        // Ensure command line is drawn with prompt and cursor placed properly
        {
            let state = self.state.lock().unwrap();
            print_ansi(&state.render_command_line());
        }

        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 64];

        loop {
            // Check terminal resize
            {
                let current_layout = Layout::current();
                let mut state = self.state.lock().unwrap();
                if current_layout != state.layout {
                    state.layout = current_layout;
                    let redraw = state.render_full();
                    drop(state);
                    print_ansi(&redraw);
                }
            }

            tokio::select! {
                result = tokio::signal::ctrl_c() => {
                    if let Err(e) = result {
                        return Err(io::Error::new(io::ErrorKind::Other, e));
                    }
                    let mut state = self.state.lock().unwrap();
                    if state.input_buffer.is_empty() {
                        return Ok(None);
                    } else {
                        state.input_buffer.clear();
                        state.cursor_pos = 0;
                        state.history_idx = None;
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                    }
                }
                read_res = stdin.read(&mut buf) => {
                    let n = match read_res {
                        Ok(0) => return Ok(None),
                        Ok(n) => n,
                        Err(e) => return Err(e),
                    };

                    let bytes = &buf[..n];
                    let mut state = self.state.lock().unwrap();

                    // Check for newline / enter
                    if let Some(pos) = bytes.iter().position(|&b| b == b'\r' || b == b'\n') {
                        let before_newline = &bytes[..pos];
                        if let Ok(text) = std::str::from_utf8(before_newline) {
                            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
                            let mut chars: Vec<char> = state.input_buffer.chars().collect();
                            for ch in printable.chars() {
                                chars.insert(state.cursor_pos, ch);
                                state.cursor_pos += 1;
                            }
                            state.input_buffer = chars.into_iter().collect();
                        }
                        let line = state.input_buffer.clone();
                        state.input_buffer.clear();
                        state.cursor_pos = 0;
                        state.history_idx = None;
                        if !line.trim().is_empty() {
                            if state.history.last().map(|s| s.as_str()) != Some(line.as_str()) {
                                state.history.push(line.clone());
                            }
                        }
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                        return Ok(Some(line));
                    } else if bytes == b"\x03" {
                        // Ctrl-C byte fallback
                        if state.input_buffer.is_empty() {
                            drop(state);
                            return Ok(None);
                        } else {
                            state.input_buffer.clear();
                            state.cursor_pos = 0;
                            state.history_idx = None;
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x04" {
                        // Ctrl-D
                        if state.input_buffer.is_empty() {
                            drop(state);
                            return Ok(None);
                        }
                    } else if bytes == b"\x7f" || bytes == b"\x08" {
                        // Backspace
                        if state.cursor_pos > 0 {
                            let mut chars: Vec<char> = state.input_buffer.chars().collect();
                            chars.remove(state.cursor_pos - 1);
                            state.input_buffer = chars.into_iter().collect();
                            state.cursor_pos -= 1;
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[3~" {
                        // Delete
                        let mut chars: Vec<char> = state.input_buffer.chars().collect();
                        if state.cursor_pos < chars.len() {
                            chars.remove(state.cursor_pos);
                            state.input_buffer = chars.into_iter().collect();
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[A" || bytes == b"\x1bOA" {
                        // Up arrow
                        if !state.history.is_empty() {
                            match state.history_idx {
                                None => {
                                    state.saved_input = state.input_buffer.clone();
                                    let last = state.history.len() - 1;
                                    state.history_idx = Some(last);
                                    state.input_buffer = state.history[last].clone();
                                    state.cursor_pos = state.input_buffer.chars().count();
                                }
                                Some(idx) if idx > 0 => {
                                    state.history_idx = Some(idx - 1);
                                    state.input_buffer = state.history[idx - 1].clone();
                                    state.cursor_pos = state.input_buffer.chars().count();
                                }
                                _ => {}
                            }
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[B" || bytes == b"\x1bOB" {
                        // Down arrow
                        if let Some(idx) = state.history_idx {
                            if idx + 1 < state.history.len() {
                                state.history_idx = Some(idx + 1);
                                state.input_buffer = state.history[idx + 1].clone();
                                state.cursor_pos = state.input_buffer.chars().count();
                            } else {
                                state.history_idx = None;
                                state.input_buffer = state.saved_input.clone();
                                state.cursor_pos = state.input_buffer.chars().count();
                            }
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[D" || bytes == b"\x1bOD" {
                        // Left arrow
                        if state.cursor_pos > 0 {
                            state.cursor_pos -= 1;
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[C" || bytes == b"\x1bOC" {
                        // Right arrow
                        let count = state.input_buffer.chars().count();
                        if state.cursor_pos < count {
                            state.cursor_pos += 1;
                            let redraw = state.render_command_line();
                            drop(state);
                            print_ansi(&redraw);
                        }
                    } else if bytes == b"\x1b[H" || bytes == b"\x1b[1~" || bytes == b"\x01" {
                        // Home / Ctrl-A
                        state.cursor_pos = 0;
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                    } else if bytes == b"\x1b[F" || bytes == b"\x1b[4~" || bytes == b"\x05" {
                        // End / Ctrl-E
                        state.cursor_pos = state.input_buffer.chars().count();
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                    } else if bytes == b"\x0b" {
                        // Ctrl-K (kill to end of line)
                        let mut chars: Vec<char> = state.input_buffer.chars().collect();
                        chars.truncate(state.cursor_pos);
                        state.input_buffer = chars.into_iter().collect();
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                    } else if bytes == b"\x15" {
                        // Ctrl-U (kill entire line)
                        state.input_buffer.clear();
                        state.cursor_pos = 0;
                        let redraw = state.render_command_line();
                        drop(state);
                        print_ansi(&redraw);
                    } else if bytes == b"\x0c" {
                        // Ctrl-L (redraw full screen)
                        state.layout = Layout::current();
                        let redraw = state.render_full();
                        drop(state);
                        print_ansi(&redraw);
                    } else if bytes == b"\t" {
                        // Tab autocomplete
                        let known_cmds = [
                            "announcepeer", "announcevalue", "findnode", "findpeer", "findvalue",
                            "help", "info", "log", "status", "exit", "quit",
                        ];
                        let current = state.input_buffer.trim_start();
                        if !current.contains(' ') {
                            let matches: Vec<&&str> = known_cmds
                                .iter()
                                .filter(|cmd| cmd.starts_with(current))
                                .collect();
                            if matches.len() == 1 {
                                state.input_buffer = format!("{} ", matches[0]);
                                state.cursor_pos = state.input_buffer.chars().count();
                                let redraw = state.render_command_line();
                                drop(state);
                                print_ansi(&redraw);
                            }
                        }
                    } else if !bytes.starts_with(b"\x1b") {
                        // Printable text
                        if let Ok(text) = std::str::from_utf8(bytes) {
                            let printable: String = text.chars().filter(|c| !c.is_control()).collect();
                            if !printable.is_empty() {
                                let mut chars: Vec<char> = state.input_buffer.chars().collect();
                                for ch in printable.chars() {
                                    chars.insert(state.cursor_pos, ch);
                                    state.cursor_pos += 1;
                                }
                                state.input_buffer = chars.into_iter().collect();
                                let redraw = state.render_command_line();
                                drop(state);
                                print_ansi(&redraw);
                            }
                        }
                    }
                }
            }
        }
    }
}

const PANE_PADDING_X: u16 = 1;

impl UiState {
    fn push_text(&mut self, pane: Pane, text: &str) {
        let inner_width = match pane {
            Pane::Log => self.layout.left_inner_width(),
            Pane::Result => self.layout.right_inner_width(),
        };
        for raw_line in split_lines(text) {
            for line in wrap_line(&raw_line, inner_width) {
                match pane {
                    Pane::Log => push_line(&mut self.logs, line),
                    Pane::Result => push_line(&mut self.results, line),
                }
            }
        }
    }

    fn render_full(&self) -> String {
        let mut out = String::new();
        out.push_str(clear_screen());
        out.push_str(&self.render_frame(true));
        out.push_str(&self.render_top());
        out.push_str(&self.render_command_line());
        out
    }

    fn render_frame(&self, clear_command_body: bool) -> String {
        let mut out = String::new();
        out.push_str(&draw_box_outline(
            1,
            1,
            self.layout.cols,
            self.layout.rows,
            " Boson Shell ",
        ));
        out.push_str(&draw_box_with_body(
            self.layout.inner_x(),
            self.layout.input_y(),
            self.layout.inner_width(),
            self.layout.input_height,
            " Command ",
            clear_command_body,
        ));
        out
    }

    fn render_top(&self) -> String {
        let mut out = String::new();
        out.push_str(&draw_box_with_body(
            self.layout.inner_x(),
            self.layout.inner_y(),
            self.layout.left_width,
            self.layout.top_height,
            " Log console ",
            true,
        ));
        out.push_str(&draw_box_with_body(
            self.layout.right_x(),
            self.layout.inner_y(),
            self.layout.right_width,
            self.layout.top_height,
            " Output ",
            true,
        ));
        out.push_str(&self.render_pane(Pane::Log));
        out.push_str(&self.render_pane(Pane::Result));
        out
    }

    fn render_pane(&self, pane: Pane) -> String {
        let (x, y, inner_width, lines) = match pane {
            Pane::Log => (
                self.layout.inner_x(),
                self.layout.inner_y(),
                self.layout.left_inner_width(),
                &self.logs,
            ),
            Pane::Result => (
                self.layout.right_x(),
                self.layout.inner_y(),
                self.layout.right_inner_width(),
                &self.results,
            ),
        };

        let inner_height = self.layout.top_inner_height() as usize;

        let mut visual_lines = Vec::new();
        for line in lines {
            if visible_width(line) <= inner_width {
                visual_lines.push(line.clone());
            } else {
                for wrapped in wrap_line(line, inner_width) {
                    visual_lines.push(wrapped);
                }
            }
        }

        let start = visual_lines.len().saturating_sub(inner_height);
        let visible = &visual_lines[start..];

        let mut out = String::new();
        for row in 0..inner_height {
            let row_y = y + 1 + row as u16;
            let col_x = x + 1 + PANE_PADDING_X;
            out.push_str(&move_to(row_y, col_x));
            out.push_str(&" ".repeat(inner_width));
            if let Some(line) = visible.get(row) {
                out.push_str(&move_to(row_y, col_x));
                out.push_str(&fit_line(line, inner_width));
            }
        }
        out
    }

    fn render_command_line(&self) -> String {
        let mut out = String::new();
        let prompt = "boson> ";
        let row = self.layout.input_y() + 1;
        let start_col = self.layout.inner_x() + 2;
        let available_input_width =
            (self.layout.inner_width().saturating_sub(4) as usize).saturating_sub(prompt.len());

        let input_chars: Vec<char> = self.input_buffer.chars().collect();
        let cursor = self.cursor_pos.min(input_chars.len());

        let (view_start, view_chars) = if input_chars.len() <= available_input_width {
            (0, &input_chars[..])
        } else {
            let start = if cursor >= available_input_width {
                cursor - available_input_width + 1
            } else {
                0
            };
            let end = (start + available_input_width).min(input_chars.len());
            (start, &input_chars[start..end])
        };

        let displayed_input: String = view_chars.iter().collect();
        let padding = available_input_width.saturating_sub(displayed_input.chars().count());

        out.push_str(&move_to(row, start_col));
        out.push_str(prompt);
        out.push_str(&displayed_input);
        out.push_str(&" ".repeat(padding));

        // Line 2: subtle command console hint / status line utilizing the extra room
        let hint_row = self.layout.input_y() + 2;
        let hint_text = "\x1b[90mEnter 'help' for commands, 'exit' to quit\x1b[0m";
        let max_hint_w = self.layout.inner_width().saturating_sub(4) as usize;
        let visible_hint_len = 39;
        out.push_str(&move_to(hint_row, start_col));
        if max_hint_w >= visible_hint_len {
            out.push_str(hint_text);
            out.push_str(&" ".repeat(max_hint_w - visible_hint_len));
        } else {
            out.push_str(&" ".repeat(max_hint_w));
        }

        // Line 3: blank breathing room before the bottom border
        let space_row = self.layout.input_y() + 3;
        out.push_str(&move_to(space_row, start_col));
        out.push_str(&" ".repeat(max_hint_w));

        let cursor_screen_col = start_col + prompt.len() as u16 + (cursor - view_start) as u16;
        out.push_str(&move_to(row, cursor_screen_col));
        out
    }
}

impl Layout {
    fn current() -> Self {
        let (cols, rows) = terminal_size().unwrap_or((120, 40));
        let cols = cols.max(40);
        let rows = rows.max(15);
        let inner_width = cols.saturating_sub(2);
        let inner_height = rows.saturating_sub(2);
        let input_height = 5;
        let top_height = inner_height.saturating_sub(input_height).max(6);
        let left_width = (inner_width / 2).max(19);
        let right_width = inner_width.saturating_sub(left_width).max(19);

        Self {
            rows,
            cols,
            top_height,
            left_width,
            right_width,
            input_height,
        }
    }

    fn top_inner_height(&self) -> u16 {
        self.top_height.saturating_sub(2)
    }

    fn inner_x(&self) -> u16 {
        2
    }

    fn inner_y(&self) -> u16 {
        2
    }

    fn inner_width(&self) -> u16 {
        self.cols.saturating_sub(2)
    }

    fn right_x(&self) -> u16 {
        self.inner_x() + self.left_width
    }

    fn input_y(&self) -> u16 {
        self.inner_y() + self.top_height
    }

    fn left_inner_width(&self) -> usize {
        self.left_width.saturating_sub(2 + 2 * PANE_PADDING_X) as usize
    }

    fn right_inner_width(&self) -> usize {
        self.right_width.saturating_sub(2 + 2 * PANE_PADDING_X) as usize
    }
}

fn terminal_size() -> Option<(u16, u16)> {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let rc = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };
    if rc == 0 && size.ws_col > 0 && size.ws_row > 0 {
        Some((size.ws_col, size.ws_row))
    } else {
        None
    }
}

const MAX_BUFFER_LINES: usize = 1000;

fn push_line(lines: &mut VecDeque<String>, line: String) {
    while lines.len() >= MAX_BUFFER_LINES {
        lines.pop_front();
    }
    lines.push_back(line);
}

pub(crate) fn colorize_log_line(line: &str) -> String {
    let clean = strip_ansi(line);
    let trimmed = clean.trim_start();

    // Red: Error
    if clean.contains("[ERRO]")
        || clean.contains("[ERROR]")
        || trimmed.starts_with("ERROR")
        || trimmed.starts_with("error")
    {
        format!("\x1b[31m{clean}\x1b[0m")
    }
    // Yellow: Warning
    else if clean.contains("[WARN]")
        || clean.contains("[WARNING]")
        || trimmed.starts_with("WARN")
        || trimmed.starts_with("warn")
    {
        format!("\x1b[33m{clean}\x1b[0m")
    }
    // Blue: Info
    else if clean.contains("[INFO]")
        || trimmed.starts_with("INFO")
        || trimmed.starts_with("info")
    {
        format!("\x1b[34m{clean}\x1b[0m")
    }
    // White: Trace
    else if clean.contains("[TRAC]")
        || clean.contains("[TRACE]")
        || trimmed.starts_with("TRACE")
        || trimmed.starts_with("trace")
    {
        format!("\x1b[37m{clean}\x1b[0m")
    }
    // Cyan: Debug
    else if clean.contains("[DEBU]")
        || clean.contains("[DEBUG]")
        || trimmed.starts_with("DEBUG")
        || trimmed.starts_with("debug")
    {
        format!("\x1b[36m{clean}\x1b[0m")
    }
    // If it already has custom ANSI codes, keep as is
    else if line.contains("\x1b[") {
        line.to_string()
    } else {
        line.to_string()
    }
}

fn update_active_style(active: &mut Option<String>, s: &str) {
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            let mut seq = String::from("\x1b[");
            chars.next(); // consume '['
            while let Some(next) = chars.next() {
                seq.push(next);
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            if seq == "\x1b[0m" || seq == "\x1b[m" {
                *active = None;
            } else if seq.ends_with('m') {
                *active = Some(seq);
            }
        }
    }
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    if visible_width(line) <= width {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;
    let mut active_style: Option<String> = None;

    // Split line into alternating tokens of words and spaces, keeping ANSI sequences attached
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut in_space = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            token.push(ch);
            token.push(chars.next().unwrap());
            while let Some(next) = chars.next() {
                token.push(next);
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }

        if ch == ' ' {
            if !in_space && !token.is_empty() {
                tokens.push(token);
                token = String::new();
            }
            in_space = true;
            token.push(ch);
        } else {
            if in_space && !token.is_empty() {
                tokens.push(token);
                token = String::new();
            }
            in_space = false;
            token.push(ch);
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }

    let push_line_break = |result: &mut Vec<String>,
                           current: &mut String,
                           current_len: &mut usize,
                           active_style: &Option<String>| {
        let mut trimmed = current.trim_end().to_string();
        if !trimmed.is_empty() {
            if active_style.is_some() && !trimmed.ends_with("\x1b[0m") {
                trimmed.push_str("\x1b[0m");
            }
            result.push(trimmed);
        }
        *current = match active_style {
            Some(style) => style.clone(),
            None => String::new(),
        };
        *current_len = 0;
    };

    for tok in tokens {
        let tok_len = visible_width(&tok);
        let is_space = strip_ansi(&tok).chars().all(|c| c == ' ');

        if is_space {
            if current_len == 0 {
                if tok_len <= width {
                    current.push_str(&tok);
                    current_len = tok_len;
                } else {
                    let mut sliced = fit_line(&tok, width);
                    if active_style.is_some() && !sliced.ends_with("\x1b[0m") {
                        sliced.push_str("\x1b[0m");
                    }
                    result.push(sliced);
                    current = match &active_style {
                        Some(s) => s.clone(),
                        None => String::new(),
                    };
                    current_len = 0;
                }
            } else if current_len + tok_len <= width {
                current.push_str(&tok);
                current_len += tok_len;
            } else {
                push_line_break(&mut result, &mut current, &mut current_len, &active_style);
            }
        } else {
            update_active_style(&mut active_style, &tok);
            if current_len + tok_len <= width {
                current.push_str(&tok);
                current_len += tok_len;
            } else {
                if current_len > 0 {
                    push_line_break(&mut result, &mut current, &mut current_len, &active_style);
                }
                if tok_len <= width {
                    current.push_str(&tok);
                    current_len = tok_len;
                } else {
                    // Chunk large word by visible width
                    let mut chunk = match &active_style {
                        Some(s) => s.clone(),
                        None => String::new(),
                    };
                    let mut chunk_vis = 0;
                    let mut tchars = tok.chars().peekable();

                    while let Some(c) = tchars.next() {
                        if c == '\x1b' && tchars.peek() == Some(&'[') {
                            let mut seq = String::from("\x1b[");
                            tchars.next();
                            while let Some(n) = tchars.next() {
                                seq.push(n);
                                if n.is_ascii_alphabetic() {
                                    break;
                                }
                            }
                            update_active_style(&mut active_style, &seq);
                            chunk.push_str(&seq);
                        } else {
                            if chunk_vis == width {
                                if active_style.is_some() && !chunk.ends_with("\x1b[0m") {
                                    chunk.push_str("\x1b[0m");
                                }
                                result.push(chunk);
                                chunk = match &active_style {
                                    Some(s) => s.clone(),
                                    None => String::new(),
                                };
                                chunk_vis = 0;
                            }
                            chunk.push(c);
                            chunk_vis += 1;
                        }
                    }
                    if chunk_vis > 0 || !chunk.is_empty() {
                        current = chunk;
                        current_len = chunk_vis;
                    }
                }
            }
        }
    }

    let mut trimmed = current.trim_end().to_string();
    if !trimmed.is_empty() || result.is_empty() {
        if active_style.is_some() && !trimmed.ends_with("\x1b[0m") {
            trimmed.push_str("\x1b[0m");
        }
        result.push(trimmed);
    }

    result
}

fn split_lines(text: &str) -> Vec<String> {
    let sanitized = sanitize_ansi_text(text);
    if sanitized.is_empty() {
        return vec![String::new()];
    }
    sanitized.lines().map(str::to_owned).collect()
}

pub(crate) fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            result.push(ch);
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        while let Some(next) = chars.next() {
            if next.is_ascii_alphabetic() {
                break;
            }
        }
    }
    result
}

pub(crate) fn visible_width(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

fn sanitize_ansi_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            out.push(ch);
            out.push(chars.next().unwrap());
            while let Some(next) = chars.next() {
                out.push(next);
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else if ch == '\n' {
            out.push('\n');
        } else if ch == '\t' {
            out.push(' ');
        } else if !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn fit_line(line: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(line.len() + 8);
    let mut current_visible = 0;
    let mut has_ansi = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            has_ansi = true;
            out.push(ch);
            out.push(chars.next().unwrap());
            while let Some(next) = chars.next() {
                out.push(next);
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            if current_visible >= width {
                break;
            }
            out.push(ch);
            current_visible += 1;
        }
    }

    if has_ansi && !out.ends_with("\x1b[0m") {
        out.push_str("\x1b[0m");
    }
    out
}

fn draw_box_outline(x: u16, y: u16, width: u16, height: u16, title: &str) -> String {
    draw_box_with_body(x, y, width, height, title, false)
}

fn draw_box_with_body(
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
    clear_body: bool,
) -> String {
    let mut out = String::new();
    let width = width.max(2);
    let height = height.max(2);
    let inner_span = width.saturating_sub(2) as usize;

    let border_color = "\x1b[90m";
    let title_color = "\x1b[0m\x1b[1m";
    let reset = "\x1b[0m";

    let horizontal_char = '┄';
    let vertical_char = '┆';
    let top_left = '╭';
    let top_right = '╮';
    let bottom_left = '╰';
    let bottom_right = '╯';

    let title_len = title.chars().count();
    out.push_str(&move_to(y, x));
    out.push_str(border_color);
    out.push(top_left);

    if title_len > 0 && inner_span >= title_len + 3 {
        out.push(horizontal_char);
        out.push_str(title_color);
        out.push_str(title);
        out.push_str(reset);
        out.push_str(border_color);
        let rem = inner_span.saturating_sub(title_len + 1);
        for _ in 0..rem {
            out.push(horizontal_char);
        }
    } else {
        for _ in 0..inner_span {
            out.push(horizontal_char);
        }
    }
    out.push(top_right);
    out.push_str(reset);

    let body_clear_spaces = if clear_body {
        " ".repeat(inner_span)
    } else {
        String::new()
    };

    for row in 1..height.saturating_sub(1) {
        let row_y = y + row;
        out.push_str(&move_to(row_y, x));
        out.push_str(border_color);
        out.push(vertical_char);
        out.push_str(reset);

        if clear_body {
            out.push_str(&body_clear_spaces);
        } else {
            out.push_str(&move_to(row_y, x + width - 1));
        }

        out.push_str(border_color);
        out.push(vertical_char);
        out.push_str(reset);
    }

    out.push_str(&move_to(y + height - 1, x));
    out.push_str(border_color);
    out.push(bottom_left);
    for _ in 0..inner_span {
        out.push(horizontal_char);
    }
    out.push(bottom_right);
    out.push_str(reset);

    out
}

fn move_to(row: u16, col: u16) -> String {
    format!("\x1b[{row};{col}H")
}

fn clear_screen() -> &'static str {
    "\x1b[2J"
}

fn enter_alt_screen() -> &'static str {
    "\x1b[?1049h\x1b[?25h"
}

fn leave_alt_screen() -> &'static str {
    "\x1b[?1049l"
}

fn save_cursor() -> &'static str {
    "\x1b7"
}

fn restore_cursor() -> &'static str {
    "\x1b8"
}

fn print_ansi(text: &str) {
    let mut stdout = io::stdout();
    _ = stdout.write_all(text.as_bytes());
    _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_computes_within_bounds() {
        let layout = Layout {
            rows: 30,
            cols: 100,
            top_height: 23,
            left_width: 49,
            right_width: 49,
            input_height: 5,
        };
        assert!(layout.input_y() + layout.input_height <= layout.rows);
        assert!(layout.right_x() + layout.right_width <= layout.cols);
    }

    #[test]
    fn command_line_contains_boson_prompt() {
        let layout = Layout {
            rows: 30,
            cols: 100,
            top_height: 23,
            left_width: 49,
            right_width: 49,
            input_height: 5,
        };
        let state = UiState {
            layout,
            logs: VecDeque::new(),
            results: VecDeque::new(),
            input_buffer: "status".to_string(),
            cursor_pos: 6,
            history: Vec::new(),
            history_idx: None,
            saved_input: String::new(),
        };
        let cmd_line = state.render_command_line();
        assert!(cmd_line.contains("boson> status"));
        // Prompt is rendered at input_y + 1 (row 26) and start_col 4
        let expected_prompt_pos = move_to(26, 4);
        assert!(cmd_line.contains(&expected_prompt_pos));
    }

    #[test]
    fn render_full_contains_all_panes_and_prompt() {
        let layout = Layout {
            rows: 30,
            cols: 100,
            top_height: 23,
            left_width: 49,
            right_width: 49,
            input_height: 5,
        };
        let state = UiState {
            layout,
            logs: VecDeque::new(),
            results: VecDeque::new(),
            input_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_idx: None,
            saved_input: String::new(),
        };
        let full = state.render_full();
        assert!(full.contains(" Boson Shell "));
        assert!(full.contains(" Log console "));
        assert!(full.contains(" Output "));
        assert!(full.contains(" Command "));
        assert!(full.contains("boson> "));
    }

    #[test]
    fn test_wrap_line_short_and_long() {
        // Short line fits
        assert_eq!(wrap_line("hello world", 20), vec!["hello world"]);

        // Long line with spaces wraps at word boundary
        let wrapped = wrap_line("this is a test of word wrapping across lines", 15);
        for line in &wrapped {
            assert!(line.chars().count() <= 15);
        }
        assert_eq!(
            wrapped,
            vec!["this is a test", "of word", "wrapping across", "lines"]
        );

        // Long line without spaces (e.g. hex key) chunks into width
        let hex_key = "0xbc12dc1054f83fcf0eba7720b706b369b58069c7e3b453be8d1f5493f69d72d1";
        let wrapped_key = wrap_line(hex_key, 20);
        for line in &wrapped_key {
            assert!(line.chars().count() <= 20);
        }
        assert_eq!(
            wrapped_key.concat(),
            hex_key
        );

        // Line with label and long key
        let label_and_key = format!("Private Key: {hex_key}");
        let wrapped_labeled = wrap_line(&label_and_key, 25);
        for line in &wrapped_labeled {
            assert!(line.chars().count() <= 25);
        }
        // First line contains label
        assert!(wrapped_labeled[0].starts_with("Private Key:"));
        // Key is completely preserved across chunks
        let reconstructed: String = wrapped_labeled.join(" ");
        assert!(reconstructed.contains("0xbc12dc10"));
        assert!(reconstructed.contains("69d72d1"));
    }

    #[test]
    fn test_render_pane_displays_wrapped_multiline_output() {
        let layout = Layout {
            rows: 20,
            cols: 80,
            top_height: 13,
            left_width: 39,
            right_width: 39,
            input_height: 5,
        };
        // inner_width = 39 - 2 - 2 = 35 (with 1 space edge padding on each side)
        let hex_key = "0xbc12dc1054f83fcf0eba7720b706b369b58069c7e3b453be8d1f5493f69d72d1";
        let mut results = VecDeque::new();
        // A single long result line that requires more than 1 line to display
        results.push_back(format!("Private Key: {hex_key}"));

        let state = UiState {
            layout,
            logs: VecDeque::new(),
            results,
            input_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_idx: None,
            saved_input: String::new(),
        };

        let rendered = state.render_pane(Pane::Result);
        // Both the start and the end of the long key should be visible in the output pane
        assert!(rendered.contains("Private Key:"));
        assert!(rendered.contains("0xbc12dc10"));
        assert!(rendered.contains("69d72d1"));
    }

    #[test]
    fn test_push_multiline_and_long_line() {
        let layout = Layout {
            rows: 20,
            cols: 80,
            top_height: 13,
            left_width: 39,
            right_width: 39,
            input_height: 5,
        };
        let mut state = UiState {
            layout,
            logs: VecDeque::new(),
            results: VecDeque::new(),
            input_buffer: String::new(),
            cursor_pos: 0,
            history: Vec::new(),
            history_idx: None,
            saved_input: String::new(),
        };

        // Push a multi-line message containing both newlines and long content
        let hex_key = "0xbc12dc1054f83fcf0eba7720b706b369b58069c7e3b453be8d1f5493f69d72d1";
        let message = format!("Device ID: 4WF77gvegeWyeGProxCxX2V1o996vneixdnewuE2XUpg\nPrivate Key: {hex_key}");
        state.push_text(Pane::Result, &message);

        // Since the message has 2 lines and the private key line exceeds pane width,
        // it must have been wrapped into more than 2 lines.
        assert!(state.results.len() > 2);
        let rendered = state.render_pane(Pane::Result);
        assert!(rendered.contains("Device ID:"));
        assert!(rendered.contains("Private Key:"));
        assert!(rendered.contains("69d72d1"));
    }

    #[test]
    fn test_colorize_log_levels() {
        let err = colorize_log_line("[node    ] [ERRO] Connection failed");
        assert!(err.starts_with("\x1b[31m"));
        assert!(err.ends_with("\x1b[0m"));

        let info = colorize_log_line("[node    ] [INFO] Node started");
        assert!(info.starts_with("\x1b[34m"));
        assert!(info.ends_with("\x1b[0m"));

        let warn = colorize_log_line("[node    ] [WARN] Suspicious peer detected");
        assert!(warn.starts_with("\x1b[33m"));
        assert!(warn.ends_with("\x1b[0m"));

        let trace = colorize_log_line("[node    ] [TRAC] Sent ping to peer");
        assert!(trace.starts_with("\x1b[37m"));
        assert!(trace.ends_with("\x1b[0m"));

        let debug = colorize_log_line("[node    ] [DEBU] Periodic maintenance");
        assert!(debug.starts_with("\x1b[36m"));
        assert!(debug.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_ansi_wrap_and_fit() {
        let colored = "\x1b[34m[node    ] [INFO] hello world test\x1b[0m";
        let width = 20;
        let wrapped = wrap_line(colored, width);
        for line in &wrapped {
            assert!(visible_width(line) <= width);
        }
        // First line has blue color and reset
        assert!(wrapped[0].contains("\x1b[34m"));
        assert!(wrapped[0].contains("\x1b[0m"));
        // Second line continues with blue color and reset
        assert!(wrapped[1].contains("\x1b[34m"));
        assert!(wrapped[1].contains("\x1b[0m"));

        // fit_line preserves visible width and resets ANSI
        let fitted = fit_line(colored, 10);
        assert_eq!(visible_width(&fitted), 10);
        assert!(fitted.ends_with("\x1b[0m"));
    }
}
