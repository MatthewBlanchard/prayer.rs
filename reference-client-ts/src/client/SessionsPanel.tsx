import { CSSProperties, type FormEvent, useEffect, useState } from "react";
import { SessionInfo, type Squad } from "../shared/types.js";
import { type PassengerInfo } from "./api.js";
import { CreditAmount } from "./Credits.js";
import { fetchSquads } from "./api.js";
import { registerBot, type RegisterBotResult } from "./api/config.js";
import type { ObservedPlayerView } from "./prayer/selectors.js";

export type RunningScript = {
  script: string;
  currentLine: number | null;
  isRunning: boolean;
  frameKind: string;
  frameName: string | null;
};

export type SessionLocation = {
  system: string | null;
  poi: string | null;
  inTransit: boolean;
  transitDestSystem: string | null;
  transitDestPoi: string | null;
  activeRouteDestSystem: string | null;
  activeRouteDestPoi: string | null;
  activeRouteHops: string[];
};

export type SessionState = SessionInfo & {
  /** Stable Prayer fleet identity used by SDK-backed workflows. */
  botId: string;
  username: string | null;
  connected: boolean;
  credits: number | null;
  fuel: number | null;
  maxFuel: number | null;
  hull: number | null;
  maxHull: number | null;
  cargoUsed: number | null;
  cargoCapacity: number | null;
  passengerBerths: number | null;
  cargo: Record<string, number>;
  passengersAboard: PassengerInfo[];
  inBattle: boolean;
  combatStance: string | null;
  combatTarget: string | null;
  battleStartedAt: string | null;
  runningScript: RunningScript | null;
  location: SessionLocation;
  observedPlayers: Record<string, ObservedPlayerView>;
};

type SessionIconName = "cargo" | "script";

const EMPIRE_LOCATION_COLORS = {
  frontier: "#4fb56a",
  solarian: "#e8cf62",
  voidborn: "#9b6ae8",
  crimson: "#e0555f",
  trade: "#5f9fed",
};

const LOCATION_PLACE_STYLE: CSSProperties = {
  color: "#ffffff",
  fontSize: "10px",
};

function normalizeEmpire(value: string | null): keyof typeof EMPIRE_LOCATION_COLORS | null {
  const normalized =
    value
      ?.trim()
      .toLowerCase()
      .replace(/[\s-]+/g, "_") ?? "";
  if (!normalized) return null;
  if (normalized.includes("frontier") || normalized.includes("nebula")) return "frontier";
  if (normalized.includes("solarian") || normalized === "sol") return "solarian";
  if (normalized.includes("voidborn") || normalized.includes("void")) return "voidborn";
  if (normalized.includes("crimson")) return "crimson";
  if (normalized.includes("trade") || normalized.includes("outerrim") || normalized.includes("outer_rim") || normalized.includes("outer")) {
    return "trade";
  }
  return null;
}

function locationColor(system: string | null, systemEmpires: Record<string, string>): string {
  const systemId = system?.trim() ?? "";
  const mappedEmpire = systemId ? (systemEmpires[systemId] ?? systemEmpires[systemId.toLowerCase()]) : null;
  const empire = normalizeEmpire(mappedEmpire ?? null) ?? normalizeEmpire(system);
  return empire ? EMPIRE_LOCATION_COLORS[empire] : "#ffffff";
}

function SessionIcon({ name }: { name: SessionIconName }) {
  if (name === "cargo") {
    return (
      <svg className="session-btn-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
        <path d="M3 5.5 8 3l5 2.5-5 2.6-5-2.6Z" />
        <path d="M3 5.5v5L8 13V8.1L3 5.5Z" />
        <path d="M13 5.5v5L8 13V8.1l5-2.6Z" />
      </svg>
    );
  }

  return (
    <svg className="session-btn-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="M5.5 4 2.8 8l2.7 4" />
      <path d="M10.5 4 13.2 8l-2.7 4" />
      <path d="M8.8 3.5 7.2 12.5" />
    </svg>
  );
}

function PassengerBerthIcon() {
  return (
    <svg className="session-passenger-icon" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <circle cx="8" cy="5" r="2.4" />
      <path d="M3.8 13c.7-2.5 2.1-3.8 4.2-3.8s3.5 1.3 4.2 3.8" />
    </svg>
  );
}

