import fs from "fs";
import path from "path";

// Resolve relative to the repo root regardless of where the compiled file lives.
// __filename → dist/server/logger.js → ../../.. → repo root
const LOGS_DIR = path.resolve(
  path.dirname(new URL(import.meta.url).pathname),
  "../../..",
  "logs"
);

function timestamp(): string {
  return new Date().toISOString();
}

function slug(): string {
  return new Date()
    .toISOString()
    .replace(/[:.]/g, "-")
    .replace("T", "_")
    .slice(0, 19);
}

export class ConvoLogger {
  private stream: fs.WriteStream;
  private filePath: string;

  constructor() {
    this.filePath = this.open();
    this.stream = fs.createWriteStream(this.filePath, { flags: "a" });
  }

  private open(): string {
    fs.mkdirSync(LOGS_DIR, { recursive: true });
    return path.join(LOGS_DIR, `chat-${slug()}.log`);
  }

  // Start a fresh file (called on /api/reset)
  rotate(): void {
    this.stream.end();
    this.filePath = this.open();
    this.stream = fs.createWriteStream(this.filePath, { flags: "a" });
  }

  private write(line: string): void {
    this.stream.write(line + "\n");
  }

  logUser(content: string): void {
    this.write(`\n[${timestamp()}] [user]`);
    this.write(content);
  }

  logAssistant(content: string): void {
    this.write(`\n[${timestamp()}] [assistant]`);
    this.write(content);
  }

  logToolCall(name: string, argsPreview: string): void {
    this.write(`\n[${timestamp()}] [tool call] ${name}`);
    if (argsPreview) this.write(argsPreview);
  }

  logToolResult(name: string, outcome: "ok" | "error", resultPreview: string): void {
    this.write(`[${timestamp()}] [tool result: ${outcome}] ${name}`);
    if (resultPreview) this.write(resultPreview);
  }

  logError(message: string): void {
    this.write(`\n[${timestamp()}] [error] ${message}`);
  }

  close(): void {
    this.stream.end();
  }

  get path(): string {
    return this.filePath;
  }
}
