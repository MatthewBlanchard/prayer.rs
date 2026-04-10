import { useState } from "react";
import { AgentFeedItem, AgentInfo } from "../shared/types.js";

export type AgentState = AgentInfo & { feed: AgentFeedItem[] };

interface AgentCardProps {
  agent: AgentState;
  onPause: () => void;
  onResume: () => void;
  onObjective: (objective: string) => void;
}

function AgentCard({ agent, onPause, onResume, onObjective }: AgentCardProps) {
  const [objectiveInput, setObjectiveInput] = useState("");
  const [showObjective, setShowObjective] = useState(false);

  function handleObjectiveSubmit() {
    const trimmed = objectiveInput.trim();
    if (!trimmed) return;
    onObjective(trimmed);
    setObjectiveInput("");
    setShowObjective(false);
  }

  return (
    <div className={`agent-card ${agent.paused ? "agent-card--paused" : "agent-card--running"}`}>
      <div className="agent-card-header">
        <span className="agent-handle">{agent.sessionHandle}</span>
        <span className={`agent-badge ${agent.paused ? "agent-badge--paused" : "agent-badge--running"}`}>
          {agent.paused ? "paused" : "running"}
        </span>
        <div className="agent-controls">
          {agent.paused ? (
            <button className="agent-btn agent-btn--resume" onClick={onResume}>resume</button>
          ) : (
            <button className="agent-btn agent-btn--pause" onClick={onPause}>pause</button>
          )}
          <button
            className="agent-btn agent-btn--obj"
            onClick={() => setShowObjective((v) => !v)}
          >
            objective
          </button>
        </div>
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

      <div className="agent-feed">
        {agent.feed.length === 0 && (
          <div className="agent-feed-empty">no activity yet</div>
        )}
        {agent.feed.map((item, i) => {
          if (item.kind === "error") {
            return (
              <div key={i} className="agent-feed-error">{item.message}</div>
            );
          }
          return (
            <div key={i} className={`agent-feed-tool agent-feed-tool--${item.status}`}>
              <span className="agent-feed-tool-status">[{item.status}]</span>
              <span className="agent-feed-tool-name">{item.name}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

interface AgentsPanelProps {
  agents: AgentState[];
  onPause: (handle: string) => void;
  onResume: (handle: string) => void;
  onObjective: (handle: string, objective: string) => void;
  onSync: () => void;
}

export default function AgentsPanel({ agents, onPause, onResume, onObjective, onSync }: AgentsPanelProps) {
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
        {agents.map((agent) => (
          <AgentCard
            key={agent.sessionHandle}
            agent={agent}
            onPause={() => onPause(agent.sessionHandle)}
            onResume={() => onResume(agent.sessionHandle)}
            onObjective={(obj) => onObjective(agent.sessionHandle, obj)}
          />
        ))}
      </div>
    </div>
  );
}