function PassengerBerthBadge({ berths }: { berths: number | null }) {
  if (berths == null || !Number.isFinite(berths) || berths <= 0) return null;
  const count = Math.floor(berths);
  const visible = Math.min(count, 3);
  return (
    <span
      className="session-passenger-berths"
      title={`${count.toLocaleString()} passenger berth${count === 1 ? "" : "s"}`}
      aria-label={`${count.toLocaleString()} passenger berth${count === 1 ? "" : "s"}`}
    >
      {Array.from({ length: visible }).map((_, index) => (
        <PassengerBerthIcon key={index} />
      ))}
      {count > visible ? <strong>+{count - visible}</strong> : null}
    </span>
  );
}

function formatPlace(system: string | null, poi: string | null): string {
  const cleanSystem = system?.trim() || null;
  const cleanPoi = poi?.trim() || null;
  if (cleanPoi && cleanSystem && cleanPoi !== cleanSystem) {
    return `${cleanSystem}/${cleanPoi}`;
  }
  return cleanPoi ?? cleanSystem ?? "unknown";
}

function PlaceLabel({ system, poi, systemEmpires }: { system: string | null; poi: string | null; systemEmpires: Record<string, string> }) {
  const place = formatPlace(system, poi);

  return (
    <span className="session-location-place" style={{ ...LOCATION_PLACE_STYLE, color: locationColor(system, systemEmpires) }} title={place}>
      {place}
    </span>
  );
}

function SessionLocationRow({ location, systemEmpires }: { location: SessionLocation; systemEmpires: Record<string, string> }) {
  const destinationSystem = location.inTransit ? location.transitDestSystem : location.activeRouteDestSystem;
  const destinationPoi = location.inTransit ? location.transitDestPoi : location.activeRouteDestPoi;
  const destination = formatPlace(destinationSystem, destinationPoi);

  if (destination !== "unknown") {
    return (
      <div className="session-location-row session-location-row--route">
        <PlaceLabel system={location.system} poi={location.poi} systemEmpires={systemEmpires} />
        <span className="session-location-arrow">-&gt;</span>
        <PlaceLabel system={destinationSystem} poi={destinationPoi} systemEmpires={systemEmpires} />
      </div>
    );
  }

  return (
    <div className="session-location-row session-location-row--current">
      <PlaceLabel system={location.system} poi={location.poi} systemEmpires={systemEmpires} />
    </div>
  );
}

function SessionStatsRow({
  credits,
  hull,
  maxHull,
  cargoUsed,
  cargoCapacity,
  passengerBerths,
}: {
  credits: number | null;
  hull: number | null;
  maxHull: number | null;
  cargoUsed: number | null;
  cargoCapacity: number | null;
  passengerBerths: number | null;
}) {
  return (
    <div className="session-stats-row">
      <CreditAmount value={credits} />
      <span>
        hull {formatStat(hull)}/{formatStat(maxHull)}
      </span>
      <span className="session-cargo-stat">
        cargo {formatStat(cargoUsed)}/{formatStat(cargoCapacity)}
        <PassengerBerthBadge berths={passengerBerths} />
      </span>
    </div>
  );
}

function ScriptViewer({ script, currentLine }: { script: string; currentLine: number | null }) {
  const lines = script.replace(/\n+$/, "").split("\n");
  return (
    <div className="session-script-viewer">
      {lines.map((line, i) => {
        const active = currentLine !== null && i + 1 === currentLine;
        return (
          <div key={i} className={`session-script-line${active ? " session-script-line--active" : ""}`}>
            <span className="session-script-lineno">{i + 1}</span>
            <span className="session-script-text">{line || "\u00a0"}</span>
          </div>
        );
      })}
    </div>
  );
}

function formatItemId(itemId: string): string {
  return itemId.replace(/[_-]+/g, " ");
}

function formatPassengerClass(className: string): string {
  return className.replace(/[_-]+/g, " ").trim() || "passenger";
}

function formatPassengerDestination(passenger: PassengerInfo): string {
  return passenger.destinationName || passenger.destination || passenger.destinationSystem || "unknown";
}

