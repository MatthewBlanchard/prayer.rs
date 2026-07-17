import { useEffect, useMemo, useState } from "react";
import { CatalogEntry, CatalogIngredient, CatalogState, RecipeCatalogEntry, ShipCatalogEntry } from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { CreditAmount } from "./Credits.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectCatalog } from "./prayer/worldSelectors.js";

type CatalogPanelProps = {
  sessions: SessionState[];
};

type CatalogKind = "items" | "ships" | "recipes" | "facilities";

type ShipFilters = {
  tier: string;
  minCargo: string;
  maxCargo: string;
  minPrice: string;
  maxPrice: string;
  minSpeed: string;
  minHull: string;
  minShield: string;
  classText: string;
};

const emptyShipFilters: ShipFilters = {
  tier: "",
  minCargo: "",
  maxCargo: "",
  minPrice: "",
  maxPrice: "",
  minSpeed: "",
  minHull: "",
  minShield: "",
  classText: "",
};

function formatNumber(value: number | null): string {
  return value === null ? "-" : value.toLocaleString();
}

function ingredientQty(entry: CatalogIngredient): string {
  return formatNumber(entry.quantity ?? entry.amount ?? entry.count);
}

function ingredientList(entries: CatalogIngredient[]): string {
  if (!entries.length) return "-";
  return entries.map((entry) => `${ingredientQty(entry)} ${entry.name || entry.itemId}`).join(", ");
}

function skillList(skills: Record<string, number>): string {
  const entries = Object.entries(skills);
  if (!entries.length) return "-";
  return entries.map(([id, level]) => `${id} ${level}`).join(", ");
}

function itemSearch(entry: CatalogEntry): string {
  return [
    entry.id,
    entry.name,
    entry.category,
    entry.typeName,
    entry.classId,
    entry.className,
    Object.keys(entry.materials).join(" "),
    ingredientList(entry.inputs),
    ingredientList(entry.outputs),
  ]
    .join(" ")
    .toLowerCase();
}

function shipSearch(entry: ShipCatalogEntry): string {
  return [itemSearch(entry), entry.defaultModules.join(" "), JSON.stringify(entry.inherentCapabilities)].join(" ").toLowerCase();
}

function recipeSearch(entry: RecipeCatalogEntry): string {
  return [entry.id, entry.name, ingredientList(entry.inputs), ingredientList(entry.outputs), skillList(entry.requiredSkills)].join(" ").toLowerCase();
}

function materialList(materials: Record<string, number>): string {
  const entries = Object.entries(materials);
  if (!entries.length) return "-";
  return entries.map(([id, qty]) => `${formatNumber(qty)} ${id}`).join(", ");
}

function parseFilterNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function shipCargo(entry: ShipCatalogEntry): number | null {
  return entry.cargoCapacity ?? entry.cargo;
}

function shipSpeed(entry: ShipCatalogEntry): number | null {
  return entry.speed ?? entry.baseSpeed;
}

function shipHull(entry: ShipCatalogEntry): number | null {
  return entry.hull ?? entry.baseHull;
}

function shipShield(entry: ShipCatalogEntry): number | null {
  return entry.shield ?? entry.baseShield;
}

function matchesMin(value: number | null, min: number | null): boolean {
  return min === null || (value !== null && value >= min);
}

function matchesMax(value: number | null, max: number | null): boolean {
  return max === null || (value !== null && value <= max);
}

function matchesShipFilters(entry: ShipCatalogEntry, filters: ShipFilters): boolean {
  const tier = parseFilterNumber(filters.tier);
  if (tier !== null && entry.tier !== tier) return false;
  if (!matchesMin(shipCargo(entry), parseFilterNumber(filters.minCargo))) return false;
  if (!matchesMax(shipCargo(entry), parseFilterNumber(filters.maxCargo))) return false;
  if (!matchesMin(entry.price, parseFilterNumber(filters.minPrice))) return false;
  if (!matchesMax(entry.price, parseFilterNumber(filters.maxPrice))) return false;
  if (!matchesMin(shipSpeed(entry), parseFilterNumber(filters.minSpeed))) return false;
  if (!matchesMin(shipHull(entry), parseFilterNumber(filters.minHull))) return false;
  if (!matchesMin(shipShield(entry), parseFilterNumber(filters.minShield))) return false;

  const classNeedle = filters.classText.trim().toLowerCase();
  if (classNeedle) {
    const haystack = [entry.classId, entry.className, entry.category, entry.typeName, entry.name].join(" ").toLowerCase();
    if (!haystack.includes(classNeedle)) return false;
  }
  return true;
}

