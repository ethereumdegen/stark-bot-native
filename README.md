# stark-bot

A terminal UI client for [Starflask](https://starflask.com) — the AI agent orchestration platform. Chat with your agents, manage tasks, track sessions, and monitor credits, all from your terminal.

Built with Rust, async I/O, and the [iocraft](https://github.com/ccbrown/iocraft) component framework.

```
┌──────────────────────────────────────────────────────────┐
│  stark-bot v2.1.0          agent: researcher   ⚡ 1,240  │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ❯ What's the latest on the project?                     │
│                                                          │
│  researcher> I've reviewed the current state...          │
│                                                          │
│  --- Session abc123 completed (researcher)               │
│                                                          │
│  ❯ /tasks todo                                           │
│                                                          │
│  --- Tasks (todo):                                       │
│      #1 [todo] Refactor auth module (high)               │
│      #3 [todo] Add unit tests (medium)                   │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  /tasks ░                                                │
└──────────────────────────────────────────────────────────┘
```

## What is Starflask?

[Starflask](https://starflask.com) is an AI agent orchestration platform. You create agents with different capabilities (research, coding, writing, etc.), assign them to projects, and interact with them through chat channels. Agents can be scheduled, maintain memories across sessions, and work within project-based task boards.

**stark-bot** is the official terminal client for Starflask. Instead of using the web dashboard, you get a fast, keyboard-driven interface that fits naturally into a developer workflow. Messages sent through stark-bot go through the same Starflask API — your agents, projects, tasks, and session history are all in sync.

Key concepts:
- **Agents** — AI assistants with specific capabilities, created and configured on Starflask
- **Projects** — Workspaces that group agents, tasks, and chat channels together
- **Sessions** — Individual query/response interactions with agents, tracked with full history
- **Tasks** — Project-scoped items with status tracking (todo, in_progress, done, blocked)
- **Memories** — Persistent context an agent retains across sessions

## Installation

### From source

```bash
git clone https://github.com/andybrandt/stark-bot-tui.git
cd stark-bot-tui
cargo build --release

# Optionally copy to your PATH
cp target/release/stark-bot ~/.local/bin/
```

**Requirements:** Rust 2024 edition (1.85+)

## Getting Started

### 1. Get a Starflask API key

Sign up at [starflask.com](https://starflask.com) and grab your API key from the dashboard.

### 2. Run the setup wizard

```bash
stark-bot setup
```

Or just launch `stark-bot` — it will detect a missing key and walk you through setup automatically.

The wizard will:
- Prompt for your API key
- Sync your available agents
- Let you pick a default project

Configuration is stored in `~/.config/starkbot/config.yaml` and `~/.config/starkbot/.env`.

### 3. Start chatting

```bash
stark-bot
```

Type a message and press **Enter**. Your message is sent to the project chat channel (if a project is configured) or directly to the selected agent. Responses stream in as sessions complete.

## Commands

### In-TUI slash commands

Type these directly in the chat input:

| Command | Args | Description |
|---------|------|-------------|
| `/help`, `/h` | | Show all available commands |
| `/agents` | | List your available agents |
| `/agent <name>` | name | Switch to a different agent |
| `/default <name>` | name | Set and persist the default agent |
| `/tasks [status]` | `todo` `in_progress` `done` `blocked` | List project tasks, optionally filtered |
| `/task <title>` | `title \| description \| priority` | Create a new task (pipe-separated fields) |
| `/done <id>` | task ID | Mark a task as done |
| `/schedules` | | List scheduled runs for the current agent |
| `/credits` | | Show your credit balance and subscription info |
| `/history [n]` | count (default: 10) | Show recent sessions |
| `/memories [n]` | count (default: 20) | Show agent memories |
| `/provision`, `/sync` | | Re-sync agents from the Starflask API |
| `/connect` | | Set or update your API key |
| `/reset` | | Wipe all config and start fresh |
| `/clear` | | Clear the message screen |
| `/quit`, `/q`, `/exit` | | Exit |

Slash commands autocomplete as you type — use **arrow keys** to navigate suggestions and **Tab** to complete.

### CLI subcommands

These run outside the TUI and exit immediately:

```bash
stark-bot agents                       # List available agents
stark-bot provision [--file <path>]    # Sync agents from API (or load a seed pack)
stark-bot query <agent> <message>      # One-shot query to an agent
stark-bot setup                        # Run the setup wizard
stark-bot config [key] [value]         # Get or set a config value
```

One-shot queries are handy for scripts and pipelines:

```bash
stark-bot query researcher "Summarize the latest news on Rust async"
```

## Configuration

Config lives in `~/.config/starkbot/`:

| File | Purpose |
|------|---------|
| `config.yaml` | Default agent, project ID, polling settings |
| `.env` | `STARFLASK_API_KEY` |

### config.yaml

```yaml
default_agent: general
project_id: null
base_url: https://starflask.com/api
poll_interval_secs: 3
poll_timeout_secs: 600
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `STARFLASK_API_KEY` | Your API authentication token (required) |
| `STARFLASK_BASE_URL` | Override the API base URL (optional) |

## Architecture

```
src/
├── main.rs            # Entry point, CLI dispatch, TUI bootstrap
├── app.rs             # Core state: agents, config, slash command routing
├── cli.rs             # CLI argument definitions (clap)
├── config.rs          # YAML + env config loading/saving
├── db.rs              # Agent model parsing
├── starflask.rs       # HTTP + WebSocket client for the Starflask API
├── commands/
│   ├── agents.rs      # `stark-bot agents` subcommand
│   ├── config_cmd.rs  # `stark-bot config` subcommand
│   ├── provision.rs   # `stark-bot provision` subcommand
│   ├── query.rs       # `stark-bot query` subcommand
│   └── setup.rs       # Setup wizard
└── ui/
    ├── app.rs         # Main TUI component (iocraft)
    ├── header.rs      # Top bar: agent name, credit balance
    ├── input.rs       # Text input bar
    ├── messages.rs    # Scrollable chat message list
    ├── message.rs     # Message type definitions
    ├── command_hint.rs# Slash command autocomplete popup
    ├── spinner.rs     # Loading indicator
    └── theme.rs       # Color constants
```

The TUI is built with **iocraft** — a React-like component framework for terminal UIs. It runs on the **smol** async executor with **tokio** handling heavier async work (HTTP, WebSocket). The Starflask client uses **reqwest** for REST calls and **async-tungstenite** for real-time WebSocket session streaming.

## Query modes

**Project mode** (default when a project is configured): Messages go to the project's chat channel. Starflask routes them to the appropriate agent based on project rules.

**Direct mode**: When no project is set, or when you switch to a specific agent with `/agent`, queries go directly to that agent.

## Real-time updates

stark-bot opens a WebSocket connection to Starflask and listens for session completion events. When an agent finishes processing (even if triggered externally), the result appears in your terminal in real time.

## License

MIT
