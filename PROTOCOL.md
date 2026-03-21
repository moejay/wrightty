# Wrightty Protocol Specification

**Version:** 0.1.0-draft
**Transport:** WebSocket + JSON-RPC 2.0
**Default endpoint:** `ws://127.0.0.1:9420`

---

## 1. Transport

All communication happens over a single WebSocket connection using [JSON-RPC 2.0](https://www.jsonrpc.org/specification).

- **Requests** flow client → server
- **Responses** flow server → client (matched by `id`)
- **Notifications/Events** flow server → client (no `id`, triggered by subscriptions)
- **Batching** is supported per JSON-RPC 2.0 spec

### 1.1 Connection Lifecycle

```
Client                          Server
  │                               │
  ├── WebSocket connect ─────────►│
  │                               │
  ├── Wrightty.getInfo ──────────►│  (optional handshake)
  │◄── { version, capabilities } ─┤
  │                               │
  ├── Session.create ────────────►│
  │◄── { sessionId } ─────────────┤
  │                               │
  ├── Events.subscribe ──────────►│  (start receiving events)
  │◄── subscription confirmed ────┤
  │                               │
  │  ... interact ...             │
  │                               │
  ├── Session.destroy ───────────►│
  │◄── ok ─────────────────────────┤
  │                               │
  └── WebSocket close ───────────►│
```

### 1.2 Discovery

When an emulator or daemon starts with wrightty support enabled, it sets:

```
WRIGHTTY_SOCKET=ws://127.0.0.1:9420
```

Clients check this env var to auto-connect. If multiple sessions exist (e.g., emulator with tabs), the client uses `Session.list` to enumerate them.

---

## 2. Domains

Methods are namespaced as `Domain.methodName`.

### 2.1 Wrightty (meta)

| Method | Description |
|--------|-------------|
| `Wrightty.getInfo` | Server metadata and capability negotiation |

#### `Wrightty.getInfo`

**Params:** none

**Result:**
```json
{
  "version": "0.1.0",
  "implementation": "wrightty-server",
  "capabilities": {
    "screenshot": ["text", "ansi", "json", "svg", "png"],
    "maxSessions": 64,
    "supportsResize": true,
    "supportsScrollback": true,
    "supportsMouse": false,
    "supportsSessionCreate": true,
    "supportsColorPalette": true,
    "supportsRawOutput": true,
    "supportsShellIntegration": true,
    "events": [
      "Screen.updated", "Session.output", "Session.exited",
      "Terminal.bell", "Terminal.titleChanged", "Terminal.cwdChanged",
      "Terminal.alternateScreen", "Terminal.cursorChanged",
      "Shell.promptStart", "Shell.commandStart", "Shell.outputStart",
      "Shell.commandFinished", "Terminal.notification",
      "Terminal.clipboardSet", "Terminal.modeChanged",
      "Terminal.progressChanged"
    ]
  }
}
```

The `capabilities.events` array advertises which event types this implementation can emit. Clients should check this before subscribing.

---

### 2.2 Session

Manages terminal session lifecycle. In daemon mode, each session is a PTY + shell process. In native emulator mode, sessions map to tabs/panes.

| Method | Description |
|--------|-------------|
| `Session.create` | Spawn a new terminal session |
| `Session.destroy` | Kill a session |
| `Session.list` | List active sessions |
| `Session.getInfo` | Get info about a specific session |

#### `Session.create`

**Params:**
```json
{
  "shell": "/bin/bash",       // optional, default: user's $SHELL or /bin/sh
  "args": ["--norc"],         // optional, shell arguments
  "cols": 80,                 // optional, default: 80
  "rows": 24,                 // optional, default: 24
  "env": {                    // optional, additional env vars (merged with inherited env)
    "TERM": "xterm-256color",
    "LANG": "en_US.UTF-8"
  },
  "cwd": "/home/user/project" // optional, working directory
}
```

**Result:**
```json
{
  "sessionId": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Notes:**
- `TERM` defaults to `xterm-256color` if not specified
- In native emulator mode, `shell`/`args`/`env`/`cwd` may be ignored (session already exists)
- The session is ready for input immediately after this returns

#### `Session.destroy`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "signal": "SIGTERM"          // optional, default: SIGTERM then SIGKILL after 5s
}
```

**Result:**
```json
{
  "exitCode": 0               // null if process was killed
}
```

#### `Session.list`

**Params:** none

**Result:**
```json
{
  "sessions": [
    {
      "sessionId": "a1b2c3d4...",
      "title": "bash",
      "cwd": "/home/user/project",
      "cols": 80,
      "rows": 24,
      "pid": 12345,
      "running": true,
      "alternateScreen": false
    }
  ]
}
```

#### `Session.getInfo`

**Params:**
```json
{
  "sessionId": "a1b2c3d4..."
}
```

**Result:** Same shape as one entry in `Session.list.sessions`.

---

### 2.3 Input

Send keystrokes and text to a session.

| Method | Description |
|--------|-------------|
| `Input.sendKeys` | Send structured key events |
| `Input.sendText` | Send raw text (no key interpretation) |
| `Input.sendMouse` | Send mouse events (if supported) |

#### `Input.sendKeys`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "keys": [
    { "key": "Char", "char": "l", "modifiers": [] },
    { "key": "Char", "char": "s", "modifiers": [] },
    { "key": "Enter" }
  ]
}
```

**Result:** `{}`

**Key types:**

| key | Additional fields | Example |
|-----|-------------------|---------|
| `"Char"` | `char`: single character | `{ "key": "Char", "char": "a" }` |
| `"Enter"` | — | `{ "key": "Enter" }` |
| `"Tab"` | — | `{ "key": "Tab" }` |
| `"Backspace"` | — | |
| `"Delete"` | — | |
| `"Escape"` | — | |
| `"ArrowUp"` | — | |
| `"ArrowDown"` | — | |
| `"ArrowLeft"` | — | |
| `"ArrowRight"` | — | |
| `"Home"` | — | |
| `"End"` | — | |
| `"PageUp"` | — | |
| `"PageDown"` | — | |
| `"Insert"` | — | |
| `"F"` | `n`: 1-24 | `{ "key": "F", "n": 5 }` |

**Modifiers** (optional array on any key):
```json
{ "key": "Char", "char": "c", "modifiers": ["Ctrl"] }
{ "key": "ArrowUp", "modifiers": ["Shift", "Alt"] }
```

Valid modifiers: `"Ctrl"`, `"Alt"`, `"Shift"`, `"Meta"`

**Shorthand:** For simple cases, a string shorthand is accepted in the `keys` array:

```json
{
  "keys": ["h", "e", "l", "l", "o", "Enter", "Ctrl+c"]
}
```

Where:
- Single character → `Char` key
- Named key → looked up from the key type table
- `Mod+key` → modifier combination

#### `Input.sendText`

Sends raw text bytes to the PTY. No key interpretation — newlines are `\n`, not Enter key sequences. Useful for pasting.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "text": "echo hello world\n"
}
```

