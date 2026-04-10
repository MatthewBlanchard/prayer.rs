import { IMcpClient, McpTool } from "./mcp.js";

// Tools the player agent should never see — commander-only
const COMMANDER_ONLY = new Set([
  "list_sessions",
  "create_session",
  "register_session",
  "remove_session",
]);

/**
 * Wraps a shared IMcpClient and scopes it to a single session:
 * - Filters out commander-only tools
 * - Strips `session_handle` from every tool's input schema
 * - Auto-injects `session_handle` on every callTool invocation
 * - close() is a no-op (does not close the shared connection)
 */
export class SessionScopedMcpProxy implements IMcpClient {
  constructor(
    private readonly inner: IMcpClient,
    private readonly sessionHandle: string
  ) {}

  async listTools(): Promise<McpTool[]> {
    const tools = await this.inner.listTools();
    return tools
      .filter((t) => !COMMANDER_ONLY.has(t.name))
      .map(stripSessionHandleParam);
  }

  async callTool(
    name: string,
    args: Record<string, unknown>
  ): Promise<{ text: string; isError: boolean }> {
    return this.inner.callTool(name, { ...args, session_handle: this.sessionHandle });
  }

  async readResource(uri: string): Promise<string> {
    return this.inner.readResource(uri);
  }

  async close(): Promise<void> {
    // Intentionally a no-op — we don't own the shared connection
  }
}

function stripSessionHandleParam(tool: McpTool): McpTool {
  const schema = tool.inputSchema;
  if (!schema || typeof schema !== "object") return tool;

  const props = (schema as Record<string, unknown>)["properties"];
  if (!props || typeof props !== "object" || !("session_handle" in (props as object))) {
    return tool;
  }

  const newProps = { ...(props as Record<string, unknown>) };
  delete newProps["session_handle"];

  const required = (schema as Record<string, unknown>)["required"];
  const newRequired = Array.isArray(required)
    ? required.filter((r: unknown) => r !== "session_handle")
    : required;

  return {
    ...tool,
    inputSchema: {
      ...schema,
      properties: newProps,
      ...(newRequired !== undefined && { required: newRequired }),
    },
  };
}
