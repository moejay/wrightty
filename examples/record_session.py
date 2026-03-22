#!/usr/bin/env python3
"""record_session.py -- Start a session recording, run commands, save the asciicast file.

Demonstrates Wrightty's recording capabilities:
  - Session recording produces an asciicast v2 (.cast) file
  - Compatible with `asciinema play` for playback and sharing

Usage:
    python examples/record_session.py

Prerequisites:
    - wrightty-server running on ws://127.0.0.1:9420
      Start with: cargo run -p wrightty-server

    - Python SDK installed (from repo root):
      pip install -e sdks/python

    - Optional: asciinema installed to play back the recording
      pip install asciinema   or   brew install asciinema
"""

import json
import os

from wrightty import Terminal

OUTPUT_PATH = "/tmp/wrightty_demo.cast"


def main():
    print("Spawning terminal session...")

    with Terminal.spawn(cols=100, rows=30) as term:
        info = term.get_info()
        print(f"Connected to {info['implementation']} v{info['version']}")
        print()

        # --- Start session recording ---
        print("Starting session recording...")
        recording_id = term.start_session_recording(include_input=False)
        print(f"Recording ID: {recording_id}")
        print()

        # --- Run some commands while recording ---
        print("Running commands (these will be captured in the recording)...")

        output = term.run("echo 'Hello from Wrightty!'")
        print(f"  echo output: {output.strip()}")

        output = term.run("date")
        print(f"  date output: {output.strip()}")

        output = term.run("uname -s")
        print(f"  uname output: {output.strip()}")

        output = term.run("echo 'Recording complete.'")
        print(f"  final echo: {output.strip()}")
        print()

        # --- Stop the recording ---
        print("Stopping recording...")
        result = term.stop_session_recording(recording_id)

        asciicast_data = result["data"]
        duration = result.get("duration", 0)
        event_count = result.get("events", 0)

        print(f"Recorded {event_count} events over {duration:.1f}s")
        print()

        # --- Save the asciicast file ---
        with open(OUTPUT_PATH, "w") as f:
            f.write(asciicast_data)

        file_size = os.path.getsize(OUTPUT_PATH)
        print(f"Saved asciicast to {OUTPUT_PATH} ({file_size} bytes)")
        print()

        # Show the asciicast header so the user can inspect it
        lines = asciicast_data.splitlines()
        if lines:
            header = json.loads(lines[0])
            print("Asciicast header:")
            print(f"  version: {header.get('version')}")
            print(f"  width:   {header.get('width')}")
            print(f"  height:  {header.get('height')}")
            if "title" in header:
                print(f"  title:   {header['title']}")
        print()

    print(f"Done. To play back the recording:")
    print(f"  asciinema play {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