**Result:** `{}`

#### `Input.sendMouse`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "event": "press",         // "press", "release", "move", "scroll"
  "button": "left",         // "left", "right", "middle", "none" (for move)
  "row": 5,
  "col": 12,
  "modifiers": []           // optional
}
```

For scroll: `"button"` is `"scrollUp"` or `"scrollDown"`.

**Result:** `{}`

---

### 2.4 Screen

Read terminal screen state.

| Method | Description |
|--------|-------------|
| `Screen.getContents` | Full cell grid with attributes |
| `Screen.getText` | Plain text extraction |
| `Screen.getScrollback` | Read scrollback buffer |
| `Screen.screenshot` | Render to various formats |
| `Screen.waitForText` | Block until text appears |
| `Screen.waitForCursor` | Block until cursor at position |

#### `Screen.getContents`

Returns the full cell grid with styling attributes.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "region": {               // optional, default: entire visible screen
    "top": 0,
    "left": 0,
    "bottom": 23,           // inclusive
    "right": 79             // inclusive
  }
}
```

**Result:**
```json
{
  "rows": 24,
  "cols": 80,
  "cursor": {
    "row": 5,
    "col": 12,
    "visible": true,
    "shape": "block"        // "block", "underline", "bar"
  },
  "cells": [
    [
      {
        "char": "$",
        "width": 1,          // 1 for normal, 2 for wide (CJK), 0 for continuation
        "fg": { "r": 255, "g": 255, "b": 255 },
        "bg": { "r": 0, "g": 0, "b": 0 },
        "attrs": {
          "bold": false,
          "italic": false,
          "underline": "none",   // "none", "single", "double", "curly", "dotted", "dashed"
          "underlineColor": null, // optional RGB, null = use fg
          "strikethrough": false,
          "dim": false,
          "blink": false,
          "reverse": false,
          "hidden": false
        },
        "hyperlink": null    // URL string if OSC 8 hyperlink
      }
    ]
  ],
  "alternateScreen": false
}
```

**Notes:**
- `cells` is a 2D array: `cells[row][col]`
- Wide characters: the first cell has `width: 2`, the next cell has `width: 0` (continuation/spacer)
- Colors are always resolved to RGB (indexed/named colors resolved against the active palette)

