import { useEffect, useReducer, useRef, useState } from "react";
import {
  connectEvents,
  fetchAgents,
  pauseAgent,
  resumeAgent,
  resetConversation,
  sendMessage,
  setAgentObjective,
  syncAgents,
  ServerEvent,
} from "./api.js";
import ChatPane from "./ChatPane.js";
import InputBar from "./InputBar.js";
import AgentsPanel, { AgentState } from "./AgentsPanel.js";
import { AgentFeedItem, TranscriptItem } from "../shared/types.js";

// ---------------------------------------------------------------------------
// State management
// ---------------------------------------------------------------------------

type AppState = {
  items: TranscriptItem[];
  busy: boolean;
  model: string;
  status: string;
};

type Action =
  | { type: "state_sync"; messages: Record<string, unknown>[]; model: string; busy: boolean }
  | { type: "turn_started" }
  | { type: "assistant_draft"; content: string | null }
  | { type: "tool_call_started"; toolCallId: string; name: string; argsPreview: string }
  | { type: "tool_call_completed"; toolCallId: string; name: string; outcome: "ok" | "error"; resultPreview: string }
  | { type: "turn_completed" }
  | { type: "error"; message: string }
  | { type: "user_message"; content: string }
  | { type: "reset" }
  | { type: "set_model"; model: string };

function syncItems(messages: Record<string, unknown>[]): TranscriptItem[] {
  const items: TranscriptItem[] = [];

  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    const role = msg["role"] as string;
    const content = (msg["content"] as string | null | undefined) ?? "";

    if (role === "user" && content) {
      items.push({ kind: "user", content });
    } else if (role === "assistant") {
      if (content) items.push({ kind: "assistant", content });

      const toolCalls = msg["tool_calls"] as Array<Record<string, unknown>> | undefined;
      if (toolCalls) {
        for (const call of toolCalls) {
          const callId = (call["id"] as string | undefined) ?? "";
          const fn = call["function"] as Record<string, unknown> | undefined;
          const name = (fn?.["name"] as string | undefined) ?? "unknown";

          // Find matching tool result
          const result = messages.slice(i + 1).find(
            (m) =>
              (m["role"] as string) === "tool" &&
              (m["tool_call_id"] as string | undefined) === callId
          );

          const argsPreview = (fn?.["arguments"] as string | undefined) ?? "{}";
          const resultPreview = result
            ? ((result["content"] as string | undefined) ?? "")
            : null;

          const status: "ok" | "error" | "running" = result
            ? (result["isError"] as boolean | undefined) ? "error" : "ok"
            : "running";

          items.push({ kind: "tool_card", toolCallId: callId, name, status, argsPreview, resultPreview });
        }
      }
    }
    // skip "tool" messages — shown as part of tool_card above
  }

  return items;
}

function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "state_sync": {
      return {
        ...state,
        items: syncItems(action.messages),
        busy: action.busy,
        model: action.model || state.model,
      };
    }

    case "set_model":
      return { ...state, model: action.model };

    case "turn_started":
      return { ...state, busy: true, status: "Thinking..." };

    case "user_message":
      return {
        ...state,
        items: [...state.items, { kind: "user", content: action.content }],
      };

    case "assistant_draft": {
      if (!action.content?.trim()) return state;
      // Replace the last assistant item if it exists, otherwise append
      const items = [...state.items];
      const lastIdx = items.length - 1;
      if (lastIdx >= 0 && items[lastIdx].kind === "assistant") {
        items[lastIdx] = { kind: "assistant", content: action.content };
      } else {
        items.push({ kind: "assistant", content: action.content });
      }
      return { ...state, items, status: "Responding..." };
    }

    case "tool_call_started": {
      const items = [...state.items];
      items.push({
        kind: "tool_card",
        toolCallId: action.toolCallId,
        name: action.name,
        status: "running",
        argsPreview: action.argsPreview,
        resultPreview: null,
      });
      return { ...state, items, status: `Calling ${action.name}...` };
    }

    case "tool_call_completed": {
      const items = state.items.map((item) => {
        if (item.kind === "tool_card" && item.toolCallId === action.toolCallId) {
          return {
            ...item,
            status: action.outcome,
            resultPreview: action.resultPreview,
          };
        }
        return item;
      });
      return { ...state, items, status: `${action.name}: ${action.outcome}` };
    }

    case "turn_completed":
      return { ...state, busy: false, status: "Ready" };

    case "error": {
      const items = [...state.items, { kind: "error" as const, message: action.message }];
      return { ...state, items, busy: false, status: "Error" };
    }

    case "reset":
      return { ...state, items: [], busy: false, status: "Conversation cleared" };

    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Agent state helpers
// ---------------------------------------------------------------------------

const MAX_FEED_ITEMS = 30;

