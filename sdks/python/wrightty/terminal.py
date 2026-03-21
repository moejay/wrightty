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

    # --- Recording ---

    def start_session_recording(self, include_input: bool = False) -> str:
        """Start recording raw PTY I/O (asciicast v2 format).

        Returns a recording ID. Stop with stop_session_recording().
        The recording can be played back with `asciinema play`.
        """
        result = self._client.request(
            "Recording.startSession",
            {"sessionId": self._session_id, "includeInput": include_input},
        )
        return result["recordingId"]

    def stop_session_recording(self, recording_id: str) -> dict:
        """Stop a session recording and return asciicast data.

        Returns dict with keys: format, data, duration, events.
        Save `data` to a .cast file for asciinema playback.
        """
        return self._client.request("Recording.stopSession", {"recordingId": recording_id})

    def start_action_recording(self, format: str = "python") -> str:
        """Start recording wrightty API calls as a replayable script.

        Args:
            format: "python", "json", or "cli"

        Returns a recording ID. Stop with stop_action_recording().
        """
        result = self._client.request(
            "Recording.startActions",
            {"sessionId": self._session_id, "format": format},
        )
        return result["recordingId"]

    def stop_action_recording(self, recording_id: str) -> dict:
        """Stop action recording and return the generated script.

        Returns dict with keys: format, data, actions, duration.
        """
        return self._client.request("Recording.stopActions", {"recordingId": recording_id})

    def capture_screen(self, format: str = "svg") -> dict:
        """Capture a single screen frame.

        Returns dict with keys: frameId, timestamp, format, data.
        """
        return self._client.request(
            "Recording.captureScreen",
            {"sessionId": self._session_id, "format": format},
        )

    def start_screen_recording(self, interval_ms: int = 1000, format: str = "svg") -> str:
        """Start periodic screen capture.

        Args:
            interval_ms: Capture interval in milliseconds (default: 1000)
            format: "svg", "text", or "png"

        Returns a recording ID. Stop with stop_screen_recording().
        """
        result = self._client.request(
            "Recording.startScreenCapture",
            {"sessionId": self._session_id, "intervalMs": interval_ms, "format": format},
        )
        return result["recordingId"]

    def stop_screen_recording(self, recording_id: str) -> dict:
        """Stop screen capture and return all frames.

        Returns dict with keys: frames, duration, frameCount, format.
        """
        return self._client.request("Recording.stopScreenCapture", {"recordingId": recording_id})
