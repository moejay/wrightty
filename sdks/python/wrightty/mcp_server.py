"""Wrightty MCP Server — expose terminal control as tools for AI agents.

Run with:
    python -m wrightty.mcp_server
    # or via MCP config in Claude/Cursor/etc.

This exposes the following tools to AI agents:
    - run_command: Run a shell command and return output
    - read_terminal: Read the current terminal screen
    - send_keys: Send keystrokes (for TUI apps)
    - screenshot: Take a terminal screenshot (SVG for rendering)
    - wait_for_text: Wait until specific text appears
    - terminal_info: Get terminal info and dimensions
"""

from __future__ import annotations

import asyncio
import json
import os
import re
import time
from typing import Any

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import TextContent, ImageContent, Tool

from wrightty.client import WrighttyClient


# Global client state.
_client: WrighttyClient | None = None
_session_id: str = "0"
_prompt_pattern = r"[$#>%]\s*$"


async def get_client() -> WrighttyClient:
    global _client
    if _client is None:
        url = os.environ.get("WRIGHTTY_SOCKET", "ws://127.0.0.1:9420")
        _client = await WrighttyClient.connect(url)
    return _client


async def read_screen() -> str:
    client = await get_client()
    result = await client.request("Screen.getText", {"sessionId": _session_id})
    return result["text"]


async def wait_for_prompt(timeout: float = 10) -> str:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        screen = await read_screen()
        if re.search(_prompt_pattern, screen):
            return screen
        await asyncio.sleep(0.2)
    return await read_screen()


app = Server("wrightty")


@app.list_tools()
async def list_tools() -> list[Tool]:
    return [
        Tool(
            name="run_command",
            description=(
                "Run a shell command in the terminal and return its output. "
                "The command is typed into a real terminal, executed, and the output "
                "is captured after the command completes (when the prompt returns). "
                "Use this for any shell operation: building, testing, file manipulation, git, etc."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run",
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Max seconds to wait for the command to finish (default: 30)",
                        "default": 30,
                    },
                },
                "required": ["command"],
            },
        ),
        Tool(
            name="read_terminal",
            description=(
                "Read the current visible content of the terminal screen. "
                "Returns the text currently displayed, including any running program's output. "
                "Useful for checking the state of long-running processes, TUI apps, or "
                "reading content that was printed before."
            ),
            inputSchema={
                "type": "object",
                "properties": {},
            },
        ),
        Tool(
            name="send_keys",
            description=(
                "Send keystrokes to the terminal. Use this for interactive programs like vim, "
                "htop, less, or any TUI application. Supports special keys and modifiers.\n\n"
                "Key names: Enter, Tab, Escape, Backspace, Delete, ArrowUp, ArrowDown, "
                "ArrowLeft, ArrowRight, Home, End, PageUp, PageDown, F1-F12\n"
                "Modifiers: Ctrl+c, Alt+x, Shift+Tab\n"
                "Single characters: a, b, 1, /, etc."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "keys": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": 'List of keys to send, e.g. ["Escape", ":", "w", "q", "Enter"]',
                    },
                },
                "required": ["keys"],
            },
        ),
        Tool(
            name="send_text",
            description=(
                "Send raw text to the terminal without any key interpretation. "
                "Use \\n for newline. Useful for pasting content or sending multi-line input."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to send",
                    },
                },
                "required": ["text"],
            },
        ),
        Tool(
            name="screenshot",
            description=(
                "Take a screenshot of the terminal. Returns an SVG image showing the terminal "
                "with colors, fonts, and styling. Useful for understanding visual layout of TUI apps."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["text", "svg"],
                        "default": "svg",
                        "description": "Screenshot format: 'text' for plain text, 'svg' for styled image",
                    },
                },
            },
        ),
        Tool(
            name="wait_for_text",
            description=(
                "Wait until specific text appears on the terminal screen. "
                "Blocks until the text is found or timeout is reached. "
                "Useful for waiting for compilation, test results, prompts, etc."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Text to wait for",
                    },
                    "timeout": {
                        "type": "number",
                        "description": "Max seconds to wait (default: 30)",
                        "default": 30,
                    },
                    "regex": {
                        "type": "boolean",
                        "description": "Treat pattern as regex (default: false)",
                        "default": False,
                    },
                },
                "required": ["pattern"],
            },
        ),
        Tool(
            name="terminal_info",
            description="Get terminal information: dimensions, server version, capabilities.",
            inputSchema={
                "type": "object",
                "properties": {},
            },
        ),
    ]


@app.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> list[TextContent | ImageContent]:
    client = await get_client()

    if name == "run_command":
        command = arguments["command"]
        timeout = arguments.get("timeout", 30)

        # Read screen before.
        before = await read_screen()

        # Send command.
        await client.request(
            "Input.sendText", {"sessionId": _session_id, "text": command + "\n"}
        )

        # Wait for prompt to return.
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            screen = await read_screen()
            # Check if prompt appeared after the command.
            lines = screen.strip().split("\n")
            if lines and re.search(_prompt_pattern, lines[-1]) and command not in lines[-1]:
                break
            await asyncio.sleep(0.3)

        # Read final screen and extract output.
        screen = await read_screen()
        lines = screen.strip().split("\n")

        output_lines = []
        found_cmd = False
        for line in lines:
            if not found_cmd:
                if command in line:
                    found_cmd = True
                continue
            if re.search(_prompt_pattern, line):
                break
            output_lines.append(line)

        output = "\n".join(output_lines) if output_lines else screen
        return [TextContent(type="text", text=output)]

    elif name == "read_terminal":
        screen = await read_screen()
        return [TextContent(type="text", text=screen)]

    elif name == "send_keys":
        keys = arguments["keys"]
        await client.request("Input.sendKeys", {"sessionId": _session_id, "keys": keys})
        await asyncio.sleep(0.3)
        screen = await read_screen()
        return [TextContent(type="text", text=screen)]

    elif name == "send_text":
        text = arguments["text"]
        await client.request("Input.sendText", {"sessionId": _session_id, "text": text})
        await asyncio.sleep(0.3)
        screen = await read_screen()
        return [TextContent(type="text", text=screen)]

    elif name == "screenshot":
        fmt = arguments.get("format", "svg")
        result = await client.request(
            "Screen.screenshot", {"sessionId": _session_id, "format": fmt}
        )
        return [TextContent(type="text", text=result["data"])]

    elif name == "wait_for_text":
        pattern = arguments["pattern"]
        timeout = arguments.get("timeout", 30)
        is_regex = arguments.get("regex", False)

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            screen = await read_screen()
            if is_regex:
                if re.search(pattern, screen):
                    return [TextContent(type="text", text=screen)]
            else:
                if pattern in screen:
                    return [TextContent(type="text", text=screen)]
            await asyncio.sleep(0.3)

        return [TextContent(type="text", text=f"Timeout: '{pattern}' not found after {timeout}s")]

    elif name == "terminal_info":
        info = await client.request("Wrightty.getInfo")
        size = await client.request("Terminal.getSize", {"sessionId": _session_id})
        info["size"] = size
        return [TextContent(type="text", text=json.dumps(info, indent=2))]

    else:
        return [TextContent(type="text", text=f"Unknown tool: {name}")]


async def serve():
    async with stdio_server() as (read_stream, write_stream):
        await app.run(read_stream, write_stream, app.create_initialization_options())


def main():
    asyncio.run(serve())


if __name__ == "__main__":
    main()
