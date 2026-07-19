import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { CatalogDumpItemsItem } from "@prayer/sdk/types";
import type { JobConfig, JobDefinition, JobRun, Squad } from "../shared/types.js";
import { connectEvents, createSquad, fetchJobDefinitions, fetchJobRuns, fetchSquads, startJobRun, stopJobRun, updateSquad } from "./api.js";
import EmbeddedGalaxyMap from "./EmbeddedGalaxyMap.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import type { SessionState } from "./SessionsPanel.js";
import { clientPlugins } from "./plugins.js";
import { readVersionedStoredRecord, writeVersionedStored } from "./persistence.js";
import { findNearestStationPoi, isStationPoi } from "./nearestStation.js";

const active = new Set(["queued", "starting", "running", "stopping"]);
export default function JobsPanel({
  sessions,
  requestedRunId = null,
  requestedSquadId = null,
  navigationRequest = 0,
}: {
  sessions: SessionState[];
  requestedRunId?: string | null;
  requestedSquadId?: string | null;
  navigationRequest?: number;
}) {
  const prayer = usePrayer();
  const [definitions, setDefinitions] = useState<JobDefinition[]>([]);
  const [runs, setRuns] = useState<JobRun[]>([]);
  const [squads, setSquads] = useState<Squad[]>([]);
  const [squadId, setSquadId] = useState<string>("");
  const [kind, setKind] = useState<string>("navigate");
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [jobMenuOpen, setJobMenuOpen] = useState(false);
  const [draft, setDraft] = useState<Record<string, unknown>>({});
  const [error, setError] = useState<string | null>(null);
  const handledNavigationRequest = useRef<number | null>(null);
  useEffect(() => {
    void Promise.all([fetchJobDefinitions(), fetchJobRuns(), fetchSquads()])
      .then(([defs, history, loadedSquads]) => {
        setDefinitions(defs);
        setRuns(history);
        setSquads(loadedSquads);
        setSquadId(requestedSquadId && loadedSquads.some((item) => item.id === requestedSquadId) ? requestedSquadId : (loadedSquads[0]?.id ?? ""));
        setKind((current) => (defs.some((item) => item.kind === current) ? current : (defs[0]?.kind ?? current)));
      })
      .catch((reason) => setError(String(reason)));
    return connectEvents(
      (event) => {
        if (event.type === "job_run_updated") setRuns((current) => upsert(current, event.run));
        if (event.type === "state_sync" && event.jobRuns) setRuns((current) => mergeRuns(current, event.jobRuns!));
      },
      () => {},
    );
  }, []);
  const definition = definitions.find((item) => item.kind === kind);
  const squad = squads.find((item) => item.id === squadId);
  const selectedRun = runs.find((run) => run.id === selectedRunId) ?? runs.find((run) => run.squadId === squadId && active.has(run.status));
  useEffect(() => {
    if (!requestedSquadId || navigationRequest === 0) return;
    setSquadId(requestedSquadId);
    setSelectedRunId(requestedRunId);
  }, [navigationRequest, requestedRunId, requestedSquadId]);
  useEffect(() => {
    if (!requestedRunId || handledNavigationRequest.current === navigationRequest) return;
    const requestedRun = runs.find((run) => run.id === requestedRunId);
    if (!requestedRun) return;
    handledNavigationRequest.current = navigationRequest;
    setKind(requestedRun.kind);
    setSquadId(requestedRun.squadId);
    setSelectedRunId(requestedRun.id);
  }, [navigationRequest, requestedRunId, runs]);
  const lockedBotIds = new Set(runs.filter((run) => active.has(run.status)).flatMap((run) => run.config.botIds));
  const assignedBotIds = new Set(squads.flatMap((item) => item.botIds));
  const availableSessions = sessions.filter((session) => !assignedBotIds.has(session.botId ?? session.sessionHandle));
  const resourceIds = useMemo(
    () =>
      Object.entries(prayer.galaxyResources?.poisByResource ?? {})
        .filter(([, poiIds]) => poiIds.length > 0)
        .map(([resourceId]) => resourceId)
        .sort((a, b) =>
          resourceLabel(a, prayer.catalog?.itemsById[a]).localeCompare(resourceLabel(b, prayer.catalog?.itemsById[b]), undefined, {
            numeric: true,
            sensitivity: "base",
          }),
        ),
    [prayer.catalog, prayer.galaxyResources],
  );
  const miningPoiIds =
    definition?.kind === "mine" && typeof draft["resourceId"] === "string" ? (prayer.galaxyResources?.poisByResource[draft["resourceId"]] ?? []) : [];
  const destinationPoi = typeof draft["destinationPoi"] === "string" ? draft["destinationPoi"] : "";
  const factionStorageAvailable =
    definition?.kind === "mine" &&
    Boolean(destinationPoi) &&
    Boolean(squad?.botIds.length) &&
    squad!.botIds.every((botId) => {
      const bot = prayer.fleet.find((candidate) => candidate.id === botId || candidate.username === botId);
      const factionId = typeof bot?.state.player.faction_id === "string" ? bot.state.player.faction_id : null;
      return factionId != null && Object.prototype.hasOwnProperty.call(prayer.factionStorageByFactionPoi[factionId] ?? {}, destinationPoi);
    });
  useEffect(() => {
    if (!definition) return;
    const saved = readSavedConfig(definition.kind);
    setDraft(saved ? saved : { ...definition.defaults });
    setSelectedRunId(null);
  }, [definition?.kind]);
  useEffect(() => {
    if (definition?.kind !== "mine" || !resourceIds.length) return;
    const resourceId = typeof draft["resourceId"] === "string" && resourceIds.includes(draft["resourceId"]) ? draft["resourceId"] : resourceIds[0]!;
    const poiIds = prayer.galaxyResources?.poisByResource[resourceId] ?? [];
    const miningPoi = typeof draft["miningPoi"] === "string" && poiIds.includes(draft["miningPoi"]) ? draft["miningPoi"] : "";
    if (resourceId !== draft["resourceId"] || miningPoi !== draft["miningPoi"]) setDraft((current) => ({ ...current, resourceId, miningPoi }));
  }, [definition?.kind, draft, prayer.galaxyResources, resourceIds]);
  useEffect(() => {
    const miningPoi = typeof draft["miningPoi"] === "string" ? draft["miningPoi"] : "";
    if (definition?.kind !== "mine" || !miningPoi || destinationPoi || !prayer.galaxyMap) return;
    let cancelled = false;
    void findNearestStationPoi(prayer.galaxyMap, miningPoi)
      .then((stationPoi) => {
        if (!cancelled && stationPoi) setDraft((current) => ({ ...current, destinationPoi: stationPoi }));
      })
      .catch((reason) => {
        if (!cancelled) setError(`Could not resolve the nearest station: ${reason instanceof Error ? reason.message : String(reason)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [definition?.kind, destinationPoi, draft, prayer.galaxyMap]);
  useEffect(() => {
    if (definition?.kind === "mine" && draft["storageTarget"] === "faction" && !factionStorageAvailable) {
      setDraft((current) => ({ ...current, storageTarget: "personal" }));
    }
  }, [definition?.kind, draft, factionStorageAvailable]);
  const config = useMemo(() => buildJobConfig(definition, draft, squad?.botIds ?? []), [definition, draft, squad]);
  const canExecute =
    Boolean(squad && config?.botIds.length) &&
    !config?.botIds.some((id) => lockedBotIds.has(id)) &&
    (config?.kind !== "mine" ||
      (typeof config.resourceId === "string" &&
        resourceIds.includes(config.resourceId) &&
        typeof config.miningPoi === "string" &&
        miningPoiIds.includes(config.miningPoi) &&
        typeof config.destinationPoi === "string" &&
        prayer.galaxyMap?.knownPois.some((poi) => poi.id === config.destinationPoi && isStationPoi(poi))));
  const launchDisabledReason = (() => {
    if (!squad) return "Select a squad first.";
    if (!squad.botIds.length) return "Add at least one bot to the squad.";
    const busyBots = squad.botIds.filter((id) => lockedBotIds.has(id));
    if (busyBots.length) return `${busyBots.join(", ")} ${busyBots.length === 1 ? "is" : "are"} already running another job.`;
    if (!config) {
      const values = { ...definition?.defaults, ...draft };
      const missing = definition?.fields.find(
        (field) => field.name !== "botIds" && field.required && (values[field.name] === undefined || values[field.name] === ""),
      );
      return missing ? `Complete the required ${missing.label} field.` : "Complete the job configuration.";
    }
    if (config.kind === "mine") {
      if (typeof config.resourceId !== "string" || !resourceIds.includes(config.resourceId)) return "Select a known resource.";
      if (typeof config.miningPoi !== "string" || !miningPoiIds.includes(config.miningPoi)) return "Select a highlighted mining location.";
      if (typeof config.destinationPoi !== "string" || !prayer.galaxyMap?.knownPois.some((poi) => poi.id === config.destinationPoi && isStationPoi(poi)))
        return "Select a known station as the drop-off location.";
    }
    return null;
  })();
  async function execute() {
    if (!config) return;
    setError(null);
    try {
      writeVersionedStored(`prayer-job-config:${config.kind}`, 1, draft);
      const run = await startJobRun(squadId, config);
      setRuns((current) => upsert(current, run));
      setSelectedRunId(run.id);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }
  async function setSquadMembers(nextBotIds: string[]) {
    if (!squad) return;
    setSquads((current) => current.map((item) => (item.id === squad.id ? { ...item, botIds: nextBotIds } : item)));
    try {
      await updateSquad(squad.id, { botIds: nextBotIds });
      window.dispatchEvent(new Event("prayer-squads-updated"));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setSquads(await fetchSquads());
    }
  }
  return (
    <div className="jobs-panel job-center">
      <aside className="jobs-list">
        <div className="jobs-list-head">
          <span className="jobs-list-title">Squads</span>
          <button
            className="session-btn"
            title="New squad"
            aria-label="New squad"
            onClick={() =>
              void createSquad().then((created) => {
                setSquads((current) => [...current, created]);
                setSquadId(created.id);
                setSelectedRunId(null);
                window.dispatchEvent(new Event("prayer-squads-updated"));
              })
            }
          >
            +
          </button>
        </div>
        <div className="jobs-list-items jobs-squad-list">
          {squads.map((item) => (
            <button
              key={item.id}
              className="jobs-list-item"
              data-active={item.id === squadId}
              onClick={() => {
                setSquadId(item.id);
                setSelectedRunId(null);
              }}
            >
              <span className="jobs-list-name">
                <i className="jobs-color-dot" style={{ background: item.color }} />
                {item.name}
              </span>
              <span className="jobs-list-meta">
                {runs.some((run) => run.squadId === item.id && active.has(run.status)) ? "● active" : `${item.botIds.length} bots`}
              </span>
            </button>
          ))}
        </div>
        <div className="jobs-list-head">
          <span className="jobs-list-title">Recent runs</span>
        </div>
        <div className="jobs-list-items jobs-recent-list">
          {runs.slice(0, 8).map((run) => (
            <button
              key={run.id}
              className="jobs-list-item"
              onClick={() => {
                setKind(run.kind);
                if (run.squadId) setSquadId(run.squadId);
                setSelectedRunId(run.id);
              }}
            >
              <span>{definitions.find((item) => item.kind === run.kind)?.title ?? run.kind}</span>
              <span className="jobs-list-meta">{run.status}</span>
            </button>
          ))}
        </div>
      </aside>
      <section className="jobs-detail">
        {definition ? (
          selectedRun ? (
            <RunView
              run={selectedRun}
              onEdit={() => setSelectedRunId(null)}
              onStop={async (mode) => {
                try {
                  const stopped = await stopJobRun(selectedRun.id, mode);
                  setRuns((current) => upsert(current, stopped));
                } catch (reason) {
                  setError(String(reason));
                }
              }}
            />
          ) : (
            <div className="job-config job-config-shell" data-kind={definition.kind}>
              <div className="squad-hero" style={{ "--job-color": squad?.color ?? "var(--yellow)" } as CSSProperties}>
                <div className="squad-hero-copy">
                  <span className="squad-hero-eyebrow">Selected squad</span>
                  <h2>{squad?.name ?? "Select a squad"}</h2>
                  {squad ? (
                    <div className="jobs-roster" aria-label={`${squad.name} members`}>
                      {squad.botIds.map((botId) => {
                        const session = sessions.find((item) => (item.botId ?? item.sessionHandle) === botId || item.sessionHandle === botId);
                        const label = session?.username ?? session?.sessionHandle ?? botId;
                        return (
                          <button
                            key={botId}
                            type="button"
                            className="jobs-chip"
                            disabled={lockedBotIds.has(botId)}
                            onClick={() => void setSquadMembers(squad.botIds.filter((id) => id !== botId))}
                            title={lockedBotIds.has(botId) ? "Bot is locked by an active job" : "Remove from squad"}
                          >
                            {label} ×
                          </button>
                        );
                      })}
                      {availableSessions.length > 0 ? (
                        <select
                          className="jobs-add-select"
                          value=""
                          onChange={(event) => {
                            const botId = event.target.value;
                            if (botId) void setSquadMembers([...squad.botIds, botId]);
                          }}
                        >
                          <option value="">assign bot</option>
                          {availableSessions.map((session) => {
                            const botId = session.botId ?? session.sessionHandle;
                            return (
                              <option key={botId} value={botId}>
                                {session.username ?? session.sessionHandle}
                              </option>
                            );
                          })}
                        </select>
                      ) : null}
                    </div>
                  ) : (
                    <p>Choose a squad from the left.</p>
                  )}
                </div>
              </div>
              {!squad ? <div className="error-banner">Select a squad before running a job.</div> : null}
              <div className="job-kind-picker">
                <button
                  className="job-kind-trigger"
                  type="button"
                  aria-expanded={jobMenuOpen}
                  aria-controls="job-kind-menu"
                  onClick={() => setJobMenuOpen((current) => !current)}
                >
                  <span>Job</span>
                  <strong>{definition.title}</strong>
                  <span className="job-kind-trigger-arrow">{jobMenuOpen ? "‹" : "›"}</span>
                </button>
                {jobMenuOpen ? (
                  <div className="job-kind-menu" id="job-kind-menu">
                    {definitions.map((item) => (
                      <button
                        key={item.kind}
                        type="button"
                        data-active={item.kind === kind}
                        onClick={() => {
                          setKind(item.kind);
                          setJobMenuOpen(false);
                        }}
                      >
                        <strong>{item.title}</strong>
                        <span>{item.description}</span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
              <section className="job-options-panel" data-kind={definition.kind}>
                {definition.kind !== "navigate" && definition.kind !== "mine" ? (
                  <div className="job-options-head">
                    <span>Configuration</span>
                    <small>{definition.title}</small>
                  </div>
                ) : null}
                <div className="job-field-grid">
                  {definition.fields
                    .filter((field) => field.name !== "botIds" && !(definition.kind === "mine" && field.name === "destinationPoi"))
                    .map((field) => (
                      <label className="job-field" key={field.name}>
                        <span>
                          {field.label}
                          {field.required ? " *" : ""}
                        </span>
                        {definition.kind === "mine" && field.name === "resourceId" ? (
                          <select
                            value={String(draft[field.name] ?? "")}
                            onChange={(event) => setDraft({ ...draft, resourceId: event.target.value, miningPoi: "", destinationPoi: "" })}
                          >
                            {resourceIds.length ? null : <option value="">No known resources</option>}
                            {resourceIds.map((resourceId) => (
                              <option key={resourceId} value={resourceId}>
                                {resourceLabel(resourceId, prayer.catalog?.itemsById[resourceId])}
                              </option>
                            ))}
                          </select>
                        ) : definition.kind === "mine" && field.name === "miningPoi" ? (
                          <MineRoutePicker
                            map={prayer.galaxyMap}
                            sessions={sessions.filter((session) => squad?.botIds.includes(session.botId) || squad?.botIds.includes(session.sessionHandle))}
                            resourceId={String(draft["resourceId"] ?? "")}
                            miningPoiIds={miningPoiIds}
                            resourceSystemIds={prayer.galaxyResources?.systemsByResource[String(draft["resourceId"] ?? "")] ?? []}
                            miningPoi={String(draft["miningPoi"] ?? "")}
                            destinationPoi={String(draft["destinationPoi"] ?? "")}
                            onChange={(miningPoi, destinationPoi) => setDraft((current) => ({ ...current, miningPoi, destinationPoi }))}
                          />
                        ) : definition.kind === "mine" && field.name === "storageTarget" ? (
                          <select
                            value={String(draft[field.name] ?? "personal")}
                            onChange={(event) => setDraft({ ...draft, storageTarget: event.target.value })}
                          >
                            <option value="personal">Personal storage</option>
                            {factionStorageAvailable ? <option value="faction">Faction storage</option> : null}
                          </select>
                        ) : definition.kind === "navigate" && field.name === "destination" ? (
                          <GoDestinationPicker
                            destination={String(draft[field.name] ?? "")}
                            map={prayer.galaxyMap}
                            sessions={sessions.filter((session) => squad?.botIds.includes(session.botId) || squad?.botIds.includes(session.sessionHandle))}
                            onChange={(destination) => setDraft({ ...draft, destination })}
                          />
                        ) : field.type === "textarea" ? (
                          <textarea
                            rows={12}
                            value={String(draft[field.name] ?? "")}
                            onChange={(event) => setDraft({ ...draft, [field.name]: event.target.value })}
                          />
                        ) : field.type === "boolean" ? (
                          <input
                            type="checkbox"
                            checked={Boolean(draft[field.name])}
                            onChange={(event) => setDraft({ ...draft, [field.name]: event.target.checked })}
                          />
                        ) : (
                          <input
                            type={field.type}
                            value={String(draft[field.name] ?? "")}
                            onChange={(event) =>
                              setDraft({ ...draft, [field.name]: field.type === "number" ? Number(event.target.value) : event.target.value })
                            }
                          />
                        )}
                        {field.description && !(definition.kind === "mine" && field.name === "miningPoi") ? <small>{field.description}</small> : null}
                      </label>
                    ))}
                </div>
                {definition.kind === "explore" ? (
                  <ExploreMapPreview
                    map={prayer.galaxyMap}
                    sessions={sessions.filter((session) => squad?.botIds.includes(session.botId) || squad?.botIds.includes(session.sessionHandle))}
                    exclusionHops={Number(draft["strongholdExclusionHops"] ?? definition.defaults["strongholdExclusionHops"] ?? 0)}
                    manuallyBlacklisted={arrayValue(draft["manuallyBlacklistedSystemIds"])}
                    manuallyUnblacklisted={arrayValue(draft["manuallyUnblacklistedSystemIds"])}
                    onOverridesChange={(manuallyBlacklistedSystemIds, manuallyUnblacklistedSystemIds) =>
                      setDraft((current) => ({ ...current, manuallyBlacklistedSystemIds, manuallyUnblacklistedSystemIds }))
                    }
                  />
                ) : null}
              </section>
              {error && <div className="error-banner">{error}</div>}
              <div className="job-execute">
                <div>
                  <strong>{definition.title}</strong>
                  <span>{squad ? `Run for ${squad.name}` : "Select a squad to continue"}</span>
                </div>
                <span className="job-execute-button-tooltip" title={launchDisabledReason ?? undefined}>
                  <button className="job-execute-button" disabled={!canExecute} onClick={() => void execute()}>
                    Launch job <span>→</span>
                  </button>
                </span>
              </div>
            </div>
          )
        ) : (
          <div>Loading job catalog…</div>
        )}
      </section>
    </div>
  );
}

type GoMap = NonNullable<ReturnType<typeof usePrayer>["galaxyMap"]>;

function ExploreMapPreview({
  map,
  sessions,
  exclusionHops,
  manuallyBlacklisted,
  manuallyUnblacklisted,
  onOverridesChange,
}: {
  map: GoMap | null;
  sessions: SessionState[];
  exclusionHops: number;
  manuallyBlacklisted: string[];
  manuallyUnblacklisted: string[];
  onOverridesChange: (blacklisted: string[], unblacklisted: string[]) => void;
}) {
  const { automaticallyBlacklisted, blacklistedSystems } = useMemo(() => {
    if (!map) return { automaticallyBlacklisted: new Set<string>(), blacklistedSystems: [] };
    const systemsById = new Map(map.systems.map((system) => [system.id, system]));
    const distances = new Map<string, number>();
    const queue = map.systems.filter((system) => system.isStronghold).map((system) => system.id);
    for (const systemId of queue) distances.set(systemId, 0);
    for (let index = 0; index < queue.length; index += 1) {
      const systemId = queue[index]!;
      const distance = distances.get(systemId)!;
      for (const connectedId of systemsById.get(systemId)?.connections ?? []) {
        if (!systemsById.has(connectedId) || distances.has(connectedId)) continue;
        distances.set(connectedId, distance + 1);
        queue.push(connectedId);
      }
    }
    const limit = Number.isFinite(exclusionHops) ? Math.max(0, Math.floor(exclusionHops)) : 0;
    const automatic = new Set(map.systems.filter((system) => (distances.get(system.id) ?? Number.POSITIVE_INFINITY) <= limit).map((system) => system.id));
    const unblacklisted = new Set(manuallyUnblacklisted);
    const combined = new Set([...automatic].filter((id) => !unblacklisted.has(id)));
    for (const id of manuallyBlacklisted) if (systemsById.has(id)) combined.add(id);
    return { automaticallyBlacklisted: automatic, blacklistedSystems: [...combined].sort() };
  }, [exclusionHops, manuallyBlacklisted, manuallyUnblacklisted, map]);

  function toggleSystem(systemId: string) {
    const isBlacklisted = blacklistedSystems.includes(systemId);
    if (automaticallyBlacklisted.has(systemId)) {
      const next = new Set(manuallyUnblacklisted);
      if (isBlacklisted) next.add(systemId);
      else next.delete(systemId);
      onOverridesChange(
        manuallyBlacklisted.filter((id) => id !== systemId),
        [...next].sort(),
      );
    } else {
      const next = new Set(manuallyBlacklisted);
      if (isBlacklisted) next.delete(systemId);
      else next.add(systemId);
      onOverridesChange(
        [...next].sort(),
        manuallyUnblacklisted.filter((id) => id !== systemId),
      );
    }
  }

  return (
    <div className="explore-map-preview">
      <div className="explore-map-controls">
        <span>Click systems to blacklist or unblacklist them.</span>
        <button
          type="button"
          className="session-btn"
          onClick={() => onOverridesChange([], [])}
          disabled={!manuallyBlacklisted.length && !manuallyUnblacklisted.length}
        >
          Reset blacklist
        </button>
      </div>
      <div className="go-mini-map" aria-label="Explore galactic map">
        <EmbeddedGalaxyMap sessions={sessions} highlightedSystemIds={blacklistedSystems} onSystemClick={toggleSystem} />
      </div>
    </div>
  );
}

function GoDestinationPicker({
  destination,
  map,
  sessions,
  onChange,
}: {
  destination: string;
  map: GoMap | null;
  sessions: SessionState[];
  onChange: (destination: string) => void;
}) {
  const options = useMemo(() => {
    if (!map) return [];
    const systems = map.systems.map((system) => ({ id: system.id, label: `${humanize(system.id)} — system` }));
    const pois = map.knownPois.map((poi) => ({ id: poi.id, label: `${poi.name || humanize(poi.id)} — ${humanize(poi.systemId)}` }));
    return [...systems, ...pois].sort((a, b) => a.label.localeCompare(b.label));
  }, [map]);
  const selectedSystem = useMemo(() => {
    if (!map) return destination;
    return map.knownPois.find((poi) => poi.id === destination)?.systemId ?? destination;
  }, [destination, map]);

  return (
    <div className="go-destination-picker">
      <div className="go-destination-search">
        <input
          type="search"
          list="go-destination-options"
          value={destination}
          placeholder="Search a system or POI…"
          onChange={(event) => onChange(event.target.value)}
        />
        <datalist id="go-destination-options">
          {options.map((option) => (
            <option key={`${option.id}:${option.label}`} value={option.id}>
              {option.label}
            </option>
          ))}
        </datalist>
        <span>{options.length ? `${options.length} known destinations` : "Galaxy data unavailable"}</span>
      </div>
      <div className="go-mini-map" aria-label="Select a destination system">
        <EmbeddedGalaxyMap sessions={sessions} selectedSystemId={selectedSystem} onSelectSystem={onChange} />
      </div>
    </div>
  );
}

function MineRoutePicker({
  map,
  sessions,
  resourceId,
  miningPoiIds,
  resourceSystemIds,
  miningPoi,
  destinationPoi,
  onChange,
}: {
  map: GoMap | null;
  sessions: SessionState[];
  resourceId: string;
  miningPoiIds: string[];
  resourceSystemIds: string[];
  miningPoi: string;
  destinationPoi: string;
  onChange: (miningPoi: string, destinationPoi: string) => void;
}) {
  const miningPoiSet = useMemo(() => new Set(miningPoiIds), [miningPoiIds]);
  const stationPoiIds = useMemo(() => (map?.knownPois ?? []).filter(isStationPoi).map((poi) => poi.id), [map]);
  const stationPoiSet = useMemo(() => new Set(stationPoiIds), [stationPoiIds]);
  const miningSystemIds = useMemo(
    () => new Set((map?.knownPois ?? []).filter((poi) => miningPoiSet.has(poi.id)).map((poi) => poi.systemId)),
    [map, miningPoiSet],
  );
  const dimmedSystemIds = useMemo(
    () => (map?.systems ?? []).filter((system) => !miningSystemIds.has(system.id)).map((system) => system.id),
    [map, miningSystemIds],
  );
  const stationSystemIds = useMemo(
    () => new Set((map?.knownPois ?? []).filter(isStationPoi).map((poi) => poi.systemId)),
    [map],
  );
  const dimmedDropOffSystemIds = useMemo(
    () => (map?.systems ?? []).filter((system) => !stationSystemIds.has(system.id)).map((system) => system.id),
    [map, stationSystemIds],
  );
  const miningPoiIdsKey = [...miningPoiIds].sort().join("|");
  const resourceSystemIdsKey = [...resourceSystemIds].sort().join("|");
  const dimmedSystemIdsKey = [...dimmedSystemIds].sort().join("|");
  useEffect(() => {
    if (!resourceId || !map) return;
    const mapSystemIds = new Set(map.systems.map((system) => system.id));
    const poiSystemById = new Map(map.knownPois.map((poi) => [poi.id, poi.systemId]));
    const missingPoiIds = miningPoiIds.filter((poiId) => !poiSystemById.has(poiId));
    const resolvedPoiSystems = [...new Set(miningPoiIds.map((poiId) => poiSystemById.get(poiId)).filter((systemId): systemId is string => Boolean(systemId)))];
    const indexedSystemsMissingFromMap = resourceSystemIds.filter((systemId) => !mapSystemIds.has(systemId));
    const resolvedSystemsMissingFromMap = resolvedPoiSystems.filter((systemId) => !mapSystemIds.has(systemId));
    const indexedButUnresolvedSystems = resourceSystemIds.filter((systemId) => !miningSystemIds.has(systemId));
    const diagnostic = {
      resourceId,
      resourcePoiIds: miningPoiIds,
      resourceSystemIds,
      resolvedPoiSystems,
      missingPoiIds,
      indexedSystemsMissingFromMap,
      resolvedSystemsMissingFromMap,
      indexedButUnresolvedSystems,
      visibleSystemIds: map.systems.filter((system) => !dimmedSystemIds.includes(system.id)).map((system) => system.id),
      mapSystemCount: map.systems.length,
      knownPoiCount: map.knownPois.length,
    };
    const inconsistent =
      missingPoiIds.length > 0 || indexedSystemsMissingFromMap.length > 0 || resolvedSystemsMissingFromMap.length > 0 || indexedButUnresolvedSystems.length > 0;
    const message = JSON.stringify(diagnostic);
    if (inconsistent) console.warn("[MineRoutePicker] resource/map mismatch", message);
    else console.info("[MineRoutePicker] resource/map diagnostic", message);
  }, [dimmedSystemIdsKey, map, miningPoiIdsKey, miningSystemIds, resourceId, resourceSystemIdsKey]);
  const miningLabel = poiLabel(map, miningPoi);
  const destinationLabel = poiLabel(map, destinationPoi);
  const selectingDropOff = Boolean(miningPoi);

  return (
    <div className="mine-route-picker">
      <div className="mine-route-steps">
        <button type="button" data-active={!selectingDropOff} onClick={() => onChange("", "")}>
          <span>1 · Mining location</span>
          <strong>{miningLabel || (resourceId ? "Choose a highlighted POI" : "Select a resource first")}</strong>
        </button>
        <button type="button" data-active={selectingDropOff && !destinationPoi} disabled={!miningPoi} onClick={() => onChange(miningPoi, "")}>
          <span>2 · Drop-off</span>
          <strong>{destinationLabel || (miningPoi ? "Choose a station" : "Choose mining location first")}</strong>
        </button>
      </div>
      <div className="go-mini-map mine-route-map" aria-label={selectingDropOff ? "Select a drop-off POI" : "Select a mining POI"}>
        <EmbeddedGalaxyMap
          sessions={sessions}
          dimmedSystemIds={selectingDropOff ? dimmedDropOffSystemIds : dimmedSystemIds}
          selectablePoiIds={selectingDropOff ? stationPoiIds : miningPoiIds}
          onSelectSystem={(poiId) => {
            if (!selectingDropOff) {
              if (miningPoiSet.has(poiId)) onChange(poiId, "");
              return;
            }
            if (stationPoiSet.has(poiId)) onChange(miningPoi, poiId);
          }}
        />
      </div>
    </div>
  );
}

function poiLabel(map: GoMap | null, poiId: string): string {
  if (!poiId) return "";
  const poi = map?.knownPois.find((candidate) => candidate.id === poiId);
  return poi ? `${poi.name || humanize(poi.id)} — ${humanize(poi.systemId)}` : humanize(poiId);
}

function humanize(value: string) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}
function RunView({ run, onEdit, onStop }: { run: JobRun; onEdit: () => void; onStop: (mode: "after_current" | "halt_now") => Promise<void> }) {
  const PluginRunView = clientPlugins.flatMap((plugin) => plugin.runViews ?? []).find((view) => view.kind === run.kind)?.component;
  return (
    <div className="job-run-view">
      <div className="jobs-toolbar job-run-header">
        <div>
          <span className="squad-hero-eyebrow">{run.squadName ?? "Legacy squad"}</span>
          <h2>{run.kind.replaceAll("_", " ")}</h2>
          <p>
            <span className="job-status-pill" data-status={run.status}>
              {run.status}
            </span>{" "}
            {run.phase} · revision {run.revision}
          </p>
        </div>
        {active.has(run.status) ? (
          <div>
            <button className="session-btn" onClick={() => void onStop("after_current")}>
              Stop after current
            </button>
            <button className="session-btn" onClick={() => void onStop("halt_now")}>
              Halt now
            </button>
          </div>
        ) : (
          <button className="session-btn" onClick={onEdit}>
            Run again / edit
          </button>
        )}
      </div>
      {run.lastError && <div className="error-banner">{run.lastError.message}</div>}
      {PluginRunView ? <PluginRunView run={run} /> : null}
      <table className="job-bot-table">
        <thead>
          <tr>
            <th>Bot</th>
            <th>Status</th>
            <th>Current work</th>
            <th>Prayer run</th>
          </tr>
        </thead>
        <tbody>
          {Object.values(run.botStates).map((bot) => (
            <tr key={bot.botId}>
              <td>{bot.name ?? bot.botId}</td>
              <td>{bot.status}</td>
              <td title={bot.lastError}>
                {bot.currentWork ?? "—"}
                {bot.lastError && <div className="job-bot-error">{bot.lastError}</div>}
              </td>
              <td>{bot.prayerRunId ?? "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="job-events">
        {[...run.events].reverse().map((event, index) => (
          <div key={`${event.at}-${index}`} data-level={event.level}>
            <time>{new Date(event.at).toLocaleTimeString()}</time> {event.message}
          </div>
        ))}
      </div>
    </div>
  );
}

const arrayValue = (value: unknown): string[] =>
  Array.isArray(value)
    ? value.map(String)
    : typeof value === "string"
      ? value
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean)
      : [];

export function buildJobConfig(definition: JobDefinition | undefined, draft: Record<string, unknown>, botIds: string[]): JobConfig | null {
  if (!definition) return null;
  const merged: Record<string, unknown> = { ...definition.defaults, ...draft };
  for (const field of definition.fields) {
    // Bot selection belongs to the squad picker, not the persisted form draft.
    if (field.name === "botIds") continue;
    const value = merged[field.name];
    if (value === undefined || value === "") {
      if (field.required) return null;
      continue;
    }
    if (field.type === "number" && typeof value !== "number") return null;
    if (field.type === "boolean" && typeof value !== "boolean") return null;
    if ((field.type === "text" || field.type === "textarea") && typeof value !== "string") return null;
  }
  return {
    ...merged,
    kind: definition.kind,
    botIds: [...botIds],
    ...(draft["itemIds"] !== undefined ? { itemIds: arrayValue(draft["itemIds"]) } : {}),
  };
}
const upsert = (runs: JobRun[], run: JobRun) => {
  const existing = runs.find((item) => item.id === run.id);
  if (existing && existing.revision >= run.revision) return runs;
  return [run, ...runs.filter((item) => item.id !== run.id)].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
};
const mergeRuns = (current: JobRun[], incoming: JobRun[]) => incoming.reduce(upsert, current);
const readSavedConfig = (kind: string): Record<string, unknown> | null => {
  return readVersionedStoredRecord(`prayer-job-config:${kind}`);
};
const resourceLabel = (resourceId: string, item: CatalogDumpItemsItem | undefined): string => {
  if (item?.name.trim() && item.name !== resourceId) return `${item.name} (${resourceId})`;
  return resourceId;
};
