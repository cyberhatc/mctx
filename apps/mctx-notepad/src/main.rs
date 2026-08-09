//! `mctx` — a lightweight notepad for `.mctx` memory context files.
//!
//! Terminal UI with two panels: a section list (from the `%%INDEX`) on the
//! left and a body editor on the right. Edit a section and save — the file
//! gets a version bump and a rebuilt index automatically via `src/mctx.rs`.
//!
//! Keys:
//!   Tab               switch panel (sections <-> editor)
//!   a                 add a new section (name, then tier)
//!   c                 open the `checkpoint` section (creates if missing)
//!   Enter             edit selected section / newline in editor
//!   Ctrl+S            save current section body
//!   Esc               back to section list
//!   q                 quit (safe: ignored while unsaved); Q forces quit

use mctx::Store;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line as TextLine, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};

use std::{
    fs,
    io::{self},
    path::{Path, PathBuf},
};

// ---- date --------------------------------------------------------------------

/// Compose `#mctx v1.1 | updated:<ISO date>` for today.
fn make_header() -> String {
    let (y, m, d) = today_ymd();
    format!("#mctx v1.1 | updated:{y:04}-{m:02}-{d:02}")
}

/// Convert unix-seconds-since-epoch into a civil (year, month, day) using
/// Howard Hinnant's `civil_from_days` algorithm — no chrono dependency.
fn today_ymd() -> (i64, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86400))
}