#### `Screen.getText`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "region": null,            // optional, same as getContents
  "trimTrailingWhitespace": true  // optional, default: true
}
```

**Result:**
```json
{
  "text": "$ echo hello\nhello\n$ _"
}
```

Rows separated by `\n`. Trailing whitespace on each row is trimmed by default.

#### `Screen.getScrollback`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "lines": 100,              // number of scrollback lines to retrieve
  "offset": 0                // optional, 0 = most recent scrollback line
}
```

**Result:**
```json
{
  "lines": [
    { "text": "previous output line 1", "lineNumber": -100 },
    { "text": "previous output line 2", "lineNumber": -99 }
  ],
  "totalScrollback": 5000
}
```

`lineNumber` is negative, counting up toward 0 (the first visible row).

#### `Screen.screenshot`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "format": "png",           // "text", "ansi", "json", "svg", "png"
  "theme": null,              // optional, color theme override
  "font": {                   // optional, for svg/png
    "family": "JetBrains Mono",
    "size": 14
  }
}
```

**Result:**
```json
{
  "format": "png",
  "data": "iVBORw0KGgo...",  // base64 for png, raw string for text/ansi/svg/json
  "width": 960,               // pixels, only for png/svg
  "height": 480
}
```

**Format details:**
- `text` — plain text, same as `Screen.getText`
- `ansi` — text with ANSI escape codes preserved (can be printed to another terminal)
- `json` — same as `Screen.getContents` response
- `svg` — terminal rendered as SVG with colors, font, background
- `png` — rasterized SVG

#### `Screen.waitForText`

Blocks until the specified text/pattern appears on screen, or timeout.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "pattern": "\\$\\s*$",     // text or regex
  "isRegex": true,            // default: false (plain text match)
  "region": null,              // optional, limit search to region
  "timeout": 5000,            // milliseconds, default: 30000
  "interval": 100             // optional, polling interval in ms, default: 100
}
```

**Result:**
```json
{
  "found": true,
  "matches": [
    {
      "text": "$ ",
      "row": 5,
      "col": 0,
      "length": 2
    }
  ],
  "elapsed": 1234             // ms waited
}
```

If `found` is `false`, the method returns (not an error) after timeout with an empty `matches` array.

#### `Screen.waitForCursor`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "row": 5,                   // optional, null = any row
  "col": 12,                  // optional, null = any col
  "timeout": 5000
}
```

**Result:**
```json
{
  "cursor": { "row": 5, "col": 12, "visible": true, "shape": "block" },
  "elapsed": 500
}
```

---

### 2.5 Terminal

Control terminal properties.

| Method | Description |
|--------|-------------|
| `Terminal.resize` | Change dimensions |
| `Terminal.getSize` | Query dimensions |
| `Terminal.setColorPalette` | Override color palette |
| `Terminal.getColorPalette` | Read current palette |
| `Terminal.getModes` | Query active terminal modes |

#### `Terminal.resize`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "cols": 120,
  "rows": 40
}
```

**Result:** `{}`

The underlying PTY and process receive `SIGWINCH`.

#### `Terminal.getSize`

**Params:**
```json
{
  "sessionId": "a1b2c3d4..."
}
```

**Result:**
```json
{
  "cols": 120,
  "rows": 40
}
```

#### `Terminal.setColorPalette`

