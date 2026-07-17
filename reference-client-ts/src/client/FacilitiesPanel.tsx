import { useCallback, useEffect, useMemo, useState } from "react";
import { FacilitiesData, FacilityInfo, FacilityTypeInfo } from "./api.js";
import { actions } from "@prayer/sdk";
import type { Action, ActionRunOutcome } from "@prayer/sdk/types";
import { SessionState } from "./SessionsPanel.js";
import SearchableSessionSelect from "./SearchableSessionSelect.js";
import { CreditAmount } from "./Credits.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectFacilities } from "./prayer/worldSelectors.js";

type FacilitiesPanelProps = {
  sessions: SessionState[];
};

type Scope = "personal" | "faction";
type FacilitiesTab = "owned" | "here" | "build";
type FacilityRowAction = "set_access" | "set_output_price" | "set_name";

const ACTING_SESSION_KEY = "prayer-facilities-acting-session";

function formatLabel(value: string): string {
  return value.replace(/[_-]+/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function formatNumber(value: number): string {
  return value.toLocaleString();
}

function formatLocation(facility: FacilityInfo): string {
  if (facility.locationName && facility.locationId && facility.locationName !== facility.locationId) {
    return `${facility.locationName} (${facility.locationId})`;
  }
  return facility.locationName || facility.locationId || facility.systemId || "";
}

function facilityName(facility: FacilityInfo): string {
  return facility.name || formatLabel(facility.facilityType || facility.facilityId);
}

function facilityTypeName(type: FacilityTypeInfo): string {
  return type.name || formatLabel(type.facilityType);
}

function facilityDetails(facility: FacilityInfo): Array<[string, string]> {
  const details: Array<[string, string]> = [];
  const location = formatLocation(facility);
  if (location) details.push(["where", location]);
  if (facility.systemId && facility.systemId !== facility.locationId) details.push(["system", facility.systemId]);
  if (facility.level != null) details.push(["level", formatNumber(facility.level)]);
  if (facility.status) details.push(["status", facility.status]);
  if (facility.public != null) details.push(["access", facility.public ? "public" : "private"]);
  if (facility.rentPerCycle != null) details.push(["rent", `${formatNumber(facility.rentPerCycle)} / cycle`]);
  if (facility.buildTime != null) details.push(["build", `${formatNumber(facility.buildTime)} ticks`]);
  return details;
}

function summaryStats(data: FacilitiesData | null): Array<[string, string]> {
  if (!data) return [];
  const stats: Array<[string, string]> = [];
  if (data.factionId) stats.push(["faction", data.factionId]);
  if (data.factionRentPerCycle != null) stats.push(["faction rent", `${formatNumber(data.factionRentPerCycle)} / cycle`]);
  if (data.factionArrearsOwed != null) stats.push(["arrears owed", formatNumber(data.factionArrearsOwed)]);
  return stats;
}

function typeFacts(type: FacilityTypeInfo): string[] {
  const facts = [type.category].filter(Boolean);
  if (type.level != null) facts.push(`L${formatNumber(type.level)}`);
  return facts;
}

function materialEntries(type: FacilityTypeInfo): Array<[string, number]> {
  return Object.entries(type.requiredItems)
    .filter(([, quantity]) => Number.isFinite(quantity) && quantity > 0)
    .sort(([leftId, leftQuantity], [rightId, rightQuantity]) => rightQuantity - leftQuantity || formatLabel(leftId).localeCompare(formatLabel(rightId)));
}

function materialSummary(type: FacilityTypeInfo): string {
  const materials = materialEntries(type);
  if (materials.length === 0) return "No item materials listed";
  return materials.map(([itemId, quantity]) => `${formatNumber(quantity)} ${formatLabel(itemId)}`).join(", ");
}

function dedupeFacilities(rows: FacilityInfo[]): FacilityInfo[] {
  const byKey = new Map<string, FacilityInfo>();
  for (const row of rows) {
    const key = row.facilityId || `${row.ownerKind}:${row.locationId}:${row.facilityType}:${row.name}`;
    if (!byKey.has(key)) byKey.set(key, row);
  }
  return [...byKey.values()];
}

function resultMessage(result: ActionRunOutcome | undefined): string {
  if (!result || result.status === "succeeded") return "Facility action completed.";
  return "message" in result ? result.message : result.reason;
}

function matchesQuery(facility: FacilityInfo, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [
    facility.facilityId,
    facility.facilityType,
    facility.name,
    facility.category,
    facility.status,
    facility.ownerName,
    facility.locationId,
    facility.locationName,
    facility.systemId,
  ].some((value) => value.toLowerCase().includes(needle));
}

function matchesTypeQuery(type: FacilityTypeInfo, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [type.facilityType, type.name, type.category, materialSummary(type)].some((value) => value.toLowerCase().includes(needle));
}

export default function FacilitiesPanel({ sessions }: FacilitiesPanelProps) {
  const prayer = usePrayer();
  const [actingHandle, setActingHandle] = useState<string | null>(() => {
    try {
      return window.localStorage.getItem(ACTING_SESSION_KEY);
    } catch {
      return null;
    }
  });
  const [query, setQuery] = useState("");
  const [typeQuery, setTypeQuery] = useState("");
  const [buildScope, setBuildScope] = useState<Scope>("personal");
  const [buildType, setBuildType] = useState("");
  const [activeTab, setActiveTab] = useState<FacilitiesTab>("here");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [busyFacilityId, setBusyFacilityId] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [loadedAt, setLoadedAt] = useState<Date | null>(null);
  const [facilityNames, setFacilityNames] = useState<Record<string, string>>({});
  const [outputItems, setOutputItems] = useState<Record<string, string>>({});
  const [outputPrices, setOutputPrices] = useState<Record<string, string>>({});
  const selectedBot = actingHandle ? (prayer.fleet.find((bot) => bot.id === actingHandle || bot.username === actingHandle) ?? null) : null;
  const data = useMemo(
    () =>
      selectFacilities(selectedBot, prayer.galaxyMap, prayer.catalog, prayer.facilitiesByPoi, prayer.ownedFacilitiesByPlayer, prayer.ownedFacilitiesByFaction),
    [prayer.catalog, prayer.facilitiesByPoi, prayer.galaxyMap, prayer.ownedFacilitiesByFaction, prayer.ownedFacilitiesByPlayer, selectedBot],
  );

  useEffect(() => {
    if (actingHandle && sessions.some((s) => s.sessionHandle === actingHandle)) return;
    setActingHandle(sessions[0]?.sessionHandle ?? null);
  }, [sessions, actingHandle]);

  useEffect(() => {
    if (!actingHandle) return;
    try {
      window.localStorage.setItem(ACTING_SESSION_KEY, actingHandle);
    } catch {
      // best effort
    }
  }, [actingHandle]);

  const load = useCallback(
    async (opts: { quiet?: boolean } = {}) => {
      if (!actingHandle) {
        return;
      }
      if (!opts.quiet) setLoading(true);
      try {
        await prayer.refresh();
        setLoadedAt(new Date());
      } catch (err) {
        setStatus(err instanceof Error ? err.message : String(err));
      } finally {
        if (!opts.quiet) setLoading(false);
      }
    },
    [actingHandle, prayer],
  );

  const allOwned = useMemo(
    () =>
      dedupeFacilities([...(data?.owned ?? []), ...(data?.factionOwned ?? [])])
        .filter((facility) => matchesQuery(facility, query))
        .sort(
          (a, b) =>
            formatLocation(a).localeCompare(formatLocation(b)) ||
            a.ownerKind.localeCompare(b.ownerKind) ||
            facilityName(a).localeCompare(facilityName(b)) ||
            a.facilityId.localeCompare(b.facilityId),
        ),
    [data, query],
  );

  const current = useMemo(
    () =>
      dedupeFacilities([...(data?.current ?? []), ...(data?.factionCurrent ?? [])])
        .filter((facility) => matchesQuery(facility, query))
        .sort((a, b) => a.ownerKind.localeCompare(b.ownerKind) || facilityName(a).localeCompare(facilityName(b)) || a.facilityId.localeCompare(b.facilityId)),
    [data, query],
  );

  const facilityTypes = useMemo(
    () =>
      (data?.types ?? [])
        .filter((type) => {
          const category = type.category.trim().toLowerCase();
          return buildScope === "faction" ? category === "faction" : category !== "faction";
        })
        .filter((type) => !type.upgradesFrom.trim())
        .filter((type) => matchesTypeQuery(type, typeQuery))
        .sort(
          (a, b) =>
            (a.category || "").localeCompare(b.category || "") || (a.level ?? 999) - (b.level ?? 999) || facilityTypeName(a).localeCompare(facilityTypeName(b)),
        ),
    [data, typeQuery, buildScope],
  );
  const stats = useMemo(() => summaryStats(data), [data]);

  useEffect(() => {
    if (buildType && facilityTypes.some((type) => type.facilityType === buildType)) return;
    setBuildType(facilityTypes[0]?.facilityType ?? "");
  }, [facilityTypes, buildType]);

  async function handleBuild() {
    if (!actingHandle) return;
    const facilityType = buildType.trim();
    if (!facilityType) {
      setStatus("Choose a facility type.");
      return;
    }
    const action = buildScope === "faction" ? "faction_build" : "build";
    setBusy(true);
    setStatus(`Building ${formatLabel(facilityType)}…`);
    try {
      const request =
        action === "faction_build" ? actions.factionFacilityBuild({ facility_type: facilityType }) : actions.facilityBuild({ facility_type: facilityType });
      await executeFacilityAction(request, `Building ${formatLabel(facilityType)}`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleFacilityAction(facility: FacilityInfo, action: FacilityRowAction, payload: Record<string, unknown>) {
    if (!actingHandle || !facility.facilityId) return;
    setBusy(true);
    setBusyFacilityId(facility.facilityId);
    setStatus(`Updating ${facilityName(facility)}…`);
    try {
      const request =
        action === "set_access"
          ? actions.facilitySetAccess({ facility_id: facility.facilityId, access: String(payload.access ?? "") })
          : action === "set_name"
            ? actions.facilitySetName({ facility_id: facility.facilityId, custom_name: String(payload.custom_name ?? "") })
            : actions.facilitySetOutputPrice({ facility_id: facility.facilityId, item: String(payload.item_id ?? ""), price: Number(payload.price) });
      await executeFacilityAction(request, `Updating ${facilityName(facility)}`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
      setBusyFacilityId(null);
    }
  }

  async function executeFacilityAction(request: Action, label: string) {
    if (!actingHandle) return;
    const bot = await prayer.bot(actingHandle);
    const run = await bot.start([request], { idempotencyKey: crypto.randomUUID() });
    setStatus(`${label}… (${run.id})`);
    const terminal = await run.wait();
    if (terminal.status !== "succeeded") throw new Error(run.errorMessage || `Facility action ${terminal.status}.`);
    await load({ quiet: true });
    setStatus(resultMessage(terminal.outcome));
  }

  if (sessions.length === 0) {
    return (
      <div className="facilities-panel">
        <div className="facilities-empty">No sessions registered.</div>
      </div>
    );
  }

  return (
    <div className="facilities-panel">
      <div className="facilities-toolbar">
        <div>
          <div className="facilities-title">Facilities</div>
          <div className="facilities-meta">
            {data?.latestPoi || data?.latestSystem || "No location snapshot"}
            {data?.docked === false ? " · undocked" : ""}
            {loadedAt ? ` · updated ${loadedAt.toLocaleTimeString()}` : ""}
          </div>
        </div>
        <label>
          <span>Acting as</span>
          <SearchableSessionSelect
            sessions={sessions}
            value={actingHandle}
            onChange={setActingHandle}
            disabled={busy}
            ariaLabel="Acting session"
          />
        </label>
        <button className="session-btn" onClick={() => void load()} disabled={loading || busy}>
          refresh
        </button>
      </div>

      {status && <div className="facilities-status">{status}</div>}
      {stats.length > 0 && (
        <div className="facilities-summary">
          {stats.map(([label, value]) => (
            <span key={label}>
              <strong>{label}</strong>
              {value}
            </span>
          ))}
        </div>
      )}

      <div className="facilities-controls">
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="facility, owner, station" />
      </div>

      <div className="facilities-tabs" role="tablist" aria-label="Facilities sections">
        <button role="tab" data-active={activeTab === "here"} onClick={() => setActiveTab("here")}>
          Here <span>{current.length}</span>
        </button>
        <button role="tab" data-active={activeTab === "owned"} onClick={() => setActiveTab("owned")}>
          Owned <span>{allOwned.length}</span>
        </button>
        <button role="tab" data-active={activeTab === "build"} onClick={() => setActiveTab("build")}>
          Build <span>{facilityTypes.length}</span>
        </button>
      </div>

      <div className="facilities-body">
        {activeTab === "owned" ? (
          <section className="facilities-section">
            <div className="facilities-section-head">
              <div>
                <div className="facilities-section-title">Owned Facilities</div>
                <div className="facilities-section-meta">{allOwned.length} visible</div>
              </div>
            </div>
            <FacilityTable
              rows={allOwned}
              empty={loading ? "Loading facilities…" : "No owned facilities found."}
              editable
              busy={busy}
              busyFacilityId={busyFacilityId}
              facilityNames={facilityNames}
              outputItems={outputItems}
              outputPrices={outputPrices}
              onFacilityNameChange={(facilityId, value) => setFacilityNames((prev) => ({ ...prev, [facilityId]: value }))}
              onOutputItemChange={(facilityId, value) => setOutputItems((prev) => ({ ...prev, [facilityId]: value }))}
              onOutputPriceChange={(facilityId, value) => setOutputPrices((prev) => ({ ...prev, [facilityId]: value }))}
              onAction={handleFacilityAction}
            />
          </section>
        ) : activeTab === "here" ? (
          <section className="facilities-section">
            <div className="facilities-section-head">
              <div>
                <div className="facilities-section-title">Here</div>
                <div className="facilities-section-meta">{current.length} visible at current station</div>
              </div>
            </div>
            <FacilityTable rows={current} empty={loading ? "Loading current station…" : "No current-station facilities found."} />
          </section>
        ) : (
          <section className="facilities-build">
            <div className="facilities-section-head">
              <div>
                <div className="facilities-section-title">Build</div>
                <div className="facilities-section-meta">{facilityTypes.length} facility types</div>
              </div>
            </div>
            <div className="facilities-build-controls">
              <div className="facilities-segment" role="group" aria-label="Build ownership">
                <button data-active={buildScope === "personal"} onClick={() => setBuildScope("personal")} disabled={busy}>
                  personal
                </button>
                <button data-active={buildScope === "faction"} onClick={() => setBuildScope("faction")} disabled={busy}>
                  faction
                </button>
              </div>
              <select value={buildType} onChange={(e) => setBuildType(e.target.value)} disabled={busy}>
                {facilityTypes.map((type) => (
                  <option key={type.facilityType} value={type.facilityType}>
                    {facilityTypeName(type)} ({type.facilityType})
                  </option>
                ))}
              </select>
              <button className="session-btn" onClick={() => void handleBuild()} disabled={busy || loading}>
                build
              </button>
            </div>
            <input className="facilities-type-filter" value={typeQuery} onChange={(e) => setTypeQuery(e.target.value)} placeholder="filter facility types" />
            <div className="facilities-type-list">
              {facilityTypes.slice(0, 80).map((type) => {
                const materials = materialEntries(type);
                return (
                  <button
                    key={type.facilityType}
                    className="facilities-type-row"
                    data-active={buildType === type.facilityType}
                    onClick={() => setBuildType(type.facilityType)}
                    disabled={busy}
                  >
                    <span className="facilities-type-main">
                      <strong>{facilityTypeName(type)}</strong>
                      <small>{type.facilityType}</small>
                    </span>
                    <span className="facilities-type-facts">
                      {typeFacts(type).join(" · ")}
                      {type.price != null && (
                        <>
                          {typeFacts(type).length ? " · " : ""}
                          <CreditAmount value={type.price} />
                        </>
                      )}
                    </span>
                    <span className="facilities-type-materials">
                      {materials.length > 0 ? (
                        materials.map(([itemId, quantity]) => (
                          <span key={itemId}>
                            <strong>{formatNumber(quantity)}</strong>
                            {formatLabel(itemId)}
                          </span>
                        ))
                      ) : (
                        <span>No item materials listed</span>
                      )}
                    </span>
                  </button>
                );
              })}
              {!loading && facilityTypes.length === 0 && <div className="facilities-empty">No facility types found.</div>}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function FacilityTable({
  rows,
  empty,
  editable = false,
  busy = false,
  busyFacilityId = null,
  facilityNames = {},
  outputItems = {},
  outputPrices = {},
  onFacilityNameChange,
  onOutputItemChange,
  onOutputPriceChange,
  onAction,
}: {
  rows: FacilityInfo[];
  empty: string;
  editable?: boolean;
  busy?: boolean;
  busyFacilityId?: string | null;
  facilityNames?: Record<string, string>;
  outputItems?: Record<string, string>;
  outputPrices?: Record<string, string>;
  onFacilityNameChange?: (facilityId: string, value: string) => void;
  onOutputItemChange?: (facilityId: string, value: string) => void;
  onOutputPriceChange?: (facilityId: string, value: string) => void;
  onAction?: (facility: FacilityInfo, action: FacilityRowAction, payload: Record<string, unknown>) => void;
}) {
  return (
    <div className="facilities-table-wrap">
      <table className="facilities-table">
        <thead>
          <tr>
            <th>facility</th>
            <th>owner</th>
            <th>details</th>
            {editable && <th>config</th>}
          </tr>
        </thead>
        <tbody>
          {rows.map((facility, index) => {
            const details = facilityDetails(facility);
            const facilityId = facility.facilityId;
            const rowBusy = busy && (!busyFacilityId || busyFacilityId === facilityId);
            const nameValue = facilityNames[facilityId] ?? facility.name ?? "";
            const itemValue = outputItems[facilityId] ?? "";
            const priceValue = outputPrices[facilityId] ?? "";
            const priceNumber = Number.parseInt(priceValue.replace(/,/g, ""), 10);
            const canSetPrice = itemValue.trim().length > 0 && Number.isInteger(priceNumber) && priceNumber >= 0;
            return (
              <tr key={facility.facilityId || `${facility.facilityType}:${index}`} data-owner={facility.ownerKind}>
                <td>
                  <div className="facilities-name">{facilityName(facility)}</div>
                  {(facility.facilityType || facility.facilityId) && <div className="facilities-id">{facility.facilityType || facility.facilityId}</div>}
                </td>
                <td>
                  <div className="facilities-owner">{facility.ownerKind}</div>
                  {facility.ownerName && <div className="facilities-id">{facility.ownerName}</div>}
                </td>
                <td>
                  {details.length > 0 && (
                    <div className="facilities-detail-list">
                      {details.map(([label, value]) => (
                        <span key={label}>
                          <strong>{label}</strong>
                          {value}
                        </span>
                      ))}
                    </div>
                  )}
                </td>
                {editable && (
                  <td>
                    <div className="facilities-config">
                      <div className="facilities-config-row">
                        <button
                          className="session-btn"
                          onClick={() => onAction?.(facility, "set_access", { access: facility.public ? "private" : "public" })}
                          disabled={rowBusy || !facilityId}
                        >
                          {facility.public ? "make private" : "make public"}
                        </button>
                      </div>
                      <div className="facilities-config-row">
                        <input
                          value={nameValue}
                          onChange={(event) => onFacilityNameChange?.(facilityId, event.target.value)}
                          disabled={rowBusy || !facilityId}
                          placeholder="custom name"
                        />
                        <button
                          className="session-btn"
                          onClick={() => onAction?.(facility, "set_name", { custom_name: nameValue })}
                          disabled={rowBusy || !facilityId}
                        >
                          name
                        </button>
                      </div>
                      <div className="facilities-config-row">
                        <input
                          value={itemValue}
                          onChange={(event) => onOutputItemChange?.(facilityId, event.target.value)}
                          disabled={rowBusy || !facilityId}
                          placeholder="output item"
                        />
                        <input
                          value={priceValue}
                          onChange={(event) => onOutputPriceChange?.(facilityId, event.target.value)}
                          disabled={rowBusy || !facilityId}
                          placeholder="cr"
                        />
                        <button
                          className="session-btn"
                          onClick={() => onAction?.(facility, "set_output_price", { item_id: itemValue.trim(), price: priceNumber })}
                          disabled={rowBusy || !facilityId || !canSetPrice}
                        >
                          price
                        </button>
                      </div>
                    </div>
                  </td>
                )}
              </tr>
            );
          })}
          {rows.length === 0 && (
            <tr>
              <td className="facilities-empty" colSpan={editable ? 4 : 3}>
                {empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
