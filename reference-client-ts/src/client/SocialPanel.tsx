import { useMemo, useState } from "react";
import { SocialBot } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectSocialBots } from "./prayer/worldSelectors.js";
import { readVersionedStoredStringSet, writeVersionedStored } from "./persistence.js";

type SocialPanelProps = {
  sessions: SessionState[];
};

const TRACKED_FACTIONS_KEY = "prayer-social-tracked-factions";
const TRACKED_BOTS_KEY = "prayer-social-tracked-bots";

type FactionSummary = {
  id: string;
  tag: string | null;
  memberCount: number;
  onlineCount: number;
  lastSeenUtc: string;
};

function loadTracked(key: string): Set<string> {
  return readVersionedStoredStringSet(key);
}

function saveTracked(key: string, values: Set<string>): void {
  writeVersionedStored(key, 1, [...values].sort());
}

function botKey(bot: SocialBot): string {
  return bot.playerId ?? bot.username;
}

function titleizeId(id: string): string {
  return id.replace(/[_-]/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function factionLabel(faction: Pick<FactionSummary, "id" | "tag">, names: Record<string, string | null> = {}): string {
  if (faction.id === "synthetic:pirates") return "Pirates";
  const empire = faction.id.match(/^synthetic:empire:(.+)$/)?.[1];
  if (empire) return `${titleizeId(empire)} Empire`;
  const name = names[faction.id];
  if (name) return faction.tag ? `${name} [${faction.tag}]` : name;
  return faction.tag ? `[${faction.tag}]` : faction.id;
}

function relativeTime(iso: string): string {
  const then = Date.parse(iso);
  if (!Number.isFinite(then)) return "?";
  const seconds = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(then).toLocaleDateString();
}

function summarizeFactions(bots: SocialBot[]): FactionSummary[] {
  const byId = new Map<string, FactionSummary>();
  for (const bot of bots) {
    if (!bot.factionId) continue;
    const existing = byId.get(bot.factionId);
    if (existing) {
      existing.memberCount += 1;
      if (!bot.offline) existing.onlineCount += 1;
      if (bot.lastSeenUtc > existing.lastSeenUtc) existing.lastSeenUtc = bot.lastSeenUtc;
      if (!existing.tag && bot.factionTag) existing.tag = bot.factionTag;
    } else {
      byId.set(bot.factionId, {
        id: bot.factionId,
        tag: bot.factionTag,
        memberCount: 1,
        onlineCount: bot.offline ? 0 : 1,
        lastSeenUtc: bot.lastSeenUtc,
      });
    }
  }
  return [...byId.values()];
}

function matchesSearch(bot: SocialBot, needle: string): boolean {
  if (!needle) return true;
  return [
    bot.username,
    bot.playerId,
    bot.factionId,
    bot.factionTag,
    bot.clanTag,
    bot.actorKind,
    bot.shipClass,
    bot.shipName,
    bot.statusMessage,
    bot.lastSeenSystem,
  ].some((field) => field?.toLowerCase().includes(needle));
}

export default function SocialPanel({ sessions }: SocialPanelProps) {
  const prayer = usePrayer();
  const bots = useMemo(() => selectSocialBots(prayer.agentSightings, prayer.fleet), [prayer.agentSightings, prayer.fleet]);
  const error = prayer.connection === "unavailable" ? (prayer.error?.message ?? "Prayer API unavailable") : null;
  const [search, setSearch] = useState("");
  const [factionFilter, setFactionFilter] = useState<string | null>(null);
  const factionNames: Record<string, string | null> = {};
  const [trackedFactions, setTrackedFactions] = useState<Set<string>>(() => loadTracked(TRACKED_FACTIONS_KEY));
  const [trackedBots, setTrackedBots] = useState<Set<string>>(() => loadTracked(TRACKED_BOTS_KEY));

  function toggleTrackedFaction(id: string) {
    setTrackedFactions((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      saveTracked(TRACKED_FACTIONS_KEY, next);
      return next;
    });
  }

  function toggleTrackedBot(key: string) {
    setTrackedBots((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      saveTracked(TRACKED_BOTS_KEY, next);
      return next;
    });
  }

  const factions = useMemo(() => {
    const summaries = summarizeFactions(bots);
    summaries.sort((a, b) => {
      const aTracked = trackedFactions.has(a.id) ? 0 : 1;
      const bTracked = trackedFactions.has(b.id) ? 0 : 1;
      return aTracked - bTracked || b.memberCount - a.memberCount || a.id.localeCompare(b.id);
    });
    return summaries;
  }, [bots, trackedFactions]);

  const needle = search.trim().toLowerCase();
  const visibleBots = useMemo(() => {
    const filtered = bots.filter((bot) => (factionFilter === null || bot.factionId === factionFilter) && matchesSearch(bot, needle));
    filtered.sort((a, b) => {
      const aTracked = trackedBots.has(botKey(a)) || (a.factionId !== null && trackedFactions.has(a.factionId)) ? 0 : 1;
      const bTracked = trackedBots.has(botKey(b)) || (b.factionId !== null && trackedFactions.has(b.factionId)) ? 0 : 1;
      return aTracked - bTracked || b.lastSeenUtc.localeCompare(a.lastSeenUtc) || a.username.localeCompare(b.username);
    });
    return filtered;
  }, [bots, factionFilter, needle, trackedBots, trackedFactions]);

  const trackedCount = useMemo(
    () => bots.filter((bot) => trackedBots.has(botKey(bot)) || (bot.factionId !== null && trackedFactions.has(bot.factionId))).length,
    [bots, trackedBots, trackedFactions],
  );

  if (!sessions.length) {
    return (
      <div className="social-panel">
        <div className="social-empty">No sessions registered — sightings appear once a session is online.</div>
      </div>
    );
  }

  return (
    <div className="social-panel">
      <div className="social-toolbar">
        <div>
          <div className="social-title">Social</div>
          <div className="social-meta">
            {prayer.connection === "connecting" ? "Loading sightings..." : `${bots.length} contacts sighted · ${trackedCount} tracked`}
            {error ? ` · ${error}` : ""}
          </div>
        </div>
        <input
          className="social-search"
          type="search"
          placeholder="Search names, factions, ships, systems..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Search sighted contacts"
        />
      </div>

      <div className="social-body">
        <aside className="social-factions" aria-label="Factions">
          <button className="social-faction" data-active={factionFilter === null} onClick={() => setFactionFilter(null)}>
            <span className="social-faction-name">All contacts</span>
            <span className="social-faction-count">{bots?.length ?? 0}</span>
          </button>
          {factions.map((faction) => (
            <div key={faction.id} className="social-faction" data-active={factionFilter === faction.id} data-tracked={trackedFactions.has(faction.id)}>
              <button
                className="social-faction-select"
                onClick={() => setFactionFilter((current) => (current === faction.id ? null : faction.id))}
                title={faction.id}
              >
                <span className="social-faction-name">{factionLabel(faction, factionNames)}</span>
                <span className="social-faction-count">
                  {faction.onlineCount}/{faction.memberCount}
                </span>
              </button>
              <button
                className="social-track-btn"
                data-tracked={trackedFactions.has(faction.id)}
                onClick={() => toggleTrackedFaction(faction.id)}
                title={trackedFactions.has(faction.id) ? "Untrack faction" : "Track faction"}
                aria-label={`${trackedFactions.has(faction.id) ? "Untrack" : "Track"} faction ${factionLabel(faction, factionNames)}`}
              >
                {trackedFactions.has(faction.id) ? "★" : "☆"}
              </button>
            </div>
          ))}
        </aside>

        <div className="social-list" role="list">
          {bots !== null && visibleBots.length === 0 && (
            <div className="social-empty">
              {bots.length === 0
                ? "No contacts sighted yet. Sightings accumulate as your sessions move around the galaxy."
                : "No contacts match the current filter."}
            </div>
          )}
          {visibleBots.map((bot) => {
            const key = botKey(bot);
            const tracked = trackedBots.has(key);
            const factionTracked = bot.factionId !== null && trackedFactions.has(bot.factionId);
            return (
              <div key={key} className="social-bot" role="listitem" data-tracked={tracked || factionTracked}>
                <span
                  className="social-bot-swatch"
                  style={{
                    background: bot.primaryColor ?? "var(--border)",
                    borderColor: bot.secondaryColor ?? "var(--border)",
                  }}
                  title={[bot.primaryColor, bot.secondaryColor].filter(Boolean).join(" / ") || "no colors reported"}
                />
                <div className="social-bot-main">
                  <div className="social-bot-name-row">
                    <span className="social-bot-name">{bot.username || bot.playerId}</span>
                    {bot.clanTag && <span className="social-bot-clan">&lt;{bot.clanTag}&gt;</span>}
                    {bot.factionTag && (
                      <button
                        className="social-bot-faction"
                        onClick={() => bot.factionId && setFactionFilter(bot.factionId)}
                        title={bot.factionId ?? undefined}
                      >
                        [{bot.factionTag}]
                      </button>
                    )}
                    {bot.inCombat && <span className="social-bot-flag social-bot-flag--combat">in combat</span>}
                    {bot.offline && <span className="social-bot-flag social-bot-flag--offline">offline</span>}
                    {bot.synthetic && bot.actorKind && <span className="social-bot-flag">{bot.actorKind}</span>}
                  </div>
                  <div className="social-bot-detail">
                    {bot.shipName || bot.shipClass ? (
                      <span>
                        {bot.shipName ?? bot.shipClass}
                        {bot.shipName && bot.shipClass ? ` (${bot.shipClass})` : ""}
                      </span>
                    ) : (
                      <span className="social-bot-unknown">unknown ship</span>
                    )}
                    {bot.statusMessage && <span className="social-bot-status">“{bot.statusMessage}”</span>}
                  </div>
                </div>
                <div className="social-bot-seen">
                  <div className="social-bot-seen-where" title={bot.lastSeenSystem}>
                    {bot.lastSeenSystem}
                  </div>
                  <div className="social-bot-seen-when" title={bot.lastSeenUtc}>
                    {relativeTime(bot.lastSeenUtc)} · ×{bot.timesSeen}
                  </div>
                </div>
                <button
                  className="social-track-btn"
                  data-tracked={tracked}
                  onClick={() => toggleTrackedBot(key)}
                  title={tracked ? "Untrack contact" : "Track contact"}
                  aria-label={`${tracked ? "Untrack" : "Track"} ${bot.username}`}
                >
                  {tracked ? "★" : "☆"}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