Override the 256-color palette used for color resolution.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "palette": {
    "0":  { "r": 0,   "g": 0,   "b": 0   },
    "1":  { "r": 204, "g": 0,   "b": 0   },
    "15": { "r": 255, "g": 255, "b": 255 },
    "foreground": { "r": 200, "g": 200, "b": 200 },
    "background": { "r": 30,  "g": 30,  "b": 30  },
    "cursor":     { "r": 255, "g": 255, "b": 0   }
  }
}
```

Only specified entries are overridden; unspecified entries keep their defaults. Affects `Screen.getContents` color resolution and screenshots.

**Result:** `{}`

#### `Terminal.getModes`

Query the active terminal modes. Useful for understanding what the running application has configured.

**Params:**
```json
{
  "sessionId": "a1b2c3d4..."
}
```

**Result:**
```json
{
  "cursorKeyMode": "normal",      // "normal" or "application" (DECCKM)
  "keypadMode": "numeric",        // "numeric" or "application" (DECKPAM/DECKPNM)
  "alternateScreen": false,        // modes 47/1047/1049
  "bracketedPaste": true,          // mode 2004
  "mouseTracking": "none",         // "none", "x10", "normal", "cellMotion", "allMotion"
  "mouseEncoding": "sgr",          // "default", "utf8", "sgr", "urxvt", "sgrPixel"
  "focusReporting": false,         // mode 1004
  "cursorVisible": true,           // DECTCEM, mode 25
  "cursorStyle": "block",          // "block", "underline", "bar" (and blinking variants)
  "autoWrap": true,                // DECAWM, mode 7
  "reverseVideo": false,           // DECSCNM, mode 5
  "originMode": false,             // DECOM, mode 6
  "synchronizedOutput": false      // mode 2026
}
```

---

### 2.6 Recording

Record terminal sessions, actions, and screen captures.

| Method | Description |
|--------|-------------|
| `Recording.startSession` | Start recording raw PTY I/O (asciicast format) |
| `Recording.stopSession` | Stop session recording and return the data |
| `Recording.startActions` | Start recording wrightty API calls as a replayable script |
| `Recording.stopActions` | Stop action recording and return the script |
| `Recording.captureScreen` | Capture a single screen frame (append to a screen recording) |
| `Recording.startScreenCapture` | Start periodic screen capture |
| `Recording.stopScreenCapture` | Stop screen capture and return all frames |

#### `Recording.startSession`

Begin recording all PTY output bytes with timestamps. Compatible with [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/) format — can be played back with `asciinema play`.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "includeInput": true          // optional, also record input events (default: false)
}
```

**Result:**
```json
{
  "recordingId": "rec_001"
}
```

#### `Recording.stopSession`

Stop a session recording and return the asciicast data.

**Params:**
```json
{
  "recordingId": "rec_001"
}
```

**Result:**
```json
{
  "format": "asciicast-v2",
  "data": "{\"version\":2,\"width\":80,\"height\":24,...}\n[0.5,\"o\",\"$ \"]\n[1.2,\"o\",\"hello\\r\\n\"]\n",
  "duration": 12.5,
  "events": 42
}
```

The `data` field contains the full asciicast v2 file as a string. Each line after the header is `[timestamp, type, data]`:
- `"o"` = output (PTY → screen)
- `"i"` = input (user → PTY, only if `includeInput` was true)

#### `Recording.startActions`

Begin recording all wrightty API calls (sendKeys, sendText, etc.) as a replayable script. Like Playwright's codegen.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "format": "python"           // "python", "json", or "cli"
}
```

**Result:**
```json
{
  "recordingId": "rec_002"
}
```

#### `Recording.stopActions`

Stop action recording and return the generated script.

**Params:**
```json
{
  "recordingId": "rec_002"
}
```

**Result (format=python):**
```json
{
  "format": "python",
  "data": "from wrightty import Terminal\n\nterm = Terminal.connect()\nterm.send_text('ls -la\\n')\nterm.wait_for('$')\nterm.send_keys('Ctrl+c')\nterm.close()\n",
  "actions": 3,
  "duration": 8.2
}
```

**Result (format=json):**
```json
{
  "format": "json",
  "data": [
    { "time": 0.0, "method": "Input.sendText", "params": { "text": "ls -la\n" } },
    { "time": 2.1, "method": "Screen.waitForText", "params": { "pattern": "$" } },
    { "time": 5.5, "method": "Input.sendKeys", "params": { "keys": ["Ctrl+c"] } }
  ],
  "actions": 3,
  "duration": 8.2
}
```

**Result (format=cli):**
```json
{
  "format": "cli",
  "data": "wrightty send-text 'ls -la\\n'\nwrightty wait-for '$'\nwrightty send-keys Ctrl+c\n",
  "actions": 3,
  "duration": 8.2
}
```

#### `Recording.captureScreen`

Capture a single screen frame. Can be called at any time. Frames can be collected into a GIF/video later.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "format": "svg"              // "svg", "text", or "png"
}
```

**Result:**
```json
{
  "frameId": 0,
  "timestamp": 1679000000000,
  "format": "svg",
  "data": "<svg ...>"
}
```

#### `Recording.startScreenCapture`

