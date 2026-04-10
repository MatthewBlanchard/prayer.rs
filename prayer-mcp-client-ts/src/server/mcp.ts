import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";

export interface McpTool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface IMcpClient {
  listTools(): Promise<McpTool[]>;
  callTool(name: string, args: Record<string, unknown>): Promise<{ text: string; isError: boolean }>;
  readResource(uri: string): Promise<string>;
  close(): Promise<void>;
}

export class McpClient implements IMcpClient {
  private client: Client;
  private connected = false;

  constructor(
    private readonly mcpUrl: string,
    private readonly requestTimeoutMs: number
  ) {
    this.client = new Client({ name: "prayer-mcp-client-ts", version: "1.0.0" });
  }

  async connect(): Promise<void> {
    const transport = new StreamableHTTPClientTransport(new URL(this.mcpUrl));
    await this.client.connect(transport);
    this.connected = true;
  }

  async listTools(): Promise<McpTool[]> {
    const result = await this.client.listTools(
      undefined,
      this.requestOptions()
    );
    return result.tools.map((t) => ({
      name: t.name,
      description: t.description ?? "",
      inputSchema: (t.inputSchema as Record<string, unknown>) ?? {},
    }));
  }

  async callTool(name: string, args: Record<string, unknown>): Promise<{ text: string; isError: boolean }> {
    const result = await this.client.callTool(
      { name, arguments: args },
      undefined,
      this.requestOptions()
    );
    console.log(`[mcp] callTool raw response for "${name}":`, JSON.stringify(result));
    const content = result.content;
    let text: string;
    if (Array.isArray(content)) {
      text = content
        .filter((c) => c.type === "text")
        .map((c) => (c as { type: "text"; text: string }).text)
        .join("\n");
    } else {
      text = JSON.stringify(content);
    }
    const isError = result.isError === true;
    console.log(`[mcp] callTool "${name}" isError=${isError} text=${text.slice(0, 200)}`);
    return { text, isError };
  }

  async readResource(uri: string): Promise<string> {
    const result = await this.client.readResource(
      { uri },
      this.requestOptions()
    );
    const contents = result.contents;
    if (Array.isArray(contents)) {
      return contents
        .filter((c) => "text" in c)
        .map((c) => (c as { text: string }).text)
        .join("\n");
    }
    return JSON.stringify(contents);
  }

  async close(): Promise<void> {
    if (this.connected) {
      await this.client.close();
      this.connected = false;
    }
  }

  private requestOptions(): {
    timeout: number;
    resetTimeoutOnProgress: boolean;
  } {
    return {
      timeout: this.requestTimeoutMs,
      resetTimeoutOnProgress: true,
    };
  }
}

// ---------------------------------------------------------------------------
// Argument helpers (mirror mcp.rs)
// ---------------------------------------------------------------------------

export function parseJsonObject(
  json: string
): Record<string, unknown> | undefined {
  try {
    const parsed = JSON.parse(json);
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // ignore
  }
  return undefined;
}

export function extractUriArgument(argsJson: string): string | undefined {
  const obj = parseJsonObject(argsJson);
  if (!obj) return undefined;
  const uri = obj["uri"];
  return typeof uri === "string" ? uri : undefined;
}
