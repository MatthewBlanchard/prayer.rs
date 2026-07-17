import { useEffect, useMemo, useState } from "react";
import { WildlifeCreature, WildlifeState } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectWildlifeStates } from "./prayer/worldSelectors.js";

type WildlifePanelProps = {
  sessions: SessionState[];
};

type LoadResult = {
  handle: string;
  state: WildlifeState | null;
  error: string | null;
};

type SpeciesEntry = {
  species: string;
  name: string;
  role: string;
  count: number;
  systems: string[];
  pois: string[];
  observedAtUnix: number;
  creatures: WildlifeCreature[];
  observedBy: string[];
};

function formatAge(observedAtUnix: number): string {
  if (!observedAtUnix) return "unknown";
  const minutes = Math.max(0, Math.floor((Date.now() / 1000 - observedAtUnix) / 60));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `${hours}h ${minutes % 60}m`;
  return `${Math.floor(hours / 24)}d`;
}

function hpPercent(creature: WildlifeCreature): number {
  if (creature.maxHull <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((creature.hull / creature.maxHull) * 100)));
}

function normalizeRole(role: string): string {
  return role.trim().toLowerCase() || "unknown";
}

function mergeSpecies(results: LoadResult[]): SpeciesEntry[] {
  const bySpecies = new Map<string, SpeciesEntry>();
  const creatureKeys = new Set<string>();

  for (const result of results) {
    const state = result.state;
    if (!state) continue;

    for (const system of state.systems) {
      for (const row of system.species) {
        const existing = bySpecies.get(row.species) ?? {
          species: row.species,
          name: row.name,
          role: row.role,
          count: 0,
          systems: [],
          pois: [],
          observedAtUnix: 0,
          creatures: [],
          observedBy: [],
        };
        existing.name = existing.name || row.name;
        existing.role = existing.role === "unknown" ? row.role : existing.role;
        existing.count = Math.max(existing.count, row.count);
        existing.observedAtUnix = Math.max(existing.observedAtUnix, system.observedAtUnix);
        if (!existing.systems.includes(system.systemId)) existing.systems.push(system.systemId);
        for (const poi of system.pois) {
          if (!existing.pois.includes(poi)) existing.pois.push(poi);
        }
        if (!existing.observedBy.includes(result.handle)) existing.observedBy.push(result.handle);
        bySpecies.set(row.species, existing);
      }
    }

    for (const poi of state.pois) {
      for (const creature of poi.creatures) {
        const existing = bySpecies.get(creature.species) ?? {
          species: creature.species,
          name: creature.name,
          role: creature.role,
          count: 0,
          systems: [],
          pois: [],
          observedAtUnix: 0,
          creatures: [],
          observedBy: [],
        };
        existing.name = existing.name || creature.name;
        existing.role = existing.role === "unknown" ? creature.role : existing.role;
        existing.count = Math.max(existing.count, poi.creatureCount);
        existing.observedAtUnix = Math.max(existing.observedAtUnix, creature.observedAtUnix);
        if (!existing.systems.includes(creature.systemId)) existing.systems.push(creature.systemId);
        if (!existing.pois.includes(creature.poiId)) existing.pois.push(creature.poiId);
        if (!existing.observedBy.includes(result.handle)) existing.observedBy.push(result.handle);
        const key = `${creature.systemId}:${creature.poiId}:${creature.creatureId}`;
        if (!creatureKeys.has(key)) {
          creatureKeys.add(key);
          existing.creatures.push(creature);
        }
        bySpecies.set(creature.species, existing);
      }
    }
  }

  return [...bySpecies.values()]
    .map((entry) => ({
      ...entry,
      systems: entry.systems.sort(),
      pois: entry.pois.sort(),
      observedBy: entry.observedBy.sort(),
      creatures: entry.creatures.sort(
        (a, b) =>
          b.observedAtUnix - a.observedAtUnix ||
          a.systemId.localeCompare(b.systemId) ||
          a.poiId.localeCompare(b.poiId) ||
          a.creatureId.localeCompare(b.creatureId),
      ),
    }))
    .sort((a, b) => normalizeRole(a.role).localeCompare(normalizeRole(b.role)) || a.name.localeCompare(b.name) || a.species.localeCompare(b.species));
}

