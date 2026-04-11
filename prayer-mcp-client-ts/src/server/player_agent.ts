import { CompletionError, CompletionProvider, Message } from "./llm.js";
import { IMcpClient } from "./mcp.js";
import { SessionScopedMcpProxy } from "./session_proxy.js";
import {
  ChatToolLoop,
  CompactionState,
  defaultLoopConfig,
  newCompactionState,
  ToolLoopEvent,
} from "./tool_loop.js";

const IDLE_SLEEP_MS = 5_000;
const ERROR_BACKOFF_MS = 15_000;
const SESSION_SYNC_INTERVAL_MS = 60_000;
const DEFAULT_MIND_HISTORY_LIMIT = 60;

export type AgentMindMessage = {
  role: string;
  content: string | null;
  toolCallId?: string;
  isError?: boolean;
  toolCalls?: Array<{ id: string; name: string; arguments: string }>;
};

export type AgentMindSnapshot = {
  objective: string;
  compactionSummary: string | null;
  messages: AgentMindMessage[];
};

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

function buildPlayerSystemPrompt(
  sessionHandle: string,
  dslReference: string | undefined,
  objective: string
): string {
  let prompt = [
    `You are an autonomous agent managing SpaceMolt player "${sessionHandle}".`,
    "Your tools are pre-scoped to your session — do not include session_handle in any tool arguments.",
    "At the start of a new turn, call fs_ls on / to get your bearings before choosing actions.",
    "Operate continuously: mine resources, fulfill missions, manage cargo and fuel as needed.",
    "When running a script, pass only raw PrayerLang script text — no markdown fences, no prose.",
    "Use fs_read/fs_query to inspect state before acting.",
    "After each script completes, assess the result and decide your next action.",
    "If there is nothing productive to do right now, call passthrough with a lightweight status check.",
  ].join(" ");

  if (objective.trim()) {
    prompt += `\n\n## Current Objective\n${objective}`;
  }

  if (dslReference?.trim()) {
    prompt += `\n\n## MCP Reference\n${dslReference}`;
  }

  return prompt;
}

// ---------------------------------------------------------------------------
// PlayerAgent
// ---------------------------------------------------------------------------

export class PlayerAgent {
  private readonly loop: ChatToolLoop;
  private readonly sessionMcp: IMcpClient;
  private readonly messages: Message[];
  private readonly compaction: CompactionState;
  private stopped = false;
  private pauseGate: Promise<void> | undefined;
  private resolvePause: (() => void) | undefined;
  private haltInFlight = false;
  private objective = "";

  constructor(
    private readonly sessionHandle: string,
    provider: CompletionProvider,
    sharedMcp: IMcpClient,
    model: string,
    private readonly dslReference: string | undefined,
    private readonly onEvent: (sessionHandle: string, event: ToolLoopEvent) => void
  ) {
    const proxy = new SessionScopedMcpProxy(sharedMcp, sessionHandle);
    this.sessionMcp = proxy;
    this.loop = new ChatToolLoop(provider, proxy, model, {
      ...defaultLoopConfig,
      includeSyntheticReadResource: true,
    });
    this.messages = [
      {
        role: "system",
        content: buildPlayerSystemPrompt(sessionHandle, dslReference, this.objective),
      },
    ];
    this.compaction = newCompactionState();

    // New agents start paused by default.
    this.pause();
  }

  setObjective(objective: string): void {
    this.objective = objective;
    this.messages.push({
      role: "user",
      content: objective,
    });
    this.resume();
  }

  pause(): void {
    if (this.pauseGate) return; // already paused
    this.pauseGate = new Promise((resolve) => {
      this.resolvePause = resolve;
    });
    void this.haltSessionNow();
  }

  resume(): void {
    this.resolvePause?.();
    this.pauseGate = undefined;
    this.resolvePause = undefined;
  }

  isPaused(): boolean {
    return this.pauseGate !== undefined;
  }

  stop(): void {
    this.stopped = true;
  }

