import { useEffect, useMemo, useState } from "react";
import { actions } from "@prayer/sdk";
import type { Action } from "@prayer/sdk/types";
import { FactionMember } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import SearchableSessionSelect from "./SearchableSessionSelect.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectFaction } from "./prayer/worldSelectors.js";

type FactionPanelProps = {
  sessions: SessionState[];
};

const ACTING_SESSION_KEY = "prayer-faction-acting-session";

// Built-in faction role ladder exposed by the faction_set_role DSL command.
const ROLES = ["recruit", "member", "officer", "leader"];

function isAlreadyInvited(message: string): boolean {
  return message.includes("already_invited");
}

export default function FactionPanel({ sessions }: FactionPanelProps) {
  const prayer = usePrayer();
  const [actingHandle, setActingHandle] = useState<string | null>(() => {
    try {
      return window.localStorage.getItem(ACTING_SESSION_KEY);
    } catch {
      return null;
    }
  });
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const actingSession = sessions.find((session) => session.sessionHandle === actingHandle);
  const faction = selectFaction(actingSession ? prayer.factionBySession[actingSession.botId] : undefined);
  const loading = prayer.connection === "connecting";
  const error = prayer.error?.message ?? null;

  // Default the acting session to the first registered one.
  useEffect(() => {
    if (actingHandle && sessions.some((s) => s.sessionHandle === actingHandle)) return;
    const first = sessions[0]?.sessionHandle ?? null;
    setActingHandle(first);
  }, [sessions, actingHandle]);

  useEffect(() => {
    if (!actingHandle) return;
    try {
      window.localStorage.setItem(ACTING_SESSION_KEY, actingHandle);
    } catch {
      // persistence is best-effort
    }
  }, [actingHandle]);

  // Registered sessions that are not already members of this faction — the
  // candidates for recruiting.
  const memberHandles = useMemo(() => {
    const names = new Set<string>();
    for (const m of faction?.members ?? []) {
      if (m.username) names.add(m.username.toLowerCase());
    }
    return names;
  }, [faction]);

  const recruitCandidates = useMemo(
    () => sessions.filter((s) => s.sessionHandle !== actingHandle && (!s.username || !memberHandles.has(s.username.toLowerCase()))),
    [sessions, actingHandle, memberHandles],
  );

  async function runAction(label: string, handle: string, action: Action): Promise<boolean> {
    setBusy(true);
    setStatus(`${label}…`);
    try {
      const bot = await prayer.bot(handle);
      const run = await bot.start([action], { idempotencyKey: crypto.randomUUID() });
      const result = await run.wait();
      const succeeded = result.status === "succeeded";
      setStatus(`${label}: ${succeeded ? "Done." : `run ${result.status}`}`);
      if (succeeded) await prayer.refresh();
      return succeeded;
    } catch (err) {
      setStatus(`${label}: ${err instanceof Error ? err.message : String(err)}`);
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function handleRecruit(invitee: SessionState) {
    if (!actingHandle || !faction?.id) return;
    const inviteeHandle = invitee.sessionHandle;
    const inviteeName = invitee.username;
    if (!inviteeName) {
      setStatus(`Invite ${inviteeHandle}: waiting for player identity.`);
      return;
    }
    setBusy(true);
    setStatus(`Inviting ${inviteeName}…`);
    try {
      try {
        await runAction(`Invite ${inviteeName}`, actingHandle, actions.factionInvite({ player: inviteeName }));
      } catch (err) {
        // A pending invite already exists — that's fine, fall through to accept.
        const message = err instanceof Error ? err.message : String(err);
        if (!isAlreadyInvited(message)) throw err;
      }
      await runAction(`Recruit ${inviteeName}`, inviteeHandle, actions.factionAcceptInvite({ faction: faction.id }));
    } catch (err) {
      setStatus(`Recruit ${inviteeName}: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBusy(false);
      void prayer.refresh();
    }
  }

  async function handleRecruitAll() {
    for (const candidate of recruitCandidates) {
      await handleRecruit(candidate);
    }
  }

  async function handlePromote(member: FactionMember, role: string) {
    if (!actingHandle) return;
    const memberId = member.playerId ?? member.username;
    if (!memberId) return;
    const ok = await runAction(`Set ${member.username ?? memberId} → ${role}`, actingHandle, actions.factionSetRole({ player: memberId, role }));
    if (ok) void prayer.refresh();
  }

  async function handleKick(member: FactionMember) {
    if (!actingHandle) return;
    const memberId = member.username ?? member.playerId;
    if (!memberId) return;
    const ok = await runAction(`Kick ${member.username ?? memberId}`, actingHandle, actions.factionKick({ player: memberId }));
    if (ok) void prayer.refresh();
  }

  function renderSessionPicker() {
    return (
      <label className="faction-session-picker">
        <span>Acting as</span>
        <SearchableSessionSelect
          sessions={sessions}
          value={actingHandle}
          onChange={setActingHandle}
          disabled={busy || sessions.length === 0}
          ariaLabel="Faction acting session"
        />
      </label>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="faction-panel">
        <div className="faction-empty">No sessions registered — register a session to manage factions.</div>
      </div>
    );
  }

  return (
    <div className="faction-panel">
      <div className="faction-toolbar">
        <div>
          <div className="faction-title">Faction</div>
          <div className="faction-meta">
            {loading
              ? "Loading faction…"
              : faction
                ? `${faction.name ?? faction.id ?? "Unknown"}${faction.tag ? ` [${faction.tag}]` : ""}`
                : "Not in a faction"}
            {error ? ` · ${error}` : ""}
          </div>
        </div>
        {renderSessionPicker()}
        <button className="session-btn" onClick={() => void prayer.refresh()} disabled={busy || loading}>
          refresh
        </button>
      </div>

      {status && <div className="faction-status">{status}</div>}

      {!loading && !faction && actingHandle && (
        <CreateFactionForm handle={actingHandle} busy={busy} onCreated={() => void prayer.refresh()} onStatus={setStatus} setBusy={setBusy} prayer={prayer} />
      )}

      {faction && (
        <div className="faction-body">
          <section className="faction-card">
            <div className="faction-card-head">Overview</div>
            <dl className="faction-stats">
              <div>
                <dt>Leader</dt>
                <dd>{faction.leaderUsername ?? faction.leaderId ?? "—"}</dd>
              </div>
              <div>
                <dt>Members</dt>
                <dd>{faction.memberCount ?? faction.members.length}</dd>
              </div>
              <div>
                <dt>Treasury</dt>
                <dd>{faction.treasury?.toLocaleString() ?? "—"}</dd>
              </div>
              <div>
                <dt>Roles</dt>
                <dd>{faction.roles.length}</dd>
              </div>
            </dl>
            {faction.description && <p className="faction-desc">{faction.description}</p>}
          </section>

          <section className="faction-card">
            <div className="faction-card-head">
              Recruit sessions
              {recruitCandidates.length > 0 && (
                <button
                  className="session-btn"
                  onClick={() => void handleRecruitAll()}
                  disabled={busy || !faction.id}
                  title="Invite and accept for every eligible session"
                >
                  recruit all
                </button>
              )}
            </div>
            {recruitCandidates.length === 0 ? (
              <div className="faction-empty-row">All registered sessions are already members.</div>
            ) : (
              <ul className="faction-recruit-list">
                {recruitCandidates.map((s) => (
                  <li key={s.sessionHandle}>
                    <span>{s.username ?? `${s.sessionHandle} (loading identity)`}</span>
                    <button className="session-btn" onClick={() => void handleRecruit(s)} disabled={busy || !faction.id || !s.username}>
                      invite &amp; accept
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="faction-card">
            <div className="faction-card-head">Members &amp; roles</div>
            <ul className="faction-member-list">
              {faction.members.map((m, i) => {
                const key = m.playerId ?? m.username ?? `member-${i}`;
                const isLeader = (m.playerId && m.playerId === faction.leaderId) || (m.username && m.username === faction.leaderUsername);
                return (
                  <li key={key} className="faction-member">
                    <div className="faction-member-main">
                      <span className="faction-member-name">{m.username ?? m.playerId ?? "unknown"}</span>
                      {m.online === true && <span className="faction-member-flag">online</span>}
                      {isLeader && <span className="faction-member-flag faction-member-flag--leader">leader</span>}
                    </div>
                    <div className="faction-member-actions">
                      <select
                        value={ROLES.includes((m.role ?? "").toLowerCase()) ? (m.role ?? "").toLowerCase() : ""}
                        onChange={(e) => e.target.value && void handlePromote(m, e.target.value)}
                        disabled={busy}
                        aria-label={`Set role for ${m.username ?? key}`}
                      >
                        <option value="">{m.role ?? "set role"}</option>
                        {ROLES.map((role) => (
                          <option key={role} value={role}>
                            {role}
                          </option>
                        ))}
                      </select>
                      <button
                        className="session-btn session-btn--danger"
                        onClick={() => void handleKick(m)}
                        disabled={busy || Boolean(isLeader)}
                        title={isLeader ? "Cannot kick the leader" : "Kick from faction"}
                      >
                        kick
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        </div>
      )}
    </div>
  );
}

function CreateFactionForm({
  handle,
  busy,
  onCreated,
  onStatus,
  setBusy,
  prayer,
}: {
  handle: string;
  busy: boolean;
  onCreated: () => void;
  onStatus: (s: string) => void;
  setBusy: (b: boolean) => void;
  prayer: ReturnType<typeof usePrayer>;
}) {
  const [name, setName] = useState("");
  const [tag, setTag] = useState("");

  async function submit() {
    if (!name.trim() || !tag.trim()) {
      onStatus("Faction name and tag are required.");
      return;
    }
    setBusy(true);
    onStatus(`Creating ${name}…`);
    try {
      const bot = await prayer.bot(handle);
      const run = await bot.start([actions.factionCreate({ name: name.trim(), tag: tag.trim() })], { idempotencyKey: crypto.randomUUID() });
      const result = await run.wait();
      onStatus(`Create: ${result.status === "succeeded" ? "Done." : `run ${result.status}`}`);
      if (result.status === "succeeded") onCreated();
    } catch (err) {
      onStatus(`Create: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="faction-card faction-create">
      <div className="faction-card-head">Create a faction</div>
      <div className="faction-create-row">
        <input placeholder="Name (unique)" value={name} onChange={(e) => setName(e.target.value)} disabled={busy} />
        <input placeholder="Tag (2–4)" maxLength={4} value={tag} onChange={(e) => setTag(e.target.value.toUpperCase())} disabled={busy} />
        <button className="session-btn" onClick={() => void submit()} disabled={busy}>
          create
        </button>
      </div>
    </section>
  );
}
