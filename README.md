# yessir

**A lightweight TUI dashboard for monitoring multiple Claude Code sessions and handling permission requests in real time.**

`yessir` is built for people who run multiple Claude Code sessions at once and do not want permission prompts to constantly interrupt the flow.

It listens to Claude Code hook events, shows live session status in a terminal dashboard, and can automatically reply to permission requests per session.

## Why yessir?

Most Claude Code monitoring tools aim to be broad dashboards for analytics, history, config, or workspace management.

`yessir` is intentionally narrower. It focuses on one operational question:

> When you are running many Claude Code sessions, who is working, who is waiting, and who is blocked on permissions?

`yessir` gives you a fast, local, terminal-native control surface for that workflow.

## What it does

- Monitors multiple Claude Code sessions in real time
- Tracks session state across hook events
- Shows whether a session is idle, working, running tools, waiting, or stopped
- Surfaces the current tool, latest prompt context, and permission mode
- Auto-approves or requires manual handling for permission requests
- Runs locally with a minimal setup and no web UI required

## What it is not

`yessir` is **not** trying to be a full Claude Code management suite.

It does not aim to cover:

- deep analytics
- cost reporting
- config editing
- full history search
- web dashboards
- MCP or server management

Instead, it stays focused on **live session awareness + permission flow control**.

## Best for

- developers running multiple Claude Code sessions in parallel
- agent-heavy workflows where permission prompts become operational overhead
- terminal-first users who want a simple local dashboard
- unattended or semi-unattended coding workflows

## Positioning in one line

**`yessir` is the lightweight operations console for Claude Code permission flow.**

## Installation

### Option 1: npm

Install and configure hooks automatically:

```bash
npx -y @artshooter/yessir install
```

Start the dashboard:

```bash
npx -y @artshooter/yessir
```

Useful commands:

```bash
npx -y @artshooter/yessir status
npx -y @artshooter/yessir update
npx -y @artshooter/yessir uninstall
```

This installs the binaries locally and adds the required Claude Code hooks to `~/.claude/settings.json`.

### Option 2: build from source

```bash
cargo build
cargo run --bin yessir
```

For local hook testing:

```bash
cargo run --bin yessir-hook -- SessionStart
```

## How it works

```text
Claude Code -> hook event -> yessir-hook -> local HTTP server -> session state -> TUI
```

For `PermissionRequest` events, `yessir` can immediately return an `allow` or `deny` decision back through the hook response path.

## TUI controls

- `j` / `k` or arrow keys: move between sessions
- `a`: set auto-reply to allow for the selected session
- `d`: clear auto-reply and switch back to manual handling
- `q`: quit

## Scope and philosophy

`yessir` is designed to be small, fast, and operationally useful.

Instead of building a broad control plane for everything Claude Code can do, it focuses on the tight inner loop of:

1. receiving hook events
2. tracking live session state
3. spotting blocked sessions immediately
4. keeping permission prompts from breaking flow

If that is the workflow you care about, `yessir` is the tool.