fn civil_from_days(z0: i64) -> (i64, u32, u32) {
    let z = z0 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

// ---- body text buffer ---------------------------------------------------------

/// A lightweight, testable text buffer for one section body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Buffer {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

impl Buffer {
    fn empty() -> Self {
        Buffer {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    /// Build a buffer from a stored section body.
    fn from_body(body: &str) -> Self {
        let mut lines: Vec<String> = body.split('\n').map(String::from).collect();
        // split() leaves a trailing "" for the trailing newline; drop it.
        if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        Buffer {
            lines,
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    fn line_len(&self) -> usize {
        self.lines[self.cursor_row].len()
    }

    fn insert_char(&mut self, c: char) {
        if c == '\n' {
            let right = self.lines[self.cursor_row].split_off(self.cursor_col);
            self.lines.insert(self.cursor_row + 1, right);
            self.cursor_row += 1;
            self.cursor_col = 0;
        } else {
            self.lines[self.cursor_row].insert(self.cursor_col, c);
            self.cursor_col += 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.lines[self.cursor_row].remove(self.cursor_col - 1);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let prev_len = self.lines[self.cursor_row - 1].len();
            let rest = self.lines.remove(self.cursor_row);
            self.lines[self.cursor_row - 1].push_str(&rest);
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
        }
    }

    fn delete(&mut self) {
        if self.cursor_col < self.line_len() {
            self.lines[self.cursor_row].remove(self.cursor_col);
        } else if self.cursor_row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    fn move_cursor(&mut self, dr: i64, dc: i64) {
        let rows = self.lines.len() as i64;
        self.cursor_row = (self.cursor_row as i64 + dr).clamp(0, rows - 1) as usize;
        let len = self.line_len() as i64;
        self.cursor_col = (self.cursor_col as i64 + dc).clamp(0, len) as usize;
    }

    fn home(&mut self) {
        self.cursor_col = 0;
    }

    fn end(&mut self) {
        self.cursor_col = self.line_len();
    }

    /// Serialize the buffer back into a stored body (trailing newline added).
    fn body_string(&self) -> String {
        let mut body = self.lines.join("\n");
        if !body.is_empty() {
            body.push('\n');
        }
        body
    }
}

// ---- app state ----------------------------------------------------------------

#[derive(Clone)]
struct SectionInfo {
    name: String,
    tier: String,
    version: u32,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum Focus {
    List,
    Edit,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum PromptKind {
    NewName,
    NewTier,
}

struct Prompt {
    kind: PromptKind,
    label: String,
    input: String,
    /// Name captured when the user already answered NewName.
    pending_name: Option<String>,
}

struct App {
    path: PathBuf,
    store: Store,
    sections: Vec<SectionInfo>,
    selected: usize,
    list_scroll: u16,
    buffer: Buffer,
    edit_scroll: (u16, u16),
    focus: Focus,
    prompt: Option<Prompt>,
    status: String,
    dirty: bool,
    quit: bool,
}

impl App {
    fn open(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            fs::write(path, format!("{}\n", make_header()))?;
        }
        let store = Store::open(&path.to_string_lossy())?;
        let mut app = App {
            path: path.to_path_buf(),
            store,
            sections: Vec::new(),
            selected: 0,
            list_scroll: 0,
            buffer: Buffer::empty(),
            edit_scroll: (0, 0),
            focus: Focus::List,
            prompt: None,
            status: String::new(),
            dirty: false,
            quit: false,
        };
        app.refresh_sections();
        if app.selected < app.sections.len() {
            app.load_section(app.selected);
        }
        app.set_status(
            "Tab to switch, a=add, c=checkpoint, Ctrl+S=save, q=quit".to_string(),
        );
        Ok(app)
    }

    fn refresh_sections(&mut self) {
        self.sections = self
            .store
            .index()
            .iter()
            .map(|s| SectionInfo {
                name: s.name.clone(),
                tier: s.tier.clone(),
                version: s.version,
            })
            .collect();
        if self.selected >= self.sections.len() {
            self.selected = self.sections.len().saturating_sub(1);
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status = msg;
    }

    fn selected_info(&self) -> Option<&SectionInfo> {
        self.sections.get(self.selected)
    }

    fn load_section(&mut self, idx: usize) {
        if let Some(info) = self.sections.get(idx) {
            match self.store.read(&info.name) {
                Ok(body) => {
                    self.buffer = Buffer::from_body(&body);
                }
                Err(_) => {
                    self.buffer = Buffer::empty();
                    self.set_status(format!("could not read '{}'", info.name));
                }
            }
            self.edit_scroll = (0, 0);
            self.dirty = false;
        }
    }

    fn save(&mut self) {
        let Some(info) = self.selected_info() else {
            self.set_status("no section selected".into());
            return;
        };
        let (name, tier) = (info.name.clone(), info.tier.clone());
        let body = self.buffer.body_string();
        match self.store.write(&name, &tier, &body) {
            Ok(()) => {
                self.refresh_sections();
                let new_v = self
                    .sections
                    .iter()
                    .find(|s| s.name == name)
                    .map(|s| s.version)
                    .unwrap_or(0);
                self.dirty = false;
                self.set_status(format!("saved '{name}' v{new_v}"));
            }
            Err(e) => self.set_status(format!("save failed: {e}")),
        }
    }

    fn open_checkpoint(&mut self) {
        let exists = self.sections.iter().any(|s| s.name == "checkpoint");
        if !exists {
            if let Err(e) = self.store.write("checkpoint", "!volatile", "") {
                self.set_status(format!("checkpoint failed: {e}"));
                return;
            }
            self.refresh_sections();
        }
        if let Some(idx) = self.sections.iter().position(|s| s.name == "checkpoint") {
            self.selected = idx;
            self.load_section(idx);
            self.focus = Focus::Edit;
            self.set_status("editing checkpoint (!volatile)".into());
        }
    }

    // ---- key handling --------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        if let Some(prompt) = self.prompt.take() {
            self.on_prompt(prompt, key);
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') if self.focus == Focus::List && !self.dirty => self.quit = true,
            KeyCode::Char('Q') if self.focus == Focus::List => self.quit = true,
            KeyCode::Char('a') if self.focus == Focus::List => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::NewName,
                    label: "new section name:".into(),
                    input: String::new(),
                    pending_name: None,
                });
            }
            KeyCode::Char('c') if self.focus == Focus::List => self.open_checkpoint(),
            KeyCode::Tab => {
                self.focus = if self.focus == Focus::List {
                    Focus::Edit
                } else {
                    Focus::List
                };
            }
            KeyCode::Esc if self.focus == Focus::Edit => self.focus = Focus::List,
            KeyCode::Enter if self.focus == Focus::List => {
                self.load_section(self.selected);
                self.focus = Focus::Edit;
            }
            KeyCode::Enter => {
                self.buffer.insert_char('\n');
                self.dirty = true;
            }
            KeyCode::Up if self.focus == Focus::List => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down if self.focus == Focus::List => {
                if self.selected + 1 < self.sections.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Up => {
                self.buffer.move_cursor(-1, 0);
                self.dirty = true;
            }
            KeyCode::Down => {
                self.buffer.move_cursor(1, 0);
                self.dirty = true;
            }
            KeyCode::Left => {
                self.buffer.move_cursor(0, -1);
                self.dirty = true;
            }
            KeyCode::Right => {
                self.buffer.move_cursor(0, 1);
                self.dirty = true;
            }
            KeyCode::Home => {
                self.buffer.home();
                self.dirty = true;
            }
            KeyCode::End => {
                self.buffer.end();
                self.dirty = true;
            }
            KeyCode::Backspace => {
                self.buffer.backspace();
                self.dirty = true;
            }
            KeyCode::Delete => {
                self.buffer.delete();
                self.dirty = true;
            }
            KeyCode::Char('s') if ctrl && self.focus == Focus::Edit => self.save(),
            KeyCode::Char(c) if self.focus == Focus::Edit && !c.is_control() => {
                self.buffer.insert_char(c);
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn on_prompt(&mut self, mut prompt: Prompt, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.set_status("cancelled".into());
            }
            KeyCode::Enter => {
                let input = prompt.input.trim().to_string();
                match prompt.kind {
                    PromptKind::NewName => {
                        if input.is_empty() {
                            self.prompt = Some(prompt);
                            self.set_status("name cannot be empty".into());
                            return;
                        }
                        self.prompt = Some(Prompt {
                            kind: PromptKind::NewTier,
                            label: "tier (fixed|durable|volatile):".into(),
                            input: "durable".into(),
                            pending_name: Some(input),
                        });
                    }
                    PromptKind::NewTier => {
                        let name = prompt.pending_name.clone().unwrap_or_default();
                        let tier = match normalize_tier(&input) {
                            Some(t) => t,
                            None => {
                                self.prompt = Some(prompt);
                                self.set_status(format!("invalid tier '{input}'"));
                                return;
                            }
                        };
                        if let Some(idx) = self.sections.iter().position(|s| s.name == name) {
                            self.selected = idx;
                            self.load_section(idx);
                            self.focus = Focus::Edit;
                            self.set_status(format!("opened existing '{name}'"));
                        } else {
                            match self.store.write(&name, tier, "") {
                                Ok(()) => {
                                    self.refresh_sections();
                                    if let Some(idx) =
                                        self.sections.iter().position(|s| s.name == name)
                                    {
                                        self.selected = idx;
                                        self.load_section(idx);
                                        self.focus = Focus::Edit;
                                        self.set_status(format!("created '{name}' {tier} v1"));
                                    }
                                }
                                Err(e) => self.set_status(format!("create failed: {e}")),
                            }
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                prompt.input.pop();
                self.prompt = Some(prompt);
            }
            KeyCode::Char(c) if !c.is_control() => {
                // typing replaces the prefilled "durable" default instead of
                // appending to it
                if prompt.kind == PromptKind::NewTier && prompt.input == "durable" {
                    prompt.input.clear();
                }
                prompt.input.push(c);
                self.prompt = Some(prompt);
            }
            _ => {
                self.prompt = Some(prompt);
            }
        }
    }
}

/// Accept `fixed`, `durable`, `volatile` and the `!`-prefixed forms.
fn normalize_tier(input: &str) -> Option<&'static str> {
    match input.trim() {
        "!fixed" | "fixed" => Some("!fixed"),
        "!durable" | "durable" => Some("!durable"),
        "!volatile" | "volatile" => Some("!volatile"),
        _ => None,
    }
}

// ---- terminal plumbing ---------------------------------------------------------

fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        while !app.quit {
            terminal.draw(|f| render(f, &mut app))?;
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    app.on_key(key);
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

// ---- rendering -------------------------------------------------------------------

fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(area);

    let (list_area, edit_area) = {
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);
        (h[0], h[1])
    };

    draw_section_list(f, list_area, app);
    draw_editor(f, edit_area, app);

    // status bar
    let status_style = Style::default()
        .fg(Color::Black)
        .bg(if app.dirty { Color::Yellow } else { Color::DarkGray });
    let status = if app.dirty {
        format!("[*] {}", app.status)
    } else {
        app.status.clone()
    };
    f.render_widget(
        Paragraph::new(status).style(status_style),
        chunks[1],
    );

    // help bar
    f.render_widget(
        Paragraph::new(TextLine::from(Span::styled(
            "Tab switch | a add | c checkpoint | Enter edit | Ctrl+S save | Esc back | q quit",
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[2],
    );

    if let Some(prompt) = &app.prompt {
        draw_prompt(f, area, prompt);
    }
}

fn draw_section_list(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let selected = app.focus == Focus::List && i == app.selected;
            let tier_color = match s.tier.as_str() {
                "!fixed" => Color::Cyan,
                "!durable" => Color::Green,
                _ => Color::Yellow,
            };
            let line = TextLine::from(vec![
                Span::raw(s.name.clone()),
                Span::raw(" "),
                Span::styled(s.tier.clone(), Style::default().fg(Color::DarkGray)),
                Span::raw(format!(" v{}", s.version)),
            ])
            .style(if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(tier_color)
            });
            ListItem::new(line)
        })
        .collect();

    let visible = area.height.saturating_sub(2) as usize;
    if app.selected >= app.list_scroll as usize + visible && visible > 0 {
        app.list_scroll = (app.selected + 1 - visible) as u16;
    } else if app.selected < app.list_scroll as usize {
        app.list_scroll = app.selected as u16;
    }

    let list_title = format!(" SECTIONS — {} ", app.path.display());
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(list_title));
    f.render_widget(list, area);
}

fn draw_editor(f: &mut Frame, area: Rect, app: &mut App) {
    let title = match app.selected_info() {
        Some(s) => format!(
            " {} {} v{} {} ",
            s.name,
            s.tier,
            s.version,
            if app.dirty { "*" } else { "" }
        ),
        None => " (no sections) ".into(),
    };

    let body: Vec<TextLine> = app
        .buffer
        .lines
        .iter()
        .map(|l| TextLine::raw(l.as_str()))
        .collect();
    let paragraph = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll(app.edit_scroll);

    f.render_widget(paragraph, area);

    if app.focus == Focus::Edit && app.selected_info().is_some() && area.width >= 2 && area.height >= 2 {
        // keep the cursor visible by scrolling rows/cols so it stays inside.
        let h = area.height - 2;
        let w = area.width - 2;
        let row = app.buffer.cursor_row as u16;
        let col = app.buffer.cursor_col as u16;
        if h > 0 && row >= app.edit_scroll.0 + h {
            app.edit_scroll.0 = row + 1 - h;
        } else if row < app.edit_scroll.0 {
            app.edit_scroll.0 = row;
        }
        if w > 0 && col >= app.edit_scroll.1 + w {
            app.edit_scroll.1 = col + 1 - w;
        } else if col < app.edit_scroll.1 {
            app.edit_scroll.1 = col;
        }
        let x = area.x + 1 + (col - app.edit_scroll.1);
        let y = area.y + 1 + (row - app.edit_scroll.0);
        f.set_cursor_position((x, y));
    }
}

fn draw_prompt(f: &mut Frame, area: Rect, prompt: &Prompt) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = (area.width / 2).clamp(30, 60).min(area.width);
    let height = 3u16.min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = Rect { x, y, width, height };

    let text = format!("{} {}", prompt.label, prompt.input);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" INPUT ")
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(Paragraph::new(text).block(block), rect);
    let cx = (x + 1 + (prompt.label.len() + prompt.input.len()) as u16).min(x + width - 1);
    let cy = y + 1;
    f.set_cursor_position((cx, cy));
}

// ---- entry point ----------------------------------------------------------------

const USAGE: &str = "\
mctx 1.1.0 — terminal notepad for .mctx memory context files

USAGE:
    mctx [FILE.mctx]                 interactive notepad (default: ./memory.mctx)
    mctx show FILE                   print the raw .mctx file
    mctx md FILE                     human-readable Markdown view
    mctx json FILE                   AI view: structured JSON (sections/tiers/v/offsets)
    mctx list FILE                   index rows: name <tab> tier <tab> v<N> <tab> offset
    mctx get FILE SECTION            print one section's body
    mctx set FILE SECTION TIER BODY  write a section (bumps its v:); BODY may be '-'
                                     for stdin. Creates the file if missing.
    mctx checkpoint FILE BODY        write the !volatile checkpoint; '-' = stdin
    mctx index FILE                  rebuild the %%INDEX after hand edits
    mctx new FILE                    create a fresh file with a header

If FILE does not exist it is created with a fresh header; the default is
./memory.mctx.

KEYS:
    Tab        switch between section list and editor
    a          add a new section (name, then tier)
    c          open the checkpoint section (creates if missing)
    Enter      edit selected section / newline in editor
    Ctrl+S     save current section body (bumps its v:)
    Esc        back to the section list
    q          quit (ignored while unsaved); Q quits regardless
";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 {
        match args[1].as_str() {
            "show" | "md" | "json" | "list" | "get" | "set" | "checkpoint" | "index" | "new" => {
                return run_cli(&args);
            }
            _ => {}
        }
    }
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        print!("{USAGE}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V" || a == "version") {
        println!("mctx 1.1.0");
        return Ok(());
    }

    let path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("memory.mctx"));

    let app = App::open(&path)?;
    run(app)
}

