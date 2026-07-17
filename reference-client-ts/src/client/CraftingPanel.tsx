import { useCallback, useDeferredValue, useEffect, useMemo, useState } from "react";
import { actions } from "@prayer/sdk";
import {
  CatalogEntry,
  CatalogIngredient,
  CatalogState,
  CraftStorageSource,
  CraftingQueueJob,
  EconomyMarketData,
  FacilityInfo,
  FacilityTypeInfo,
  RecipeCatalogEntry,
} from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import SearchableSessionSelect from "./SearchableSessionSelect.js";
import { CreditAmount } from "./Credits.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectCatalog, selectEconomyMarket, selectFacilities } from "./prayer/worldSelectors.js";

type CraftingPanelProps = {
  sessions: SessionState[];
};

type PricedIngredient = CatalogIngredient & {
  quantityValue: number;
  unitPrice: number | null;
  totalValue: number | null;
};

type RecipePricing = {
  recipe: RecipeCatalogEntry;
  inputs: PricedIngredient[];
  outputs: PricedIngredient[];
  facilities: CraftFacility[];
  publicFacilities: CraftFacility[];
  usableFacilities: CraftFacility[];
  hasPublicFacility: boolean;
  hasUsableFacility: boolean;
  inputCost: number | null;
  outputValue: number | null;
  averageOutputPrice: number | null;
  profitPerCraft: number | null;
  roi: number | null;
  missingPrices: number;
};

type CraftQuantityProjection = {
  requestedQuantity: number;
  runs: number;
  inputs: PricedIngredient[];
  outputs: PricedIngredient[];
  inputCost: number | null;
  outputValue: number | null;
  averageOutputPrice: number | null;
  profit: number | null;
  roi: number | null;
};

type SortMode = "profit" | "roi" | "inputCost" | "outputValue" | "name";

const craftStorageSources: CraftStorageSource[] = ["storage", "faction", "cargo"];

export type CraftFacility = {
  id: string;
  targetId: string | null;
  name: string;
  detail: string;
  locationId: string | null;
};

type RecipeFacilities = {
  facilities: CraftFacility[];
  publicFacilities: CraftFacility[];
  usableFacilities: CraftFacility[];
};

type FacilityRecipeSource = {
  id: string;
  name: string;
  inputs: CatalogIngredient[];
  outputs: CatalogIngredient[];
  recipeIds: string[];
};

type RecipeSearchEntry = {
  recipe: RecipeCatalogEntry;
  recipeText: string;
  inputText: string;
  outputText: string;
};

export type FacilityMatchIndex = {
  byId: Map<string, FacilityRecipeSource>;
  byRecipeId: Map<string, FacilityRecipeSource[]>;
  bySignature: Map<string, FacilityRecipeSource[]>;
  instancesByFacilityId: Map<string, FacilityInfo[]>;
  instancesByType: Map<string, FacilityInfo[]>;
  publicInstancesByType: Map<string, FacilityInfo[]>;
};

const initialRecipeLimit = 120;
const recipeLimitStep = 120;

function ingredientQuantity(entry: CatalogIngredient): number {
  return entry.quantity ?? entry.amount ?? entry.count ?? 1;
}

export function formatQty(value: number): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

function formatPercent(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "-";
  return `${(value * 100).toLocaleString(undefined, { maximumFractionDigits: 1 })}%`;
}

function formatStatusText(value: string): string {
  return value.replace(/[_-]+/g, " ");
}

function ingredientLabel(entry: CatalogIngredient): string {
  return entry.name || entry.itemId || entry.id || entry.item || "unknown";
}

function outputSummary(lines: PricedIngredient[]): string {
  return lines.map((line) => `${formatQty(line.quantityValue)} ${ingredientLabel(line)}`).join(", ") || "-";
}

function recipeSearch(entry: RecipeCatalogEntry): string {
  return [entry.id, entry.name, ...Object.keys(entry.requiredSkills)].join(" ").toLowerCase();
}

function ingredientSearch(lines: CatalogIngredient[]): string {
  return lines
    .map((line) => `${line.itemId} ${line.name} ${line.item}`)
    .join(" ")
    .toLowerCase();
}

