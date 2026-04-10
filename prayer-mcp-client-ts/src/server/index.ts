import express, { Request, Response } from "express";
import { fileURLToPath } from "url";
import path from "path";
import { McpClient } from "./mcp.js";
import { GeminiProvider, Message, OpenAiProvider } from "./llm.js";
import {
  ChatToolLoop,
  CompactionState,
  defaultLoopConfig,
  LoopConfig,
  newCompactionState,
  ToolLoopEvent,
} from "./tool_loop.js";
import { ConvoLogger } from "./logger.js";
import { PlayerAgentManager } from "./player_agent.js";

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

function parseArgs(): {
  provider: "google" | "openai";
  model: string;
  llmBaseUrl: string;
  apiKey: string;
  mcpUrl: string;
  prayerApiUrl: string;
  mcpRequestTimeoutMs: number;
  port: number;
} {
  const EFFECTIVE_NO_TIMEOUT_MS = 2_147_483_647; // max safe setTimeout delay (~24.8 days)

  const args = process.argv.slice(2);
  const get = (flag: string, env?: string, fallback = ""): string => {
    const idx = args.indexOf(flag);
    if (idx !== -1 && args[idx + 1]) return args[idx + 1];
    if (env && process.env[env]) return process.env[env]!;
    return fallback;
  };

  const provider = (
    get("--provider", "PRAYER_MCP_CLIENT_PROVIDER", "google") as string
  ).toLowerCase() as "google" | "openai";

  const model = get("--model", "PRAYER_MCP_CLIENT_MODEL", "gemma-4-31b-it");
  const llmBaseUrl = get(
    "--llm-base-url",
    "PRAYER_MCP_CLIENT_LLM_BASE_URL",
    "https://api.openai.com/v1"
  );
  const mcpUrl = get(
    "--mcp-url",
    "PRAYER_MCP_CLIENT_MCP_URL",
    "http://127.0.0.1:5000/mcp"
  );
  const mcpRequestTimeoutMsStr = get(
    "--mcp-request-timeout-ms",
    "PRAYER_MCP_CLIENT_REQUEST_TIMEOUT_MS",
    "1800000"
  );
  const parsedMcpTimeout = parseInt(mcpRequestTimeoutMsStr, 10);
  const mcpRequestTimeoutMs =
    Number.isFinite(parsedMcpTimeout) && parsedMcpTimeout > 0
      ? parsedMcpTimeout
      : parsedMcpTimeout === 0
        ? EFFECTIVE_NO_TIMEOUT_MS
        : 1_800_000;
  const portStr = get("--port", "PRAYER_MCP_CLIENT_PORT", "3001");
  const port = parseInt(portStr, 10) || 3001;

  let apiKey = get("--api-key", "PRAYER_MCP_CLIENT_API_KEY");
  if (!apiKey) {
    apiKey =
      provider === "google"
        ? (process.env["GEMINI_API_KEY"] ?? "")
        : (process.env["OPENAI_API_KEY"] ?? "");
  }

  if (!apiKey) {
    const envName = provider === "google" ? "GEMINI_API_KEY" : "OPENAI_API_KEY";
    console.error(
      `Missing API key. Set ${envName} or pass --api-key.`
    );
    process.exit(1);
  }

  const prayerApiUrl = get(
    "--prayer-api-url",
    "PRAYER_MCP_CLIENT_API_URL",
    "http://127.0.0.1:3000"
  );

  return {
    provider,
    model,
    llmBaseUrl,
    apiKey,
    mcpUrl,
    prayerApiUrl,
    mcpRequestTimeoutMs,
    port,
  };
}

// ---------------------------------------------------------------------------
// System prompt (mirrors main.rs build_message_history)
// ---------------------------------------------------------------------------

function buildSystemPrompt(
  bootstrapReference: string | undefined,
  bootstrapSessionSnapshot: string | undefined
): string {
  let prompt = [
    "You are the fleet overmind for SpaceMolt, managing operations by writing PrayerLang scripts.",
    "When the user asks for an action or automation, write the script AND immediately run it with run_script — do not echo the script as text and wait.",
    "When generating a script, pass only script text to run_script (no markdown fences, no prose).",
    "Do not imply mined resources move automatically: mining fills cargo only.",
    "Use explicit commands for disposition: stash to storage, sell to market.",
    "If the user asks for an 'overview', interpret it as a sessions/bots overview unless they specify another target.",
    "If the user asks a direct question instead of requesting a script, answer briefly and accurately.",
    "When invoking session-scoped MCP tools, pass session_handle (from list_sessions playerName), never session IDs.",
    "For world-state questions, inspect MCP VFS first (fs_read/fs_query) before answering.",
    "For travel options, read /status.json for current system, then check /systems/{system_id}.json.",
    "For nearby POIs or stations in the current system, read /systems/{system_id}.json.",
    "When listing POIs, use the POI objects from /systems/{system_id}.json directly.",
    "Use direct fs_read on concrete paths for location/travel questions; avoid broad fs_query filters when a path is known.",
    "If a required source is missing or unreadable, report the relevant data as unknown/unavailable in current state.",
    "If the source is present and a field is explicitly empty (for example resources: []), report that as no resources listed/known there right now (not unknown).",
  ].join(" ");

  if (bootstrapReference?.trim()) {
    prompt += "\n\n## MCP Reference\n" + bootstrapReference;
  }

  if (bootstrapSessionSnapshot?.trim()) {
    prompt +=
      "\n\n## Startup Session Snapshot\n" +
      "This was captured at client startup and may be stale. Refresh with list_sessions when needed.\n" +
      bootstrapSessionSnapshot;
  }

  return prompt;
}