Start automatically capturing screen frames at a fixed interval.

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "intervalMs": 500,           // capture every 500ms (default: 1000)
  "format": "svg"              // "svg", "text", or "png"
}
```

**Result:**
```json
{
  "recordingId": "rec_003"
}
```

#### `Recording.stopScreenCapture`

Stop screen capture and return all captured frames.

**Params:**
```json
{
  "recordingId": "rec_003"
}
```

**Result:**
```json
{
  "frames": [
    { "frameId": 0, "timestamp": 0, "data": "<svg ...>" },
    { "frameId": 1, "timestamp": 500, "data": "<svg ...>" },
    { "frameId": 2, "timestamp": 1000, "data": "<svg ...>" }
  ],
  "duration": 1.0,
  "frameCount": 3,
  "format": "svg"
}
```

---

### 2.7 Events

Events use a unified subscription model. Instead of per-event subscribe/unsubscribe methods, clients use a single `Events.subscribe` method and specify which event types they want.

#### `Events.subscribe`

**Params:**
```json
{
  "sessionId": "a1b2c3d4...",
  "events": ["Screen.updated", "Session.exited", "Shell.commandFinished"],
  "options": {
    "screenDebounceMs": 16    // optional, debounce for Screen.updated, default: 16
  }
}
```

`events` accepts `"*"` as a wildcard to subscribe to all available events:
```json
{ "sessionId": "abc123", "events": ["*"] }
```

**Result:**
```json
{
  "subscriptionId": "sub_001",
  "subscribedEvents": ["Screen.updated", "Session.exited", "Shell.commandFinished"]
}
```

`subscribedEvents` confirms which events were actually subscribed (may be a subset if some are not supported).

#### `Events.unsubscribe`

**Params:**
```json
{
  "subscriptionId": "sub_001"
}
```

**Result:** `{}`

---

## 3. Event Catalog

All events share a common envelope:

```json
{
  "jsonrpc": "2.0",
  "method": "Events.event",
  "params": {
    "subscriptionId": "sub_001",
    "event": "<EventType>",
    "sessionId": "a1b2c3d4...",
    "timestamp": 1679000000000,
    "data": { ... }
  }
}
```

Events are organized into tiers. Tier 1 events MUST be supported by all implementations. Tier 2 and 3 are optional and advertised via `capabilities.events`.

---

### Tier 1 — Core Events (required)

#### `Screen.updated`

Screen content changed. Debounced to avoid flooding (default 16ms / 60fps).

```json
{
  "dirtyRegion": {            // bounding box of changed area, null if unknown
    "top": 0, "left": 0, "bottom": 5, "right": 79
  }
}
```

The event does NOT include screen contents. Clients call `Screen.getText` or `Screen.getContents` if they need the data.

#### `Session.exited`

The shell/process in this session has exited.

```json
{
  "exitCode": 0,             // null if killed by signal
  "signal": null              // e.g., "SIGTERM", "SIGKILL"
}
```

#### `Session.output`

Raw PTY output bytes, base64-encoded. Useful for session recording/replay (asciicast format, etc.).

```json
{
  "data": "G1szMW1oZWxsbw=="  // base64-encoded bytes
}
```

#### `Terminal.bell`

BEL character (0x07) received.

```json
{}
```

#### `Terminal.titleChanged`

Window title changed via OSC 0 or OSC 2.

```json
{
  "title": "vim README.md",
  "iconName": null             // OSC 1 icon name, if set separately
}
```

#### `Terminal.cwdChanged`

Working directory changed. Detected via OSC 7 (`file://host/path`), OSC 9;9, or OSC 1337;CurrentDir.

```json
{
  "cwd": "/home/user/project",
  "uri": "file://hostname/home/user/project"  // original OSC 7 URI if available
}
```

#### `Terminal.alternateScreen`

Application entered or exited the alternate screen buffer (modes 47/1047/1049). Critical for detecting TUI app launch/exit.

```json
{
  "active": true              // true = entered, false = exited
}
```

#### `Terminal.cursorChanged`

Cursor visibility or style changed (DECTCEM mode 25, CSI N SP q).

```json
{
  "visible": true,
  "shape": "bar",             // "block", "underline", "bar"
  "blinking": true
}
```

---

### Tier 2 — Shell Integration Events (optional, requires OSC 133)

These events require the shell to emit OSC 133 sequences (bash, zsh, fish with shell integration enabled). They provide semantic understanding of what's happening in the terminal.

#### `Shell.promptStart`

Shell has begun rendering its prompt (OSC 133;A).

```json
{}
```

#### `Shell.commandStart`

User has finished the prompt; command input area begins (OSC 133;B).

```json
{}
```

#### `Shell.outputStart`

Shell is executing the command; output begins (OSC 133;C).

```json
{
  "command": null             // the command text, if extractable from the screen between promptStart and outputStart
}
```

#### `Shell.commandFinished`

Command has completed (OSC 133;D).

```json
{
  "exitCode": 0
}
```

**Why these matter:** An AI agent can use `Shell.commandFinished` to know when a command is done instead of fragile heuristics like "wait for the prompt pattern." A test framework can assert on exit codes without parsing output.

---

