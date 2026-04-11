import { useState } from "react";
import { AgentInfo } from "../shared/types.js";

export type RunningScript = {
  script: string;
  currentLine: number | null;
};

export type AgentState = AgentInfo & {
  runningScript: RunningScript | null;
  currentSystem: string | null;
};

function ScriptViewer({ script, currentLine }: { script: string; currentLine: number | null }) {
  const lines = script.split("\n");
  return (
    <div className="agent-script-viewer">
      {lines.map((line, i) => {
        const active = currentLine !== null && i === currentLine;
        return (
          <div key={i} className={`agent-script-line${active ? " agent-script-line--active" : ""}`}>
            <span className="agent-script-lineno">{i + 1}</span>
            <span className="agent-script-text">{line || "\u00a0"}</span>
          </div>
        );
      })}
    </div>
  );
}

interface AgentCardProps {
  agent: AgentState;
  onTogglePaused: () => void;
  onObjective: (objective: string) => void;
  onClearContext: () => void;
  onMindToggle: () => void;
  mindActive: boolean;
}

function AgentCard({ agent, onTogglePaused, onObjective, onClearContext, onMindToggle, mindActive }: AgentCardProps) {
  const [objectiveInput, setObjectiveInput] = useState("");
  const [showObjective, setShowObjective] = useState(false);
  const visuallyPaused = agent.paused && !agent.runningScript;

  function handleObjectiveSubmit() {
    const trimmed = objectiveInput.trim();
    if (!trimmed) return;
    onObjective(trimmed);
    setObjectiveInput("");
    setShowObjective(false);
  }

  return (
    <div className={`agent-card ${visuallyPaused ? "agent-card--paused" : "agent-card--running"}`}>
      <div className="agent-card-header">
        <span className="agent-handle">{agent.sessionHandle}</span>
        <div className="agent-controls">
          <button
            className={`agent-btn ${visuallyPaused ? "agent-btn--resume" : "agent-btn--pause"}`}
            onClick={onTogglePaused}
            title={visuallyPaused ? "Resume agent" : "Pause agent"}
            aria-label={visuallyPaused ? "Resume agent" : "Pause agent"}
          >
            {visuallyPaused ? "▶️" : "⏸️"}
          </button>
          <button
            className="agent-btn agent-btn--obj"
            onClick={() => setShowObjective((v) => !v)}
            title="Set objective"
            aria-label="Set objective"
          >
            💬
          </button>
          <button
            className="agent-btn agent-btn--clear"
            onClick={onClearContext}
            title="Clear agent context"
            aria-label="Clear agent context"
          >
            ♻️
          </button>
          <button
            className={`agent-btn agent-btn--mind${mindActive ? " agent-btn--mind-open" : ""}`}
            onClick={onMindToggle}
            title="View agent mind"
            aria-label="View agent mind"
          >
            👁️
          </button>
        </div>
      </div>

      <div className="agent-system-row">
        <span className="agent-system-label">system</span>
        <span className="agent-system-value">{agent.currentSystem ?? "unknown"}</span>
      </div>

      {showObjective && (
        <div className="agent-objective-row">
          <input
            className="agent-objective-input"
            placeholder="Set new objective..."
            value={objectiveInput}
            onChange={(e) => setObjectiveInput(e.currentTarget.value)}
            onKeyDown={(e) => { if (e.key === "Enter") handleObjectiveSubmit(); }}
            autoFocus
          />
          <button className="agent-btn agent-btn--resume" onClick={handleObjectiveSubmit}>set</button>
        </div>
      )}

      {agent.runningScript ? (
        <ScriptViewer script={agent.runningScript.script} currentLine={agent.runningScript.currentLine} />
      ) : (
        <div className="agent-feed agent-feed--idle">
          <div className="agent-feed-empty">idle</div>
        </div>
      )}

    </div>
  );
}

interface AgentsPanelProps {
  agents: AgentState[];
  onPause: (handle: string) => void;
  onResume: (handle: string) => void;
  onObjective: (handle: string, objective: string) => void;
  onClearContext: (handle: string) => void;
  onMindToggle: (handle: string) => void;
  activeMindHandle: string | null;
  onSync: () => void;
}

export default function AgentsPanel({
  agents,
  onPause,
  onResume,
  onObjective,
  onClearContext,
  onMindToggle,
  activeMindHandle,
  onSync,
}: AgentsPanelProps) {
  return (
    <div className="agents-panel">
      <div className="agents-panel-header">
        <span className="agents-panel-title">agents</span>
        <button className="agent-btn agent-btn--sync" onClick={onSync}>sync</button>
      </div>
      <div className="agents-list">
        {agents.length === 0 && (
          <div className="agents-empty">no active agents</div>
        )}
        {agents.map((agent) => {
          const visuallyPaused = agent.paused && !agent.runningScript;
          return (
            <AgentCard
              key={agent.sessionHandle}
              agent={agent}
              onTogglePaused={() => (visuallyPaused ? onResume(agent.sessionHandle) : onPause(agent.sessionHandle))}
              onObjective={(obj) => onObjective(agent.sessionHandle, obj)}
              onClearContext={() => onClearContext(agent.sessionHandle)}
              onMindToggle={() => onMindToggle(agent.sessionHandle)}
              mindActive={activeMindHandle === agent.sessionHandle}
            />
          );
        })}
      </div>
    </div>
  );
}