function hasShipFilters(filters: ShipFilters): boolean {
  return Object.values(filters).some((value) => value.trim());
}

export default function CatalogPanel({ sessions }: CatalogPanelProps) {
  const prayer = usePrayer();
  const [sourceHandle, setSourceHandle] = useState("");
  const [kind, setKind] = useState<CatalogKind>("items");
  const [query, setQuery] = useState("");
  const [shipFilters, setShipFilters] = useState<ShipFilters>(emptyShipFilters);
  const data: CatalogState | null = useMemo(() => selectCatalog(prayer.catalog), [prayer.catalog]);
  const status = prayer.error?.message ?? (prayer.catalog && !data ? "Catalog data was empty." : null);
  const loading = prayer.connection === "connecting";

  useEffect(() => {
    if (!sessions.length) return;
    if (!sessions.some((session) => session.sessionHandle === sourceHandle)) {
      setSourceHandle(sessions[0]!.sessionHandle);
    }
  }, [sessions, sourceHandle]);

  const needle = query.trim().toLowerCase();
  const items = useMemo(() => (data?.items ?? []).filter((entry) => !needle || itemSearch(entry).includes(needle)), [data, needle]);
  const ships = useMemo(
    () => (data?.ships ?? []).filter((entry) => (!needle || shipSearch(entry).includes(needle)) && matchesShipFilters(entry, shipFilters)),
    [data, needle, shipFilters],
  );
  const recipes = useMemo(() => (data?.recipes ?? []).filter((entry) => !needle || recipeSearch(entry).includes(needle)), [data, needle]);
  const facilities = useMemo(() => {
    const all = data?.facilities ?? [];
    const filtered = all.filter((entry) => !needle || itemSearch(entry).includes(needle));
    console.info("[catalog-tab] facilities view counts", {
      totalFacilities: all.length,
      filteredFacilities: filtered.length,
      query: needle,
      sampleIds: all.slice(0, 8).map((entry) => entry.id),
    });
    return filtered;
  }, [data, needle]);

  if (!sessions.length) {
    return (
      <div className="catalog-panel">
        <div className="catalog-empty">No registered sessions.</div>
      </div>
    );
  }

  return (
    <div className="catalog-panel">
      <div className="catalog-toolbar">
        <div>
          <div className="catalog-title">Catalog</div>
          <div className="catalog-meta">
            {data
              ? `${data.items.length} items / ${data.ships.length} ships / ${data.recipes.length} recipes / ${data.facilities.length} facilities`
              : "not loaded"}
          </div>
        </div>
        <select value={sourceHandle} onChange={(event) => setSourceHandle(event.target.value)}>
          {sessions.map((session) => (
            <option key={session.sessionHandle} value={session.sessionHandle}>
              {session.sessionHandle}
            </option>
          ))}
        </select>
        <button className="session-btn" onClick={() => void prayer.refreshKnowledge()} disabled={loading}>
          refresh
        </button>
      </div>

      {status && <div className="catalog-status">{status}</div>}

      <div className="catalog-controls">
        <div className="catalog-tabs" role="tablist" aria-label="Catalog type">
          {(["items", "ships", "recipes", "facilities"] as CatalogKind[]).map((mode) => (
            <button key={mode} type="button" data-active={kind === mode} onClick={() => setKind(mode)}>
              {mode}
            </button>
          ))}
        </div>
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="name, id, category" />
      </div>

      {kind === "ships" && (
        <ShipCatalogFilters
          filters={shipFilters}
          entries={data?.ships ?? []}
          filteredCount={ships.length}
          onChange={(key, value) => setShipFilters((current) => ({ ...current, [key]: value }))}
          onReset={() => setShipFilters(emptyShipFilters)}
        />
      )}

      {kind === "items" ? (
        <ItemCatalog entries={items} loading={loading} />
      ) : kind === "ships" ? (
        <ShipCatalog entries={ships} loading={loading} />
      ) : kind === "recipes" ? (
        <RecipeCatalog entries={recipes} loading={loading} />
      ) : (
        <FacilityCatalog entries={facilities} loading={loading} />
      )}
    </div>
  );
}

