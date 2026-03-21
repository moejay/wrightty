---
name: wrightty
description: Control terminal emulators programmatically via the wrightty protocol. Use when you need to run commands in a real terminal, interact with TUI applications, read terminal screen output, take screenshots, send keystrokes, or automate terminal workflows. Triggers on tasks involving terminal automation, running shell commands through a real PTY, controlling vim/htop/other TUI apps, or capturing terminal output.
license: MIT
metadata:
  author: moejay
  version: "0.1.0"
---

# wrightty — Terminal Automation Protocol

Wrightty lets you control terminal emulators over WebSocket JSON-RPC. You can run commands, read the screen, send keystrokes, and take screenshots — just like a human would interact with a terminal, but programmatically.

## When to use

- Running shell commands in a real terminal (not subprocess) where you need the full PTY environment
- Interacting with TUI applications (vim, htop, less, etc.)
- Reading terminal screen output with colors and formatting
- Taking terminal screenshots (SVG)
- Automating interactive terminal workflows
- Waiting for specific output to appear before proceeding

## Setup

Wrightty needs a terminal backend. Pick one:

**Alacritty (native, zero overhead):**
```bash
git clone -b wrightty-support https://github.com/moejay/alacritty.git
cd alacritty && cargo build --features wrightty
./target/debug/alacritty --wrightty  # listens on ws://127.0.0.1:9420
```

**WezTerm (via bridge):**
```bash
# Start WezTerm normally, then:
cargo run -p wrightty-bridge-wezterm  # listens on ws://127.0.0.1:9421

# For flatpak:
WEZTERM_CMD="flatpak run --command=wezterm org.wezfurlong.wezterm" cargo run -p wrightty-bridge-wezterm
```

**Headless daemon (no GUI):**
```bash
cargo run -p wrightty-server  # listens on ws://127.0.0.1:9420
```

## Python SDK

The SDK has zero external dependencies.

```python
from wrightty import Terminal

# Connect to a running terminal
term = Terminal.connect()                          # ws://127.0.0.1:9420
term = Terminal.connect("ws://127.0.0.1:9421")    # WezTerm bridge

# Run a command and get output
output = term.run("cargo test", timeout=120)

# Read the current screen
screen = term.read_screen()

# Wait for text to appear
term.wait_for("tests passed", timeout=60)
term.wait_for(r"error\[\w+\]", regex=True)

# Send keystrokes (TUI apps)
term.send_keys("Escape", ":", "w", "q", "Enter")  # vim: save and quit
term.send_keys("Ctrl+c")                           # interrupt
term.send_keys("ArrowUp", "Enter")                 # repeat last command

# Screenshots
svg = term.screenshot("svg")

# Terminal info
cols, rows = term.get_size()

term.close()
```

Install: `pip install /path/to/wrightty/sdks/python` or set `PYTHONPATH=/path/to/wrightty/sdks/python`.

## CLI

```bash
wrightty run "ls -la"                              # run command, print output
wrightty run "cargo test" --timeout 120            # with timeout
wrightty read                                       # dump current screen
wrightty send-text "echo hello\n"                   # send raw text
wrightty send-keys Ctrl+c                           # send keystroke
wrightty send-keys Escape : w q Enter               # vim: :wq
wrightty wait-for "BUILD SUCCESS" --timeout 60      # block until text appears
wrightty screenshot --format svg -o terminal.svg    # take screenshot
wrightty info                                       # server capabilities
wrightty size                                       # terminal dimensions
wrightty --url ws://127.0.0.1:9421 run "ls"         # connect to different server
```

## MCP Server

Add to your MCP config (Claude, Cursor, etc.):

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

Available MCP tools:

| Tool | Description |
|------|-------------|
| `run_command` | Run a shell command, return output |
| `read_terminal` | Read current screen text |
| `send_keys` | Send keystrokes (`["Ctrl+c"]`, `["Escape", ":", "q", "Enter"]`) |
| `send_text` | Send raw text (use `\n` for newline) |
| `screenshot` | Terminal screenshot as SVG |
| `wait_for_text` | Block until pattern appears on screen |
| `terminal_info` | Terminal dimensions and capabilities |

## Protocol reference

WebSocket JSON-RPC 2.0 on `ws://127.0.0.1:9420`. Full spec: [PROTOCOL.md](https://github.com/moejay/wrightty/blob/main/PROTOCOL.md).

### Core methods

**Run a command:**
```json
{"jsonrpc":"2.0","id":1,"method":"Input.sendText","params":{"sessionId":"0","text":"ls -la\n"}}
```

**Read the screen:**
```json
{"jsonrpc":"2.0","id":2,"method":"Screen.getText","params":{"sessionId":"0"}}
// → {"result":{"text":"$ ls -la\ntotal 140\ndrwxr-xr-x  8 user user 4096 ...\n$"}}
```

**Send keystrokes:**
```json
{"jsonrpc":"2.0","id":3,"method":"Input.sendKeys","params":{"sessionId":"0","keys":["Ctrl+c"]}}
```

**Take a screenshot:**
```json
{"jsonrpc":"2.0","id":4,"method":"Screen.screenshot","params":{"sessionId":"0","format":"svg"}}
// → {"result":{"format":"svg","data":"<svg ...>"}}
```

**Get terminal size:**
```json
{"jsonrpc":"2.0","id":5,"method":"Terminal.getSize","params":{"sessionId":"0"}}
// → {"result":{"cols":120,"rows":40}}
```

**Server capabilities:**
```json
{"jsonrpc":"2.0","id":6,"method":"Wrightty.getInfo","params":{}}
```

### Key names for sendKeys

Single characters: `a`, `1`, `/`, `.`
Special keys: `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `PageUp`, `PageDown`, `F1`-`F12`
Modifiers: `Ctrl+c`, `Alt+x`, `Shift+Tab`

### Session management (headless daemon only)

```json
// Create session
{"jsonrpc":"2.0","id":1,"method":"Session.create","params":{"cols":80,"rows":24}}
// → {"result":{"sessionId":"abc-123"}}

// List sessions
{"jsonrpc":"2.0","id":2,"method":"Session.list","params":{}}

// Destroy session
{"jsonrpc":"2.0","id":3,"method":"Session.destroy","params":{"sessionId":"abc-123"}}
```

Native emulator mode uses `"sessionId":"0"` for the active terminal. Session create/destroy is not supported — the emulator manages its own sessions.

## Tips

- Use `term.run()` for most commands — it sends the command and waits for the prompt to return
- Use `term.send_keys()` for interactive/TUI apps where you need specific keystrokes
- Use `term.wait_for()` before reading output from slow commands (builds, tests)
- The default prompt detection pattern is `[$#>%]\s*$` — override with `term.set_prompt_pattern()` if your prompt is different
- Screenshots are SVG by default — good for rendering in browsers and docs, and readable by vision models
