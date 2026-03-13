use crate::state::{SessionStatus, StateManager, Session};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::DefaultTerminal;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ========== 配色方案 ==========

mod theme {
    use ratatui::style::Color;

    // 透明背景，跟随终端主题
    pub const BG: Color = Color::Reset;
    pub const FG: Color = Color::Reset;
    pub const HEADER_FG: Color = Color::DarkGray;
    pub const BORDER: Color = Color::DarkGray;
    pub const TITLE: Color = Color::Cyan;
    pub const SELECTED_BG: Color = Color::Rgb(60, 60, 70);
    pub const SELECTED_FG: Color = Color::White;

    pub const STATUS_WAITING: Color = Color::Yellow;
    pub const STATUS_ACTIVE: Color = Color::Green;
    pub const STATUS_RUNNING: Color = Color::Cyan;
    pub const STATUS_IDLE: Color = Color::Blue;
    pub const STATUS_STOPPED: Color = Color::Red;
    pub const STATUS_STARTING: Color = Color::DarkGray;

    pub const MODE_ALLOW: Color = Color::Green;
    pub const MODE_MANUAL: Color = Color::Yellow;

    pub const DETAIL_FG: Color = Color::Yellow;
    pub const FOOTER_FG: Color = Color::DarkGray;
    pub const MSG_FG: Color = Color::Cyan;
}

// ========== 工具函数 ==========

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn format_time_ago(t: f64) -> String {
    if t == 0.0 {
        return "—".to_string();
    }
    let delta = now() - t;
    if delta < 60.0 {
        format!("{}s", delta as i64)
    } else if delta < 3600.0 {
        format!("{}m", (delta / 60.0) as i64)
    } else {
        format!("{}h", (delta / 3600.0) as i64)
    }
}

fn get_project_name(cwd: &str) -> &str {
    if cwd.is_empty() {
        return "?";
    }
    cwd.rsplit('/').next().unwrap_or(cwd)
}

/// 按列宽折行，最多保留 max_lines 行，超出部分用 "…" 表示
fn wrap_text(s: &str, col_width: usize, max_lines: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    if col_width == 0 || max_lines == 0 {
        return String::new();
    }

    let clean = s.replace('\r', "");
    let mut wrapped: Vec<String> = Vec::new();

    for line in clean.lines() {
        if line.is_empty() {
            wrapped.push(String::new());
        } else {
            let mut cur = String::new();
            let mut cur_w: usize = 0;

            for ch in line.chars() {
                let w = UnicodeWidthChar::width(ch).unwrap_or(0);
                if cur_w + w > col_width && cur_w > 0 {
                    wrapped.push(cur);
                    cur = String::new();
                    cur_w = 0;
                }
                cur.push(ch);
                cur_w += w;
            }
            if !cur.is_empty() {
                wrapped.push(cur);
            }
        }
        if wrapped.len() > max_lines {
            break;
        }
    }

    if wrapped.len() > max_lines {
        wrapped.truncate(max_lines);
        if let Some(last) = wrapped.last_mut() {
            last.push('…');
        }
    }

    wrapped.join("\n")
}

/// 在文本前加空行，使其在 row_height 行内垂直居中
fn vcenter(text: &str, row_height: u16) -> String {
    let lines = text.lines().count().max(1);
    let pad = (row_height as usize).saturating_sub(lines) / 2;
    if pad == 0 {
        return text.to_string();
    }
    let mut result = "\n".repeat(pad);
    result.push_str(text);
    result
}

fn status_color(status: SessionStatus) -> Color {
    match status {
        SessionStatus::Waiting => theme::STATUS_WAITING,
        SessionStatus::Active => theme::STATUS_ACTIVE,
        SessionStatus::Running => theme::STATUS_RUNNING,
        SessionStatus::Idle => theme::STATUS_IDLE,
        SessionStatus::Stopped => theme::STATUS_STOPPED,
        SessionStatus::Starting => theme::STATUS_STARTING,
    }
}

fn mode_text_and_color(auto_reply: &Option<String>) -> (&str, Color) {
    match auto_reply.as_deref() {
        Some("allow") => ("全通过", theme::MODE_ALLOW),
        None => ("手动", theme::MODE_MANUAL),
        _ => ("全通过", theme::MODE_ALLOW),
    }
}

// ========== TUI 主结构 ==========

