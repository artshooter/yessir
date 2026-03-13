use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    // 读取命令行参数：yessir-hook SessionStart
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return;
    }
    let event_name = &args[1];

    // 从 stdin 读 JSON（Claude Code 通过 stdin 传入事件数据）
    let mut input = String::new();

    // DEBUG: 把原始输入写到 /tmp/yessir-debug.log
    if std::io::stdin().read_to_string(&mut input).is_ok() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open("/tmp/yessir-debug.log") {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let _ = writeln!(f, "=== {} | {} ===", ts, event_name);
            let _ = writeln!(f, "{}", input);
            let _ = writeln!(f, "---");
        }
    }

    let mut data: serde_json::Value = if !input.is_empty() {
        serde_json::from_str(&input).unwrap_or(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::Value::Object(Default::default())
    };

    // 注入事件名
    if let Some(obj) = data.as_object_mut() {
        obj.insert(
            "hook_event_name".to_string(),
            serde_json::Value::String(event_name.clone()),
        );
    }

    // 读端口（支持环境变量覆盖）
    let port: u16 = env::var("YESSIR_PORT")
        .ok()                       // Result → Option
        .and_then(|p| p.parse().ok()) // 尝试转数字
        .unwrap_or(7878);

    // 发 POST 请求到 yessir server
    let body = serde_json::to_string(&data).unwrap_or_default();
    if let Some(response) = post_json(port, &body) {
        // 有内容且不是空 JSON，就输出到 stdout（Claude Code 会读取）
        if !response.is_empty() && response.trim() != "{}" {
            println!("{}", response);
        }
    }
    // 连接失败（server 没跑）就静默退出，不报错
}

/// 用原始 TCP 发 HTTP POST 请求（只给 localhost 用，不需要引入 HTTP 客户端库）
fn post_json(port: u16, body: &str) -> Option<String> {
    // 建立 TCP 连接
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    // 手写 HTTP 请求（就是一段文本，这就是 HTTP 协议的本质）
    let request = format!(
        "POST /api/event HTTP/1.1\r\n\
         Host: 127.0.0.1:{}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        port,
        body.len(),
        body
    );

    // 发出去
    stream.write_all(request.as_bytes()).ok()?;
    stream.flush().ok()?;

    // 读响应
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    // 解析 HTTP 响应：跳过 header，取 body
    // HTTP 响应格式：header\r\n\r\nbody
    let body_start = response.find("\r\n\r\n").map(|i| i + 4)?;
    Some(response[body_start..].trim().to_string())
}
