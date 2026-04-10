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
  private readonly messages: Message[];
  private readonly compaction: CompactionState;
  private stopped = false;
  private pauseGate: Promise<void> | undefined;
  private resolvePause: (() => void) | undefined;
  private objective = "";

  constructor(
    private readonly sessionHandle: string,
    provider: CompletionProvider,
    sharedMcp: IMcpClient,
    model: string,
    dslReference: string | undefined,
    private readonly onEvent: (sessionHandle: string, event: ToolLoopEvent) => void
  ) {
    const proxy = new SessionScopedMcpProxy(sharedMcp, sessionHandle);
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
    // Start paused — waits for an objective before calling the LLM
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