function recipeFacilityIds(recipe: RecipeCatalogEntry): string[] {
  return recipe.requiredFacilityTypes;
}

function facilityRecipeIds(facility: FacilityRecipeSource): string[] {
  return facility.recipeIds;
}

function ingredientSignature(lines: CatalogIngredient[]): string {
  return lines
    .map((line) => `${line.itemId}:${ingredientQuantity(line)}`)
    .sort()
    .join("|");
}

function recipeIngredientSignature(inputs: CatalogIngredient[], outputs: CatalogIngredient[]): string | null {
  const inputSignature = ingredientSignature(inputs);
  const outputSignature = ingredientSignature(outputs);
  if (!inputSignature || !outputSignature) return null;
  return `${inputSignature}=>${outputSignature}`;
}

function facilityLabel(facility: FacilityRecipeSource): CraftFacility {
  return {
    id: facility.id,
    targetId: null,
    name: facility.name || facility.id,
    detail: facility.id,
    locationId: null,
  };
}

function catalogFacilitySource(facility: CatalogEntry): FacilityRecipeSource {
  return {
    id: facility.id,
    name: facility.name || facility.id,
    inputs: facility.inputs,
    outputs: facility.outputs,
    recipeIds: facility.recipeIds,
  };
}

function typeFacilitySource(type: FacilityTypeInfo): FacilityRecipeSource {
  return {
    id: type.facilityType,
    name: type.name || type.facilityType,
    inputs: [],
    outputs: [],
    recipeIds: type.recipeId ? [type.recipeId] : [],
  };
}

function instanceFacilityLabel(facility: FacilityInfo): CraftFacility {
  const location = facility.locationName || facility.locationId || facility.systemId;
  const owner = facility.ownerKind === "faction" || facility.ownerKind === "personal" ? facility.ownerKind : facility.ownerName;
  const status = facility.status && facility.status !== "active" ? facility.status : "";
  const facilityId = facility.facilityId.trim();
  return {
    id: facilityId || `${facility.facilityType}:${facility.locationId}`,
    targetId: facilityId || null,
    name: facility.name || facility.facilityType,
    detail: [owner, location, status].filter(Boolean).join(" · ") || facility.facilityType,
    locationId: facility.locationId || null,
  };
}

function facilityIsAvailable(facility: FacilityInfo): boolean {
  const status = facility.status.trim().toLowerCase();
  if (!status) return true;
  return !["build", "building", "construction", "disabled", "inactive", "offline", "unavailable"].some((blocked) => status.includes(blocked));
}

function facilityInstanceKey(facility: FacilityInfo): string {
  return facility.facilityId || `${facility.facilityType}:${facility.locationId}`;
}

function facilityIsOwnedUsable(facility: FacilityInfo): boolean {
  return facilityIsAvailable(facility) && (facility.ownerKind === "personal" || facility.ownerKind === "faction");
}

function pushMapValue<K, V>(map: Map<K, V[]>, key: K, value: V): void {
  const existing = map.get(key);
  if (existing) {
    existing.push(value);
  } else {
    map.set(key, [value]);
  }
}

function uniqueFacilitySources(sources: FacilityRecipeSource[]): FacilityRecipeSource[] {
  const seen = new Set<string>();
  const unique: FacilityRecipeSource[] = [];
  for (const source of sources) {
    if (seen.has(source.id)) continue;
    seen.add(source.id);
    unique.push(source);
  }
  return unique;
}

