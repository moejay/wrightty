"""High-level Terminal API for AI agents and automation."""

from __future__ import annotations

import re
import time
from typing import Any

from wrightty.client import WrighttyClient, WrighttyError

PORT_RANGE_START = 9420
PORT_RANGE_END = 9440


class Terminal:
    """High-level terminal automation interface.

    Usage:
        # Connect to a running wrightty server (daemon or native emulator)
        term = Terminal.connect()
        output = term.run("ls -la")
        print(output)

        # Spawn a new session on a wrightty-server daemon
        term = Terminal.spawn()
        term.run("echo hello")
        term.close()
    """

    def __init__(self, client: WrighttyClient, session_id: str):
        self._client = client
        self._session_id = session_id
        self._prompt_pattern = r"[$#>%]\s*$"

    @staticmethod
    def discover(host: str = "127.0.0.1") -> list[dict]:
        """Scan for running wrightty servers on ports 9420-9440.

        Returns a list of dicts with keys: url, version, implementation, capabilities.

        Example:
            servers = Terminal.discover()
            for s in servers:
                print(f"{s['url']} — {s['implementation']}")
            # ws://127.0.0.1:9420 — alacritty-wrightty
            # ws://127.0.0.1:9421 — wrightty-bridge-wezterm
        """
        found = []
        for port in range(PORT_RANGE_START, PORT_RANGE_END + 1):
            url = f"ws://{host}:{port}"
            try:
                client = WrighttyClient.connect(url)
                info = client.request("Wrightty.getInfo")
                client.close()
                found.append({
                    "url": url,
                    "port": port,
                    "version": info.get("version", "unknown"),
                    "implementation": info.get("implementation", "unknown"),
                    "capabilities": info.get("capabilities", {}),
                })
            except (ConnectionError, ConnectionRefusedError, OSError, WrighttyError):
                continue
        return found

    @classmethod
    def connect(
        cls,
        url: str | None = None,
        session_id: str | None = None,
    ) -> Terminal:
        """Connect to a wrightty server.

        If no URL is given, auto-discovers the first available server
        by scanning ports 9420-9440.
        """
        if url is None:
            servers = cls.discover()
            if not servers:
                raise ConnectionError(
                    "No wrightty server found. Start one with:\n"
                    "  alacritty --wrightty\n"
                    "  cargo run -p wrightty-server\n"
                    "  cargo run -p wrightty-bridge-wezterm"
                )
            url = servers[0]["url"]

        client = WrighttyClient.connect(url)

        if session_id is None:
            result = client.request("Session.list")
            sessions = result.get("sessions", [])
            if sessions:
                session_id = sessions[0]["sessionId"]
            else:
                session_id = "0"

        return cls(client, session_id)

    @classmethod
    def spawn(
        cls,
        shell: str | None = None,
        cols: int = 120,
        rows: int = 40,
        cwd: str | None = None,
        server_url: str = "ws://127.0.0.1:9420",
    ) -> Terminal:
        """Connect to a wrightty-server daemon and create a new session."""
        client = WrighttyClient.connect(server_url)

        params: dict[str, Any] = {"cols": cols, "rows": rows}
        if shell:
            params["shell"] = shell
        if cwd:
            params["cwd"] = cwd

        result = client.request("Session.create", params)
        session_id = result["sessionId"]

        term = cls(client, session_id)
        term.wait_for_prompt(timeout=5)
        return term

    def close(self):
        """Close the connection."""
        self._client.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    # --- High-level API ---

    def run(self, command: str, timeout: float = 30) -> str:
        """Run a command and return its output.

        Sends the command, waits for the prompt to reappear, and returns
        everything between the command echo and the next prompt.
        """
        self.send_text(command + "\n")

        # Wait for prompt to come back.
        self.wait_for_prompt(timeout=timeout)

        # Read screen and extract output.
        screen = self.read_screen()
        lines = screen.strip().split("\n")

        output_lines = []
        found_cmd = False
        for line in lines:
            if not found_cmd:
                if command in line:
                    found_cmd = True
                continue
            if re.search(self._prompt_pattern, line):
                break
            output_lines.append(line)

        return "\n".join(output_lines)

    def send_text(self, text: str):
        """Send raw text to the terminal."""
        self._client.request("Input.sendText", {"sessionId": self._session_id, "text": text})

    def send_keys(self, *keys: str):
        """Send structured keystrokes.

        Examples:
            term.send_keys("Ctrl+c")
            term.send_keys("ArrowUp", "Enter")
            term.send_keys("Escape", ":", "w", "q", "Enter")  # vim :wq
        """
        self._client.request(
            "Input.sendKeys", {"sessionId": self._session_id, "keys": list(keys)}
        )

    def read_screen(self) -> str:
        """Read the current visible screen as text."""
        result = self._client.request("Screen.getText", {"sessionId": self._session_id})
        return result["text"]

    def screenshot(self, format: str = "svg") -> str | bytes:
        """Take a screenshot. Returns str for text/svg, bytes for png."""
        result = self._client.request(
            "Screen.screenshot", {"sessionId": self._session_id, "format": format}
        )
        data = result["data"]
        if format == "png":
            import base64
            return base64.b64decode(data)
        return data

    def wait_for(self, pattern: str, timeout: float = 30, regex: bool = False) -> str:
        """Wait until a pattern appears on screen. Returns the screen text when found."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            screen = self.read_screen()
            if regex:
                if re.search(pattern, screen):
                    return screen
            else:
                if pattern in screen:
                    return screen
            time.sleep(0.2)
        raise TimeoutError(f"Pattern {pattern!r} not found within {timeout}s")

    def wait_for_prompt(self, timeout: float = 10) -> str:
        """Wait for the shell prompt to appear."""
        return self.wait_for(self._prompt_pattern, timeout=timeout, regex=True)

    def set_prompt_pattern(self, pattern: str):
        """Override the regex used to detect the shell prompt."""
        self._prompt_pattern = pattern

    def get_size(self) -> tuple[int, int]:
        """Get terminal dimensions as (cols, rows)."""
        result = self._client.request("Terminal.getSize", {"sessionId": self._session_id})
        return result["cols"], result["rows"]

    def resize(self, cols: int, rows: int):
        """Resize the terminal."""
        self._client.request(
            "Terminal.resize", {"sessionId": self._session_id, "cols": cols, "rows": rows}
        )

    def get_info(self) -> dict:
        """Get server info and capabilities."""
        return self._client.request("Wrightty.getInfo")
