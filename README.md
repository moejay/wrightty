# wrightty

[![CI](https://github.com/moejay/wrightty/actions/workflows/ci.yml/badge.svg)](https://github.com/moejay/wrightty/actions/workflows/ci.yml)
[![Release](https://github.com/moejay/wrightty/actions/workflows/release.yml/badge.svg)](https://github.com/moejay/wrightty/actions/workflows/release.yml)
[![crates.io](https://img.shields.io/crates/v/wrightty.svg)](https://crates.io/crates/wrightty)
[![PyPI](https://img.shields.io/pypi/v/wrightty.svg)](https://pypi.org/project/wrightty/)
[![npm](https://img.shields.io/npm/v/@moejay/wrightty.svg)](https://www.npmjs.com/package/@moejay/wrightty)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A CDP-like protocol for terminal automation. Control any terminal emulator programmatically — send keystrokes, read the screen, take screenshots, and run commands over WebSocket JSON-RPC.

Built for AI coding agents that need to interact with terminals the way humans do.

## Install

### One-liner (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/moejay/wrightty/main/install.sh | sh
```

### Cargo (from crates.io)

```bash
cargo install wrightty
```

### From source

```bash
git clone https://github.com/moejay/wrightty.git
cd wrightty
cargo build --release -p wrightty
# binary is at target/release/wrightty
```

#### Feature flags

The `headless` feature (enabled by default) pulls in `alacritty_terminal` for virtual PTY support. For a lighter build with only bridge support:

```bash
cargo build --release -p wrightty --no-default-features --features bridge-tmux,bridge-wezterm,client
```

### SDKs

```bash
pip install wrightty             # Python
npm install @moejay/wrightty     # Node.js
```

## Quick start

### 1. Start a terminal server

```bash
# Headless (no GUI, great for CI/automation)
wrightty term --headless

# Or bridge to your existing terminal:
wrightty term --bridge-tmux       # tmux (works with any terminal)
wrightty term --bridge-wezterm    # WezTerm
wrightty term --bridge-kitty      # Kitty
wrightty term --bridge-ghostty    # Ghostty
wrightty term --bridge-zellij     # Zellij

# Or use the Alacritty fork with native support:
wrightty term --alacritty

# With a name and password:
wrightty term --headless --name my-server --password secret123
```

### 2. Control it

```bash
wrightty run "ls -la"                    # run a command, print output
wrightty read                            # read the screen
wrightty send-keys Ctrl+c               # send keystrokes
wrightty screenshot --format svg -o t.svg  # take a screenshot
wrightty wait-for "BUILD SUCCESS"        # wait for text
wrightty discover                        # find running servers
```

### 3. Or use the SDK

**Python:**

```python
from wrightty import Terminal

term = Terminal.connect()  # auto-discovers running server
output = term.run("cargo test")
print(output)

term.wait_for("$")
term.send_keys("Ctrl+c")
svg = term.screenshot("svg")
term.close()
```

**Node.js:**

```typescript
import { Terminal } from "wrightty";

const term = await Terminal.connect();
const output = await term.run("cargo test");
console.log(output);

await term.waitFor("$");
await term.sendKeys("Ctrl+c");
const screenshot = await term.screenshot("svg");
term.close();
```

**Raw protocol:**

```bash
wscat -c ws://127.0.0.1:9420
```

```json
{"jsonrpc":"2.0","id":1,"method":"Screen.getText","params":{"sessionId":"0"}}
```

## Terminal modes

### Headless (`--headless`)

Spawns a virtual terminal with no window. Uses Alacritty's terminal emulator under the hood. Good for CI, testing, and headless automation.

```bash
wrightty term --headless
wrightty term --headless --port 9420 --max-sessions 64
```

```python
# Spawn a new session on the headless server
term = Terminal.spawn(server_url="ws://127.0.0.1:9420")
output = term.run("echo hello")
term.close()
```

### Alacritty (`--alacritty`)

Uses a [fork of Alacritty](https://github.com/moejay/alacritty/tree/wrightty-support) with wrightty built in. Zero overhead — reads directly from Alacritty's own terminal state.

```bash
# Install the fork first:
git clone -b wrightty-support https://github.com/moejay/alacritty.git
cd alacritty && cargo install --path alacritty --features wrightty

# Then launch via wrightty:
wrightty term --alacritty
```

### Bridges

Bridges translate wrightty protocol calls into your terminal's existing IPC. No terminal modifications needed.

| Bridge | Command | Requirements |
|--------|---------|-------------|
| **tmux** | `wrightty term --bridge-tmux` | tmux server running |
| **WezTerm** | `wrightty term --bridge-wezterm` | WezTerm running |
| **Kitty** | `wrightty term --bridge-kitty` | `allow_remote_control yes` in kitty.conf |
| **Zellij** | `wrightty term --bridge-zellij` | Run from within a Zellij session |
| **Ghostty** | `wrightty term --bridge-ghostty` | Ghostty running; `xdotool` for input on Linux |

For terminals without IPC (foot, GNOME Terminal, Rio, etc.), use **tmux** or **zellij** as a bridge — it works with any terminal.

> **Native Ghostty fork also available:** [moejay/ghostty](https://github.com/moejay/ghostty/tree/wrightty) has Wrightty built directly into Ghostty.

## Python SDK

```bash
pip install wrightty
# or: pip install wrightty[mcp]  # for MCP server support
```

```python
from wrightty import Terminal

# Connect to a running terminal
term = Terminal.connect("ws://127.0.0.1:9420")

# Run a command and get its output
output = term.run("cargo build 2>&1", timeout=120)

# Read the screen
screen = term.read_screen()

# Wait for text to appear
term.wait_for("tests passed", timeout=60)
term.wait_for(r"error\[\w+\]", regex=True)

# Send keystrokes (for TUI apps)
term.send_keys("Escape", ":", "w", "q", "Enter")  # vim save & quit
term.send_keys("Ctrl+c")                           # interrupt

# Screenshots
svg = term.screenshot("svg")
text = term.screenshot("text")

# Terminal info
cols, rows = term.get_size()
info = term.get_info()

# Recording — session (asciicast, compatible with asciinema)
rec_id = term.start_session_recording(include_input=True)
term.run("make build")
result = term.stop_session_recording(rec_id)
open("build.cast", "w").write(result["data"])  # asciinema play build.cast

# Recording — actions (generates replayable scripts)
rec_id = term.start_action_recording(format="python")
term.send_text("echo hello\n")
term.send_keys("Ctrl+c")
result = term.stop_action_recording(rec_id)
print(result["data"])  # prints a Python script that replays these actions

term.close()
```

## Node.js SDK

```bash
npm install @moejay/wrightty
```

```typescript
import { Terminal } from "@moejay/wrightty";

// Auto-discover running server
const term = await Terminal.connect();

// Or connect to a specific server
const term = await Terminal.connect({ url: "ws://127.0.0.1:9420" });

// Run commands
const output = await term.run("cargo test", 120_000);

// Read screen
const screen = await term.readScreen();

// Wait for text
await term.waitFor("tests passed", 60_000);
await term.waitFor(/error\[\w+\]/);

// Send keystrokes
await term.sendKeys("Escape", ":", "w", "q", "Enter");
await term.sendKeys("Ctrl+c");

// Screenshots
const svg = await term.screenshot("svg");

// Terminal info
const [cols, rows] = await term.getSize();
const info = await term.getInfo();

// Recording
const recId = await term.startSessionRecording(true);
await term.run("make build");
const result = await term.stopSessionRecording(recId);

term.close();
```

## CLI reference

```bash
# Server / bridge modes
wrightty term --headless                # headless terminal server
wrightty term --alacritty               # alacritty fork
wrightty term --bridge-tmux             # tmux bridge
wrightty term --bridge-wezterm          # wezterm bridge
wrightty term --bridge-kitty            # kitty bridge
wrightty term --bridge-zellij           # zellij bridge
wrightty term --bridge-ghostty          # ghostty bridge

# Common options for `wrightty term`
  --host 127.0.0.1                      # bind address
  --port 9420                           # port (default: auto-select)
  --max-sessions 64                     # max sessions (headless)
  --watchdog-interval 5                 # health check interval (bridges)

# Client commands
wrightty run "ls -la"                   # run command, print output
wrightty run "cargo test" --timeout 120 # with timeout
wrightty read                           # read terminal screen
wrightty send-text "echo hello\n"       # send raw text
wrightty send-keys Ctrl+c              # send keystrokes
wrightty send-keys Escape : w q Enter  # multiple keys
wrightty screenshot --format svg -o t.svg  # screenshot
wrightty wait-for "BUILD SUCCESS"       # wait for text
wrightty wait-for "error" --regex       # regex pattern
wrightty discover                       # find servers
wrightty discover --json                # machine-readable
wrightty info                           # server info
wrightty size                           # terminal dimensions
wrightty session list                   # list sessions
wrightty session create                 # create session (headless)
wrightty session destroy <id>           # destroy session

# Connection options (all client commands)
  --url ws://127.0.0.1:9420             # server URL
  --session <id>                        # session ID
```

## MCP Server (for Claude, Cursor, etc.)

Wrightty includes an MCP server that exposes terminal control as tools for AI agents.

```json
{
  "mcpServers": {
    "wrightty": {
      "command": "python3",
      "args": ["-m", "wrightty.mcp_server"],
      "env": {
        "WRIGHTTY_SOCKET": "ws://127.0.0.1:9420"
      }
    }
  }
}
```

Tools exposed: `run_command`, `read_terminal`, `send_keys`, `send_text`, `screenshot`, `wait_for_text`, `terminal_info`, `start_recording`, `stop_recording`, `capture_screen_frame`.

## Protocol

The full protocol specification is in [PROTOCOL.md](PROTOCOL.md).

7 domains, 28 methods, all over WebSocket JSON-RPC 2.0:

| Domain | Methods |
|--------|---------|
| **Wrightty** | `getInfo` — capability negotiation |
| **Session** | `create`, `destroy`, `list`, `getInfo` |
| **Input** | `sendKeys`, `sendText`, `sendMouse` |
| **Screen** | `getContents`, `getText`, `getScrollback`, `screenshot`, `waitForText`, `waitForCursor` |
| **Terminal** | `resize`, `getSize`, `setColorPalette`, `getColorPalette`, `getModes` |
| **Recording** | `startSession`, `stopSession`, `startActions`, `stopActions`, `captureScreen`, `startVideo`, `stopVideo` |
| **Events** | `subscribe`, `unsubscribe` — screen updates, bell, title change, shell integration |

## Architecture

```
wrightty/
├── crates/
│   ├── wrightty/                   # Unified CLI binary (this is what you install)
│   ├── wrightty-protocol/          # Protocol types (serde, no logic)
│   ├── wrightty-core/              # Headless terminal engine (alacritty_terminal + PTY)
│   ├── wrightty-server/            # WebSocket daemon library
│   ├── wrightty-client/            # Rust client SDK
│   ├── wrightty-bridge-wezterm/    # WezTerm bridge
│   ├── wrightty-bridge-tmux/       # tmux bridge
│   ├── wrightty-bridge-kitty/      # Kitty bridge
│   ├── wrightty-bridge-zellij/     # Zellij bridge
│   └── wrightty-bridge-ghostty/    # Ghostty bridge
├── sdks/
│   ├── python/                     # Python SDK + MCP server
│   └── node/                       # Node.js/TypeScript SDK
├── install.sh                      # One-liner installer
└── PROTOCOL.md                     # Full protocol specification
```

## Terminal compatibility

| Terminal | Read screen | Send input | Sessions | Screenshot | Integration |
|----------|:-----------:|:----------:|:--------:|:----------:|-------------|
| **Headless** | ✅ | ✅ | ✅ | ✅ text/json | `wrightty term --headless` |
| **Alacritty** | ✅ | ✅ | — | ✅ SVG | `wrightty term --alacritty` |
| **WezTerm** | ✅ | ✅ | ✅ | — | `wrightty term --bridge-wezterm` |
| **Kitty** | ✅ | ✅ | ✅ | — | `wrightty term --bridge-kitty` |
| **tmux** | ✅ | ✅ | ✅ | — | `wrightty term --bridge-tmux` |
| **Zellij** | ✅ | ✅ | ✅ | — | `wrightty term --bridge-zellij` |
| **Ghostty** | ✅ | ✅ | ✅ | ✅ text | `wrightty term --bridge-ghostty` |
| **foot, GNOME Terminal, etc.** | ✅ | ✅ | ✅ | — | Use with tmux/zellij bridge |

### Adding support for a new terminal

If your terminal has any way to read screen content and send input (CLI, socket, D-Bus, API), a wrightty bridge can be built. See `crates/wrightty-bridge-wezterm/` for a reference implementation.

For terminals with no IPC, pair them with **tmux** or **zellij** and use that bridge.

## Star History

<a href="https://www.star-history.com/?repos=moejay%2Fwrightty&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=moejay/wrightty&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=moejay/wrightty&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=moejay/wrightty&type=date&legend=top-left" />
 </picture>
</a>

## License

MIT
