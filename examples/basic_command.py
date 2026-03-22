#!/usr/bin/env python3
"""basic_command.py -- Connect to wrightty, run a command, read output, disconnect.

Usage:
    python examples/basic_command.py

Prerequisites:
    - wrightty-server running on ws://127.0.0.1:9420
      Start with: cargo run -p wrightty-server

    - Python SDK installed (from repo root):
      pip install -e sdks/python
"""

from wrightty import Terminal


def main():
    print("Connecting to wrightty server...")

    # Connect to a running server (auto-discovers on ports 9420-9440)
    with Terminal.connect() as term:
        info = term.get_info()
        print(f"Connected to {info['implementation']} v{info['version']}")
        print()

        # Run a simple command and capture output
        print("Running: uname -a")
        output = term.run("uname -a")
        print(f"Output:\n  {output}")
        print()

        # Run a multi-line command
        print("Running: ls -la /tmp | head -5")
        output = term.run("ls -la /tmp | head -5")
        print("Output:")
        for line in output.splitlines():
            print(f"  {line}")
        print()

        # Check the current directory
        output = term.run("pwd")
        print(f"Working directory: {output.strip()}")

        # Get terminal dimensions
        cols, rows = term.get_size()
        print(f"Terminal size: {cols}x{rows}")

    print("\nDone. Connection closed.")


if __name__ == "__main__":
    main()