export function buildFacilityMatchIndex(
  catalog: CatalogState | null,
  facilityTypes: FacilityTypeInfo[],
  facilityInstances: FacilityInfo[],
  publicFacilityInstances: FacilityInfo[],
): FacilityMatchIndex {
  const sources = [...(catalog?.facilities ?? []).map(catalogFacilitySource), ...facilityTypes.map(typeFacilitySource)];
  const byId = new Map(sources.map((facility) => [facility.id, facility]));
  const byRecipeId = new Map<string, FacilityRecipeSource[]>();
  const bySignature = new Map<string, FacilityRecipeSource[]>();
  const instancesByFacilityId = new Map<string, FacilityInfo[]>();
  const instancesByType = new Map<string, FacilityInfo[]>();
  const publicInstancesByType = new Map<string, FacilityInfo[]>();

  for (const source of sources) {
    for (const recipeId of facilityRecipeIds(source)) {
      pushMapValue(byRecipeId, recipeId, source);
    }
    const signature = recipeIngredientSignature(source.inputs, source.outputs);
    if (signature) pushMapValue(bySignature, signature, source);
  }

  for (const instance of facilityInstances) {
    if (instance.facilityId) pushMapValue(instancesByFacilityId, instance.facilityId, instance);
    if (instance.facilityType) pushMapValue(instancesByType, instance.facilityType, instance);
  }

  for (const instance of publicFacilityInstances) {
    if (instance.facilityType) pushMapValue(publicInstancesByType, instance.facilityType, instance);
  }

  return {
    byId,
    byRecipeId,
    bySignature,
    instancesByFacilityId,
    instancesByType,
    publicInstancesByType,
  };
}

function matchingFacilityTypesForRecipe(recipe: RecipeCatalogEntry, facilityIndex: FacilityMatchIndex): Set<string> {
  const matchingFacilityTypeIds = new Set(recipeFacilityIds(recipe));
  const signature = recipeIngredientSignature(recipe.inputs, recipe.outputs);
  const matchingSources = uniqueFacilitySources([
    ...(facilityIndex.byRecipeId.get(recipe.id) ?? []),
    ...(signature ? (facilityIndex.bySignature.get(signature) ?? []) : []),
  ]);

  for (const facility of matchingSources) {
    matchingFacilityTypeIds.add(facility.id);
  }

  return matchingFacilityTypeIds;
}

export function facilitiesForRecipe(recipe: RecipeCatalogEntry, facilityIndex: FacilityMatchIndex): RecipeFacilities {
  const facilities = new Map<string, CraftFacility>();
  const publicFacilities = new Map<string, CraftFacility>();
  const usableFacilities = new Map<string, CraftFacility>();
  const matchingFacilityTypeIds = matchingFacilityTypesForRecipe(recipe, facilityIndex);

  const addUsableOwnedFacility = (instance: FacilityInfo) => {
    if (!facilityIsOwnedUsable(instance)) return;
    usableFacilities.set(facilityInstanceKey(instance), instanceFacilityLabel(instance));
  };

  for (const id of matchingFacilityTypeIds) {
    const facility = facilityIndex.byId.get(id);
    facilities.set(id, facility ? facilityLabel(facility) : { id, targetId: null, name: id, detail: id, locationId: null });
    for (const instance of facilityIndex.instancesByFacilityId.get(id) ?? []) {
      facilities.set(facilityInstanceKey(instance), instanceFacilityLabel(instance));
      addUsableOwnedFacility(instance);
    }
    for (const instance of facilityIndex.instancesByType.get(id) ?? []) {
      facilities.set(facilityInstanceKey(instance), instanceFacilityLabel(instance));
      addUsableOwnedFacility(instance);
    }
  }

  for (const facilityType of matchingFacilityTypeIds) {
    for (const instance of facilityIndex.instancesByType.get(facilityType) ?? []) {
      addUsableOwnedFacility(instance);
    }
    for (const instance of facilityIndex.publicInstancesByType.get(facilityType) ?? []) {
      if (!facilityIsAvailable(instance)) continue;
      const key = facilityInstanceKey(instance);
      const label = instanceFacilityLabel(instance);
      publicFacilities.set(key, label);
    }
  }

  const sortFacilities = (values: Iterable<CraftFacility>) => [...values].sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));

  return {
    facilities: sortFacilities(facilities.values()),
    publicFacilities: sortFacilities(publicFacilities.values()),
    usableFacilities: sortFacilities(usableFacilities.values()),
  };
}

type PriceSide = "buy" | "sell";

function unitPrice(itemId: string, market: EconomyMarketData | null, side: PriceSide): number | null {
  if (!market) return null;
  const price = side === "buy" ? market.globalMedianBuyPrices[itemId] : market.globalMedianSellPrices[itemId];
  if (typeof price === "number" && Number.isFinite(price) && price > 0) return price;
  return null;
}

