# wrightty

A CDP-like protocol for terminal automation. Control any terminal emulator programmatically — send keystrokes, read the screen, take screenshots, and run commands over WebSocket JSON-RPC.

Built for AI coding agents that need to interact with terminals the way humans do.

## How it works

Wrightty exposes terminal state over a WebSocket. You connect, send JSON-RPC commands, and get back structured responses. It works in two modes:

**Mode 1 — Headless daemon:** Wrightty spawns its own PTY and runs a virtual terminal (using Alacritty's terminal emulator). No GUI needed. Good for CI, testing, and headless automation.

**Mode 2 — Native emulator:** Your existing terminal (Alacritty, WezTerm) speaks the wrightty protocol directly. You control what's actually on your screen.

```
┌──────────────┐     WebSocket      ┌──────────────────────┐
│  Your agent  │ ◄── JSON-RPC ────► │  Terminal emulator   │
│  or script   │                    │  (Alacritty/WezTerm/ │
│              │                    │   headless daemon)   │
└──────────────┘                    └──────────────────────┘
```

## Quick start

### Python SDK (easiest)

```python
from wrightty import Terminal

term = Terminal.connect()  # connects to ws://127.0.0.1:9420

output = term.run("cargo test")
print(output)

term.wait_for("$")
term.send_keys("Ctrl+c")

svg = term.screenshot("svg")
```

### CLI

```bash
wrightty run "ls -la"
wrightty read
wrightty screenshot -o terminal.svg
wrightty send-keys Ctrl+c
wrightty wait-for "BUILD SUCCESS" --timeout 60
```

### Raw protocol

```bash
# Connect with any WebSocket client
wscat -c ws://127.0.0.1:9420
```

```json
{"jsonrpc":"2.0","id":1,"method":"Screen.getText","params":{"sessionId":"0"}}
```

```json
{"result":{"text":"moe@pop-os:~$ ls\nCargo.toml  src  tests\nmoe@pop-os:~$"}}
```

## Setting up a terminal

### Option A: Alacritty (native, recommended)

Uses a [fork of Alacritty](https://github.com/moejay/alacritty/tree/wrightty-support) with wrightty built in. Zero overhead — reads directly from Alacritty's own terminal state.

```bash
git clone -b wrightty-support https://github.com/moejay/alacritty.git
cd alacritty
cargo build --features wrightty
./target/debug/alacritty --wrightty        # default port 9420
./target/debug/alacritty --wrightty 8080   # custom port
```

Then connect:

```python
from wrightty import Terminal
term = Terminal.connect()  # ws://127.0.0.1:9420
output = term.run("echo hello")
```

### Option B: WezTerm (via bridge)

Works with any WezTerm installation. The bridge translates wrightty protocol calls into `wezterm cli` commands.

```bash
# Start WezTerm normally, then start the bridge:
cargo run -p wrightty-bridge-wezterm

# For flatpak WezTerm:
WEZTERM_CMD="flatpak run --command=wezterm org.wezfurlong.wezterm" \
  cargo run -p wrightty-bridge-wezterm
```

The bridge listens on port 9421:

```python
term = Terminal.connect("ws://127.0.0.1:9421")
```

### Option C: Headless daemon (no GUI)

Spawns a virtual terminal with no window. Uses Alacritty's terminal emulator under the hood. Good for CI and testing.

```bash
cargo run -p wrightty-server
```

```python
# Spawn a new session
term = Terminal.spawn(server_url="ws://127.0.0.1:9420")
output = term.run("echo hello")
term.close()
```

## Python SDK

Install (zero dependencies, uses raw sockets):

```bash
pip install /path/to/wrightty/sdks/python
# or
PYTHONPATH=/path/to/wrightty/sdks/python python3 your_script.py
```

API reference:

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

# Send raw text
term.send_text("echo hello\n")

# Screenshots
svg = term.screenshot("svg")     # str
text = term.screenshot("text")   # str

# Terminal info
cols, rows = term.get_size()
info = term.get_info()

term.close()
```

## CLI

```bash
# Run commands
wrightty run "ls -la"
wrightty run "cargo test" --timeout 120

# Read screen
wrightty read

# Send input
wrightty send-text "echo hello\n"
wrightty send-keys Ctrl+c
wrightty send-keys Escape : w q Enter

# Wait for output
wrightty wait-for "BUILD SUCCESS"
wrightty wait-for "error" --regex

# Screenshots
wrightty screenshot --format svg -o terminal.svg

# Info
wrightty info
wrightty size

# Connect to a different server
wrightty --url ws://127.0.0.1:9421 run "ls"
```

## MCP Server (for Claude, Cursor, etc.)

Wrightty includes an MCP server that exposes terminal control as tools for AI agents.

Add to your Claude/Cursor MCP config:

```json
{
  "mcpServers": {
    "wrightty": {
      "command": "python3",
      "args": ["-m", "wrightty.mcp_server"],
      "env": {
        "PYTHONPATH": "/path/to/wrightty/sdks/python",
        "WRIGHTTY_SOCKET": "ws://127.0.0.1:9420"
      }
    }
  }
}
```

This gives the AI agent these tools:

| Tool | Description |
|------|-------------|
| `run_command` | Run a shell command and return output |
| `read_terminal` | Read the current terminal screen |
| `send_keys` | Send keystrokes (for vim, htop, etc.) |
| `send_text` | Send raw text input |
| `screenshot` | Take an SVG screenshot of the terminal |
| `wait_for_text` | Wait until specific text appears |
| `terminal_info` | Get terminal dimensions and capabilities |

## Protocol

The full protocol specification is in [PROTOCOL.md](PROTOCOL.md).

Quick overview — 6 domains, 16 methods, all over WebSocket JSON-RPC 2.0:

| Domain | Methods |
|--------|---------|
| **Wrightty** | `getInfo` — capability negotiation |
| **Session** | `create`, `destroy`, `list`, `getInfo` |
| **Input** | `sendKeys`, `sendText`, `sendMouse` |
| **Screen** | `getContents`, `getText`, `getScrollback`, `screenshot`, `waitForText`, `waitForCursor` |
| **Terminal** | `resize`, `getSize`, `setColorPalette`, `getModes` |
| **Events** | `subscribe`, `unsubscribe` — screen updates, bell, title change, shell integration |

Example session:

```json
// Create a session (headless daemon mode)
→ {"jsonrpc":"2.0","id":1,"method":"Session.create","params":{"cols":80,"rows":24}}
← {"result":{"sessionId":"abc123"}}

// Send a command
→ {"jsonrpc":"2.0","id":2,"method":"Input.sendText","params":{"sessionId":"abc123","text":"ls\n"}}
← {"result":{}}

// Read the screen
→ {"jsonrpc":"2.0","id":3,"method":"Screen.getText","params":{"sessionId":"abc123"}}
← {"result":{"text":"$ ls\nCargo.toml  src  tests\n$"}}

// Take a screenshot
→ {"jsonrpc":"2.0","id":4,"method":"Screen.screenshot","params":{"sessionId":"abc123","format":"svg"}}
← {"result":{"format":"svg","data":"<svg ...>"}}
```

## Architecture

```
wrightty/
├── crates/
│   ├── wrightty-protocol/        # Protocol types (serde, no logic)
│   ├── wrightty-core/            # Headless terminal engine (alacritty_terminal + PTY)
│   ├── wrightty-server/          # WebSocket daemon (port 9420)
│   ├── wrightty-client/          # Rust client SDK
│   └── wrightty-bridge-wezterm/  # WezTerm bridge (port 9421)
├── sdks/
│   └── python/                   # Python SDK, CLI, MCP server
└── PROTOCOL.md                   # Full protocol specification
```

## License

MIT