pub struct TUI {
    state: StateManager,
    port: u16,
    table_state: TableState,
    message: String,
    message_time: f64,
}

impl TUI {
    pub fn new(state: StateManager, port: u16) -> Self {
        Self {
            state,
            port,
            table_state: TableState::default().with_selected(0),
            message: String::new(),
            message_time: 0.0,
        }
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let mut terminal = ratatui::init();
        let result = self.main_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            let sessions = self.state.get_session_list();

            // 防止选中行越界
            if !sessions.is_empty() {
                let sel = self.table_state.selected().unwrap_or(0);
                if sel >= sessions.len() {
                    self.table_state.select(Some(sessions.len() - 1));
                }
            } else {
                self.table_state.select(Some(0));
            }

            terminal.draw(|frame| self.render(frame, &sessions))?;

            if event::poll(Duration::from_secs(1))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }
                    if self.handle_normal_key(key.code, &sessions) {
                        return Ok(());
                    }
                }
            }
        }
    }

    // ========== 渲染 ==========

    fn render(&mut self, frame: &mut Frame, sessions: &[Session]) {
        let area = frame.area();

        // 全局背景
        frame.render_widget(
            Block::default().style(Style::default().bg(theme::BG)),
            area,
        );

        // 三段式布局：标题栏、主表格、底栏
        let chunks = Layout::vertical([
            Constraint::Length(3), // 标题栏（含边框）
            Constraint::Min(5),   // 主表格
            Constraint::Length(3), // 底栏
        ])
        .split(area);

        self.render_header(frame, chunks[0], sessions.len());
        self.render_table(frame, chunks[1], sessions);
        self.render_footer(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, count: usize) {
        let title_text = format!("  Yes! Sir  │  {} sessions  │  port {}", count, self.port);
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(theme::TITLE).bg(theme::BG).bold())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(theme::BORDER))
                    .style(Style::default().bg(theme::BG)),
            );
        frame.render_widget(title, area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, sessions: &[Session]) {
        let row_height: u16 = 3;

        if sessions.is_empty() {
            let empty = Paragraph::new("  No active sessions. Start Claude Code to see sessions here.")
                .style(Style::default().fg(theme::FOOTER_FG).bg(theme::BG).italic())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::BORDER))
                        .title(" Sessions ")
                        .title_style(Style::default().fg(theme::HEADER_FG).bold())
                        .style(Style::default().bg(theme::BG)),
                );
            frame.render_widget(empty, area);
            return;
        }

        // 计算 Fill 列的实际宽度（用于手动折行）
        let fixed_total: u16 = 4 + 20 + 10 + 8 + 12 + 6; // 固定列宽之和
        let spacers: u16 = 7; // 8 列之间 7 个间距（默认间距 1）
        let inner_width = area.width.saturating_sub(2); // 减去左右边框
        let fill_total = inner_width.saturating_sub(fixed_total + spacers);
        let output_col_w = (fill_total * 2 / 6) as usize; // Fill(2)
        let input_col_w = (fill_total * 4 / 6) as usize;  // Fill(4)

        // 构建表格行
        let mut rows: Vec<Row> = Vec::new();

        for (i, session) in sessions.iter().enumerate() {
            let project = get_project_name(&session.cwd);
            let last_output = if session.last_output.is_empty() {
                "—".to_string()
            } else {
                wrap_text(&session.last_output, output_col_w, row_height as usize)
            };
            let last_input = if session.last_input.is_empty() {
                "—".to_string()
            } else {
                wrap_text(&session.last_input, input_col_w, row_height as usize)
            };
            let perm = if session.last_permission.is_empty() {
                "—".to_string()
            } else {
                session.last_permission.clone()
            };
            let status_text = session.status.label();
            let (mode_text, mode_color) = mode_text_and_color(&session.auto_reply);
            let age = format_time_ago(session.last_event_time);
            let s_color = status_color(session.status);

            let h = row_height;
            let selected = self.table_state.selected() == Some(i);
            let idx_label = if selected {
                format!("▶{}", i + 1)
            } else {
                format!(" {}", i + 1)
            };
            let row = Row::new(vec![
                Cell::from(vcenter(&idx_label, h)).style(Style::default().fg(if selected { theme::SELECTED_FG } else { theme::HEADER_FG })),
                Cell::from(vcenter(project, h)).style(Style::default().fg(theme::FG).bold()),
                Cell::from(vcenter(&last_output, h)).style(Style::default().fg(theme::FG)),
                Cell::from(vcenter(&last_input, h)).style(Style::default().fg(theme::FG)),
                Cell::from(vcenter(&perm, h)).style(Style::default().fg(theme::FG)),
                Cell::from(vcenter(&format!(" {} ", status_text), h)).style(Style::default().fg(s_color).bold()),
                Cell::from(vcenter(mode_text, h)).style(Style::default().fg(mode_color)),
                Cell::from(vcenter(&age, h)).style(Style::default().fg(theme::HEADER_FG)),
            ]).height(row_height);

            rows.push(row);
        }

        // 表头
        let header = Row::new(vec![
            Cell::from(" #"),
            Cell::from("Project"),
            Cell::from("Last Output"),
            Cell::from("Last Input"),
            Cell::from("Perm"),
            Cell::from("Status"),
            Cell::from("Mode"),
            Cell::from("Age"),
        ])
        .style(Style::default().fg(theme::HEADER_FG).bold())
        .bottom_margin(1);

        // 列宽：Last Output = Fill(2), Last Input = Fill(4), Perm 缩小, Mode 加宽
        let widths = [
            Constraint::Length(4),    // #
            Constraint::Length(20),   // Project
            Constraint::Fill(2),     // Last Output (Last Input 的一半)
            Constraint::Fill(4),     // Last Input
            Constraint::Length(10),  // Perm (缩小)
            Constraint::Length(8),   // Status
            Constraint::Length(12),  // Mode (加宽)
            Constraint::Length(6),   // Age
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .title(" Sessions ")
            .title_style(Style::default().fg(theme::HEADER_FG).bold())
            .style(Style::default().bg(theme::BG));

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(
                Style::default()
                    .bg(theme::SELECTED_BG)
                    .fg(theme::SELECTED_FG)
                    .bold(),
            )
            .highlight_spacing(ratatui::widgets::HighlightSpacing::Never)
            .style(Style::default().bg(theme::BG));

        let table = table.block(block);
        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let content = if !self.message.is_empty() && (now() - self.message_time < 1.0) {
            Paragraph::new(format!("  {}", self.message))
                .style(Style::default().fg(theme::MSG_FG).bg(theme::BG).bold())
        } else {
            let keys = vec![
                Span::styled("  ↑↓", Style::default().fg(theme::TITLE).bold()),
                Span::styled(" Select  ", Style::default().fg(theme::FOOTER_FG)),
                Span::styled("←→", Style::default().fg(theme::TITLE).bold()),
                Span::styled(" Mode  ", Style::default().fg(theme::FOOTER_FG)),
                Span::styled("r", Style::default().fg(theme::TITLE).bold()),
                Span::styled(" Refresh  ", Style::default().fg(theme::FOOTER_FG)),
                Span::styled("q", Style::default().fg(theme::TITLE).bold()),
                Span::styled(" Quit", Style::default().fg(theme::FOOTER_FG)),
            ];
            Paragraph::new(Line::from(keys))
                .style(Style::default().bg(theme::BG))
        };

        let footer = content.block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme::BORDER))
                .style(Style::default().bg(theme::BG)),
        );
        frame.render_widget(footer, area);
    }

    // ========== 键盘处理 ==========

    fn handle_normal_key(&mut self, key: KeyCode, sessions: &[Session]) -> bool {
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Up | KeyCode::Char('k') => {
                let sel = self.table_state.selected().unwrap_or(0);
                self.table_state.select(Some(sel.saturating_sub(1)));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !sessions.is_empty() {
                    let sel = self.table_state.selected().unwrap_or(0);
                    self.table_state.select(Some((sel + 1).min(sessions.len() - 1)));
                }
            }
            KeyCode::Right | KeyCode::Left | KeyCode::Char('l') | KeyCode::Char('h') => {
                let sel = self.table_state.selected().unwrap_or(0);
                if let Some(session) = sessions.get(sel) {
                    let next = match session.auto_reply.as_deref() {
                        None => Some("allow".to_string()),
                        _ => None,
                    };
                    self.state.set_auto_reply(&session.session_id, next);
                }
            }
            KeyCode::Char('r') => {
                self.show_message("Refreshed");
            }
            _ => {}
        }
        false
    }

    fn show_message(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.message_time = now();
    }
}