### Tier 2 — Terminal Notifications

#### `Terminal.notification`

A desktop notification was requested by the application. Covers:
- OSC 9 (iTerm2-style simple notification)
- OSC 99 (kitty rich notifications with buttons, icons, urgency)
- OSC 777 (rxvt-unicode notifications)

```json
{
  "title": "Build complete",
  "body": "Project compiled successfully",
  "urgency": "normal",        // "low", "normal", "critical"
  "source": "osc9"            // "osc9", "osc99", "osc777"
}
```

#### `Terminal.clipboardSet`

Application set clipboard content via OSC 52.

```json
{
  "selection": "clipboard",    // "clipboard", "primary", "secondary", "select"
  "text": "copied text",       // decoded content (NOT base64)
  "base64": "Y29waWVkIHRleHQ="  // raw base64 as sent by app
}
```

**Security note:** In daemon mode, wrightty intercepts OSC 52 and emits this event instead of modifying the system clipboard. The client decides whether to honor it.

#### `Terminal.progressChanged`

Taskbar/tab progress indicator changed (OSC 9;4 — ConEmu/iTerm2/Windows Terminal).

```json
{
  "state": "value",            // "none", "value", "error", "indeterminate", "warning"
  "percent": 75                // 0-100, present when state is "value"
}
```

---

### Tier 2 — Mode Changes

#### `Terminal.modeChanged`

A terminal mode was set or reset. Rather than individual events per mode, this provides a unified notification with the mode name.

```json
{
  "mode": "bracketedPaste",    // see mode names below
  "enabled": true
}
```

**Mode names:**
| Mode name | DEC mode | Description |
|-----------|----------|-------------|
| `cursorKeyMode` | 1 (DECCKM) | Application vs normal cursor keys |
| `alternateScreen` | 1049 | Alternate screen buffer (also fires `Terminal.alternateScreen`) |
| `bracketedPaste` | 2004 | Wrap pastes in escape sequences |
| `mouseTracking` | 1000-1003 | Mouse event reporting enabled |
| `focusReporting` | 1004 | Focus in/out reporting |
| `cursorVisible` | 25 (DECTCEM) | Cursor show/hide |
| `autoWrap` | 7 (DECAWM) | Auto-wrap at margin |
| `reverseVideo` | 5 (DECSCNM) | Reverse video mode |
| `synchronizedOutput` | 2026 | Buffered rendering |

---

### Tier 3 — Extended Events (optional, emulator-specific)

#### `Terminal.focusChanged`

Terminal window/pane gained or lost focus (requires mode 1004 to be enabled by the app).

```json
{
  "focused": true
}
```

#### `Terminal.hyperlinkHovered`

An OSC 8 hyperlink region is under the cursor (native emulator mode only).

```json
{
  "url": "https://example.com",
  "id": "link-1",             // hyperlink ID if specified
  "row": 5,
  "colStart": 10,
  "colEnd": 35
}
```

#### `Terminal.imageDisplayed`

An inline image was displayed via Sixel, kitty graphics protocol, or iTerm2 inline image.

```json
{
  "protocol": "kitty",        // "sixel", "kitty", "iterm2"
  "row": 5,
  "col": 0,
  "widthCells": 40,
  "heightCells": 20,
  "widthPixels": 640,
  "heightPixels": 480,
  "imageId": null              // kitty image ID if applicable
}
```

#### `Terminal.colorPaletteChanged`

Application changed terminal colors via OSC 4, OSC 10-19, or pushed/popped the color stack.

```json
{
  "changes": {
    "foreground": { "r": 200, "g": 200, "b": 200 },
    "4": { "r": 0, "g": 0, "b": 255 }
  },
  "source": "osc4"            // "osc4", "osc10", "push", "pop"
}
```

#### `Terminal.remoteHostChanged`

Remote host info changed (OSC 1337;RemoteHost). Useful for detecting SSH sessions.

```json
{
  "user": "deploy",
  "host": "prod-server-01"
}
```

#### `Terminal.userVariableSet`

iTerm2 user variable set (OSC 1337;SetUserVar).

```json
{
  "key": "gitBranch",
  "value": "main"
}
```

#### `Terminal.fileTransfer`

Kitty file transfer event (OSC 5113).

```json
{
  "action": "receive",        // "send", "receive", "progress", "complete", "error"
  "filename": "data.csv",
  "progress": 0.75,           // 0.0-1.0 for progress events
  "error": null
}
```

---

## 4. Error Codes

Standard JSON-RPC 2.0 errors plus protocol-specific codes.

