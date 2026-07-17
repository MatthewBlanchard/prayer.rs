import { useEffect, useMemo, useState } from "react";
import { SessionInfo } from "../shared/types.js";
import { CharacterSkillInfo } from "./api.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectCharacterSkills } from "./prayer/worldSelectors.js";

type Props = {
  sessions: SessionInfo[];
};

function formatNumber(value: number | null): string {
  if (value === null) return "-";
  return Number.isInteger(value) ? value.toLocaleString() : value.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

function xpPercent(skill: CharacterSkillInfo): number | null {
  if (skill.xp === null || skill.nextLevelXp === null || skill.nextLevelXp <= 0) return null;
  return Math.max(0, Math.min(100, Math.round((skill.xp / skill.nextLevelXp) * 100)));
}

function categoryLabel(value: string | null): string {
  return value?.trim() || "uncategorized";
}

export default function SkillsPanel({ sessions }: Props) {
  const prayer = usePrayer();
  const [selectedHandle, setSelectedHandle] = useState<string | null>(null);
  const [category, setCategory] = useState<string>("all");

  const orderedSessions = useMemo(() => [...sessions].sort((a, b) => a.sessionHandle.localeCompare(b.sessionHandle)), [sessions]);
  const selectedSession = selectedHandle ? (orderedSessions.find((session) => session.sessionHandle === selectedHandle) ?? null) : (orderedSessions[0] ?? null);
  const selectedBot = selectedSession
    ? (prayer.fleet.find((bot) => bot.username === selectedSession.sessionHandle || bot.id === selectedSession.sessionHandle) ?? null)
    : null;
  const state = useMemo(() => selectCharacterSkills(selectedBot, prayer.catalog), [prayer.catalog, selectedBot]);
  const busy = prayer.connection === "connecting";
  const error = prayer.connection === "unavailable" ? (prayer.error?.message ?? "Prayer API is unavailable") : null;
  const categories = useMemo(() => {
    const names = new Set((state?.skills ?? []).map((skill) => categoryLabel(skill.category)));
    return ["all", ...[...names].sort((a, b) => a.localeCompare(b))];
  }, [state?.skills]);
  const visibleSkills = useMemo(() => {
    const skills = state?.skills ?? [];
    if (category === "all") return skills;
    return skills.filter((skill) => categoryLabel(skill.category) === category);
  }, [state?.skills, category]);
  const totalLevels = (state?.skills ?? []).reduce((sum, skill) => sum + (skill.level ?? 0), 0);

  useEffect(() => {
    setSelectedHandle((current) => {
      if (current && sessions.some((session) => session.sessionHandle === current)) return current;
      return sessions[0]?.sessionHandle ?? null;
    });
  }, [sessions]);

  useEffect(() => setCategory("all"), [selectedSession?.sessionHandle]);

  async function reload() {
    if (!selectedSession) return;
    await prayer.refresh();
  }

  return (
    <div className="skills-panel">
      <aside className="skills-list">
        <div className="skills-list-head">
          <span className="skills-list-title">Character skills</span>
          <button className="session-btn" onClick={reload} disabled={!selectedSession || busy}>
            reload
          </button>
        </div>
        <div className="skills-list-items">
          {orderedSessions.length === 0 && <div className="skills-empty">no sessions</div>}
          {orderedSessions.map((session) => (
            <button
              key={session.sessionHandle}
              className="skills-list-item"
              data-active={selectedSession?.sessionHandle === session.sessionHandle}
              onClick={() => setSelectedHandle(session.sessionHandle)}
            >
              <span className="skills-list-name">{session.sessionHandle}</span>
              <span className="skills-list-meta">
                {state && selectedSession?.sessionHandle === session.sessionHandle ? `${state.skills.length} skills` : "SpaceMolt character"}
              </span>
            </button>
          ))}
        </div>
      </aside>

      <section className="skills-detail">
        {selectedSession ? (
          <>
            <div className="skills-toolbar">
              <div className="skills-title-block">
                <span className="skills-title">{state?.username ?? selectedSession.sessionHandle}</span>
                <span className="skills-subtitle">
                  {state
                    ? `${state.skills.length} skills / ${totalLevels} total levels`
                    : busy
                      ? "Loading SpaceMolt character skills"
                      : "No skill state loaded"}
                </span>
              </div>
              <div className="skills-actions">
                <select className="skills-filter" value={category} disabled={busy || categories.length <= 1} onChange={(e) => setCategory(e.target.value)}>
                  {categories.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
                {state?.stateVersion !== null && state?.stateVersion !== undefined && <span className="skills-save-note">state {state.stateVersion}</span>}
              </div>
            </div>
            {error && <div className="skills-error">{error}</div>}
            <div className="skills-summary">
              <div>
                <span className="skills-summary-value">{formatNumber(state?.skills.length ?? null)}</span>
                <span className="skills-summary-label">skills</span>
              </div>
              <div>
                <span className="skills-summary-value">{formatNumber(totalLevels)}</span>
                <span className="skills-summary-label">total levels</span>
              </div>
              <div>
                <span className="skills-summary-value">{formatNumber(visibleSkills.length)}</span>
                <span className="skills-summary-label">shown</span>
              </div>
            </div>
            <div className="skills-table-wrap">
              {busy && <div className="skills-empty">loading skills</div>}
              {!busy && visibleSkills.length === 0 && <div className="skills-empty">no skills found</div>}
              {!busy && visibleSkills.length > 0 && (
                <div className="skills-table">
                  <div className="skills-row skills-row--head">
                    <span>skill</span>
                    <span>category</span>
                    <span>level</span>
                    <span>xp</span>
                  </div>
                  {visibleSkills.map((skill) => {
                    const pct = xpPercent(skill);
                    return (
                      <div className="skills-row" key={skill.id}>
                        <span className="skills-skill-name">
                          <strong>{skill.name}</strong>
                          <small>{skill.id}</small>
                        </span>
                        <span className="skills-category">{categoryLabel(skill.category)}</span>
                        <span className="skills-level">
                          {formatNumber(skill.level)}
                          {skill.maxLevel !== null ? <small>/{formatNumber(skill.maxLevel)}</small> : null}
                        </span>
                        <span className="skills-xp">
                          <span>
                            {formatNumber(skill.xp)} / {formatNumber(skill.nextLevelXp)}
                          </span>
                          <span className="skills-xp-bar" title={pct === null ? undefined : `${pct}%`}>
                            <span style={{ width: `${pct ?? 0}%` }} />
                          </span>
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="skills-empty">no sessions</div>
        )}
      </section>
    </div>
  );
}
