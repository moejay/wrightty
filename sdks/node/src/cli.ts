#!/usr/bin/env node
/** wrightty CLI for Node.js — control terminals from the command line. */

import { Terminal } from "./terminal";

const args = process.argv.slice(2);
const command = args[0];

function usage() {
  console.log(`wrightty-js — Playwright for terminals (Node.js)

Note: This is a client CLI. To start a server: cargo install wrightty

Usage:
  wrightty-js run <command> [--timeout <s>]   Run a command and print output
  wrightty-js read                            Read the terminal screen
  wrightty-js send-text <text>                Send raw text
  wrightty-js send-keys <key> [<key>...]      Send keystrokes
  wrightty-js screenshot [--format svg|text]  Take a screenshot
  wrightty-js wait-for <pattern> [--timeout]  Wait for text on screen
  wrightty-js info                            Show server info
  wrightty-js size                            Get terminal dimensions
  wrightty-js discover                        Find running servers
  wrightty-js upgrade                         Check for updates and upgrade

Options:
  --url <url>         Server URL (default: auto-discover)
  --session <id>      Session ID (default: first available)
  --password <pass>   Password for server authentication
  --help              Show this help`);
}

function getOpt(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx >= 0 && idx + 1 < args.length ? args[idx + 1] : undefined;
}

function hasFlag(name: string): boolean {
  return args.includes(name);
}

async function getTerminal(): Promise<Terminal> {
  const url = getOpt("--url");
  const sessionId = getOpt("--session");
  const password = getOpt("--password");
  return Terminal.connect({ url, sessionId, password });
}

async function main() {
  if (!command || hasFlag("--help") || hasFlag("-h")) {
    usage();
    process.exit(0);
  }

  try {
    switch (command) {
      case "discover": {
        const servers = await Terminal.discover();
        if (servers.length === 0) {
          console.log("No wrightty servers found.");
        } else {
          for (const s of servers) {
            const name = s.name ? ` [${s.name}]` : "";
            const auth = s.authentication ? ` (auth: ${s.authentication})` : "";
            console.log(`  ${s.url}  ${s.implementation} v${s.version}${name}${auth}`);
          }
        }
        break;
      }

      case "run": {
        const cmd = args[1];
        if (!cmd) { console.error("Usage: wrightty-js run <command>"); process.exit(1); }
        const timeout = parseInt(getOpt("--timeout") ?? "30", 10) * 1000;
        const term = await getTerminal();
        const output = await term.run(cmd, timeout);
        console.log(output);
        term.close();
        break;
      }

      case "read": {
        const term = await getTerminal();
        console.log(await term.readScreen());
        term.close();
        break;
      }

      case "send-text": {
        const text = args[1];
        if (!text) { console.error("Usage: wrightty-js send-text <text>"); process.exit(1); }
        const term = await getTerminal();
        await term.sendText(text.replace(/\\n/g, "\n"));
        term.close();
        break;
      }

      case "send-keys": {
        const keys = args.slice(1).filter(k => !k.startsWith("--"));
        if (keys.length === 0) { console.error("Usage: wrightty-js send-keys <key> [...]"); process.exit(1); }
        const term = await getTerminal();
        await term.sendKeys(...keys);
        term.close();
        break;
      }

      case "screenshot": {
        const format = (getOpt("--format") ?? "text") as "text" | "svg";
        const term = await getTerminal();
        const result = await term.screenshot(format);
        console.log(result.data);
        term.close();
        break;
      }

      case "wait-for": {
        const pattern = args[1];
        if (!pattern) { console.error("Usage: wrightty-js wait-for <pattern>"); process.exit(1); }
        const timeout = parseInt(getOpt("--timeout") ?? "30", 10) * 1000;
        const term = await getTerminal();
        const screen = await term.waitFor(pattern, timeout);
        console.log(screen);
        term.close();
        break;
      }

      case "info": {
        const term = await getTerminal();
        console.log(JSON.stringify(await term.getInfo(), null, 2));
        term.close();
        break;
      }

      case "size": {
        const term = await getTerminal();
        const [cols, rows] = await term.getSize();
        console.log(`${cols}x${rows}`);
        term.close();
        break;
      }

      case "upgrade": {
        const pkg = require("../package.json");
        console.log(`Current version: ${pkg.version}`);
        console.log("Upgrading...");
        const { execSync } = require("child_process");
        try {
          execSync("npm install -g @moejay/wrightty@latest", { stdio: "inherit" });
          console.log("Upgraded successfully.");
        } catch {
          console.error("Upgrade failed. Try manually: npm install -g @moejay/wrightty@latest");
          process.exit(1);
        }
        break;
      }

      default:
        console.error(`Unknown command: ${command}`);
        usage();
        process.exit(1);
    }
  } catch (err: any) {
    console.error(`Error: ${err.message}`);
    process.exit(1);
  }
}

main();
