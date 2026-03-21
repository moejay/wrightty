"""Wrightty CLI — control terminals from the command line."""

from __future__ import annotations

import json
import sys

import click

from wrightty.terminal import Terminal


@click.group()
@click.option("--url", default="ws://127.0.0.1:9420", help="Wrightty server URL")
@click.option("--session", default=None, help="Session ID (default: auto-detect)")
@click.pass_context
def main(ctx, url, session):
    """Wrightty — Playwright for terminals."""
    ctx.ensure_object(dict)
    ctx.obj["url"] = url
    ctx.obj["session"] = session


def _connect(ctx) -> Terminal:
    return Terminal.connect(ctx.obj["url"], ctx.obj["session"])


@main.command()
@click.argument("command")
@click.option("--timeout", default=30, type=float, help="Timeout in seconds")
@click.pass_context
def run(ctx, command, timeout):
    """Run a command and print its output."""
    term = _connect(ctx)
    try:
        output = term.run(command, timeout=timeout)
        click.echo(output)
    finally:
        term.close()


@main.command("read")
@click.pass_context
def read_screen(ctx):
    """Read the current terminal screen."""
    term = _connect(ctx)
    try:
        click.echo(term.read_screen())
    finally:
        term.close()


@main.command("send-text")
@click.argument("text")
@click.pass_context
def send_text(ctx, text):
    """Send raw text to the terminal."""
    term = _connect(ctx)
    try:
        # Interpret \\n as actual newlines.
        text = text.replace("\\n", "\n")
        term.send_text(text)
    finally:
        term.close()


@main.command("send-keys")
@click.argument("keys", nargs=-1, required=True)
@click.pass_context
def send_keys(ctx, keys):
    """Send keystrokes to the terminal.

    Examples:
        wrightty send-keys Ctrl+c
        wrightty send-keys Escape : w q Enter
    """
    term = _connect(ctx)
    try:
        term.send_keys(*keys)
    finally:
        term.close()


@main.command("wait-for")
@click.argument("pattern")
@click.option("--timeout", default=30, type=float, help="Timeout in seconds")
@click.option("--regex", is_flag=True, help="Treat pattern as regex")
@click.pass_context
def wait_for(ctx, pattern, timeout, regex):
    """Wait until text appears on screen."""
    term = _connect(ctx)
    try:
        screen = term.wait_for(pattern, timeout=timeout, regex=regex)
        click.echo(screen)
    except TimeoutError as e:
        click.echo(str(e), err=True)
        sys.exit(1)
    finally:
        term.close()


@main.command()
@click.option("--format", "fmt", default="svg", type=click.Choice(["text", "svg", "png"]))
@click.option("--output", "-o", default=None, help="Output file (default: stdout)")
@click.pass_context
def screenshot(ctx, fmt, output):
    """Take a terminal screenshot."""
    term = _connect(ctx)
    try:
        data = term.screenshot(fmt)
        if output:
            mode = "wb" if fmt == "png" else "w"
            with open(output, mode) as f:
                f.write(data)
            click.echo(f"Screenshot saved to {output}")
        else:
            if fmt == "png":
                sys.stdout.buffer.write(data)
            else:
                click.echo(data)
    finally:
        term.close()


@main.command()
@click.pass_context
def info(ctx):
    """Show server info and capabilities."""
    term = _connect(ctx)
    try:
        info = term.get_info()
        click.echo(json.dumps(info, indent=2))
    finally:
        term.close()


@main.command()
@click.pass_context
def size(ctx):
    """Get terminal dimensions."""
    term = _connect(ctx)
    try:
        cols, rows = term.get_size()
        click.echo(f"{cols}x{rows}")
    finally:
        term.close()


if __name__ == "__main__":
    main()