function priceLines(lines: CatalogIngredient[], market: EconomyMarketData | null, side: PriceSide): PricedIngredient[] {
  return lines.map((line) => {
    const quantityValue = ingredientQuantity(line);
    const price = unitPrice(line.itemId, market, side);
    return {
      ...line,
      quantityValue,
      unitPrice: price,
      totalValue: price === null ? null : price * quantityValue,
    };
  });
}

function sumKnown(lines: PricedIngredient[]): number | null {
  if (lines.some((line) => line.totalValue === null)) return null;
  return lines.reduce((sum, line) => sum + (line.totalValue ?? 0), 0);
}

function priceRecipe(recipe: RecipeCatalogEntry, market: EconomyMarketData | null, facilityIndex: FacilityMatchIndex): RecipePricing {
  const inputs = priceLines(recipe.inputs, market, "sell");
  const outputs = priceLines(recipe.outputs, market, "buy");
  const { facilities, publicFacilities, usableFacilities } = facilitiesForRecipe(recipe, facilityIndex);
  const hasPublicFacility = publicFacilities.length > 0;
  const hasUsableFacility = usableFacilities.length > 0;
  const inputCost = sumKnown(inputs);
  const outputValue = sumKnown(outputs);
  const outputQuantity = outputs.reduce((sum, line) => sum + line.quantityValue, 0);
  const averageOutputPrice = outputValue === null || outputQuantity <= 0 ? null : outputValue / outputQuantity;
  const profitPerCraft = inputCost === null || outputValue === null ? null : outputValue - inputCost;
  const roi = profitPerCraft === null || inputCost === null || inputCost <= 0 ? null : profitPerCraft / inputCost;
  const missingPrices = [...inputs, ...outputs].filter((line) => line.unitPrice === null).length;
  return {
    recipe,
    inputs,
    outputs,
    facilities,
    publicFacilities,
    usableFacilities,
    hasPublicFacility,
    hasUsableFacility,
    inputCost,
    outputValue,
    averageOutputPrice,
    profitPerCraft,
    roi,
    missingPrices,
  };
}

function comparePricedRecipes(sortMode: SortMode) {
  return (a: RecipePricing, b: RecipePricing): number => {
    if (sortMode === "name") return a.recipe.name.localeCompare(b.recipe.name) || a.recipe.id.localeCompare(b.recipe.id);
    const field = sortMode === "profit" ? "profitPerCraft" : sortMode === "roi" ? "roi" : sortMode === "inputCost" ? "inputCost" : "outputValue";
    const av = a[field] ?? Number.NEGATIVE_INFINITY;
    const bv = b[field] ?? Number.NEGATIVE_INFINITY;
    return bv - av || a.recipe.name.localeCompare(b.recipe.name) || a.recipe.id.localeCompare(b.recipe.id);
  };
}

function LineList({ lines }: { lines: PricedIngredient[] }) {
  if (!lines.length) return <span className="crafting-muted">none</span>;
  return (
    <div className="crafting-lines">
      {lines.map((line) => (
        <div className="crafting-line" key={`${line.itemId}:${line.id}:${line.quantityValue}`}>
          <span title={line.itemId}>
            {formatQty(line.quantityValue)} {ingredientLabel(line)}
          </span>
          <span>
            <CreditAmount value={line.unitPrice} /> ea
          </span>
          <CreditAmount value={line.totalValue} />
        </div>
      ))}
    </div>
  );
}

function FacilityList({ facilities }: { facilities: CraftFacility[] }) {
  if (!facilities.length) return <span className="crafting-muted">unknown</span>;
  return (
    <div className="crafting-facilities">
      {facilities.map((facility) => (
        <span key={facility.id} title={facility.detail || facility.id}>
          {facility.name}
        </span>
      ))}
    </div>
  );
}

function FacilityAvailability({ priced }: { priced: RecipePricing }) {
  return (
    <div className="crafting-facility-cell">
      <FacilityList facilities={priced.facilities} />
      {priced.hasUsableFacility ? (
        <div className="crafting-facility-note">
          usable: {priced.usableFacilities.map((facility) => facility.name).join(", ")}
          {priced.hasPublicFacility ? ` · public: ${priced.publicFacilities.map((facility) => facility.name).join(", ")}` : ""}
        </div>
      ) : (
        <div className="crafting-facility-warning">no usable facility</div>
      )}
    </div>
  );
}

