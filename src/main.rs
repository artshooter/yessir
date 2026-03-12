mod server;
mod state;
mod tui;

fn main() {
    let state_manager = state::StateManager::new();

    // 启动 HTTP 服务
    if let Err(e) = server::start_server(state_manager.clone(), server::DEFAULT_PORT) {
        eprintln!("Failed to start server on port {}: {}", server::DEFAULT_PORT, e);
        eprintln!("Is another yessir already running?");
        std::process::exit(1);
    }

    // 启动 TUI（阻塞主线程，直到用户按 q 退出）
    let mut tui = tui::TUI::new(state_manager, server::DEFAULT_PORT);
    if let Err(e) = tui.run() {
        eprintln!("TUI error: {}", e);
        std::process::exit(1);
    }
}