function ShipCatalogFilters({
  filters,
  entries,
  filteredCount,
  onChange,
  onReset,
}: {
  filters: ShipFilters;
  entries: ShipCatalogEntry[];
  filteredCount: number;
  onChange: (key: keyof ShipFilters, value: string) => void;
  onReset: () => void;
}) {
  const tiers = useMemo(
    () => Array.from(new Set(entries.map((entry) => entry.tier).filter((tier): tier is number => tier !== null))).sort((a, b) => a - b),
    [entries],
  );
  const active = hasShipFilters(filters);
  return (
    <div className="catalog-ship-filters">
      <select value={filters.tier} onChange={(event) => onChange("tier", event.target.value)}>
        <option value="">any tier</option>
        {tiers.map((tier) => (
          <option key={tier} value={String(tier)}>
            tier {tier}
          </option>
        ))}
      </select>
      <input
        type="number"
        inputMode="numeric"
        value={filters.minCargo}
        onChange={(event) => onChange("minCargo", event.target.value)}
        placeholder="min cargo"
      />
      <input
        type="number"
        inputMode="numeric"
        value={filters.maxCargo}
        onChange={(event) => onChange("maxCargo", event.target.value)}
        placeholder="max cargo"
      />
      <input
        type="number"
        inputMode="numeric"
        value={filters.minPrice}
        onChange={(event) => onChange("minPrice", event.target.value)}
        placeholder="min price"
      />
      <input
        type="number"
        inputMode="numeric"
        value={filters.maxPrice}
        onChange={(event) => onChange("maxPrice", event.target.value)}
        placeholder="max price"
      />
      <input
        type="number"
        inputMode="numeric"
        value={filters.minSpeed}
        onChange={(event) => onChange("minSpeed", event.target.value)}
        placeholder="min speed"
      />
      <input type="number" inputMode="numeric" value={filters.minHull} onChange={(event) => onChange("minHull", event.target.value)} placeholder="min hull" />
      <input
        type="number"
        inputMode="numeric"
        value={filters.minShield}
        onChange={(event) => onChange("minShield", event.target.value)}
        placeholder="min shield"
      />
      <input value={filters.classText} onChange={(event) => onChange("classText", event.target.value)} placeholder="class/category" />
      <span>
        {filteredCount.toLocaleString()} / {entries.length.toLocaleString()}
      </span>
      <button type="button" className="session-btn" onClick={onReset} disabled={!active}>
        clear
      </button>
    </div>
  );
}

