/** Wrightty protocol types — mirrors wrightty-protocol Rust crate. */

export interface ServerInfo {
  version: string;
  implementation: string;
  capabilities: Capabilities;
}

export interface Capabilities {
  screenshot: ScreenshotFormat[];
  maxSessions: number;
  supportsResize: boolean;
  supportsScrollback: boolean;
  supportsMouse: boolean;
  supportsSessionCreate: boolean;
  supportsColorPalette: boolean;
  supportsRawOutput: boolean;
  supportsShellIntegration: boolean;
  events: string[];
}

export type ScreenshotFormat = "text" | "svg" | "png" | "json";

export interface SessionInfo {
  sessionId: string;
  title: string;
  cwd?: string;
  cols: number;
  rows: number;
  pid?: number;
  running: boolean;
  alternateScreen: boolean;
}

export type KeyInput = string | KeyEvent;

export interface KeyEvent {
  key: string;
  char?: string;
  n?: number;
  modifiers: string[];
}

export interface TextMatch {
  text: string;
  row: number;
  col: number;
  length: number;
}

export interface WaitForTextResult {
  found: boolean;
  matches: TextMatch[];
  elapsed: number;
}

export interface ScreenshotResult {
  format: ScreenshotFormat;
  data: string;
  width?: number;
  height?: number;
}

export interface RecordingResult {
  recordingId: string;
}

export interface SessionRecordingData {
  format: string;
  data: string;
  duration: number;
  events: number;
}

export interface ActionRecordingData {
  format: string;
  data: string;
  actions: number;
  duration: number;
}

export interface DiscoveredServer {
  url: string;
  port: number;
  version: string;
  implementation: string;
  capabilities: Capabilities;
}

export interface ConnectOptions {
  /** Server URL (default: auto-discover) */
  url?: string;
  /** Session ID (default: first available) */
  sessionId?: string;
  /** Connection timeout in ms (default: 5000) */
  timeout?: number;
}

export interface SpawnOptions {
  shell?: string;
  cols?: number;
  rows?: number;
  cwd?: string;
  serverUrl?: string;
}