export default function WildlifePanel({ sessions }: WildlifePanelProps) {
  const prayer = usePrayer();
  const results = useMemo<LoadResult[]>(
    () =>
      selectWildlifeStates(prayer.fleet, prayer.galaxyWildlife).map((result) => ({
        ...result,
        error: prayer.error?.message ?? null,
      })),
    [prayer.error, prayer.fleet, prayer.galaxyWildlife],
  );
  const [query, setQuery] = useState("");
  const [roleFilter, setRoleFilter] = useState("all");
  const [selectedSpecies, setSelectedSpecies] = useState<string | null>(null);
  const loading = prayer.connection === "connecting";
  const loadedAt = prayer.galaxyWildlife ? new Date() : null;

  const species = useMemo(() => mergeSpecies(results), [results]);
  const roles = useMemo(() => {
    const values = new Set(species.map((entry) => normalizeRole(entry.role)));
    return ["all", ...[...values].sort()];
  }, [species]);

  const filteredSpecies = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return species.filter((entry) => {
      if (roleFilter !== "all" && normalizeRole(entry.role) !== roleFilter) return false;
      if (!needle) return true;
      return (
        entry.species.toLowerCase().includes(needle) ||
        entry.name.toLowerCase().includes(needle) ||
        entry.role.toLowerCase().includes(needle) ||
        entry.systems.some((system) => system.toLowerCase().includes(needle)) ||
        entry.pois.some((poi) => poi.toLowerCase().includes(needle))
      );
    });
  }, [species, query, roleFilter]);

  useEffect(() => {
    if (filteredSpecies.length === 0) {
      setSelectedSpecies(null);
      return;
    }
    if (!selectedSpecies || !filteredSpecies.some((entry) => entry.species === selectedSpecies)) {
      setSelectedSpecies(filteredSpecies[0]!.species);
    }
  }, [filteredSpecies, selectedSpecies]);

  const selected = filteredSpecies.find((entry) => entry.species === selectedSpecies) ?? filteredSpecies[0] ?? null;
  const observedCount = results.filter((result) => result.state).length;
  const failedHandles = results.filter((result) => result.error || !result.state).map((result) => result.handle);
  const currentNearby = results.flatMap((result) => result.state?.nearbyCreatures ?? []);
  const totalCreatures = species.reduce((sum, entry) => sum + Math.max(entry.count, entry.creatures.length), 0);

  if (!sessions.length) {
    return (
      <div className="wildlife-panel">
        <div className="wildlife-empty">No registered sessions.</div>
      </div>
    );
  }

  return (
    <div className="wildlife-panel">
      <div className="wildlife-toolbar">
        <div>
          <div className="wildlife-title">Wildlife Index</div>
          <div className="wildlife-meta">
            {species.length.toLocaleString()} species · {totalCreatures.toLocaleString()} sightings · {observedCount}/{sessions.length} sessions
            {loadedAt ? ` · updated ${loadedAt.toLocaleTimeString()}` : ""}
          </div>
        </div>
        <div className="wildlife-controls">
          <div className="wildlife-segment" role="group" aria-label="Wildlife role">
            {roles.map((role) => (
              <button key={role} data-active={roleFilter === role} onClick={() => setRoleFilter(role)}>
                {role}
              </button>
            ))}
          </div>
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="species, system, POI" />
          <button className="session-btn" onClick={() => void prayer.refreshKnowledge()} disabled={loading}>
            refresh
          </button>
        </div>
      </div>

      {failedHandles.length > 0 && <div className="wildlife-status">Missing wildlife state for {failedHandles.join(", ")}.</div>}

      <div className="wildlife-body">
        <aside className="wildlife-dex-list" aria-label="Species">
          {filteredSpecies.map((entry, index) => (
            <button
              key={entry.species}
              className="wildlife-dex-entry"
              data-active={selected?.species === entry.species}
              data-role={normalizeRole(entry.role)}
              onClick={() => setSelectedSpecies(entry.species)}
            >
              <span className="wildlife-dex-number">#{String(index + 1).padStart(3, "0")}</span>
              <span className="wildlife-dex-name">{entry.name}</span>
              <span className="wildlife-dex-species">{entry.species}</span>
              <span className="wildlife-dex-meta">
                {entry.role} · {Math.max(entry.count, entry.creatures.length).toLocaleString()}
              </span>
            </button>
          ))}
          {!loading && filteredSpecies.length === 0 && <div className="wildlife-empty">No wildlife observations.</div>}
        </aside>

        <main className="wildlife-detail">
          {selected ? (
            <>
              <section className="wildlife-profile" data-role={normalizeRole(selected.role)}>
                <div className="wildlife-profile-mark" aria-hidden="true">
                  {selected.name.slice(0, 2).toUpperCase()}
                </div>
                <div className="wildlife-profile-main">
                  <div className="wildlife-profile-kicker">{selected.species}</div>
                  <h2>{selected.name}</h2>
                  <div className="wildlife-profile-tags">
                    <span>{selected.role}</span>
                    <span>{selected.systems.length} systems</span>
                    <span>{selected.pois.length} POIs</span>
                    <span>seen {formatAge(selected.observedAtUnix)} ago</span>
                  </div>
                </div>
              </section>

              <section className="wildlife-stat-grid">
                <div>
                  <span>estimated</span>
                  <strong>{selected.count.toLocaleString()}</strong>
                </div>
                <div>
                  <span>tracked</span>
                  <strong>{selected.creatures.length.toLocaleString()}</strong>
                </div>
                <div>
                  <span>nearby</span>
                  <strong>{currentNearby.filter((creature) => creature.species === selected.species).length.toLocaleString()}</strong>
                </div>
                <div>
                  <span>reported by</span>
                  <strong>{selected.observedBy.length.toLocaleString()}</strong>
                </div>
              </section>

              <section className="wildlife-locations">
                <div className="wildlife-section-title">Locations</div>
                <div className="wildlife-chip-row">
                  {selected.systems.map((system) => (
                    <span key={`system:${system}`}>{system}</span>
                  ))}
                  {selected.pois.map((poi) => (
                    <span key={`poi:${poi}`}>{poi}</span>
                  ))}
                </div>
              </section>

              <section className="wildlife-creatures">
                <div className="wildlife-section-title">Creatures</div>
                <div className="wildlife-creature-table-wrap">
                  <table className="wildlife-creature-table">
                    <thead>
                      <tr>
                        <th>id</th>
                        <th>location</th>
                        <th>hull</th>
                        <th>state</th>
                        <th>age</th>
                      </tr>
                    </thead>
                    <tbody>
                      {selected.creatures.map((creature) => (
                        <tr key={`${creature.systemId}:${creature.poiId}:${creature.creatureId}`} data-combat={creature.inCombat}>
                          <td>{creature.creatureId}</td>
                          <td>
                            {creature.systemId}/{creature.poiId}
                          </td>
                          <td>
                            <div className="wildlife-hull">
                              <span style={{ width: `${hpPercent(creature)}%` }} />
                            </div>
                            {creature.hull.toLocaleString()}/{creature.maxHull.toLocaleString()}
                          </td>
                          <td>{creature.inCombat ? "combat" : "calm"}</td>
                          <td>{formatAge(creature.observedAtUnix)}</td>
                        </tr>
                      ))}
                      {selected.creatures.length === 0 && (
                        <tr>
                          <td colSpan={5} className="wildlife-empty">
                            No individual creatures recorded for this species.
                          </td>
                        </tr>
                      )}
                    </tbody>
                  </table>
                </div>
              </section>
            </>
          ) : (
            <div className="wildlife-empty">No wildlife observations.</div>
          )}
        </main>
      </div>
    </div>
  );
}
