import { useMemo, useState } from "react";
import { CommanderStorageRow } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { CreditPair } from "./Credits.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectCommanderStorage } from "./prayer/worldSelectors.js";

type StoragePanelProps = {
  sessions: SessionState[];
};

type SourceKind = CommanderStorageRow["sourceKind"];

type StorageItemGroup = {
  itemId: string;
  totalQuantity: number;
  unitMarketPrice: number | null;
  totalMarketValue: number | null;
  unitMedianBuyPrice: number | null;
  unitMedianSellPrice: number | null;
  totalMedianBuyValue: number | null;
  totalMedianSellValue: number | null;
  marketPriceSource: string | null;
  rows: CommanderStorageRow[];
};

function formatQty(value: number): string {
  return value.toLocaleString();
}

function priceSourceLabel(source: string | null): string {
  switch (source) {
    case "globalMedianBuySell":
      return "median buy/sell";
    case "globalWeightedMid":
      return "market mid";
    case "globalMedianMid":
      return "median mid";
    case "globalMedianBuy":
      return "median bid";
    case "globalMedianSell":
      return "median ask";
    case "credits":
      return "credits";
    default:
      return "no market price";
  }
}

function displayLocation(row: CommanderStorageRow): string {
  if (row.locationName && row.locationName !== row.locationId) {
    return `${row.locationName} (${row.locationId})`;
  }
  return row.locationName || row.locationId;
}

function rowSourceMatches(row: CommanderStorageRow, sourceFilter: "all" | SourceKind): boolean {
  if (sourceFilter === "all") return true;
  if (row.sourceKind === sourceFilter) return true;
  return sourceFilter === "faction" && row.details?.["kind"] === "faction_treasury";
}

function rowMatches(row: CommanderStorageRow, query: string, sourceFilter: "all" | SourceKind): boolean {
  if (!rowSourceMatches(row, sourceFilter)) return false;
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return (
    row.itemId.toLowerCase().includes(needle) ||
    row.ownerName.toLowerCase().includes(needle) ||
    row.locationId.toLowerCase().includes(needle) ||
    (row.locationName ?? "").toLowerCase().includes(needle) ||
    (row.systemId ?? "").toLowerCase().includes(needle) ||
    row.observedBy.some((handle) => handle.toLowerCase().includes(needle))
  );
}

function compareStorageRows(a: CommanderStorageRow, b: CommanderStorageRow): number {
  return (
    a.itemId.localeCompare(b.itemId) ||
    b.quantity - a.quantity ||
    a.sourceKind.localeCompare(b.sourceKind) ||
    a.ownerName.localeCompare(b.ownerName) ||
    displayLocation(a).localeCompare(displayLocation(b))
  );
}

function compareStorageGroups(a: StorageItemGroup, b: StorageItemGroup): number {
  return b.totalQuantity - a.totalQuantity || a.itemId.localeCompare(b.itemId);
}

function groupStorageRows(rows: CommanderStorageRow[]): StorageItemGroup[] {
  const byItemId = new Map<string, StorageItemGroup>();
  for (const row of rows) {
    const group = byItemId.get(row.itemId);
    if (group) {
      group.totalQuantity += row.quantity;
      if (row.totalMarketValue !== null) {
        group.totalMarketValue = (group.totalMarketValue ?? 0) + row.totalMarketValue;
      }
      if (row.totalMedianBuyValue !== null) {
        group.totalMedianBuyValue = (group.totalMedianBuyValue ?? 0) + row.totalMedianBuyValue;
      }
      if (row.totalMedianSellValue !== null) {
        group.totalMedianSellValue = (group.totalMedianSellValue ?? 0) + row.totalMedianSellValue;
      }
      if (!group.marketPriceSource && row.marketPriceSource) {
        group.marketPriceSource = row.marketPriceSource;
      }
      group.rows.push(row);
    } else {
      byItemId.set(row.itemId, {
        itemId: row.itemId,
        totalQuantity: row.quantity,
        unitMarketPrice: row.unitMarketPrice,
        totalMarketValue: row.totalMarketValue,
        unitMedianBuyPrice: row.unitMedianBuyPrice,
        unitMedianSellPrice: row.unitMedianSellPrice,
        totalMedianBuyValue: row.totalMedianBuyValue,
        totalMedianSellValue: row.totalMedianSellValue,
        marketPriceSource: row.marketPriceSource,
        rows: [row],
      });
    }
  }

  return Array.from(byItemId.values())
    .map((group) => ({
      ...group,
      unitMarketPrice: group.totalMarketValue !== null && group.totalQuantity > 0 ? group.totalMarketValue / group.totalQuantity : group.unitMarketPrice,
      unitMedianBuyPrice:
        group.totalMedianBuyValue !== null && group.totalQuantity > 0 ? group.totalMedianBuyValue / group.totalQuantity : group.unitMedianBuyPrice,
      unitMedianSellPrice:
        group.totalMedianSellValue !== null && group.totalQuantity > 0 ? group.totalMedianSellValue / group.totalQuantity : group.unitMedianSellPrice,
      rows: [...group.rows].sort(compareStorageRows),
    }))
    .sort(compareStorageGroups);
}

