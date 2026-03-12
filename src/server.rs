use crate::state::StateManager;
pub const DEFAULT_PORT: u16 = 7878;

/// 在新线程上启动 HTTP 服务
pub fn start_server(state_manager: StateManager, port: u16) -> Result<(), String> {
    // 绑定端口，失败就返回错误
    let server = tiny_http::Server::http(format!("127.0.0.1:{}", port))
        .map_err(|e| format!("{}", e))?;

    // 开一个新线程跑 server（主线程要留给 TUI）
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();

            if request.method() == &tiny_http::Method::Post && url == "/api/event" {
                handle_event(&state_manager, request);
            } else if url == "/api/health" {
                let _ = request.respond(tiny_http::Response::from_string("ok"));
            } else if request.method() == &tiny_http::Method::Get && url == "/api/sessions" {
                handle_sessions(&state_manager, request);
            } else {
                let _ = request.respond(
                    tiny_http::Response::from_string("").with_status_code(404),
                );
            }
        }
    });

    Ok(())
}

/// 处理 POST /api/event
fn handle_event(state: &StateManager, mut request: tiny_http::Request) {
    // 读请求体
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        let _ = request.respond(
            tiny_http::Response::from_string(r#"{"error":"read failed"}"#).with_status_code(400),
        );
        return;
    }

    // 解析 JSON
    let data: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(
                tiny_http::Response::from_string(r#"{"error":"invalid json"}"#)
                    .with_status_code(400),
            );
            return;
        }
    };

    // 取出事件名，交给 StateManager 处理
    let event_name = data
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let result = state.handle_event(event_name, &data);

    // 返回结果（如果有自动回复决策就返回，否则返回空 JSON）
    let response_body = match result {
        Some(v) => serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    };

    let header =
        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
    let _ = request.respond(tiny_http::Response::from_string(response_body).with_header(header));
}

/// 处理 GET /api/sessions
fn handle_sessions(state: &StateManager, request: tiny_http::Request) {
    let sessions = state.get_session_list();
    let result: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "session_id": s.session_id,
                "cwd": s.cwd,
                "status": s.status,
                "last_input": s.last_input,
                "current_tool": s.current_tool,
                "auto_reply": s.auto_reply,
                "model": s.model,
            })
        })
        .collect();

    let body = serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string());
    let header =
        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap();
    let _ = request.respond(tiny_http::Response::from_string(body).with_header(header));
}