// ---------------------------------------------------------------------------
// SSE broadcasting
// ---------------------------------------------------------------------------

const sseClients = new Set<Response>();

function broadcast(event: ToolLoopEvent, source = "commander"): void {
  const data = `event: ${event.type}\ndata: ${JSON.stringify({ ...event, source })}\n\n`;
  for (const client of sseClients) {
    try {
      client.write(data);
    } catch {
      sseClients.delete(client);
    }
  }
}

// ---------------------------------------------------------------------------
// Prayer API proxy (resolves handle → UUID, fetches snapshots)
// ---------------------------------------------------------------------------

class PrayerApiProxy {
  private labelToId = new Map<string, string>();

  constructor(private readonly baseUrl: string) {}

  private async refreshSessions(): Promise<void> {
    const res = await fetch(`${this.baseUrl}/api/runtime/sessions`);
    if (!res.ok) return;
    const sessions = (await res.json()) as Array<Record<string, unknown>>;
    this.labelToId.clear();
    for (const s of sessions) {
      const label = s["label"] as string | undefined;
      const id = s["id"] as string | undefined;
      if (label && id) this.labelToId.set(label, id);
    }
  }

  async getSnapshot(handle: string): Promise<Record<string, unknown> | null> {
    let id = this.labelToId.get(handle);
    if (!id) {
      await this.refreshSessions();
      id = this.labelToId.get(handle);
    }
    if (!id) return null;
    const res = await fetch(`${this.baseUrl}/api/runtime/sessions/${id}/snapshot`);
    if (!res.ok) return null;
    return res.json() as Promise<Record<string, unknown>>;
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const {
    provider,
    model,
    llmBaseUrl,
    apiKey,
    mcpUrl,
    prayerApiUrl,
    mcpRequestTimeoutMs,
    port,
  } = parseArgs();

  console.log(`Connecting to MCP at ${mcpUrl}...`);
  console.log(`MCP request timeout: ${mcpRequestTimeoutMs}ms`);
  const mcp = new McpClient(mcpUrl, mcpRequestTimeoutMs);
  await mcp.connect();
  console.log("MCP connected.");

  const llmProvider =
    provider === "google"
      ? new GeminiProvider(apiKey)
      : new OpenAiProvider(llmBaseUrl, apiKey);

  const loopConfig: LoopConfig = { ...defaultLoopConfig };
  const loop = new ChatToolLoop(llmProvider, mcp, model, loopConfig);
  await loop.connect();
  console.log(`LLM provider: ${provider}, model: ${model}`);

  const playerManager = new PlayerAgentManager(
    llmProvider,
    mcp,
    model,
    loop.getBootstrapReference(),
    (sessionHandle, event) => broadcast(event, sessionHandle)
  );
  await playerManager.syncSessions();
  playerManager.startSyncInterval();
  console.log("Player agent manager started.");

  const prayerProxy = new PrayerApiProxy(prayerApiUrl);

  const systemPrompt = buildSystemPrompt(
    loop.getBootstrapReference(),
    loop.getBootstrapSessionSnapshot()
  );

  // Conversation state (single session per server instance)
  const messages: Message[] = [{ role: "system", content: systemPrompt }];
  let compaction: CompactionState = newCompactionState();
  let busy = false;
  const logger = new ConvoLogger();
  console.log(`Logging conversation to ${logger.path}`);

  // ---------------------------------------------------------------------------
  // Express app
  // ---------------------------------------------------------------------------

  const app = express();
  app.use(express.json());

  // Serve built frontend in production
  const __dirname = path.dirname(fileURLToPath(import.meta.url));
  const publicDir = path.join(__dirname, "../public");
  app.use(express.static(publicDir));

  // SSE endpoint
  app.get("/events", (req: Request, res: Response) => {
    res.setHeader("Content-Type", "text/event-stream");
    res.setHeader("Cache-Control", "no-cache");
    res.setHeader("Connection", "keep-alive");
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.flushHeaders();

    // Send current state immediately on connect
    const stateEvent = {
      type: "state_sync" as const,
      messages: messages.filter((m) => (m["role"] as string) !== "system"),
      model,
      busy,
    };
    res.write(`event: state_sync\ndata: ${JSON.stringify(stateEvent)}\n\n`);

    sseClients.add(res);

    req.on("close", () => {
      sseClients.delete(res);
    });
  });

  // Submit a user message
  app.post("/api/chat", async (req: Request, res: Response) => {
    const content = (req.body as { content?: string }).content?.trim();
    if (!content) {
      res.status(400).json({ error: "content is required" });
      return;
    }

    if (busy) {
      res.status(409).json({ error: "busy" });
      return;
    }

    messages.push({ role: "user", content });
    logger.logUser(content);
    res.json({ ok: true });

    busy = true;
    try {
      await loop.runTurn(messages, compaction, (event) => {
        broadcast(event);
        switch (event.type) {
          case "assistant_draft":
            if (event.content?.trim()) logger.logAssistant(event.content);
            break;
          case "tool_call_started":
            logger.logToolCall(event.name, event.argsPreview);
            break;
          case "tool_call_completed":
            logger.logToolResult(event.name, event.outcome, event.resultPreview);
            break;
          case "error":
            logger.logError(event.message);
            break;
        }
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      logger.logError(msg);
      broadcast({ type: "error", message: msg });
    } finally {
      busy = false;
      // Broadcast updated non-system messages so client can re-sync
      const syncEvent = {
        type: "state_sync",
        messages: messages.filter((m) => (m["role"] as string) !== "system"),
        model,
        busy: false,
      };
      for (const client of sseClients) {
        try {
          client.write(
            `event: state_sync\ndata: ${JSON.stringify(syncEvent)}\n\n`
          );
        } catch {
          sseClients.delete(client);
        }
      }
    }
  });

  // Clear conversation
  app.post("/api/reset", (_req: Request, res: Response) => {
    if (busy) {
      res.status(409).json({ error: "busy" });
      return;
    }
    messages.length = 0;
    messages.push({ role: "system", content: systemPrompt });
    compaction = newCompactionState();
    logger.rotate();
    console.log(`Logging new conversation to ${logger.path}`);
    res.json({ ok: true });

    broadcast({ type: "turn_completed", finalContent: null });
    const syncEvent = {
      type: "state_sync",
      messages: [],
      model,
      busy: false,
    };
    for (const client of sseClients) {
      try {
        client.write(
          `event: state_sync\ndata: ${JSON.stringify(syncEvent)}\n\n`
        );
      } catch {
        sseClients.delete(client);
      }
    }
  });

  // Player agent endpoints
  app.get("/api/agents", (_req: Request, res: Response) => {
    res.json(playerManager.listAgents());
  });

  app.post("/api/agents/sync", async (_req: Request, res: Response) => {
    await playerManager.syncSessions();
    res.json(playerManager.listAgents());
  });

  app.get("/api/agents/:handle/snapshot", async (req: Request, res: Response) => {
    const handle = req.params["handle"] ?? "";
    const snapshot = await prayerProxy.getSnapshot(handle);
    if (!snapshot) {
      res.status(404).json({ error: `no session found for "${handle}"` });
      return;
    }
    res.json(snapshot);
  });

  app.post("/api/agents/:handle/pause", (req: Request, res: Response) => {
    const handle = req.params["handle"] ?? "";
    const ok = playerManager.pauseAgent(handle);
    if (!ok) {
      res.status(404).json({ error: `no agent running for "${handle}"` });
      return;
    }
    res.json({ ok: true, paused: true });
  });

  app.post("/api/agents/:handle/resume", (req: Request, res: Response) => {
    const handle = req.params["handle"] ?? "";
    const ok = playerManager.resumeAgent(handle);
    if (!ok) {
      res.status(404).json({ error: `no agent running for "${handle}"` });
      return;
    }
    res.json({ ok: true, paused: false });
  });

  app.post("/api/agents/:handle/objective", (req: Request, res: Response) => {
    const handle = req.params["handle"] ?? "";
    const objective = ((req.body as { objective?: string }).objective ?? "").trim();
    if (!objective) {
      res.status(400).json({ error: "objective is required" });
      return;
    }
    const ok = playerManager.setObjective(handle, objective);
    if (!ok) {
      res.status(404).json({ error: `no agent running for "${handle}"` });
      return;
    }
    res.json({ ok: true });
  });

  // Health check
  app.get("/api/health", (_req: Request, res: Response) => {
    res.json({ ok: true, model, provider, busy });
  });

  app.listen(port, () => {
    console.log(`prayer-mcp-client-ts running at http://localhost:${port}`);
  });

  // Graceful shutdown
  process.on("SIGINT", async () => {
    console.log("\nShutting down...");
    logger.close();
    await playerManager.stopAll();
    await mcp.close();
    process.exit(0);
  });
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
