/** High-level Terminal API for AI agents and automation. */

import { WrighttyClient } from "./client";
import type {
  ConnectOptions,
  DiscoveredServer,
  ScreenshotFormat,
  ScreenshotResult,
  SessionInfo,
  SpawnOptions,
  WaitForTextResult,
  SessionRecordingData,
  ActionRecordingData,
} from "./types";

const PORT_RANGE_START = 9420;
const PORT_RANGE_END = 9520;

export class Terminal {
  private client: WrighttyClient;
  private sessionId: string;
  private promptPattern = /[$#>%]\s*$/;

  private constructor(client: WrighttyClient, sessionId: string) {
    this.client = client;
    this.sessionId = sessionId;
  }

  /** Scan for running wrightty servers on ports 9420-9520. */
  static async discover(host = "127.0.0.1"): Promise<DiscoveredServer[]> {
    const found: DiscoveredServer[] = [];

    const checks = [];
    for (let port = PORT_RANGE_START; port <= PORT_RANGE_END; port++) {
      const url = `ws://${host}:${port}`;
      checks.push(
        WrighttyClient.connect(url, 200)
          .then(async (client) => {
            try {
              const info = await client.request("Wrightty.getInfo");
              found.push({
                url,
                port,
                version: info.version ?? "unknown",
                implementation: info.implementation ?? "unknown",
                capabilities: info.capabilities ?? {},
              });
            } finally {
              client.close();
            }
          })
          .catch(() => {
            /* port not listening */
          }),
      );
    }

    await Promise.all(checks);
    return found.sort((a, b) => a.port - b.port);
  }

  /** Connect to a wrightty server. Auto-discovers if no URL given. */
  static async connect(options: ConnectOptions = {}): Promise<Terminal> {
    let url = options.url;

    if (!url) {
      const servers = await Terminal.discover();
      if (servers.length === 0) {
        throw new Error(
          "No wrightty server found. Start one with:\n" +
            "  wrightty term --headless\n" +
            "  wrightty term --bridge-tmux\n" +
            "  wrightty term --bridge-wezterm",
        );
      }
      url = servers[0].url;
    }

    const client = await WrighttyClient.connect(url, options.timeout ?? 5000);

    let sessionId = options.sessionId;
    if (!sessionId) {
      const result = await client.request("Session.list");
      const sessions: SessionInfo[] = result.sessions ?? [];
      sessionId = sessions.length > 0 ? sessions[0].sessionId : "0";
    }

    return new Terminal(client, sessionId);
  }

  /** Connect to a headless server and create a new session. */
  static async spawn(options: SpawnOptions = {}): Promise<Terminal> {
    const url = options.serverUrl ?? "ws://127.0.0.1:9420";
    const client = await WrighttyClient.connect(url);

    const result = await client.request("Session.create", {
      cols: options.cols ?? 120,
      rows: options.rows ?? 40,
      shell: options.shell,
      cwd: options.cwd,
    });

    const term = new Terminal(client, result.sessionId);
    await term.waitForPrompt(5000);
    return term;
  }

  /** Close the connection. */
  close(): void {
    this.client.close();
  }

  // --- High-level API ---

  /** Run a command and return its output. */
  async run(command: string, timeoutMs = 30000): Promise<string> {
    await this.sendText(command + "\n");
    await this.waitForPrompt(timeoutMs);

    const screen = await this.readScreen();
    const lines = screen.trim().split("\n");

    const outputLines: string[] = [];
    let foundCmd = false;
    for (const line of lines) {
      if (!foundCmd) {
        if (line.includes(command)) foundCmd = true;
        continue;
      }
      if (this.promptPattern.test(line)) break;
      outputLines.push(line);
    }

    return outputLines.join("\n");
  }

  /** Send raw text to the terminal. */
  async sendText(text: string): Promise<void> {
    await this.client.request("Input.sendText", {
      sessionId: this.sessionId,
      text,
    });
  }

  /** Send structured keystrokes. */
  async sendKeys(...keys: string[]): Promise<void> {
    await this.client.request("Input.sendKeys", {
      sessionId: this.sessionId,
      keys,
    });
  }

  /** Read the current visible screen as text. */
  async readScreen(): Promise<string> {
    const result = await this.client.request("Screen.getText", {
      sessionId: this.sessionId,
    });
    return result.text;
  }

  /** Take a screenshot. */
  async screenshot(format: ScreenshotFormat = "svg"): Promise<ScreenshotResult> {
    return this.client.request("Screen.screenshot", {
      sessionId: this.sessionId,
      format,
    });
  }

  /** Wait until a pattern appears on screen. */
  async waitFor(pattern: string | RegExp, timeoutMs = 30000): Promise<string> {
    const isRegex = pattern instanceof RegExp;
    const patternStr = isRegex ? pattern.source : pattern;

    const result: WaitForTextResult = await this.client.request("Screen.waitForText", {
      sessionId: this.sessionId,
      pattern: patternStr,
      isRegex,
      timeout: timeoutMs,
      interval: 50,
    });

    if (!result.found) {
      throw new Error(`Pattern ${patternStr} not found within ${timeoutMs}ms`);
    }

    return this.readScreen();
  }

  /** Wait for the shell prompt to appear. */
  async waitForPrompt(timeoutMs = 10000): Promise<string> {
    return this.waitFor(this.promptPattern, timeoutMs);
  }

  /** Override the regex used to detect the shell prompt. */
  setPromptPattern(pattern: RegExp): void {
    this.promptPattern = pattern;
  }

  /** Get terminal dimensions as [cols, rows]. */
  async getSize(): Promise<[number, number]> {
    const result = await this.client.request("Terminal.getSize", {
      sessionId: this.sessionId,
    });
    return [result.cols, result.rows];
  }

  /** Resize the terminal. */
  async resize(cols: number, rows: number): Promise<void> {
    await this.client.request("Terminal.resize", {
      sessionId: this.sessionId,
      cols,
      rows,
    });
  }

  /** Get server info and capabilities. */
  async getInfo(): Promise<Record<string, any>> {
    return this.client.request("Wrightty.getInfo");
  }

  // --- Recording ---

  /** Start recording raw PTY I/O (asciicast v2 format). */
  async startSessionRecording(includeInput = false): Promise<string> {
    const result = await this.client.request("Recording.startSession", {
      sessionId: this.sessionId,
      includeInput,
    });
    return result.recordingId;
  }

  /** Stop a session recording and return asciicast data. */
  async stopSessionRecording(recordingId: string): Promise<SessionRecordingData> {
    return this.client.request("Recording.stopSession", { recordingId });
  }

  /** Start recording wrightty API calls as a replayable script. */
  async startActionRecording(format: "python" | "json" | "cli" = "python"): Promise<string> {
    const result = await this.client.request("Recording.startActions", {
      sessionId: this.sessionId,
      format,
    });
    return result.recordingId;
  }

  /** Stop action recording and return the generated script. */
  async stopActionRecording(recordingId: string): Promise<ActionRecordingData> {
    return this.client.request("Recording.stopActions", { recordingId });
  }

  /** Capture a single screen frame. */
  async captureScreen(format: ScreenshotFormat = "svg"): Promise<Record<string, any>> {
    return this.client.request("Recording.captureScreen", {
      sessionId: this.sessionId,
      format,
    });
  }
}
