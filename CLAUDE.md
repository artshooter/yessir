# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build              # debug build
cargo build --release    # release build
cargo run --bin yessir   # run the TUI dashboard
cargo run --bin yessir-hook -- <EventName>  # run the hook binary (normally called by Claude Code, not manually)
```

No tests or linter configured yet.

## Architecture

**yessir** is a Claude Code session monitor. It tracks multiple Claude Code sessions in real-time via hooks and displays them in a terminal dashboard.

### Two binaries

- **`yessir`** (`src/main.rs`) — Main process: starts an HTTP server on a background thread, then runs a blocking TUI on the main thread.
- **`yessir-hook`** (`src/bin/hook.rs`) — Lightweight hook binary that Claude Code invokes for each lifecycle event. It reads the event JSON from stdin, injects the event name, and forwards it to the yessir server via raw TCP HTTP POST to `/api/event`. Designed to be fast and silent on failure (if the server isn't running, it exits without error).

### Core modules

- **`state.rs`** — `StateManager` (thread-safe via `Arc<Mutex<HashMap>>`) holds all session state. `handle_event()` is the central state machine that maps hook events (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Stop`, `SessionEnd`) to session status transitions. Also handles auto-reply logic: when a `PermissionRequest` arrives and auto-reply is set, it returns a JSON decision (`allow`/`deny`) immediately.
- **`server.rs`** — `tiny_http` server with three endpoints: `POST /api/event` (hook events), `GET /api/sessions` (session list), `GET /api/health`.
- **`tui.rs`** — `ratatui`-based dashboard. Polls `StateManager` every 1s. Key bindings: `j/k` or arrows to navigate, `a` to set auto-reply, `d` to clear auto-reply, `q` to quit.

### Data flow

```
Claude Code → hook event (stdin JSON) → yessir-hook → HTTP POST /api/event → server → StateManager
                                                                                         ↓
                                                                              TUI polls StateManager
```

For `PermissionRequest` events, the response flows back: `StateManager` → server HTTP response → `yessir-hook` stdout → Claude Code reads the decision.

### Key design decisions

- Default `auto_reply` for new sessions is `Some("allow")` — all permissions auto-approved unless changed via TUI.
- `yessir-hook` uses raw TCP instead of an HTTP client library to keep the binary minimal and fast.
- Port defaults to `7878`, overridable via `YESSIR_PORT` env var.
