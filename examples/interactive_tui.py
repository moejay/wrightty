#!/usr/bin/env python3
"""interactive_tui.py -- Launch a TUI app, interact with it, take a screenshot, exit.

Demonstrates how to:
  - Spawn a fresh terminal session via wrightty-server
  - Launch an interactive TUI (htop or top as fallback)
  - Wait for the UI to render
  - Take an SVG screenshot
  - Send keystrokes to exit cleanly

Usage:
    python examples/interactive_tui.py

Prerequisites:
    - wrightty-server running on ws://127.0.0.1:9420
      Start with: cargo run -p wrightty-server

    - Python SDK installed (from repo root):
      pip install -e sdks/python

    - htop installed (or falls back to top)
"""

import sys

from wrightty import Terminal


def main():
    print("Spawning a new terminal session on wrightty-server...")

    # Spawn creates a fresh PTY session via wrightty-server daemon
    with Terminal.spawn(cols=120, rows=40) as term:
        info = term.get_info()
        print(f"Connected to {info['implementation']} v{info['version']}")
        print()

        # Try htop first, fall back to top
        print("Launching htop (or top as fallback)...")
        term.send_text("htop 2>/dev/null || top\n")

        try:
            # htop shows "Load average" in its header
            screen = term.wait_for("Load average", timeout=5)
            tui_name = "htop"
        except TimeoutError:
            try:
                # top shows "load average" (lowercase)
                screen = term.wait_for("load average", timeout=5)
                tui_name = "top"
            except TimeoutError:
                print("Neither htop nor top appeared. Exiting.")
                term.send_keys("Ctrl+c")
                sys.exit(1)

        print(f"{tui_name} is running. Taking a screenshot...")

        # Capture an SVG screenshot of the rendered terminal
        screenshot_svg = term.screenshot(format="svg")
        out_path = "/tmp/wrightty_tui_screenshot.svg"
        with open(out_path, "w") as f:
            f.write(screenshot_svg)
        print(f"Screenshot saved to {out_path}")
        print()

        # Show a snippet of the current screen text
        screen_text = term.read_screen()
        lines = screen_text.splitlines()
        print("Screen preview (first 5 lines):")
        for line in lines[:5]:
            print(f"  {line}")
        print()

        # Exit htop with 'q', or top with 'q' as well
        print(f"Sending 'q' to exit {tui_name}...")
        term.send_keys("q")

        # Wait for the shell prompt to return
        term.wait_for_prompt(timeout=5)
        print("Exited cleanly. Shell prompt is back.")

    print("\nDone. Session closed.")


if __name__ == "__main__":
    main()
