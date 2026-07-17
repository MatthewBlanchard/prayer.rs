import { useMemo, useState } from "react";
import { PassengerInfo, PassengerSessionResult, PassengerState } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { CreditAmount } from "./Credits.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectPassengerStates } from "./prayer/worldSelectors.js";

type PassengersPanelProps = {
  sessions: SessionState[];
};

type LoadResult = PassengerSessionResult;

type PassengerRow = {
  key: string;
  passenger: PassengerInfo;
  stationId: string;
  stationName: string;
  system: string;
  observedBy: string[];
  stateVersion: number | null;
};

type StationGroup = {
  stationId: string;
  stationName: string;
  system: string;
  observedBy: string[];
  rows: PassengerRow[];
};

function displayStation(state: PassengerState): { id: string; name: string; system: string } {
  const id = state.station || state.currentPoi || "unknown";
  const name = state.station === state.currentPoi && state.currentPoiName ? state.currentPoiName : state.currentPoiName || state.station || id;
  return {
    id,
    name,
    system: state.system || "",
  };
}

function passengerIdentity(passenger: PassengerInfo): string {
  return passenger.citizenId || passenger.name;
}

function normalizeClassName(value: string): string {
  return value.trim().toLowerCase() || "unknown";
}

function classLabel(value: string): string {
  const normalized = normalizeClassName(value);
  if (normalized === "first") return "First";
  if (normalized === "business") return "Business";
  if (normalized === "economy") return "Economy";
  return value || "Unknown";
}

function fareValue(passenger: PassengerInfo): number | null {
  return passenger.estimatedFare ?? passenger.baseFare;
}

function mergeWaitingPassengers(results: LoadResult[]): StationGroup[] {
  const byStation = new Map<string, StationGroup>();

  for (const result of results) {
    const state = result.state;
    if (!state || state.waiting.length === 0) continue;
    const station = displayStation(state);
    const group = byStation.get(station.id) ?? {
      stationId: station.id,
      stationName: station.name,
      system: station.system,
      observedBy: [],
      rows: [],
    };
    if (!group.observedBy.includes(result.handle)) group.observedBy.push(result.handle);

    for (const passenger of state.waiting) {
      const identity = passengerIdentity(passenger);
      const key = `${station.id}:${identity || passenger.name}:${passenger.destination}`;
      const existing = group.rows.find((row) => row.key === key);
      if (existing) {
        if (!existing.observedBy.includes(result.handle)) existing.observedBy.push(result.handle);
        if ((existing.stateVersion ?? 0) <= (state.stateVersion ?? 0)) {
          existing.passenger = passenger;
          existing.stateVersion = state.stateVersion;
        }
        continue;
      }
      group.rows.push({
        key,
        passenger,
        stationId: station.id,
        stationName: station.name,
        system: station.system,
        observedBy: [result.handle],
        stateVersion: state.stateVersion,
      });
    }
    byStation.set(station.id, group);
  }

  return [...byStation.values()]
    .map((group) => ({
      ...group,
      observedBy: group.observedBy.sort(),
      rows: group.rows.sort(comparePassengerRows),
    }))
    .sort((a, b) => a.system.localeCompare(b.system) || a.stationName.localeCompare(b.stationName) || a.stationId.localeCompare(b.stationId));
}

function comparePassengerRows(a: PassengerRow, b: PassengerRow): number {
  return (
    normalizeClassName(a.passenger.className).localeCompare(normalizeClassName(b.passenger.className)) ||
    a.passenger.destination.localeCompare(b.passenger.destination) ||
    (fareValue(b.passenger) ?? -1) - (fareValue(a.passenger) ?? -1) ||
    a.passenger.name.localeCompare(b.passenger.name)
  );
}

function rowMatches(row: PassengerRow, query: string, classFilter: string): boolean {
  if (classFilter !== "all" && normalizeClassName(row.passenger.className) !== classFilter) {
    return false;
  }
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const passenger = row.passenger;
  return (
    passenger.name.toLowerCase().includes(needle) ||
    passenger.citizenId.toLowerCase().includes(needle) ||
    passenger.citizenship.toLowerCase().includes(needle) ||
    passenger.destination.toLowerCase().includes(needle) ||
    passenger.destinationName.toLowerCase().includes(needle) ||
    passenger.destinationSystem.toLowerCase().includes(needle) ||
    row.stationId.toLowerCase().includes(needle) ||
    row.stationName.toLowerCase().includes(needle) ||
    row.system.toLowerCase().includes(needle) ||
    row.observedBy.some((handle) => handle.toLowerCase().includes(needle))
  );
}

function knownClasses(groups: StationGroup[]): string[] {
  const values = new Set<string>();
  for (const group of groups) {
    for (const row of group.rows) values.add(normalizeClassName(row.passenger.className));
  }
  return ["all", ...[...values].filter(Boolean).sort()];
}