/// Non-interactive subcommands so agents and scripts can read and update a
/// `.mctx` memory file. Every write goes through the library, so the index and
/// version counters stay consistent.
fn run_cli(args: &[String]) -> io::Result<()> {
    let cmd = args[1].as_str();
    let file = args.get(2).map(String::as_str).unwrap_or("memory.mctx");

    match cmd {
        "show" => {
            print!("{}", fs::read_to_string(file)?);
        }
        "md" => {
            let content = fs::read_to_string(file)?;
            print!("{}", mctx::render_markdown(&content));
        }
        "json" => {
            let content = fs::read_to_string(file)?;
            print!("{}", mctx::render_json(&content));
        }
        "list" => {
            let store = Store::open(file)?;
            for section in store.index() {
                println!(
                    "{}\t{}\tv{}\t{}",
                    section.name, section.tier, section.version, section.offset
                );
            }
        }
        "get" => {
            let name = args
                .get(3)
                .ok_or_else(|| cli_err("usage: mctx get FILE SECTION"))?;
            let store = Store::open(file)?;
            print!("{}", store.read(name)?);
        }
        "set" => {
            let name = args
                .get(3)
                .ok_or_else(|| cli_err("usage: mctx set FILE SECTION TIER [BODY|-]"))?;
            let tier = args
                .get(4)
                .ok_or_else(|| cli_err("usage: mctx set FILE SECTION TIER [BODY|-]"))?;
            let body = body_from(args, 5)?;
            ensure_exists(file)?;
            let mut store = Store::open(file)?;
            store.write(name, tier, &body)?;
            println!("wrote '{name}' {tier} -> {file}");
        }
        "checkpoint" => {
            let body = body_from(args, 3)?;
            ensure_exists(file)?;
            let mut store = Store::open(file)?;
            store.checkpoint(&body)?;
            println!("checkpoint saved -> {file}");
        }
        "index" => {
            let store = Store::open(file)?;
            store.rebuild_index()?;
            println!("index rebuilt -> {file}");
        }
        "new" => {
            fs::write(file, format!("{}\n", mctx::make_header()))?;
            println!("created -> {file}");
        }
        _ => unreachable!("cli subcommand matched upstream"),
    }
    Ok(())
}