function applyAgentEvent(agents: AgentState[], source: string, event: ServerEvent): AgentState[] {
  return agents.map((a) => {
    if (a.sessionHandle !== source) return a;

    let feed = [...a.feed];

    if (event.type === "tool_call_started") {
      const item: AgentFeedItem = {
        kind: "tool_call",
        toolCallId: event.toolCallId,
        name: event.name,
        status: "running",
        resultPreview: null,
      };
      feed = [...feed, item].slice(-MAX_FEED_ITEMS);
    } else if (event.type === "tool_call_completed") {
      feed = feed.map((f) =>
        f.kind === "tool_call" && f.toolCallId === event.toolCallId
          ? { ...f, status: event.outcome, resultPreview: event.resultPreview }
          : f
      );
    } else if (event.type === "error") {
      feed = [...feed, { kind: "error" as const, message: event.message }].slice(-MAX_FEED_ITEMS);
    }

    return { ...a, feed };
  });
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export default function App() {
  const [state, dispatch] = useReducer(reducer, {
    items: [],
    busy: false,
    model: "",
    status: "Connecting...",
  });

  const [agents, setAgents] = useState<AgentState[]>([]);
  const [inputError, setInputError] = useState<string | null>(null);
  const pendingUserMsg = useRef<string | null>(null);

  // Load initial agent list
  useEffect(() => {
    fetchAgents().then((list) =>
      setAgents(list.map((a) => ({ ...a, feed: [] })))
    ).catch(() => {});
  }, []);

  useEffect(() => {
    const disconnect = connectEvents(
      (event: ServerEvent) => {
        const source = (event as Record<string, unknown>)["source"] as string | undefined;
        const isPlayerEvent = source && source !== "commander";

        if (isPlayerEvent) {
          setAgents((prev) => applyAgentEvent(prev, source, event));
          return;
        }

        switch (event.type) {
          case "state_sync":
            dispatch({
              type: "state_sync",
              messages: event.messages,
              model: event.model,
              busy: event.busy,
            });
            break;
          case "turn_started":
            dispatch({ type: "turn_started" });
            break;
          case "assistant_draft":
            dispatch({ type: "assistant_draft", content: event.content });
            break;
          case "tool_call_started":
            dispatch({
              type: "tool_call_started",
              toolCallId: event.toolCallId,
              name: event.name,
              argsPreview: event.argsPreview,
            });
            break;
          case "tool_call_completed":
            dispatch({
              type: "tool_call_completed",
              toolCallId: event.toolCallId,
              name: event.name,
              outcome: event.outcome,
              resultPreview: event.resultPreview,
            });
            break;
          case "turn_completed":
            dispatch({ type: "turn_completed" });
            break;
          case "error":
            dispatch({ type: "error", message: event.message });
            break;
        }
      },
      () => {
        dispatch({ type: "error", message: "Lost connection to server" });
      }
    );

    return disconnect;
  }, []);

  async function handleSubmit(content: string) {
    if (!content.trim() || state.busy) return;
    setInputError(null);

    if (content.trim() === "/clear") {
      await resetConversation();
      dispatch({ type: "reset" });
      return;
    }

    pendingUserMsg.current = content;
    dispatch({ type: "user_message", content });

    try {
      await sendMessage(content);
    } catch (err) {
      setInputError(err instanceof Error ? err.message : String(err));
      dispatch({ type: "error", message: String(err) });
    }
  }

  async function handlePause(handle: string) {
    setAgents((prev) => prev.map((a) => a.sessionHandle === handle ? { ...a, paused: true } : a));
    await pauseAgent(handle);
  }

  async function handleResume(handle: string) {
    setAgents((prev) => prev.map((a) => a.sessionHandle === handle ? { ...a, paused: false } : a));
    await resumeAgent(handle);
  }

  async function handleObjective(handle: string, objective: string) {
    await setAgentObjective(handle, objective);
  }

  async function handleSync() {
    const list = await syncAgents();
    setAgents((prev) => {
      const next = list.map((a) => {
        const existing = prev.find((p) => p.sessionHandle === a.sessionHandle);
        return existing ? { ...existing, paused: a.paused } : { ...a, feed: [] };
      });
      return next;
    });
  }

  const modelLabel = state.model ? ` [${state.model}]` : "";

  return (
    <div className="app">
      <header className="app-header">
        <span className="app-title">Prayer Chat{modelLabel}</span>
        <span className="app-status" data-busy={state.busy}>
          {state.busy ? "⟳ " : ""}{state.status}
        </span>
      </header>

      <div className="app-body">
        <div className="app-chat">
          <ChatPane items={state.items} busy={state.busy} />
          <InputBar
            onSubmit={handleSubmit}
            disabled={state.busy}
            error={inputError}
          />
        </div>
        <AgentsPanel
          agents={agents}
          onPause={handlePause}
          onResume={handleResume}
          onObjective={handleObjective}
          onSync={handleSync}
        />
      </div>
    </div>
  );
}