  async clearContext(): Promise<void> {
    this.pause();
    this.messages.length = 0;
    this.messages.push({
      role: "system",
      content: buildPlayerSystemPrompt(this.sessionHandle, this.dslReference, this.objective),
    });
    this.compaction.summary = undefined;
  }

  getMindSnapshot(maxMessages = DEFAULT_MIND_HISTORY_LIMIT): AgentMindSnapshot {
    const fromIndex = Math.max(1, this.messages.length - Math.max(1, maxMessages));
    const recent = this.messages.slice(fromIndex);
    return {
      objective: this.objective,
      compactionSummary: this.compaction.summary ?? null,
      messages: recent.map((m) => normalizeMindMessage(m)),
    };
  }

  async start(): Promise<void> {
    await this.loop.connect();

    while (!this.stopped) {
      // Paused: block here until resume() resolves the gate, preserving all state.
      // Any in-flight turn (e.g. blocking run_script) completes before this is
      // reached — we don't interrupt mid-execution.
      if (this.pauseGate) {
        await this.pauseGate;
        continue;
      }

      let toolCallCount = 0;

      try {
        await this.loop.runTurn(this.messages, this.compaction, (event) => {
          if (event.type === "assistant_draft") {
            toolCallCount = event.toolCallCount;
          }
          this.onEvent(this.sessionHandle, event);
        });
      } catch (err) {
        let message = err instanceof Error ? err.message : String(err);
        if (err instanceof CompletionError && err.body) {
          message += ` — ${err.body}`;
        }
        console.error(`[player-agent:${this.sessionHandle}]`, message);
        this.onEvent(this.sessionHandle, { type: "error", message });
        await sleep(ERROR_BACKOFF_MS);
        continue;
      }

      // Only idle-sleep when the LLM produced no tool calls (pure text turn).
      // run_script blocking provides natural throttling for active turns.
      if (toolCallCount === 0 && !this.stopped && !this.pauseGate) {
        await sleep(IDLE_SLEEP_MS);
      }
    }
  }