export default function PassengersPanel({ sessions }: PassengersPanelProps) {
  const prayer = usePrayer();
  const [query, setQuery] = useState("");
  const [classFilter, setClassFilter] = useState("all");
  const results = useMemo<LoadResult[]>(() => {
    const selected = selectPassengerStates(prayer.fleet, prayer.galaxyMap, prayer.stationPassengers);
    const byId = new Map(selected.map((result) => [result.state?.sessionId, result]));
    return sessions.map(
      (session) =>
        byId.get(session.botId) ??
        selected.find((result) => result.handle === session.sessionHandle) ?? {
          handle: session.sessionHandle,
          state: null,
          error: "state unavailable",
        },
    );
  }, [prayer.fleet, prayer.galaxyMap, prayer.stationPassengers, sessions]);
  const loading = prayer.connection === "connecting";
  const loadedAt = useMemo(() => {
    const observed = prayer.fleet.map((bot) => bot.observed_at).filter((value): value is string => Boolean(value));
    return observed.length ? new Date(observed.sort().at(-1)!) : null;
  }, [prayer.fleet]);

  const groups = useMemo(() => mergeWaitingPassengers(results), [results]);
  const classOptions = useMemo(() => knownClasses(groups), [groups]);
  const waiting = useMemo(() => groups.flatMap((group) => group.rows).sort(comparePassengerRows), [groups]);
  const filteredWaiting = useMemo(() => waiting.filter((row) => rowMatches(row, query, classFilter)), [waiting, query, classFilter]);
  const observedCount = results.filter((result) => result.state).length;
  const failedHandles = results.filter((result) => result.error || !result.state).map((result) => result.handle);
  const totalWaiting = groups.reduce((sum, group) => sum + group.rows.length, 0);
  const totalFare = groups.reduce((sum, group) => sum + group.rows.reduce((inner, row) => inner + (fareValue(row.passenger) ?? 0), 0), 0);

  if (!sessions.length) {
    return (
      <div className="passengers-panel">
        <div className="passengers-empty">No registered sessions.</div>
      </div>
    );
  }

  return (
    <div className="passengers-panel">
      <div className="passengers-toolbar">
        <div>
          <div className="passengers-title">Passengers</div>
          <div className="passengers-meta">
            {observedCount}/{sessions.length} sessions observed
            {loadedAt ? ` · updated ${loadedAt.toLocaleTimeString()}` : ""}
            {loading ? " · loading" : ""}
          </div>
        </div>
      </div>

      {failedHandles.length > 0 ? <div className="passengers-status">State unavailable for {failedHandles.join(", ")}</div> : null}

      <div className="passengers-body">
        <section className="passengers-card passengers-board-card">
          <div className="passengers-controls">
            <div className="passengers-card-title">Passenger board</div>
            <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="passenger, station, or destination" />
            <div className="passengers-segment" aria-label="Passenger class filter">
              {classOptions.map((option) => (
                <button key={option} type="button" data-active={classFilter === option} onClick={() => setClassFilter(option)}>
                  {option === "all" ? "All" : classLabel(option)}
                </button>
              ))}
            </div>
            <button className="session-btn" type="button" onClick={() => void prayer.refresh()} disabled={loading}>
              refresh
            </button>
          </div>
          <div className="passengers-summary">
            <span>{totalWaiting.toLocaleString()} waiting</span>
            <span>
              <CreditAmount value={totalFare} /> visible fare
            </span>
          </div>
          <PassengerTable rows={filteredWaiting} total={totalWaiting} emptyText="No matching waiting passengers." />
        </section>
      </div>
    </div>
  );
}

function PassengerTable({ rows, total, emptyText }: { rows: PassengerRow[]; total: number; emptyText: string }) {
  return (
    <section className="passengers-table-pane">
      <div className="passengers-table-head">
        <div className="passengers-card-title">Waiting</div>
        <div className="passengers-table-count">
          {rows.length < total ? `${rows.length.toLocaleString()} of ${total.toLocaleString()}` : total.toLocaleString()} passengers
        </div>
      </div>
      <div className="passengers-table-wrap">
        <table className="passengers-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Class</th>
              <th>Station</th>
              <th>Destination</th>
              <th>Fare</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key} data-class={normalizeClassName(row.passenger.className)}>
                <td>
                  <div className="passengers-name">{row.passenger.name}</div>
                  <div className="passengers-subtle">{row.passenger.citizenId || row.passenger.citizenship || "-"}</div>
                </td>
                <td>{classLabel(row.passenger.className)}</td>
                <td title={row.stationName}>{row.stationId}</td>
                <td title={row.passenger.destinationName}>{row.passenger.destination || "-"}</td>
                <td>
                  <CreditAmount value={fareValue(row.passenger)} />
                </td>
              </tr>
            ))}
            {rows.length === 0 ? (
              <tr>
                <td colSpan={5} className="passengers-empty">
                  {emptyText}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </section>
  );
}