function ItemCatalog({ entries, loading }: { entries: CatalogEntry[]; loading: boolean }) {
  return (
    <div className="catalog-table-wrap">
      <table className="catalog-table catalog-table--items">
        <thead>
          <tr>
            <th>item</th>
            <th>category</th>
            <th>type</th>
            <th>size</th>
            <th>price</th>
            <th>inputs</th>
            <th>outputs</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr key={entry.id}>
              <td>
                <div className="catalog-name">{entry.name}</div>
                <div className="catalog-id">{entry.id}</div>
              </td>
              <td>{entry.category || "-"}</td>
              <td>{entry.typeName || entry.className || "-"}</td>
              <td>{formatNumber(entry.size)}</td>
              <td>
                <CreditAmount value={entry.price} />
              </td>
              <td>{ingredientList(entry.inputs.length ? entry.inputs : entry.ingredients)}</td>
              <td>{ingredientList(entry.outputs)}</td>
            </tr>
          ))}
          {!loading && entries.length === 0 && (
            <tr>
              <td colSpan={7} className="catalog-empty">
                No items match this view.
              </td>
            </tr>
          )}
          {loading && (
            <tr>
              <td colSpan={7} className="catalog-empty">
                Loading catalog...
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function ShipCatalog({ entries, loading }: { entries: ShipCatalogEntry[]; loading: boolean }) {
  return (
    <div className="catalog-table-wrap">
      <table className="catalog-table catalog-table--ships">
        <thead>
          <tr>
            <th>ship</th>
            <th>class</th>
            <th>tier</th>
            <th>hull</th>
            <th>shield</th>
            <th>cargo</th>
            <th>speed</th>
            <th>fitting</th>
            <th>modules</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr key={entry.id}>
              <td>
                <div className="catalog-name">{entry.name}</div>
                <div className="catalog-id">{entry.id}</div>
              </td>
              <td>{entry.className || entry.classId || entry.category || "-"}</td>
              <td>{formatNumber(entry.tier)}</td>
              <td>{formatNumber(entry.hull ?? entry.baseHull)}</td>
              <td>{formatNumber(entry.shield ?? entry.baseShield)}</td>
              <td>{formatNumber(entry.cargoCapacity ?? entry.cargo)}</td>
              <td>{formatNumber(entry.speed ?? entry.baseSpeed)}</td>
              <td>
                <div className="catalog-fit">
                  <span>cpu {formatNumber(entry.cpuCapacity)}</span>
                  <span>pwr {formatNumber(entry.powerCapacity)}</span>
                  <span>
                    w/d/u {formatNumber(entry.weaponSlots)}/{formatNumber(entry.defenseSlots)}/{formatNumber(entry.utilitySlots)}
                  </span>
                </div>
              </td>
              <td>{entry.defaultModules.length ? entry.defaultModules.join(", ") : "-"}</td>
            </tr>
          ))}
          {!loading && entries.length === 0 && (
            <tr>
              <td colSpan={9} className="catalog-empty">
                No ships match this view.
              </td>
            </tr>
          )}
          {loading && (
            <tr>
              <td colSpan={9} className="catalog-empty">
                Loading catalog...
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function RecipeCatalog({ entries, loading }: { entries: RecipeCatalogEntry[]; loading: boolean }) {
  return (
    <div className="catalog-recipes">
      {entries.map((entry) => (
        <section className="catalog-recipe" key={entry.id}>
          <div className="catalog-recipe-head">
            <div>
              <div className="catalog-name">{entry.name}</div>
              <div className="catalog-id">{entry.id}</div>
            </div>
            <div className="catalog-skills">{skillList(entry.requiredSkills)}</div>
          </div>
          <div className="catalog-recipe-flow">
            <IngredientColumn label="inputs" entries={entry.inputs} />
            <div className="catalog-arrow" aria-hidden="true">
              -&gt;
            </div>
            <IngredientColumn label="outputs" entries={entry.outputs} />
          </div>
        </section>
      ))}
      {!loading && entries.length === 0 && <div className="catalog-empty">No recipes match this view.</div>}
      {loading && <div className="catalog-empty">Loading catalog...</div>}
    </div>
  );
}

function FacilityCatalog({ entries, loading }: { entries: CatalogEntry[]; loading: boolean }) {
  return (
    <div className="catalog-table-wrap">
      <table className="catalog-table catalog-table--facilities">
        <thead>
          <tr>
            <th>facility</th>
            <th>category</th>
            <th>type</th>
            <th>tier</th>
            <th>scale</th>
            <th>price</th>
            <th>materials</th>
            <th>skills</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr key={entry.id}>
              <td>
                <div className="catalog-name">{entry.name}</div>
                <div className="catalog-id">{entry.id}</div>
              </td>
              <td>{entry.category || "-"}</td>
              <td>{entry.typeName || entry.className || entry.classId || "-"}</td>
              <td>{formatNumber(entry.tier)}</td>
              <td>{formatNumber(entry.scale)}</td>
              <td>
                <CreditAmount value={entry.price} />
              </td>
              <td>{materialList(entry.materials)}</td>
              <td>{skillList(entry.requiredSkills)}</td>
            </tr>
          ))}
          {!loading && entries.length === 0 && (
            <tr>
              <td colSpan={8} className="catalog-empty">
                No facilities match this view.
              </td>
            </tr>
          )}
          {loading && (
            <tr>
              <td colSpan={8} className="catalog-empty">
                Loading catalog...
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function IngredientColumn({ label, entries }: { label: string; entries: CatalogIngredient[] }) {
  return (
    <div className="catalog-ingredients">
      <div className="catalog-ingredients-label">{label}</div>
      {entries.length ? (
        entries.map((entry, idx) => (
          <div className="catalog-ingredient" key={`${entry.itemId}:${idx}`}>
            <span>{entry.name || entry.itemId}</span>
            <strong>{ingredientQty(entry)}</strong>
          </div>
        ))
      ) : (
        <div className="catalog-ingredient catalog-ingredient--empty">-</div>
      )}
    </div>
  );
}