  private async haltSessionNow(): Promise<void> {
    if (this.haltInFlight) return;
    this.haltInFlight = true;
    try {
      await this.sessionMcp.callTool("halt_session", {
        reason: "paused by agent control",
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error(`[player-agent:${this.sessionHandle}] halt_session failed: ${message}`);
      this.onEvent(this.sessionHandle, { type: "error", message: `halt_session failed: ${message}` });
    } finally {
      this.haltInFlight = false;
    }
  }
}

// ---------------------------------------------------------------------------
// PlayerAgentManager
// ---------------------------------------------------------------------------

export class PlayerAgentManager {
  private readonly agents = new Map<string, PlayerAgent>();
  private syncTimer: ReturnType<typeof setInterval> | undefined;

  constructor(
    private readonly provider: CompletionProvider,
    private readonly sharedMcp: IMcpClient,
    private readonly model: string,
    private readonly dslReference: string | undefined,
    private readonly onEvent: (sessionHandle: string, event: ToolLoopEvent) => void
  ) {}

  async syncSessions(): Promise<void> {
    let handles: string[] = [];

    try {
      const result = await this.sharedMcp.callTool("list_sessions", {});
      if (!result.isError && result.text.trim()) {
        const parsed: unknown = JSON.parse(result.text);
        if (Array.isArray(parsed)) {
          handles = parsed
            .map((s: unknown) =>
              typeof s === "object" && s !== null
                ? ((s as Record<string, unknown>)["playerName"] as string | undefined)
                : undefined
            )
            .filter((h): h is string => typeof h === "string" && h.length > 0);
        }
      }
    } catch (err) {
      console.error("[player-agent-manager] syncSessions failed:", err);
      return;
    }

    // Start agents for new sessions
    for (const handle of handles) {
      if (!this.agents.has(handle)) {
        console.log(`[player-agent-manager] starting agent for "${handle}"`);
        this.spawnAgent(handle);
      }
    }

    // Stop agents whose sessions no longer exist
    for (const handle of [...this.agents.keys()]) {
      if (!handles.includes(handle)) {
        console.log(`[player-agent-manager] stopping agent for "${handle}"`);
        this.agents.get(handle)!.stop();
        this.agents.delete(handle);
      }
    }
  }

  setObjective(sessionHandle: string, objective: string): boolean {
    const agent = this.agents.get(sessionHandle);
    if (!agent) return false;
    agent.setObjective(objective);
    return true;
  }

  pauseAgent(sessionHandle: string): boolean {
    const agent = this.agents.get(sessionHandle);
    if (!agent) return false;
    agent.pause();
    return true;
  }

  resumeAgent(sessionHandle: string): boolean {
    const agent = this.agents.get(sessionHandle);
    if (!agent) return false;
    agent.resume();
    return true;
  }

  listAgents(): Array<{ sessionHandle: string; paused: boolean }> {
    return [...this.agents.entries()].map(([sessionHandle, agent]) => ({
      sessionHandle,
      paused: agent.isPaused(),
    }));
  }

  async clearContext(sessionHandle: string): Promise<boolean> {
    const agent = this.agents.get(sessionHandle);
    if (!agent) return false;
    await agent.clearContext();
    return true;
  }

  getMindSnapshot(
    sessionHandle: string,
    maxMessages = DEFAULT_MIND_HISTORY_LIMIT
  ): AgentMindSnapshot | null {
    const agent = this.agents.get(sessionHandle);
    if (!agent) return null;
    return agent.getMindSnapshot(maxMessages);
  }

  startSyncInterval(): void {
    this.syncTimer = setInterval(
      () => void this.syncSessions(),
      SESSION_SYNC_INTERVAL_MS
    );
  }

  async stopAll(): Promise<void> {
    if (this.syncTimer) {
      clearInterval(this.syncTimer);
      this.syncTimer = undefined;
    }
    for (const agent of this.agents.values()) {
      agent.stop();
    }
    this.agents.clear();
  }

  private spawnAgent(sessionHandle: string): void {
    const agent = new PlayerAgent(
      sessionHandle,
      this.provider,
      this.sharedMcp,
      this.model,
      this.dslReference,
      this.onEvent
    );
    this.agents.set(sessionHandle, agent);
    agent.start().catch((err: unknown) => {
      console.error(`[player-agent:${sessionHandle}] fatal error:`, err);
      this.agents.delete(sessionHandle);
    });
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function normalizeMindMessage(message: Message): AgentMindMessage {
  const role = typeof message["role"] === "string" ? message["role"] : "unknown";
  const rawContent = message["content"];
  const content = normalizeContent(rawContent);
  const out: AgentMindMessage = { role, content };

  const toolCallId = message["tool_call_id"];
  if (typeof toolCallId === "string" && toolCallId) {
    out.toolCallId = toolCallId;
  }
  if (typeof message["isError"] === "boolean") {
    out.isError = message["isError"];
  }

  const rawToolCalls = message["tool_calls"];
  if (Array.isArray(rawToolCalls)) {
    const toolCalls = rawToolCalls
      .map((raw) => {
        if (!raw || typeof raw !== "object") return null;
        const record = raw as Record<string, unknown>;
        const id = typeof record["id"] === "string" ? record["id"] : "";
        const fn =
          typeof record["function"] === "object" && record["function"] !== null
            ? (record["function"] as Record<string, unknown>)
            : undefined;
        const name = typeof fn?.["name"] === "string" ? fn["name"] : "unknown";
        const args =
          typeof fn?.["arguments"] === "string"
            ? fn["arguments"]
            : safeJsonStringify(fn?.["arguments"] ?? {});
        return { id: id || `call_${name}`, name, arguments: args };
      })
      .filter((v): v is { id: string; name: string; arguments: string } => v !== null);
    if (toolCalls.length > 0) out.toolCalls = toolCalls;
  }

  return out;
}

function normalizeContent(value: unknown): string | null {
  if (value == null) return null;
  if (typeof value === "string") return value;
  return safeJsonStringify(value);
}

function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
