use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ========== 会话状态枚举 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Starting,
    Idle,
    Active,
    Running,
    Waiting,
    Stopped,
}

impl SessionStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Starting => "...",
            Self::Idle => "IDLE",
            Self::Active => "WORK",
            Self::Running => "TOOL",
            Self::Waiting => "WAIT",
            Self::Stopped => "STOP",
        }
    }
}

// ========== 等待审批的详情 ==========

#[derive(Debug, Clone)]
pub struct WaitingDetail {
    pub tool_name: String,
    pub tool_input: Value,
    #[allow(dead_code)]
    pub timestamp: f64,
}

// ========== 会话 ==========

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub cwd: String,
    pub permission_mode: String,
    pub status: SessionStatus,
    pub last_input: String,
    pub last_input_time: f64,
    pub current_tool: String,
    pub waiting_detail: Option<WaitingDetail>,
    pub auto_reply: Option<String>,
    pub last_event_time: f64,
    pub model: String,
}

impl Session {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            cwd: String::new(),
            permission_mode: String::new(),
            status: SessionStatus::Starting,
            last_input: String::new(),
            last_input_time: 0.0,
            current_tool: String::new(),
            waiting_detail: None,
            auto_reply: Some("allow".to_string()),
            last_event_time: 0.0,
            model: String::new(),
        }
    }
}

// ========== 当前时间戳（工具函数）==========

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ========== 状态管理器 ==========

#[derive(Clone)]
pub struct StateManager {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 处理 hook 事件，返回 Some(json) 表示有自动回复决策
    pub fn handle_event(&self, event_name: &str, data: &Value) -> Option<Value> {
        // 取出 session_id，没有就忽略
        let session_id = data.get("session_id")?.as_str()?;
        if session_id.is_empty() {
            return None;
        }

        // 加锁，拿到 sessions 的可变引用
        let mut sessions = self.sessions.lock().unwrap();
        let now = now();

        // 如果是 SessionStart，创建或更新 session
        if event_name == "SessionStart" {
            let session = sessions
                .entry(session_id.to_string())
                .or_insert_with(|| Session::new(session_id.to_string()));
            session.cwd = data.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
            session.permission_mode = data.get("permission_mode").and_then(|v| v.as_str()).unwrap_or("").to_string();
            session.model = data.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();
            session.status = SessionStatus::Idle;
            session.last_event_time = now;
        } else if !sessions.contains_key(session_id) {
            // 不是 SessionStart，但 session 不存在，先创建一个
            let mut session = Session::new(session_id.to_string());
            session.cwd = data.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
            sessions.insert(session_id.to_string(), session);
        }

        // 现在 session 一定存在，根据事件类型更新状态
        let session = sessions.get_mut(session_id).unwrap();

        match event_name {
            "UserPromptSubmit" => {
                session.last_input = data.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                session.last_input_time = now;
                session.status = SessionStatus::Active;
                session.waiting_detail = None;
                session.last_event_time = now;
            }
            "PreToolUse" => {
                session.status = SessionStatus::Running;
                session.current_tool = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                session.waiting_detail = None;
                session.last_event_time = now;
            }
            "PermissionRequest" => {
                let tool_name = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let tool_input = data.get("tool_input").cloned().unwrap_or(Value::Object(Default::default()));
                session.status = SessionStatus::Waiting;
                session.waiting_detail = Some(WaitingDetail {
                    tool_name,
                    tool_input: if tool_input.is_object() { tool_input } else { Value::Object(Default::default()) },
                    timestamp: now,
                });
                session.last_event_time = now;

                // 检查是否设了自动回复
                if let Some(ref reply) = session.auto_reply {
                    if reply == "allow" || reply == "deny" {
                        let behavior = reply.clone();
                        session.status = SessionStatus::Running;
                        session.waiting_detail = None;
                        return Some(serde_json::json!({
                            "hookSpecificOutput": {
                                "hookEventName": "PermissionRequest",
                                "decision": {
                                    "behavior": behavior,
                                }
                            }
                        }));
                    }
                }
            }
            "PostToolUse" | "PostToolUseFailure" => {
                session.status = SessionStatus::Active;
                session.current_tool = String::new();
                session.waiting_detail = None;
                session.last_event_time = now;
            }
            "Stop" => {
                session.status = SessionStatus::Idle;
                session.current_tool = String::new();
                session.waiting_detail = None;
                session.last_event_time = now;
            }
            "SessionEnd" => {
                session.status = SessionStatus::Stopped;
                session.last_event_time = now;
            }
            _ => {}
        }

        None
    }

    /// 获取会话列表，排除已停止的，按最后事件时间降序
    pub fn get_session_list(&self) -> Vec<Session> {
        let sessions = self.sessions.lock().unwrap();
        let mut list: Vec<Session> = sessions
            .values()
            .filter(|s| s.status != SessionStatus::Stopped)
            .cloned()
            .collect();
        list.sort_by(|a, b| b.last_event_time.partial_cmp(&a.last_event_time).unwrap_or(std::cmp::Ordering::Equal));
        list
    }

    /// 设置自动回复：Some("allow") / Some("deny") / None
    pub fn set_auto_reply(&self, session_id: &str, reply: Option<String>) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.auto_reply = reply;
        }
    }
}