function recipeDefaultCraftQuantity(recipe: RecipeCatalogEntry): number {
  const firstOutput = recipe.outputs[0];
  if (!firstOutput) return 1;
  return Math.max(1, Math.floor(ingredientQuantity(firstOutput)));
}

function recipeOutputBatchQuantity(recipe: RecipeCatalogEntry): number {
  const firstOutput = recipe.outputs[0];
  if (!firstOutput) return 1;
  const quantity = ingredientQuantity(firstOutput);
  return Number.isFinite(quantity) && quantity > 0 ? quantity : 1;
}

function selectedCraftQuantity(priced: RecipePricing, quantities: Record<string, number>): number {
  return Math.max(1, Math.floor(quantities[priced.recipe.id] ?? recipeDefaultCraftQuantity(priced.recipe)));
}

function recipeRunCount(recipe: RecipeCatalogEntry, requestedQuantity: number): number {
  return Math.max(1, Math.ceil(requestedQuantity / recipeOutputBatchQuantity(recipe)));
}

function scalePricedLines(lines: PricedIngredient[], multiplier: number): PricedIngredient[] {
  return lines.map((line) => {
    const quantityValue = line.quantityValue * multiplier;
    return {
      ...line,
      quantityValue,
      totalValue: line.unitPrice === null ? null : line.unitPrice * quantityValue,
    };
  });
}

function craftQuantityProjection(priced: RecipePricing, requestedQuantity: number): CraftQuantityProjection {
  const runs = recipeRunCount(priced.recipe, requestedQuantity);
  const inputs = scalePricedLines(priced.inputs, runs);
  const outputs = scalePricedLines(priced.outputs, runs);
  const inputCost = sumKnown(inputs);
  const outputValue = sumKnown(outputs);
  const outputQuantity = outputs.reduce((sum, line) => sum + line.quantityValue, 0);
  const averageOutputPrice = outputValue === null || outputQuantity <= 0 ? null : outputValue / outputQuantity;
  const profit = inputCost === null || outputValue === null ? null : outputValue - inputCost;
  const roi = profit === null || inputCost === null || inputCost <= 0 ? null : profit / inputCost;
  return {
    requestedQuantity,
    runs,
    inputs,
    outputs,
    inputCost,
    outputValue,
    averageOutputPrice,
    profit,
    roi,
  };
}

function SpaceMoltCraftingQueue({ queue }: { queue: CraftingQueueJob[] }) {
  return (
    <div className="crafting-queue-section">
      <div className="crafting-queue-heading">
        SpaceMolt queue ({queue.length.toLocaleString()} {queue.length === 1 ? "job" : "jobs"})
      </div>
      <div className="crafting-queue">
        {queue.length ? (
          queue.map((job, index) => {
            const rawText = job.raw_text?.trim();
            if (rawText) {
              return (
                <div className="crafting-queue-row crafting-queue-row--raw" key={job.job_id ?? index}>
                  <span title={rawText}>{rawText}</span>
                </div>
              );
            }
            const label = job.item_id ?? job.recipe_id ?? job.job_id ?? `job ${index + 1}`;
            const detailText = [
              formatStatusText(job.status ?? "queued"),
              job.crafts != null ? `${job.crafts.toLocaleString()} crafts` : "",
              job.quantity != null ? `${job.quantity.toLocaleString()} units` : "",
              job.source ?? "",
            ]
              .filter(Boolean)
              .join(" · ");
            return (
              <div className="crafting-queue-row" key={job.job_id ?? index}>
                <span title={JSON.stringify(job)}>{label}</span>
                <span>{detailText}</span>
              </div>
            );
          })
        ) : (
          <div className="crafting-queue crafting-queue--empty">No active SpaceMolt jobs.</div>
        )}
      </div>
    </div>
  );
}