/// Collect a section body from trailing arguments, or from stdin when `-` or
/// when nothing is given and stdin is not a terminal (piped).
fn body_from(args: &[String], start: usize) -> io::Result<String> {
    use std::io::IsTerminal;
    match args.get(start) {
        Some(arg) if arg == "-" => {
            let mut buf = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
            Ok(buf)
        }
        Some(_) => Ok(args[start..].join(" ")),
        None if io::stdin().is_terminal() => Ok(String::new()),
        None => {
            let mut buf = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
            Ok(buf)
        }
    }
}

/// Create the file with a fresh header if it does not exist yet.
fn ensure_exists(path: &str) -> io::Result<()> {
    if !Path::new(path).exists() {
        fs::write(path, format!("{}\n", mctx::make_header()))?;
    }
    Ok(())
}

fn cli_err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

// ---- tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_is_iso() {
        let (y, m, d) = today_ymd();
        assert!((2024..=2100).contains(&y));
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
        let header = make_header();
        assert!(header.starts_with("#mctx v1.1 | updated:"), "{header}");
    }

    #[test]
    fn cli_set_get_list_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mctx_cli_test_{}.mctx", std::process::id()));
        let _ = fs::remove_file(&path);
        let p = path.to_string_lossy().to_string();

        let set = vec!["mctx".to_string(), "set".into(), p.clone(), "identity".into(), "!fixed".into(), "user{alias}: \"devil2\"".into()];
        run_cli(&set).unwrap();
        assert!(Path::new(&p).exists(), "set creates the file");

        let get = vec!["mctx".into(), "get".into(), p.clone(), "identity".into()];
        run_cli(&get).unwrap();
        let body = Store::open(&p).unwrap().read("identity").unwrap();
        assert_eq!(body, "user{alias}: \"devil2\"\n");
        assert!(body.contains("devil2"));

        let list = vec!["mctx".into(), "list".into(), p.clone()];
        run_cli(&list).unwrap();

        let json = vec!["mctx".into(), "json".into(), p.clone()];
        run_cli(&json).unwrap();

        let _ = fs::remove_file(&p);
    }

    #[test]
    fn insert_and_backspace() {
        let mut b = Buffer::empty();
        for c in ['h', 'i'] {
            b.insert_char(c);
        }
        assert_eq!(b.lines, vec!["hi"]);
        b.backspace();
        assert_eq!(b.lines, vec!["h"]);
    }

    #[test]
    fn newline_splits_line() {
        let mut b = Buffer::from_body("abc");
        b.cursor_col = 1;
        b.insert_char('\n');
        assert_eq!(b.lines, vec!["a", "bc"]);
        assert_eq!((b.cursor_row, b.cursor_col), (1, 0));
    }

    #[test]
    fn backspace_joins_lines() {
        let mut b = Buffer::from_body("ab\ncd");
        b.cursor_row = 1;
        b.cursor_col = 0;
        b.backspace();
        assert_eq!(b.lines, vec!["abcd"]);
        assert_eq!((b.cursor_row, b.cursor_col), (0, 2));
    }

    #[test]
    fn delete_at_line_end_joins() {
        let mut b = Buffer::from_body("ab\ncd");
        b.cursor_row = 0;
        b.cursor_col = 2;
        b.delete();
        assert_eq!(b.lines, vec!["abcd"]);
    }

    #[test]
    fn body_roundtrip() {
        let b = Buffer::from_body("a\nb\n");
        assert_eq!(b.lines, vec!["a", "b"]);
        assert_eq!(b.body_string(), "a\nb\n");
    }

    #[test]
    fn empty_body() {
        let b = Buffer::from_body("");
        assert_eq!(b.lines, vec![""]);
        assert_eq!(b.body_string(), "");
    }

    #[test]
    fn cursor_movement_clamps() {
        let mut b = Buffer::from_body("ab\ncde");
        b.cursor_row = 0;
        b.cursor_col = 0;
        b.move_cursor(-1, -5); // stay clamped at top-left
        assert_eq!((b.cursor_row, b.cursor_col), (0, 0));
        b.move_cursor(99, 99); // clamp to last line, end
        assert_eq!((b.cursor_row, b.cursor_col), (1, 3));
    }

    #[test]
    fn tier_normalization() {
        assert_eq!(normalize_tier("fixed"), Some("!fixed"));
        assert_eq!(normalize_tier("!volatile"), Some("!volatile"));
        assert_eq!(normalize_tier("FIXED"), None);
        assert_eq!(normalize_tier("eternal"), None);
    }

    // ---- app / prompt behavior ----------------------------------------------

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn type_text(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.on_key(key(KeyCode::Char(ch)));
        }
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mctx_app_test_{tag}_{}.mctx", std::process::id()))
    }

    #[test]
    fn new_section_flow_creates_section() {
        let path = temp_path("flow");
        let mut app = App::open(&path).unwrap();
        app.on_key(key(KeyCode::Char('a')));
        type_text(&mut app, "identity");
        app.on_key(key(KeyCode::Enter)); // -> tier prompt
        type_text(&mut app, "fixed");
        app.on_key(key(KeyCode::Enter)); // create
        assert!(
            app.sections.iter().any(|s| s.name == "identity" && s.tier == "!fixed"),
            "identity section created"
        );
        assert_eq!(app.focus, Focus::Edit);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn typing_replaces_default_tier() {
        let path = temp_path("tierdefault");
        let mut app = App::open(&path).unwrap();
        app.on_key(key(KeyCode::Char('a')));
        type_text(&mut app, "x");
        app.on_key(key(KeyCode::Enter));
        assert_eq!(app.prompt.as_ref().unwrap().kind, PromptKind::NewTier);
        app.on_key(key(KeyCode::Char('f')));
        assert_eq!(
            app.prompt.as_ref().unwrap().input, "f",
            "typing replaces the 'durable' default, not appends"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn invalid_tier_keeps_prompt_open() {
        let path = temp_path("badtier");
        let mut app = App::open(&path).unwrap();
        app.on_key(key(KeyCode::Char('a')));
        type_text(&mut app, "x");
        app.on_key(key(KeyCode::Enter));
        type_text(&mut app, "eternal");
        app.on_key(key(KeyCode::Enter));
        assert!(app.prompt.is_some(), "prompt stays open on invalid tier");
        assert!(app.status.contains("invalid tier"));
        assert!(app.sections.is_empty(), "no section created");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_bumps_version_and_clears_dirty() {
        let path = temp_path("save");
        let mut app = App::open(&path).unwrap();
        app.on_key(key(KeyCode::Char('a')));
        type_text(&mut app, "tasks");
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter)); // default tier -> !durable
        assert_eq!(app.focus, Focus::Edit);
        type_text(&mut app, "task: one");
        app.dirty = true;
        app.on_key(KeyEvent::new(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
        ));
        assert!(!app.dirty, "save clears dirty");
        let v = app
            .sections
            .iter()
            .find(|s| s.name == "tasks")
            .map(|s| s.version)
            .unwrap();
        assert_eq!(v, 2, "created at v1, this save bumped to v2");
        assert_eq!(
            app.store.read("tasks").unwrap(),
            "task: one\n",
            "body persisted via the store"
        );
        std::fs::remove_file(&path).ok();
    }
}
