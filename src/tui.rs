use crate::state::{SessionStatus, StateManager, Session};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::DefaultTerminal;
use ratatui::widgets::Paragraph;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ========== 工具函数 ==========

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// 把时间戳格式化为 "3s" / "5m" / "2h"
fn format_time_ago(t: f64) -> String {
    if t == 0.0 {
        return String::new();
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

/// 从完整路径提取项目名："/Users/as/yessir" → "yessir"
fn get_project_name(cwd: &str) -> &str {
    if cwd.is_empty() {
        return "?";
    }
    cwd.rsplit('/').next().unwrap_or(cwd)
}

// ========== TUI 主结构 ==========

pub struct TUI {
    state: StateManager,
    port: u16,
    selected: usize,            // 当前选中的行
    message: String,            // 底部提示消息
    message_time: f64,          // 消息显示时间
    input_mode: bool,           // 是否在输入模式
    input_buffer: String,       // 输入框内容
}

impl TUI {
    pub fn new(state: StateManager, port: u16) -> Self {
        Self {
            state,
            port,
            selected: 0,
            message: String::new(),
            message_time: 0.0,
            input_mode: false,
            input_buffer: String::new(),
        }
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let mut terminal = ratatui::init();       // 进入终端 raw 模式
        let result = self.main_loop(&mut terminal);
        ratatui::restore();                        // 恢复正常终端
        result
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            let sessions = self.state.get_session_list();

            // 防止选中行越界
            if !sessions.is_empty() {
                self.selected = self.selected.min(sessions.len() - 1);
            } else {
                self.selected = 0;
            }

            // 画界面（类似 React 的 render）
            terminal.draw(|frame| self.render(frame, &sessions))?;

            // 等 1 秒或等键盘事件（类似 addEventListener）
            if event::poll(Duration::from_secs(1))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    // Ctrl+C 退出
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }
                    if self.input_mode {
                        self.handle_input_key(key.code, &sessions);
                    } else if self.handle_normal_key(key.code, &sessions) {
                        return Ok(());
                    }
                }
            }
        }
    }

    // ========== 渲染 ==========

    fn render(&self, frame: &mut Frame, sessions: &[Session]) {
        let area = frame.area();

        // 把终端分成 7 行：标题、分隔线、列头、分隔线、内容区、分隔线、底栏
        let chunks = Layout::vertical([
            Constraint::Length(1), // 标题
            Constraint::Length(1), // ─── 分隔线
            Constraint::Length(1), // 列头
            Constraint::Length(1), // ─── 分隔线
            Constraint::Min(1),   // 会话列表（占剩余空间）
            Constraint::Length(1), // ─── 分隔线
            Constraint::Length(1), // 底栏（快捷键提示 / 输入框）
        ])
        .split(area);

        // 标题
        let header = format!(
            " Yes! Sir  │  {} sessions  │  port {}",
            sessions.len(),
            self.port
        );
        frame.render_widget(Paragraph::new(header).bold(), chunks[0]);

        // 分隔线
        let separator = "─".repeat(area.width as usize);
        frame.render_widget(Paragraph::new(separator.clone()), chunks[1]);

        // 列头
        let col_header = format!(
            " {:<3} {:<20} {:<40} {:<8} {:<6} {:<5}",
            "#", "Project", "Last Input", "Status", "Auto", "Age"
        );
        frame.render_widget(Paragraph::new(col_header).dim(), chunks[2]);

        // 分隔线
        frame.render_widget(Paragraph::new(separator.clone()), chunks[3]);

        // 会话列表
        self.render_sessions(frame, sessions, chunks[4]);

        // 分隔线
        frame.render_widget(Paragraph::new(separator), chunks[5]);

        // 底栏
        self.render_footer(frame, chunks[6]);
    }

    fn render_sessions(&self, frame: &mut Frame, sessions: &[Session], area: Rect) {
        if sessions.is_empty() {
            frame.render_widget(
                Paragraph::new(" No active sessions. Start Claude Code to see sessions here.")
                    .dim(),
                area,
            );
            return;
        }

        let mut y = 0u16;
        for (i, session) in sessions.iter().enumerate() {
            if y >= area.height {
                break;
            }

            // 格式化每一列
            let project = get_project_name(&session.cwd);
            let last_input = if session.last_input.is_empty() {
                "-"
            } else {
                &session.last_input
            };
            let last_input_clean: String = last_input.replace('\n', " ").replace('\r', " ");
            let status_text = session.status.label();
            let auto = session.auto_reply.as_deref().unwrap_or("-");
            let age = format_time_ago(session.last_event_time);

            let line = format!(
                " {:<3} {:<20.20} {:<40.40} {:<8} {:<6} {:<5}",
                i + 1,
                project,
                last_input_clean,
                status_text,
                auto,
                age,
            );

            // 根据状态选颜色
            let style = if i == self.selected {
                Style::default().bg(Color::Blue).fg(Color::White).bold()
            } else {
                match session.status {
                    SessionStatus::Waiting => Style::default().fg(Color::Yellow).bold(),
                    SessionStatus::Active | SessionStatus::Running => {
                        Style::default().fg(Color::Green)
                    }
                    SessionStatus::Idle => Style::default().fg(Color::Cyan),
                    SessionStatus::Stopped => Style::default().fg(Color::Red).dim(),
                    SessionStatus::Starting => Style::default().dim(),
                }
            };

            let row_area = Rect::new(area.x, area.y + y, area.width, 1);
            frame.render_widget(Paragraph::new(line).style(style), row_area);
            y += 1;

            // 选中行 + 等待状态：显示工具详情
            if i == self.selected && session.status == SessionStatus::Waiting {
                if let Some(ref detail) = session.waiting_detail {
                    if y < area.height {
                        let inp_str = if let Some(obj) = detail.tool_input.as_object() {
                            obj.get("command")
                                .or_else(|| obj.get("file_path"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| detail.tool_input.to_string())
                        } else {
                            detail.tool_input.to_string()
                        };
                        let detail_line = format!("     ↳ {}: {}", detail.tool_name, inp_str);
                        let detail_area = Rect::new(area.x, area.y + y, area.width, 1);
                        frame.render_widget(
                            Paragraph::new(detail_line).fg(Color::Yellow),
                            detail_area,
                        );
                        y += 1;
                    }
                }
            }
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if self.input_mode {
            let prompt = format!(
                " Auto-reply (allow/deny/empty to cancel): {}█",
                self.input_buffer
            );
            frame.render_widget(Paragraph::new(prompt), area);
        } else if !self.message.is_empty() && (now() - self.message_time < 3.0) {
            frame.render_widget(Paragraph::new(format!(" {}", self.message)).bold(), area);
        } else {
            frame.render_widget(
                Paragraph::new(" ↑↓ Select  a Auto-reply  d Clear  r Refresh  q Quit").dim(),
                area,
            );
        }
    }

    // ========== 键盘处理 ==========

    /// 返回 true 表示退出
    fn handle_normal_key(&mut self, key: KeyCode, sessions: &[Session]) -> bool {
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !sessions.is_empty() {
                    self.selected = (self.selected + 1).min(sessions.len() - 1);
                }
            }
            KeyCode::Char('a') => {
                if !sessions.is_empty() {
                    self.input_mode = true;
                    self.input_buffer.clear();
                }
            }
            KeyCode::Char('d') => {
                if let Some(session) = sessions.get(self.selected) {
                    self.state.set_auto_reply(&session.session_id, None);
                    self.show_message("Auto-reply cleared");
                }
            }
            KeyCode::Char('r') => {
                self.show_message("Refreshed");
            }
            _ => {}
        }
        false
    }

    fn handle_input_key(&mut self, key: KeyCode, sessions: &[Session]) {
        match key {
            KeyCode::Esc => {
                self.input_mode = false;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                self.input_mode = false;
                let value = self.input_buffer.trim().to_lowercase();
                self.input_buffer.clear();
                if let Some(session) = sessions.get(self.selected) {
                    match value.as_str() {
                        "allow" | "deny" => {
                            self.state
                                .set_auto_reply(&session.session_id, Some(value.clone()));
                            self.show_message(&format!("Auto-reply set to: {}", value));
                        }
                        "" => {
                            self.show_message("Cancelled");
                        }
                        _ => {
                            self.show_message("Invalid: use 'allow' or 'deny'");
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn show_message(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.message_time = now();
    }
}