export default function CraftingPanel({ sessions }: CraftingPanelProps) {
  const prayer = usePrayer();
  const [sourceHandle, setSourceHandle] = useState("");
  const catalog = useMemo<CatalogState | null>(() => selectCatalog(prayer.catalog), [prayer.catalog]);
  const selectedBot =
    prayer.fleet.find((bot) => {
      const session = sessions.find((candidate) => candidate.sessionHandle === sourceHandle);
      return bot.id === session?.botId || bot.id === sourceHandle || bot.username === sourceHandle;
    }) ?? null;
  const facilityState = useMemo(
    () =>
      selectFacilities(selectedBot, prayer.galaxyMap, prayer.catalog, prayer.facilitiesByPoi, prayer.ownedFacilitiesByPlayer, prayer.ownedFacilitiesByFaction),
    [prayer.catalog, prayer.facilitiesByPoi, prayer.galaxyMap, prayer.ownedFacilitiesByFaction, prayer.ownedFacilitiesByPlayer, selectedBot],
  );
  const facilityTypes: FacilityTypeInfo[] = facilityState?.types ?? [];
  const publicFacilityInstances: FacilityInfo[] = facilityState?.publicFacilities ?? [];
  const facilityInstances: FacilityInfo[] = facilityState
    ? [...facilityState.current, ...facilityState.owned, ...facilityState.factionCurrent, ...facilityState.factionOwned]
    : [];
  const market = useMemo<EconomyMarketData>(() => selectEconomyMarket(prayer.stationMarkets, prayer.galaxyMap), [prayer.galaxyMap, prayer.stationMarkets]);
  const [recipeQuery, setRecipeQuery] = useState("");
  const [inputQuery, setInputQuery] = useState("");
  const [outputQuery, setOutputQuery] = useState("");
  const [sortMode, setSortMode] = useState<SortMode>("profit");
  const [onlyPriced, setOnlyPriced] = useState(false);
  const [onlyPublicReady, setOnlyPublicReady] = useState(false);
  const [craftQuantities, setCraftQuantities] = useState<Record<string, number>>({});
  const [craftSources, setCraftSources] = useState<Record<string, CraftStorageSource>>({});
  const [startingRecipeId, setStartingRecipeId] = useState<string | null>(null);
  const [recipeLimit, setRecipeLimit] = useState(initialRecipeLimit);
  const [status, setStatus] = useState<string | null>(null);
  const deferredRecipeQuery = useDeferredValue(recipeQuery);
  const deferredInputQuery = useDeferredValue(inputQuery);
  const deferredOutputQuery = useDeferredValue(outputQuery);

  useEffect(() => {
    if (!sessions.length) return;
    if (!sessions.some((session) => session.sessionHandle === sourceHandle)) {
      setSourceHandle(sessions[0]!.sessionHandle);
    }
  }, [sessions, sourceHandle]);

  const craftingQueue = selectedBot?.state.crafting_queue ?? [];

  useEffect(() => {
    setRecipeLimit(initialRecipeLimit);
  }, [deferredInputQuery, deferredOutputQuery, deferredRecipeQuery, onlyPriced, onlyPublicReady, sortMode, sourceHandle]);

  const recipeSearchEntries = useMemo<RecipeSearchEntry[]>(
    () =>
      (catalog?.recipes ?? []).map((recipe) => ({
        recipe,
        recipeText: recipeSearch(recipe),
        inputText: ingredientSearch(recipe.inputs),
        outputText: ingredientSearch(recipe.outputs),
      })),
    [catalog],
  );

  const facilityIndex = useMemo(
    () => buildFacilityMatchIndex(catalog, facilityTypes, facilityInstances, publicFacilityInstances),
    [catalog, facilityInstances, facilityTypes, publicFacilityInstances],
  );

  const recipes = useMemo(() => {
    const recipeNeedle = deferredRecipeQuery.trim().toLowerCase();
    const inputNeedle = deferredInputQuery.trim().toLowerCase();
    const outputNeedle = deferredOutputQuery.trim().toLowerCase();
    return recipeSearchEntries
      .filter((entry) => !recipeNeedle || entry.recipeText.includes(recipeNeedle))
      .filter((entry) => !inputNeedle || entry.inputText.includes(inputNeedle))
      .filter((entry) => !outputNeedle || entry.outputText.includes(outputNeedle))
      .map((entry) => priceRecipe(entry.recipe, market, facilityIndex))
      .filter((recipe) => !onlyPriced || recipe.missingPrices === 0)
      .filter((recipe) => !onlyPublicReady || recipe.hasUsableFacility)
      .sort(comparePricedRecipes(sortMode));
  }, [deferredInputQuery, deferredOutputQuery, deferredRecipeQuery, facilityIndex, market, onlyPriced, onlyPublicReady, recipeSearchEntries, sortMode]);

  const pricedCount = recipes.filter((recipe) => recipe.missingPrices === 0).length;
  const profitableCount = recipes.filter((recipe) => (recipe.profitPerCraft ?? 0) > 0).length;
  const usableReadyCount = recipes.filter((recipe) => recipe.hasUsableFacility).length;
  const visibleRecipes = recipes.slice(0, recipeLimit);
  const hiddenRecipeCount = Math.max(0, recipes.length - visibleRecipes.length);
  const selectedSession = sessions.find((session) => session.sessionHandle === sourceHandle);
  const scriptBusy = selectedSession?.runningScript?.isRunning === true;

  const startCraft = useCallback(
    async (priced: RecipePricing) => {
      const facilityId = priced.usableFacilities[0]?.id;
      if (!facilityId) return;
      const quantity = selectedCraftQuantity(priced, craftQuantities);
      const source = craftSources[priced.recipe.id] ?? "storage";
      setStartingRecipeId(priced.recipe.id);
      setStatus(null);
      try {
        const bot = await prayer.bot(sourceHandle);
        const run = await bot.start(
          [
            actions.craft({
              recipe_id: priced.recipe.id,
              quantity,
              source,
              facility_id: facilityId,
            }),
          ],
          { idempotencyKey: crypto.randomUUID() },
        );
        setStatus(`Crafting ${priced.recipe.name} accepted as run ${run.id}.`);
        const terminal = await run.wait();
        if (terminal.status !== "succeeded") throw new Error(`Craft run ${run.id} ${terminal.status}.`);
        setStatus(`Crafting ${priced.recipe.name} completed as run ${run.id}.`);
        await prayer.refresh();
      } catch (err) {
        setStatus(err instanceof Error ? err.message : String(err));
      } finally {
        setStartingRecipeId(null);
      }
    },
    [craftQuantities, craftSources, prayer, sourceHandle],
  );

  if (!sessions.length) {
    return (
      <div className="crafting-panel">
        <div className="crafting-empty">No registered sessions.</div>
      </div>
    );
  }

  return (
    <div className="crafting-panel">
      <div className="crafting-toolbar">
        <div>
          <div className="crafting-title">Crafting</div>
          <div className="crafting-meta">
            {catalog
              ? `${catalog.recipes.length.toLocaleString()} recipes · ${pricedCount.toLocaleString()} fully priced · ${profitableCount.toLocaleString()} profitable · ${usableReadyCount.toLocaleString()} usable-ready`
              : "not loaded"}
          </div>
        </div>
        <SearchableSessionSelect sessions={sessions} value={sourceHandle} onChange={setSourceHandle} ariaLabel="Crafting session" />
        <button className="session-btn" onClick={() => void prayer.refresh()} disabled={prayer.connection === "connecting"}>
          refresh
        </button>
      </div>

      {status && <div className="crafting-status">{status}</div>}

      <>
        <div className="crafting-controls">
          <input value={recipeQuery} onChange={(event) => setRecipeQuery(event.target.value)} placeholder="recipe or skill" />
          <input value={inputQuery} onChange={(event) => setInputQuery(event.target.value)} placeholder="input item" />
          <input value={outputQuery} onChange={(event) => setOutputQuery(event.target.value)} placeholder="output item" />
          <select value={sortMode} onChange={(event) => setSortMode(event.target.value as SortMode)}>
            <option value="profit">profit</option>
            <option value="roi">roi</option>
            <option value="outputValue">output value</option>
            <option value="inputCost">input cost</option>
            <option value="name">name</option>
          </select>
          <label className="crafting-toggle">
            <input type="checkbox" checked={onlyPriced} onChange={(event) => setOnlyPriced(event.target.checked)} />
            fully priced
          </label>
          <label className="crafting-toggle">
            <input type="checkbox" checked={onlyPublicReady} onChange={(event) => setOnlyPublicReady(event.target.checked)} />
            usable-ready
          </label>
        </div>

        <div className="crafting-queue-wrap">
          <div className="crafting-detail-title">Queue</div>
          <SpaceMoltCraftingQueue queue={craftingQueue} />
        </div>

        <div className="crafting-recipes-wrap">
          <div className="crafting-recipe-list">
            {visibleRecipes.map((priced) => {
              const source = craftSources[priced.recipe.id] ?? "storage";
              const craftQuantity = selectedCraftQuantity(priced, craftQuantities);
              const projection = craftQuantityProjection(priced, craftQuantity);
              return (
                <details
                  className="crafting-recipe-card"
                  key={priced.recipe.id}
                  data-profitable={(priced.profitPerCraft ?? 0) > 0}
                  data-usable-facility={priced.hasUsableFacility}
                >
                  <summary className="crafting-recipe-summary">
                    <span className="crafting-recipe-title">
                      <span className="crafting-recipe-name">{priced.recipe.name}</span>
                      <span className="crafting-recipe-id">{priced.recipe.id}</span>
                    </span>
                    <span className="crafting-recipe-output" title={outputSummary(priced.outputs)}>
                      {outputSummary(priced.outputs)}
                    </span>
                    <span className={(priced.profitPerCraft ?? 0) >= 0 ? "crafting-profit-positive" : "crafting-profit-negative"}>
                      <CreditAmount value={priced.profitPerCraft} />
                    </span>
                  </summary>
                  <div className="crafting-recipe-body">
                    <div className="crafting-detail">
                      <div>
                        <div className="crafting-detail-title">Inputs · need</div>
                        <LineList lines={projection.inputs} />
                      </div>
                      <div>
                        <div className="crafting-detail-title">Outputs · produce</div>
                        <LineList lines={projection.outputs} />
                      </div>
                      <div>
                        <div className="crafting-detail-title">Facilities</div>
                        <FacilityAvailability priced={priced} />
                      </div>
                    </div>
                    <div className="crafting-metrics">
                      <span>request {formatQty(projection.requestedQuantity)}</span>
                      <span>
                        {projection.runs.toLocaleString()} {projection.runs === 1 ? "run" : "runs"}
                      </span>
                      <span>
                        input <CreditAmount value={projection.inputCost} />
                      </span>
                      <span>
                        output <CreditAmount value={projection.outputValue} />
                      </span>
                      <span>
                        out ea <CreditAmount value={projection.averageOutputPrice} />
                      </span>
                      <span>roi {formatPercent(projection.roi)}</span>
                      <span>{priced.missingPrices === 0 ? "priced" : `${priced.missingPrices} missing`}</span>
                    </div>
                    <div className="crafting-action">
                      <input
                        type="number"
                        min={1}
                        step={1}
                        value={craftQuantity}
                        onChange={(event) => {
                          const value = Math.max(1, Math.floor(Number(event.target.value) || 1));
                          setCraftQuantities((current) => ({ ...current, [priced.recipe.id]: value }));
                        }}
                        aria-label={`Craft quantity for ${priced.recipe.name}`}
                      />
                      <select
                        value={source}
                        onChange={(event) => {
                          const next = event.target.value as CraftStorageSource;
                          setCraftSources((current) => ({ ...current, [priced.recipe.id]: next }));
                        }}
                        aria-label={`Input source for ${priced.recipe.name}`}
                      >
                        {craftStorageSources.map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                      <button
                        className="session-btn"
                        disabled={!priced.hasUsableFacility || scriptBusy || startingRecipeId === priced.recipe.id}
                        onClick={() => void startCraft(priced)}
                      >
                        {startingRecipeId === priced.recipe.id ? "starting" : "craft"}
                      </button>
                    </div>
                  </div>
                </details>
              );
            })}
            {hiddenRecipeCount > 0 && (
              <button className="session-btn crafting-show-more" onClick={() => setRecipeLimit((current) => current + recipeLimitStep)}>
                show {Math.min(recipeLimitStep, hiddenRecipeCount).toLocaleString()} more of {recipes.length.toLocaleString()}
              </button>
            )}
            {prayer.connection !== "connecting" && recipes.length === 0 && <div className="crafting-empty">No recipes match this view.</div>}
          </div>
        </div>
      </>
    </div>
  );
}