| Code | Name | Description |
|------|------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid request | Not a valid JSON-RPC request |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Missing or invalid parameters |
| -32603 | Internal error | Server error |
| 1001 | SessionNotFound | No session with the given ID |
| 1002 | SessionDestroyed | Session was already destroyed |
| 1003 | WaitTimeout | `waitForText`/`waitForCursor` timed out (also returns normally with `found: false`) |
| 1004 | InvalidPattern | Regex pattern failed to compile |
| 1005 | SpawnFailed | Failed to spawn PTY/shell process |
| 1006 | NotSupported | Method or event not supported by this implementation |
| 1007 | MaxSessionsReached | Cannot create more sessions |
| 1008 | SubscriptionNotFound | Invalid subscription ID |

**Error response example:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "error": {
    "code": 1001,
    "message": "Session not found",
    "data": { "sessionId": "nonexistent-id" }
  }
}
```

---

## 5. Capability Negotiation

Not all implementations support every method or event. Native emulator integrations may only support a subset.

### 5.1 Capability Flags

Returned in `Wrightty.getInfo`:

```json
{
  "capabilities": {
    "screenshot": ["text", "ansi", "json"],
    "maxSessions": 1,
    "supportsResize": true,
    "supportsScrollback": true,
    "supportsMouse": false,
    "supportsColorPalette": false,
    "supportsSessionCreate": true,
    "supportsRawOutput": true,
    "supportsShellIntegration": true,
    "events": [
      "Screen.updated", "Session.exited", "Terminal.bell",
      "Terminal.titleChanged", "Terminal.cwdChanged"
    ]
  }
}
```

The `events` array is the authoritative list of subscribable event types. If an event type is not listed, subscribing to it will silently exclude it from the subscription (reflected in the `subscribedEvents` response).

### 5.2 Graceful Degradation

If a client calls an unsupported method, the server returns error code `1006 NotSupported`. Clients should check capabilities first and fall back accordingly.

---

## 6. Examples

### 6.1 Run a command and read output

```json
// 1. Create session
→ {"jsonrpc":"2.0","id":1,"method":"Session.create","params":{"cols":80,"rows":24}}
← {"jsonrpc":"2.0","id":1,"result":{"sessionId":"abc123"}}

// 2. Wait for shell prompt
→ {"jsonrpc":"2.0","id":2,"method":"Screen.waitForText","params":{"sessionId":"abc123","pattern":"\\$","isRegex":true,"timeout":5000}}
← {"jsonrpc":"2.0","id":2,"result":{"found":true,"matches":[{"text":"$","row":0,"col":14,"length":1}],"elapsed":120}}

// 3. Type a command
→ {"jsonrpc":"2.0","id":3,"method":"Input.sendKeys","params":{"sessionId":"abc123","keys":["e","c","h","o"," ","h","e","l","l","o","Enter"]}}
← {"jsonrpc":"2.0","id":3,"result":{}}

// 4. Wait for output
→ {"jsonrpc":"2.0","id":4,"method":"Screen.waitForText","params":{"sessionId":"abc123","pattern":"hello","timeout":5000}}
← {"jsonrpc":"2.0","id":4,"result":{"found":true,"matches":[{"text":"hello","row":1,"col":0,"length":5}],"elapsed":50}}

// 5. Read the screen
→ {"jsonrpc":"2.0","id":5,"method":"Screen.getText","params":{"sessionId":"abc123"}}
← {"jsonrpc":"2.0","id":5,"result":{"text":"user@host:~$ echo hello\nhello\nuser@host:~$"}}

// 6. Take a screenshot
→ {"jsonrpc":"2.0","id":6,"method":"Screen.screenshot","params":{"sessionId":"abc123","format":"png"}}
← {"jsonrpc":"2.0","id":6,"result":{"format":"png","data":"iVBORw0KGgo...","width":960,"height":480}}
```

### 6.2 Subscribe to events

```json
// Subscribe to multiple event types at once
→ {"jsonrpc":"2.0","id":10,"method":"Events.subscribe","params":{"sessionId":"abc123","events":["Screen.updated","Shell.commandFinished","Terminal.bell"]}}
← {"jsonrpc":"2.0","id":10,"result":{"subscriptionId":"sub_001","subscribedEvents":["Screen.updated","Shell.commandFinished","Terminal.bell"]}}

// Screen update event arrives
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_001","event":"Screen.updated","sessionId":"abc123","timestamp":1679000000000,"data":{"dirtyRegion":{"top":0,"left":0,"bottom":2,"right":79}}}}

// Command finished event (from OSC 133;D)
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_001","event":"Shell.commandFinished","sessionId":"abc123","timestamp":1679000001000,"data":{"exitCode":0}}}

