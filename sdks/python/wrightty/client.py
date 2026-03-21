"""Low-level WebSocket JSON-RPC client for the Wrightty protocol.

Uses a raw socket WebSocket implementation to avoid version issues
with the `websockets` library. Zero external dependencies.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
from typing import Any
from urllib.parse import urlparse


class WrighttyClient:
    """Raw JSON-RPC client over WebSocket. No async, no dependencies."""

    def __init__(self, sock: socket.socket):
        self._sock = sock
        self._next_id = 1

    @classmethod
    def connect(cls, url: str = "ws://127.0.0.1:9420") -> WrighttyClient:
        parsed = urlparse(url)
        host = parsed.hostname or "127.0.0.1"
        port = parsed.port or 9420

        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((host, port))
        sock.settimeout(30)

        # WebSocket handshake.
        key = base64.b64encode(os.urandom(16)).decode()
        request = (
            f"GET / HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            f"Connection: Upgrade\r\n"
            f"Upgrade: websocket\r\n"
            f"Sec-WebSocket-Version: 13\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            f"\r\n"
        )
        sock.sendall(request.encode())

        # Read response headers.
        response = b""
        while b"\r\n\r\n" not in response:
            response += sock.recv(4096)

        if b"101" not in response:
            raise ConnectionError(f"WebSocket handshake failed: {response.decode()}")

        return cls(sock)

    def close(self):
        try:
            self._sock.close()
        except Exception:
            pass

    def request(self, method: str, params: dict[str, Any] | None = None) -> Any:
        req_id = self._next_id
        self._next_id += 1

        msg = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params or {}}
        self._send_frame(json.dumps(msg))
        raw = self._recv_frame()
        resp = json.loads(raw)

        if "error" in resp:
            err = resp["error"]
            raise WrighttyError(err.get("code", -1), err.get("message", "Unknown error"))

        return resp.get("result")

    def _send_frame(self, msg: str):
        """Send a masked WebSocket text frame."""
        payload = msg.encode()
        mask = os.urandom(4)
        frame = bytearray([0x81])  # FIN + text opcode

        length = len(payload)
        if length < 126:
            frame.append(0x80 | length)
        elif length < 65536:
            frame.append(0x80 | 126)
            frame.extend(struct.pack(">H", length))
        else:
            frame.append(0x80 | 127)
            frame.extend(struct.pack(">Q", length))

        frame.extend(mask)
        for i, b in enumerate(payload):
            frame.append(b ^ mask[i % 4])
        self._sock.sendall(bytes(frame))

    def _recv_frame(self) -> str:
        """Receive a WebSocket text frame."""
        header = self._recv_exact(2)
        length = header[1] & 0x7F

        if length == 126:
            length = struct.unpack(">H", self._recv_exact(2))[0]
        elif length == 127:
            length = struct.unpack(">Q", self._recv_exact(8))[0]

        # Server frames are not masked.
        payload = self._recv_exact(length)
        return payload.decode()

    def _recv_exact(self, n: int) -> bytes:
        """Read exactly n bytes."""
        data = b""
        while len(data) < n:
            chunk = self._sock.recv(n - len(data))
            if not chunk:
                raise ConnectionError("Connection closed")
            data += chunk
        return data

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()


class WrighttyError(Exception):
    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")