function groupMeta(group: StorageItemGroup): string {
  const sourceKinds = Array.from(new Set(group.rows.map((row) => row.sourceKind))).sort();
  const owners = Array.from(new Set(group.rows.map((row) => row.ownerName))).sort();
  return `${group.rows.length} source${group.rows.length === 1 ? "" : "s"} · ${sourceKinds.join(", ")} · ${owners.join(", ")}`;
}

export default function StoragePanel({ sessions }: StoragePanelProps) {
  const prayer = usePrayer();
  const view = useMemo(
    () => selectCommanderStorage(prayer.fleet, prayer.galaxyMap, prayer.storageByPlayer, prayer.factionStorageByFactionPoi, prayer.factionBySession),
    [prayer.factionBySession, prayer.factionStorageByFactionPoi, prayer.fleet, prayer.galaxyMap, prayer.storageByPlayer],
  );
  const [query, setQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState<"all" | SourceKind>("all");
  const [expandedItemIds, setExpandedItemIds] = useState<Set<string>>(() => new Set());
  const loading = prayer.connection === "connecting";
  const error = prayer.error?.message ?? null;
  const loadedAt = useMemo(() => {
    const observed = prayer.fleet.map((bot) => bot.observed_at).filter((value): value is string => Boolean(value));
    return observed.length ? new Date(observed.sort().at(-1)!) : null;
  }, [prayer.fleet]);

  const rows = useMemo(() => (view?.rows ?? []).filter((row) => rowMatches(row, query, sourceFilter)).sort(compareStorageRows), [query, sourceFilter, view]);
  const financialGroups = useMemo(() => groupStorageRows(rows.filter((row) => row.sourceKind === "financial")), [rows]);
  const inventoryGroups = useMemo(() => groupStorageRows(rows.filter((row) => row.sourceKind !== "financial")), [rows]);
  const observedCount = view?.sessionsObserved ?? 0;
  const totalCount = view?.sessionsTotal ?? sessions.length;

  function toggleItem(itemId: string) {
    setExpandedItemIds((current) => {
      const next = new Set(current);
      if (next.has(itemId)) {
        next.delete(itemId);
      } else {
        next.add(itemId);
      }
      return next;
    });
  }

  return (
    <div className="storage-panel">
      <div className="storage-toolbar">
        <div>
          <div className="storage-title">Storage</div>
          <div className="storage-meta">
            {observedCount}/{totalCount} sessions observed
            {loadedAt ? ` · updated ${loadedAt.toLocaleTimeString()}` : ""}
          </div>
        </div>
        <div className="storage-controls">
          <div className="storage-segment" role="group" aria-label="Storage source">
            <button data-active={sourceFilter === "all"} onClick={() => setSourceFilter("all")}>
              all
            </button>
            <button data-active={sourceFilter === "cargo"} onClick={() => setSourceFilter("cargo")}>
              cargo
            </button>
            <button data-active={sourceFilter === "personal"} onClick={() => setSourceFilter("personal")}>
              personal
            </button>
            <button data-active={sourceFilter === "faction"} onClick={() => setSourceFilter("faction")}>
              faction
            </button>
          </div>
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="item, owner, station" />
          <button className="session-btn" onClick={() => void prayer.refresh()} disabled={loading}>
            refresh
          </button>
        </div>
      </div>

      {error && <div className="storage-status">Storage view unavailable: {error}</div>}

      <div className="storage-body">
        {financialGroups.length > 0 && (
          <section className="storage-items storage-financial-items">
            <div className="storage-section-heading">Credits & tax</div>
            {financialGroups.map((group) => {
              const expanded = expandedItemIds.has(group.itemId);
              return (
                <div className="storage-item storage-financial-item" key={group.itemId}>
                  <button
                    className="storage-item-header"
                    onClick={() => toggleItem(group.itemId)}
                    aria-expanded={expanded}
                    title={expanded ? `Collapse ${group.itemId}` : `Expand ${group.itemId}`}
                  >
                    <span className="storage-item-caret">{expanded ? "v" : ">"}</span>
                    <span className="storage-item-id" title={group.itemId}>
                      {group.itemId}
                    </span>
                    <span className="storage-item-count">{formatQty(group.totalQuantity)}</span>
                    <span className="storage-item-price" title={priceSourceLabel(group.marketPriceSource)}>
                      <CreditPair buy={group.unitMedianBuyPrice} sell={group.unitMedianSellPrice} /> ea
                    </span>
                    <span className="storage-item-value">
                      <CreditPair buy={group.totalMedianBuyValue} sell={group.totalMedianSellValue} />
                    </span>
                    <span className="storage-item-meta">{groupMeta(group)}</span>
                  </button>
                  {expanded && (
                    <div className="storage-source-list">
                      <div className="storage-source-row storage-source-heading">
                        <span>qty</span>
                        <span>buy/sell ea</span>
                        <span>buy/sell value</span>
                        <span>source</span>
                        <span>owner</span>
                        <span>where</span>
                        <span>seen by</span>
                        <span>assign</span>
                      </div>
                      {group.rows.map((row) => (
                        <div className="storage-source-row" key={row.key} data-source={row.sourceKind}>
                          <span className="storage-source-qty">{formatQty(row.quantity)}</span>
                          <span className="storage-source-price" title={priceSourceLabel(row.marketPriceSource)}>
                            <CreditPair buy={row.unitMedianBuyPrice} sell={row.unitMedianSellPrice} />
                          </span>
                          <span className="storage-source-value">
                            <CreditPair buy={row.totalMedianBuyValue} sell={row.totalMedianSellValue} />
                          </span>
                          <span className="storage-source-kind">{row.sourceKind}</span>
                          <span className="storage-source-owner">{row.ownerName}</span>
                          <span className="storage-source-location">{displayLocation(row)}</span>
                          <span className="storage-source-seen">{row.observedBy.join(", ")}</span>
                          <span className="storage-source-actions">—</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </section>
        )}

        <section className="storage-items">
          {inventoryGroups.map((group) => {
            const expanded = expandedItemIds.has(group.itemId);
            return (
              <div className="storage-item" key={group.itemId}>
                <button
                  className="storage-item-header"
                  data-cargo-only={group.rows.every((row) => row.sourceKind === "cargo")}
                  onClick={() => toggleItem(group.itemId)}
                  aria-expanded={expanded}
                  title={expanded ? `Collapse ${group.itemId}` : `Expand ${group.itemId}`}
                >
                  <span className="storage-item-caret">{expanded ? "v" : ">"}</span>
                  <span className="storage-item-id" title={group.itemId}>
                    {group.itemId}
                  </span>
                  <span className="storage-item-count">{formatQty(group.totalQuantity)}</span>
                  <span className="storage-item-price" title={priceSourceLabel(group.marketPriceSource)}>
                    <CreditPair buy={group.unitMedianBuyPrice} sell={group.unitMedianSellPrice} /> ea
                  </span>
                  <span className="storage-item-value">
                    <CreditPair buy={group.totalMedianBuyValue} sell={group.totalMedianSellValue} />
                  </span>
                  <span className="storage-item-meta">{groupMeta(group)}</span>
                </button>
                {expanded && (
                  <div className="storage-source-list">
                    <div className="storage-source-row storage-source-heading">
                      <span>qty</span>
                      <span>buy/sell ea</span>
                      <span>buy/sell value</span>
                      <span>source</span>
                      <span>owner</span>
                      <span>where</span>
                      <span>seen by</span>
                      <span>assign</span>
                    </div>
                    {group.rows.map((row) => (
                      <div className="storage-source-row" key={row.key} data-source={row.sourceKind}>
                        <span className="storage-source-qty">{formatQty(row.quantity)}</span>
                        <span className="storage-source-price" title={priceSourceLabel(row.marketPriceSource)}>
                          <CreditPair buy={row.unitMedianBuyPrice} sell={row.unitMedianSellPrice} />
                        </span>
                        <span className="storage-source-value">
                          <CreditPair buy={row.totalMedianBuyValue} sell={row.totalMedianSellValue} />
                        </span>
                        <span className="storage-source-kind">{row.sourceKind}</span>
                        <span className="storage-source-owner">{row.ownerName}</span>
                        <span className="storage-source-location">{displayLocation(row)}</span>
                        <span className="storage-source-seen">{row.observedBy.join(", ")}</span>
                        <span className="storage-source-actions">—</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
          {!loading && inventoryGroups.length === 0 && financialGroups.length === 0 && <div className="storage-empty">No matching cargo or storage lots.</div>}
          {!loading && inventoryGroups.length === 0 && financialGroups.length > 0 && <div className="storage-empty">No matching cargo or storage lots.</div>}
        </section>
      </div>
    </div>
  );
}
