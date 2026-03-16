# yessir

A lightweight TUI dashboard for monitoring multiple Claude Code sessions and auto-handling permission requests.

## Install

```bash
npx -y @artshooter/yessir install
```

Then open a new terminal tab (or `source ~/.zshrc`) and run:

```bash
yessir
```

## What it does

- Real-time monitoring of multiple Claude Code sessions
- Shows session state: idle, working, running tools, waiting for permission, stopped
- Auto-approve or manually handle permission requests per session
- Terminal-native, no web UI

## How it works

```
Claude Code → hook event → yessir-hook → HTTP → StateManager → TUI
```

`yessir-hook` is a lightweight binary that Claude Code invokes on each lifecycle event. It forwards events to the `yessir` server. For `PermissionRequest` events, the response flows back to Claude Code as an allow/deny decision.

## TUI controls

| Key | Action |
|-----|--------|
| `j`/`k` or arrows | Navigate sessions |
| `a` | Auto-allow permissions for selected session |
| `d` | Clear auto-reply (manual mode) |
| `q` | Quit |

## Commands

```bash
yessir              # Start the TUI dashboard
yessir status       # Show installation status
yessir update       # Update to latest version
yessir uninstall    # Remove hooks, PATH, and binaries
```

Or via npx:

```bash
npx -y @artshooter/yessir [command]
```

## Build from source

```bash
cargo build --release
```

## License

MIT
