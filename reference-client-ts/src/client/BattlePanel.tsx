import { useMemo } from "react";
import { SessionState } from "./SessionsPanel.js";

export type BattleHistoryEntry = {
  id: string;
  sessionHandle: string;
  username: string | null;
  system: string | null;
  poi: string | null;
  startedAt: string | null;
  endedAt: string;
  stance: string | null;
  target: string | null;
};

type BattlePanelProps = {
  sessions: SessionState[];
  history: BattleHistoryEntry[];
  selectedHandle: string | null;
  onSelectHandle: (handle: string | null) => void;
};

function formatPlace(system: string | null, poi: string | null): string {
  if (system && poi && system !== poi) return `${system}/${poi}`;
  return poi ?? system ?? "unknown";
}

function formatTime(iso: string | null): string {
  if (!iso) return "observed";
  const time = Date.parse(iso);
  if (!Number.isFinite(time)) return "observed";
  return new Date(time).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function formatDuration(startedAt: string | null, endedAt: string | null = null): string {
  if (!startedAt) return "unknown";
  const start = Date.parse(startedAt);
  const end = endedAt ? Date.parse(endedAt) : Date.now();
  if (!Number.isFinite(start) || !Number.isFinite(end)) return "unknown";
  const totalSeconds = Math.max(0, Math.floor((end - start) / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds.toString().padStart(2, "0")}s` : `${seconds}s`;
}

function labelForSession(session: Pick<SessionState, "sessionHandle" | "username">): string {
  return session.username ? `${session.sessionHandle} / ${session.username}` : session.sessionHandle;
}

export default function BattlePanel({ sessions, history, selectedHandle, onSelectHandle }: BattlePanelProps) {
  const activeBattles = useMemo(
    () => sessions.filter((session) => session.inBattle).sort((a, b) => a.sessionHandle.localeCompare(b.sessionHandle)),
    [sessions],
  );
  const selectedSession = selectedHandle ? (sessions.find((session) => session.sessionHandle === selectedHandle) ?? null) : (activeBattles[0] ?? null);
  const selectedHistory = selectedHandle ? history.filter((entry) => entry.sessionHandle === selectedHandle) : history;

  return (
    <div className="battle-panel">
      <div className="battle-toolbar">
        <div>
          <div className="battle-title">battle</div>
          <div className="battle-meta">
            {activeBattles.length} active / {history.length} observed
          </div>
        </div>
        <select
          className="battle-session-select"
          value={selectedHandle ?? ""}
          onChange={(event) => onSelectHandle(event.target.value || null)}
          aria-label="Select battle session"
        >
          <option value="">all sessions</option>
          {sessions.map((session) => (
            <option key={session.sessionHandle} value={session.sessionHandle}>
              {labelForSession(session)}
            </option>
          ))}
        </select>
      </div>

      <div className="battle-body">
        <section className="battle-section">
          <div className="battle-section-head">
            <span>ongoing</span>
            <span>{activeBattles.length}</span>
          </div>
          {activeBattles.length === 0 ? (
            <div className="battle-empty">no active battles</div>
          ) : (
            <div className="battle-card-grid">
              {activeBattles.map((session) => (
                <button
                  key={session.sessionHandle}
                  className="battle-active-card"
                  data-selected={selectedSession?.sessionHandle === session.sessionHandle}
                  onClick={() => onSelectHandle(session.sessionHandle)}
                >
                  <span className="battle-active-name">{labelForSession(session)}</span>
                  <span className="battle-active-place">{formatPlace(session.location.system, session.location.poi)}</span>
                  <span className="battle-active-stats">
                    hull {session.hull ?? "-"}/{session.maxHull ?? "-"}
                    <span>stance {session.combatStance ?? "unknown"}</span>
                  </span>
                  <span className="battle-active-target">target {session.combatTarget ?? "none"}</span>
                  <span className="battle-active-duration">observed {formatDuration(session.battleStartedAt)}</span>
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="battle-section battle-section--detail">
          <div className="battle-section-head">
            <span>{selectedSession ? labelForSession(selectedSession) : "battle detail"}</span>
            {selectedSession?.inBattle && <span className="battle-live-pill">live</span>}
          </div>
          {selectedSession ? (
            <div className="battle-detail">
              <div>
                <span>state</span>
                <strong>{selectedSession.inBattle ? "in battle" : "clear"}</strong>
              </div>
              <div>
                <span>location</span>
                <strong>{formatPlace(selectedSession.location.system, selectedSession.location.poi)}</strong>
              </div>
              <div>
                <span>stance</span>
                <strong>{selectedSession.combatStance ?? "unknown"}</strong>
              </div>
              <div>
                <span>target</span>
                <strong>{selectedSession.combatTarget ?? "none"}</strong>
              </div>
              <div>
                <span>hull</span>
                <strong>
                  {selectedSession.hull ?? "-"}/{selectedSession.maxHull ?? "-"}
                </strong>
              </div>
              <div>
                <span>observed</span>
                <strong>{formatDuration(selectedSession.battleStartedAt)}</strong>
              </div>
            </div>
          ) : (
            <div className="battle-empty">select a session</div>
          )}
        </section>

        <section className="battle-section battle-section--history">
          <div className="battle-section-head">
            <span>history</span>
            <span>{selectedHistory.length}</span>
          </div>
          {selectedHistory.length === 0 ? (
            <div className="battle-empty">no observed completed battles</div>
          ) : (
            <div className="battle-history-list">
              {selectedHistory.slice(0, 50).map((entry) => (
                <div className="battle-history-row" key={entry.id}>
                  <span className="battle-history-name">{entry.username ?? entry.sessionHandle}</span>
                  <span>{formatPlace(entry.system, entry.poi)}</span>
                  <span>
                    {formatTime(entry.startedAt)} - {formatTime(entry.endedAt)}
                  </span>
                  <span>{formatDuration(entry.startedAt, entry.endedAt)}</span>
                  <span>stance {entry.stance ?? "unknown"}</span>
                  <span>target {entry.target ?? "none"}</span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
