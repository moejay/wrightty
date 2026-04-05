# wrightty (Python SDK)

Python SDK for the [Wrightty](https://github.com/moejay/wrightty) terminal automation protocol.

Control any terminal emulator programmatically — send keystrokes, read the screen, take screenshots, and run commands over WebSocket JSON-RPC.

## Install

```bash
pip install wrightty
# with MCP server:
pip install wrightty[mcp]
```

You also need a wrightty server running. Install it via:

```bash
curl -fsSL https://raw.githubusercontent.com/moejay/wrightty/main/install.sh | sh
# or
cargo install wrightty-cli
```

## Quick start

```python
from wrightty import Terminal

# Connect to a running wrightty server (auto-discovers)
term = Terminal.connect()

# Run a command and get its output
output = term.run("cargo test")
print(output)

# Read the screen
screen = term.read_screen()

# Send keystrokes
term.send_keys("Ctrl+c")
term.send_keys("Escape", ":", "w", "q", "Enter")

# Wait for text to appear
term.wait_for("BUILD SUCCESS", timeout=60)

# Screenshots
svg = term.screenshot("svg")

# Recording
rec_id = term.start_session_recording()
term.run("make build")
result = term.stop_session_recording(rec_id)
open("build.cast", "w").write(result["data"])  # asciinema play

term.close()
```

## Starting a server

First start a wrightty terminal server:

```bash
wrightty term --headless                 # virtual PTY, no GUI
wrightty term --bridge-tmux              # attach to tmux
wrightty term --bridge-wezterm           # attach to WezTerm
wrightty term --bridge-kitty             # attach to Kitty
wrightty term --bridge-zellij            # attach to Zellij
wrightty term --bridge-ghostty           # attach to Ghostty
```

Then connect from Python as shown above.

## MCP server (for Claude, Cursor, etc.)

```bash
pip install wrightty[mcp]
```

Add to your MCP config:

```json
{
  "mcpServers": {
    "wrightty": {
      "command": "python3",
      "args": ["-m", "wrightty.mcp_server"],
      "env": { "WRIGHTTY_SOCKET": "ws://127.0.0.1:9420" }
    }
  }
}
```

See the [main repository](https://github.com/moejay/wrightty) for full docs.

## License

MIT
