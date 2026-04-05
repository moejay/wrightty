/** Low-level WebSocket JSON-RPC 2.0 client for the Wrightty protocol. */

import WebSocket from "ws";

export class WrighttyError extends Error {
  constructor(
    public code: number,
    message: string,
  ) {
    super(`[${code}] ${message}`);
    this.name = "WrighttyError";
  }
}

export class WrighttyClient {
  private ws: WebSocket;
  private nextId = 1;
  private pending = new Map<
    number,
    {
      resolve: (value: any) => void;
      reject: (reason: any) => void;
    }
  >();

  private constructor(ws: WebSocket) {
    this.ws = ws;

    ws.on("message", (data: WebSocket.Data) => {
      try {
        const msg = JSON.parse(data.toString());
        const entry = this.pending.get(msg.id);
        if (!entry) return;
        this.pending.delete(msg.id);

        if (msg.error) {
          entry.reject(
            new WrighttyError(msg.error.code ?? -1, msg.error.message ?? "Unknown error"),
          );
        } else {
          entry.resolve(msg.result);
        }
      } catch {
        // Ignore malformed messages
      }
    });

    ws.on("close", () => {
      for (const [id, entry] of this.pending) {
        entry.reject(new Error("Connection closed"));
        this.pending.delete(id);
      }
    });
  }

  static connect(url: string, timeoutMs = 5000): Promise<WrighttyClient> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(url);
      const timer = setTimeout(() => {
        ws.close();
        reject(new Error(`Connection timeout after ${timeoutMs}ms: ${url}`));
      }, timeoutMs);

      ws.on("open", () => {
        clearTimeout(timer);
        resolve(new WrighttyClient(ws));
      });

      ws.on("error", (err) => {
        clearTimeout(timer);
        reject(err);
      });
    });
  }

  async request(method: string, params: Record<string, any> = {}): Promise<any> {
    const id = this.nextId++;
    const msg = JSON.stringify({ jsonrpc: "2.0", id, method, params });

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(msg, (err) => {
        if (err) {
          this.pending.delete(id);
          reject(err);
        }
      });
    });
  }

  close(): void {
    this.ws.close();
  }
}