// Unsubscribe
→ {"jsonrpc":"2.0","id":11,"method":"Events.unsubscribe","params":{"subscriptionId":"sub_001"}}
← {"jsonrpc":"2.0","id":11,"result":{}}
```

### 6.3 Interact with a TUI app (vim)

```json
// Launch vim
→ {"jsonrpc":"2.0","id":1,"method":"Session.create","params":{"shell":"/usr/bin/vim","args":["test.txt"],"cols":120,"rows":40}}
← {"jsonrpc":"2.0","id":1,"result":{"sessionId":"vim123"}}

// Subscribe to know when alternate screen is entered
→ {"jsonrpc":"2.0","id":2,"method":"Events.subscribe","params":{"sessionId":"vim123","events":["Terminal.alternateScreen"]}}
← {"jsonrpc":"2.0","id":2,"result":{"subscriptionId":"sub_002","subscribedEvents":["Terminal.alternateScreen"]}}

// Alternate screen event confirms vim loaded
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_002","event":"Terminal.alternateScreen","sessionId":"vim123","timestamp":1679000000100,"data":{"active":true}}}

// Wait for vim status line
→ {"jsonrpc":"2.0","id":3,"method":"Screen.waitForText","params":{"sessionId":"vim123","pattern":"test.txt","timeout":5000}}
← {"jsonrpc":"2.0","id":3,"result":{"found":true,"matches":[{"text":"test.txt","row":39,"col":1,"length":8}],"elapsed":200}}

// Enter insert mode and type
→ {"jsonrpc":"2.0","id":4,"method":"Input.sendKeys","params":{"sessionId":"vim123","keys":["i","H","e","l","l","o"," ","w","o","r","l","d","Escape"]}}
← {"jsonrpc":"2.0","id":4,"result":{}}

// Save and quit
→ {"jsonrpc":"2.0","id":5,"method":"Input.sendKeys","params":{"sessionId":"vim123","keys":[":","w","q","Enter"]}}
← {"jsonrpc":"2.0","id":5,"result":{}}

// Alternate screen exit event confirms vim closed
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_002","event":"Terminal.alternateScreen","sessionId":"vim123","timestamp":1679000005000,"data":{"active":false}}}
```

### 6.4 AI agent with shell integration

```json
// Subscribe to shell integration events
→ {"jsonrpc":"2.0","id":1,"method":"Events.subscribe","params":{"sessionId":"abc123","events":["Shell.commandFinished","Terminal.cwdChanged","Terminal.notification"]}}
← {"jsonrpc":"2.0","id":1,"result":{"subscriptionId":"sub_003","subscribedEvents":["Shell.commandFinished","Terminal.cwdChanged"]}}

// Send a command
→ {"jsonrpc":"2.0","id":2,"method":"Input.sendKeys","params":{"sessionId":"abc123","keys":["c","d"," ","/","t","m","p","Enter"]}}
← {"jsonrpc":"2.0","id":2,"result":{}}

// CWD change detected via OSC 7
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_003","event":"Terminal.cwdChanged","sessionId":"abc123","timestamp":1679000000500,"data":{"cwd":"/tmp","uri":"file://hostname/tmp"}}}

// Command finished with exit code
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_003","event":"Shell.commandFinished","sessionId":"abc123","timestamp":1679000000600,"data":{"exitCode":0}}}

// Run a failing command
→ {"jsonrpc":"2.0","id":3,"method":"Input.sendKeys","params":{"sessionId":"abc123","keys":["l","s"," ","/","n","o","p","e","Enter"]}}
← {"jsonrpc":"2.0","id":3,"result":{}}

// Agent knows the command failed without parsing output
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_003","event":"Shell.commandFinished","sessionId":"abc123","timestamp":1679000001200,"data":{"exitCode":2}}}
```

### 6.5 Clipboard interception

```json
// Subscribe to clipboard events
→ {"jsonrpc":"2.0","id":1,"method":"Events.subscribe","params":{"sessionId":"abc123","events":["Terminal.clipboardSet"]}}
← {"jsonrpc":"2.0","id":1,"result":{"subscriptionId":"sub_004","subscribedEvents":["Terminal.clipboardSet"]}}

// User yanks text in vim (triggers OSC 52)
← {"jsonrpc":"2.0","method":"Events.event","params":{"subscriptionId":"sub_004","event":"Terminal.clipboardSet","sessionId":"abc123","timestamp":1679000002000,"data":{"selection":"clipboard","text":"yanked text here","base64":"eWFua2VkIHRleHQgaGVyZQ=="}}}
```