function CargoViewer({ cargo, passengers }: { cargo: Record<string, number>; passengers: PassengerInfo[] }) {
  const rows = Object.entries(cargo)
    .filter(([, quantity]) => Number.isFinite(quantity) && quantity !== 0)
    .sort(([itemA, qtyA], [itemB, qtyB]) => qtyB - qtyA || itemA.localeCompare(itemB));
  const passengerRows = [...passengers].sort(
    (a, b) => formatPassengerDestination(a).localeCompare(formatPassengerDestination(b)) || a.name.localeCompare(b.name),
  );

  return (
    <div className="session-cargo-viewer">
      {rows.length === 0 ? (
        <div className="session-cargo-empty">cargo empty</div>
      ) : (
        rows.map(([itemId, quantity]) => (
          <div className="session-cargo-row" key={itemId}>
            <span className="session-cargo-item" title={itemId}>
              {formatItemId(itemId)}
            </span>
            <span className="session-cargo-qty">{quantity.toLocaleString()}</span>
          </div>
        ))
      )}
      {passengerRows.length > 0 ? (
        <div className="session-cargo-section">
          <div className="session-cargo-heading">passengers</div>
          {passengerRows.map((passenger, index) => {
            const destination = formatPassengerDestination(passenger);
            const className = formatPassengerClass(passenger.className);
            const label = passenger.name || passenger.citizenId || "passenger";
            return (
              <div className="session-passenger-row" key={passenger.citizenId || `${label}-${index}`}>
                <span className="session-passenger-name" title={label}>
                  {label}
                </span>
                <span className="session-passenger-meta" title={`${className} to ${destination}`}>
                  {className} -&gt; {destination}
                </span>
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function formatStat(value: number | null): string {
  return value !== null && Number.isFinite(value) ? value.toLocaleString() : "-";
}

function fuelPercent(fuel: number | null, maxFuel: number | null): number | null {
  if (fuel === null || maxFuel === null || !Number.isFinite(fuel) || !Number.isFinite(maxFuel) || maxFuel <= 0) {
    return null;
  }
  return Math.max(0, Math.min(100, Math.round((fuel / maxFuel) * 100)));
}

function cargoPercent(cargoUsed: number | null, cargoCapacity: number | null): number | null {
  if (cargoUsed === null || cargoCapacity === null || !Number.isFinite(cargoUsed) || !Number.isFinite(cargoCapacity) || cargoCapacity <= 0) {
    return null;
  }
  return Math.max(0, Math.min(100, Math.round((cargoUsed / cargoCapacity) * 100)));
}

interface SessionCardProps {
  session: SessionState;
  systemEmpires: Record<string, string>;
  onHaltScript: () => void;
}

export function SessionCard({ session, systemEmpires, onHaltScript }: SessionCardProps) {
  const displayName = session.username?.trim() || session.sessionHandle;
  const [openView, setOpenView] = useState<"cargo" | "script" | null>(null);
  const active = Boolean(session.runningScript?.isRunning);
  const activeOverride = active && session.runningScript?.frameKind === "override";
  const hasScript = Boolean(session.runningScript);
  const fuelPct = fuelPercent(session.fuel, session.maxFuel);
  const cargoPct = cargoPercent(session.cargoUsed, session.cargoCapacity);
  const scriptStatus =
    activeOverride && session.runningScript?.frameName
      ? `OVERRIDE: ${session.runningScript.frameName}`
      : activeOverride
        ? "OVERRIDE"
        : session.runningScript?.frameKind === "skill" && session.runningScript.frameName
          ? `SKILL: ${session.runningScript.frameName}`
          : "main";

  return (
    <div
      className={`session-card ${session.inBattle ? "session-card--battle" : activeOverride ? "session-card--override" : active ? "session-card--running" : "session-card--idle"}`}
    >
      <div className="session-card-header">
        <span className="session-handle">{displayName}</span>
        <div className="session-card-actions">
          {session.inBattle && <span className="session-battle-badge">battle</span>}
          {activeOverride && <span className="session-override-badge">{scriptStatus}</span>}
          <button
            className="session-btn session-btn--icon session-btn--cargo"
            onClick={() => setOpenView("cargo")}
            title="Show ship cargo"
            aria-label={`Show ${displayName} cargo`}
            disabled={openView === "cargo"}
          >
            <SessionIcon name="cargo" />
          </button>
          {hasScript && (
            <button
              className="session-btn session-btn--icon session-btn--script"
              onClick={() => setOpenView("script")}
              title="Show ship script"
              aria-label={`Show ${displayName} script`}
              disabled={openView === "script"}
            >
              <SessionIcon name="script" />
            </button>
          )}
        </div>
      </div>
      <div
        className="session-fuel-bar"
        aria-label={fuelPct === null ? undefined : `Fuel ${fuelPct}%`}
        title={fuelPct === null ? undefined : `Fuel ${fuelPct}%`}
      >
        <div style={{ width: `${fuelPct ?? 0}%` }} />
      </div>
      <div
        className="session-cargo-bar"
        aria-label={cargoPct === null ? undefined : `Cargo ${cargoPct}%`}
        title={cargoPct === null ? undefined : `Cargo ${cargoPct}%`}
      >
        <div style={{ width: `${cargoPct ?? 0}%` }} />
      </div>

      <SessionLocationRow location={session.location} systemEmpires={systemEmpires} />
      <SessionStatsRow
        credits={session.credits}
        hull={session.hull}
        maxHull={session.maxHull}
        cargoUsed={session.cargoUsed}
        cargoCapacity={session.cargoCapacity}
        passengerBerths={session.passengerBerths}
      />

      {openView === "cargo" ? (
        <>
          <div className="session-script-toolbar">
            <span className="session-script-status">cargo</span>
            <button className="session-btn session-btn--close" onClick={() => setOpenView(null)} title="Close cargo view" aria-label="Close cargo view">
              x
            </button>
          </div>
          <CargoViewer cargo={session.cargo} passengers={session.passengersAboard} />
        </>
      ) : openView === "script" && session.runningScript ? (
        <>
          <div className="session-script-toolbar">
            <span className="session-script-status">{scriptStatus}</span>
            {session.runningScript.isRunning && (
              <button className="session-btn session-btn--pause" onClick={onHaltScript} title="Halt running script" aria-label="Halt running script">
                halt
              </button>
            )}
            <button className="session-btn session-btn--close" onClick={() => setOpenView(null)} title="Close script view" aria-label="Close script view">
              x
            </button>
          </div>
          <ScriptViewer script={session.runningScript.script} currentLine={session.runningScript.currentLine} />
        </>
      ) : active ? (
        <div className="session-feed session-feed--idle">
          <div className="session-feed-empty session-feed-empty--running">
            {activeOverride && <span className="session-run-status">{scriptStatus}</span>}
            <button className="session-btn session-btn--pause" onClick={onHaltScript} title="Halt running script" aria-label="Halt running script">
              halt
            </button>
          </div>
        </div>
      ) : (
        <div className="session-feed session-feed--idle">
          <div className="session-feed-empty">{hasScript ? "script loaded" : "idle"}</div>
        </div>
      )}
    </div>
  );
}

interface SessionsPanelProps {
  sessions: SessionState[];
  systemEmpires: Record<string, string>;
  onHaltScript: (handle: string) => void;
  onRegistered: () => Promise<void>;
}

function squadSections(sessions: SessionState[], squads: Squad[]): Array<{ id: string; title: string; color?: string; sessions: SessionState[] }> {
  const byIdentity = new Map(
    sessions.flatMap((session) => [
      [session.botId, session],
      [session.sessionHandle, session],
      ...(session.username ? [[session.username, session] as const] : []),
    ]),
  );
  const assigned = new Set<string>();
  const sections: Array<{ id: string; title: string; color?: string; sessions: SessionState[] }> = squads.flatMap((squad) => {
    const members = squad.botIds.map((id) => byIdentity.get(id)).filter((session): session is SessionState => Boolean(session));
    if (!members.some((session) => session.connected)) return [];
    members.forEach((session) => assigned.add(session.botId));
    return [{ id: squad.id, title: squad.name, color: squad.color, sessions: members }];
  });
  const unassigned = sessions.filter((session) => !assigned.has(session.botId)).sort((a, b) => a.sessionHandle.localeCompare(b.sessionHandle));
  if (unassigned.length) sections.push({ id: "__unassigned__", title: "unassigned", sessions: unassigned });
  return sections;
}

export default function SessionsPanel({ sessions, systemEmpires, onHaltScript, onRegistered }: SessionsPanelProps) {
  const [squads, setSquads] = useState<Squad[]>([]);
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(() => new Set());
  const [registerOpen, setRegisterOpen] = useState(false);
  const [username, setUsername] = useState("");
  const [empire, setEmpire] = useState("solarian");
  const [registrationCode, setRegistrationCode] = useState("");
  const [registering, setRegistering] = useState(false);
  const [registrationError, setRegistrationError] = useState<string | null>(null);
  const [registered, setRegistered] = useState<RegisterBotResult | null>(null);
  useEffect(() => {
    const refresh = () => void fetchSquads().then(setSquads);
    refresh();
    window.addEventListener("prayer-squads-updated", refresh);
    return () => window.removeEventListener("prayer-squads-updated", refresh);
  }, []);
  const groups = squadSections(sessions, squads);
  const toggleGroup = (id: string) =>
    setExpandedGroupIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const closeRegistration = () => {
    if (registering) return;
    setRegisterOpen(false);
    setRegistrationError(null);
    setRegistered(null);
  };
  const submitRegistration = async (event: FormEvent) => {
    event.preventDefault();
    setRegistering(true);
    setRegistrationError(null);
    try {
      const result = await registerBot({ username: username.trim(), empire, registrationCode: registrationCode.trim() || undefined });
      setRegistered(result);
      await onRegistered();
    } catch (error) {
      setRegistrationError(error instanceof Error ? error.message : String(error));
    } finally {
      setRegistering(false);
    }
  };
  return (
    <div className="sessions-panel">
      <div className="sessions-panel-header">
        <span className="sessions-panel-title">sessions</span>
        <button className="sessions-add-btn" onClick={() => setRegisterOpen(true)} title="Register a new bot" aria-label="Register a new bot">
          +
        </button>
      </div>
      <div className="sessions-list">
        {sessions.length === 0 && <div className="sessions-empty">no sessions</div>}
        {groups.map((group) => {
          const expanded = expandedGroupIds.has(group.id);
          return (
            <section className="sessions-squad" data-collapsed={!expanded} key={group.id} style={{ "--job-color": group.color } as CSSProperties}>
              <div className="sessions-squad-header">
                <button
                  className="sessions-squad-toggle"
                  onClick={() => toggleGroup(group.id)}
                  title={expanded ? `Collapse ${group.title}` : `Expand ${group.title}`}
                  aria-expanded={expanded}
                >
                  <span className="sessions-collapse-indicator">{expanded ? "v" : ">"}</span>
                  {group.color ? <span className="jobs-color-dot" style={{ background: group.color }} /> : null}
                  <span>{group.title}</span>
                  <span className="sessions-squad-count">{group.sessions.length}</span>
                </button>
              </div>
              {expanded &&
                group.sessions.map((session) => (
                  <SessionCard
                    key={session.botId}
                    session={session}
                    systemEmpires={systemEmpires}
                    onHaltScript={() => onHaltScript(session.sessionHandle)}
                  />
                ))}
            </section>
          );
        })}
      </div>
      {registerOpen ? (
        <div className="bot-register-overlay" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && closeRegistration()}>
          <div className="bot-register-dialog" role="dialog" aria-modal="true" aria-labelledby="bot-register-title">
            <div className="bot-register-header">
              <h2 id="bot-register-title">Register new bot</h2>
              <button type="button" onClick={closeRegistration} disabled={registering} aria-label="Close registration">
                ×
              </button>
            </div>
            {registered ? (
              <div className="bot-register-success">
                <p>
                  <strong>{registered.bot.name || username}</strong> is registered.
                </p>
                <label>
                  Player ID
                  <input readOnly value={registered.playerId} />
                </label>
                <label>
                  Password
                  <input readOnly value={registered.password} />
                </label>
                <p className="bot-register-warning">Save this password now. It will not be shown again.</p>
                <button type="button" className="session-btn" onClick={closeRegistration}>
                  Done
                </button>
              </div>
            ) : (
              <form className="bot-register-form" onSubmit={submitRegistration}>
                <label>
                  Username
                  <input value={username} onChange={(event) => setUsername(event.target.value)} required autoFocus />
                </label>
                <label>
                  Empire
                  <select value={empire} onChange={(event) => setEmpire(event.target.value)}>
                    <option value="solarian">Solarian</option>
                    <option value="voidborn">Voidborn</option>
                    <option value="crimson">Crimson</option>
                    <option value="nebula">Nebula</option>
                    <option value="outerrim">Outer Rim</option>
                  </select>
                </label>
                <label>
                  Registration code <span>(optional)</span>
                  <input value={registrationCode} onChange={(event) => setRegistrationCode(event.target.value)} />
                </label>
                {registrationError ? (
                  <div className="bot-register-error" role="alert">
                    {registrationError}
                  </div>
                ) : null}
                <div className="bot-register-actions">
                  <button type="button" className="session-btn" onClick={closeRegistration}>
                    Cancel
                  </button>
                  <button type="submit" className="session-btn" disabled={registering || !username.trim()}>
                    {registering ? "Registering…" : "Register bot"}
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
