import { useCallback, useEffect, useMemo, useState } from "react";
import { CreditAmount } from "./Credits.js";
import {
  CatalogState,
  EconomyMarketData,
  FacilityInfo,
  FacilityTypeInfo,
  RecipeCatalogEntry,
  VirtualFactionOrderInput,
  VirtualCraftOrderInput,
  fetchVirtualOrders,
  fetchVirtualCraftOrders,
  fillVirtualOrder,
  fillVirtualCraftOrder,
  releaseVirtualOrder,
  releaseVirtualCraftOrder,
  saveVirtualOrders,
  saveVirtualCraftOrders,
} from "./api.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectCatalog, selectEconomyMarket, selectFacilities } from "./prayer/worldSelectors.js";
import { buildFacilityMatchIndex, facilitiesForRecipe, formatQty } from "./CraftingPanel.js";
import { SessionState } from "./SessionsPanel.js";

type QuartermasterPanelProps = {
  sessions: SessionState[];
};

type QuartermasterAction = "craft" | "craft_until" | "commission_until";
type QuartermasterTab = "market" | "ships" | "craft";
type VirtualOrderSide = "buy" | "sell" | "buy_until" | "sell_until";
type VirtualMarketOrder = VirtualFactionOrderInput;
type VirtualCraftOrder = VirtualCraftOrderInput;

const quartermasterShipCommissioningEnabled = true;

type VirtualMarketDraft = {
  side: VirtualOrderSide;
  itemId: string;
  stationId: string;
  priceEach: string;
  quantity: string;
  tippingPoint: string;
  priority: string;
  internalOnly: boolean;
  doForever: boolean;
};

type QuartermasterDraft = {
  action: QuartermasterAction;
  recipeId: string;
  itemId: string;
  stationId: string;
  quantity: string;
  priority: string;
  facilityId: string;
  preset: string;
  doForever: boolean;
};

type ShipStockDraft = {
  shipClassId: string;
  stationId: string;
  target: string;
  priority: string;
};

const allStationsValue = "all";

function sameVirtualCraftOrders(left: VirtualCraftOrder[], right: VirtualCraftOrder[]): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function sameVirtualMarketOrders(left: VirtualMarketOrder[], right: VirtualMarketOrder[]): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function virtualMarketRemaining(order: VirtualMarketOrder): number {
  return Math.max(0, order.quantity - (order.filled ?? 0) - (order.reserved ?? 0));
}

function virtualMarketOpenLabel(order: VirtualMarketOrder): string {
  if (order.side === "buy_until" || order.side === "sell_until") return "dynamic";
  return formatQty(virtualMarketRemaining(order));
}

function virtualCraftRemaining(order: VirtualCraftOrder): number {
  return Math.max(0, order.quantity - (order.filled ?? 0) - (order.reserved ?? 0));
}

function virtualCraftOpenLabel(order: VirtualCraftOrder): string {
  if (order.action === "craft_until" || order.action === "commission_until") return "dynamic";
  if (order.action === "credit_floor") return "-";
  return formatQty(virtualCraftRemaining(order));
}

function quartermasterActionLabel(action: QuartermasterAction | string): string {
  if (action === "craft_until") return "craft until";
  if (action === "commission_until") return "ship stock";
  if (action === "commission_until_transfer") return "ship transfer";
  if (action === "credit_floor") return "credit floor";
  return "craft";
}

function virtualOrderSideLabel(side: VirtualOrderSide | string): string {
  if (side === "buy_until") return "buy until";
  if (side === "sell_until") return "sell until";
  return side === "buy" ? "buy" : "sell";
}

function isUntilSide(side: VirtualOrderSide): boolean {
  return side === "buy_until" || side === "sell_until";
}

function newVirtualCraftOrderId(action: QuartermasterAction): string {
  const random = Math.random().toString(36).slice(2, 8);
  const prefix = action === "commission_until" ? "vcs" : "vc";
  return `${prefix}-${Date.now().toString(36)}-${random}`;
}

function newVirtualMarketOrderId(): string {
  const random = Math.random().toString(36).slice(2, 8);
  return `vf-${Date.now().toString(36)}-${random}`;
}

function orderStationMatches(order: VirtualCraftOrder, selectedStation: string): boolean {
  return selectedStation === allStationsValue || order.stationId === selectedStation;
}

function marketOrderStationMatches(order: VirtualMarketOrder, selectedStation: string): boolean {
  return selectedStation === allStationsValue || order.stationId === selectedStation;
}

export default function QuartermasterPanel({ sessions }: QuartermasterPanelProps) {
  const prayer = usePrayer();
  const [sourceHandle, setSourceHandle] = useState("");
  const [selectedStation, setSelectedStation] = useState(allStationsValue);
  const [marketOrders, setMarketOrders] = useState<VirtualMarketOrder[]>([]);
  const [orders, setOrders] = useState<VirtualCraftOrder[]>([]);
  const [activeTab, setActiveTab] = useState<QuartermasterTab>("ships");
  const [marketDraft, setMarketDraft] = useState<VirtualMarketDraft>({
    side: "buy_until",
    itemId: "",
    stationId: "",
    priceEach: "",
    quantity: "",
    tippingPoint: "",
    priority: "1",
    internalOnly: false,
    doForever: true,
  });
  const [draft, setDraft] = useState<QuartermasterDraft>({
    action: "craft_until",
    recipeId: "",
    itemId: "",
    stationId: "",
    quantity: "",
    priority: "1",
    facilityId: "",
    preset: "",
    doForever: true,
  });
  const [shipDraft, setShipDraft] = useState<ShipStockDraft>({
    shipClassId: "",
    stationId: "",
    target: "",
    priority: "1",
  });
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const sourceSession = sessions.find((session) => session.sessionHandle === sourceHandle) ?? null;
  const sourceBot = prayer.fleet.find((bot) => bot.id === sourceSession?.botId) ?? null;
  const catalog: CatalogState | null = useMemo(() => selectCatalog(prayer.catalog), [prayer.catalog]);
  const market: EconomyMarketData = useMemo(() => selectEconomyMarket(prayer.stationMarkets, prayer.galaxyMap), [prayer.galaxyMap, prayer.stationMarkets]);
  const facilities = useMemo(
    () =>
      selectFacilities(sourceBot, prayer.galaxyMap, prayer.catalog, prayer.facilitiesByPoi, prayer.ownedFacilitiesByPlayer, prayer.ownedFacilitiesByFaction),
    [prayer.catalog, prayer.facilitiesByPoi, prayer.galaxyMap, prayer.ownedFacilitiesByFaction, prayer.ownedFacilitiesByPlayer, sourceBot],
  );
  const facilityTypes: FacilityTypeInfo[] = facilities?.types ?? [];
  const publicFacilityInstances: FacilityInfo[] = facilities?.publicFacilities ?? [];
  const facilityInstances: FacilityInfo[] = facilities
    ? [...facilities.current, ...facilities.owned, ...facilities.factionCurrent, ...facilities.factionOwned]
    : [];

  useEffect(() => {
    if (!sessions.length) return;
    if (!sessions.some((session) => session.sessionHandle === sourceHandle)) {
      setSourceHandle(sessions[0]!.sessionHandle);
    }
  }, [sessions, sourceHandle]);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [nextMarketOrders, nextOrders] = await Promise.all([fetchVirtualOrders(), fetchVirtualCraftOrders(), prayer.refresh()]);
      setMarketOrders((current) => (nextMarketOrders && !sameVirtualMarketOrders(current, nextMarketOrders) ? nextMarketOrders : current));
      setOrders((current) => (nextOrders && !sameVirtualCraftOrders(current, nextOrders) ? nextOrders : current));
      setStatus(null);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [prayer]);

  const loadOrderBook = useCallback(async () => {
    try {
      const next = await fetchVirtualCraftOrders();
      setOrders((current) => (sameVirtualCraftOrders(current, next) ? current : next));
      const nextMarketOrders = await fetchVirtualOrders();
      setMarketOrders((current) => (sameVirtualMarketOrders(current, nextMarketOrders) ? current : nextMarketOrders));
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const updateMarketOrders = useCallback(async (next: VirtualMarketOrder[]) => {
    setMarketOrders(await saveVirtualOrders(next));
  }, []);

  const updateOrders = useCallback(async (next: VirtualCraftOrder[]) => {
    setOrders(await saveVirtualCraftOrders(next));
  }, []);

  useEffect(() => {
    if (!sourceHandle) return;
    void loadOrderBook();
    const timer = window.setInterval(() => {
      void loadOrderBook();
    }, 10_000);
    return () => window.clearInterval(timer);
  }, [loadOrderBook, sourceHandle]);

  const recipes = catalog?.recipes ?? [];
  const ships = catalog?.ships ?? [];
  const recipeById = useMemo(() => new Map(recipes.map((recipe) => [recipe.id, recipe])), [recipes]);
  const shipById = useMemo(() => new Map(ships.map((ship) => [ship.id, ship])), [ships]);
  const facilityIndex = useMemo(
    () => buildFacilityMatchIndex(catalog, facilityTypes, facilityInstances, publicFacilityInstances),
    [catalog, facilityInstances, facilityTypes, publicFacilityInstances],
  );
  const selectedRecipeFacilities = useMemo(() => {
    const selectedRecipe = recipeById.get(draft.recipeId.trim());
    return selectedRecipe ? facilitiesForRecipe(selectedRecipe, facilityIndex) : null;
  }, [draft.recipeId, facilityIndex, recipeById]);

  const stationOptions = useMemo(() => {
    const values = new Set<string>();
    for (const [stationKey, stationMarket] of Object.entries(market?.marketsByStation ?? {})) {
      values.add(stationMarket.stationId || stationKey);
    }
    for (const facility of [...facilityInstances, ...publicFacilityInstances]) {
      if (facility.locationId) values.add(facility.locationId);
    }
    for (const order of orders) {
      if (order.stationId) values.add(order.stationId);
    }
    for (const order of marketOrders) {
      if (order.stationId) values.add(order.stationId);
    }
    return [allStationsValue, ...[...values].sort((a, b) => a.localeCompare(b))];
  }, [facilityInstances, market, marketOrders, orders, publicFacilityInstances]);

  const facilityOptions = useMemo(() => {
    const values = new Set<string>();
    const draftStationId = draft.stationId.trim();
    const stationId = draftStationId || (selectedStation === allStationsValue ? "" : selectedStation);
    const usableFacilities = selectedRecipeFacilities?.usableFacilities;
    if (usableFacilities) {
      for (const facility of usableFacilities) {
        if (stationId && facility.locationId !== stationId) continue;
        const facilityId = facility.targetId || facility.id;
        if (facilityId) values.add(facilityId);
      }
      return [...values].sort((a, b) => a.localeCompare(b));
    }
    for (const facility of [...facilityInstances, ...publicFacilityInstances]) {
      if (stationId && facility.locationId !== stationId) continue;
      if (facility.facilityId) values.add(facility.facilityId);
    }
    return [...values].sort((a, b) => a.localeCompare(b));
  }, [draft.stationId, facilityInstances, publicFacilityInstances, selectedRecipeFacilities, selectedStation]);

  const visibleOrders = useMemo(
    () =>
      [...orders]
        .filter((order) => orderStationMatches(order, selectedStation))
        .sort(
          (a, b) =>
            Number(b.enabled) - Number(a.enabled) ||
            a.action.localeCompare(b.action) ||
            a.stationId.localeCompare(b.stationId) ||
            a.recipeId.localeCompare(b.recipeId) ||
            (a.itemId ?? "").localeCompare(b.itemId ?? ""),
        ),
    [orders, selectedStation],
  );

  const visibleShipOrders = useMemo(() => visibleOrders.filter((order) => order.action === "commission_until"), [visibleOrders]);

  const visibleCraftOrders = useMemo(() => visibleOrders.filter((order) => order.action !== "commission_until"), [visibleOrders]);

  const visibleMarketOrders = useMemo(
    () =>
      [...marketOrders]
        .filter((order) => marketOrderStationMatches(order, selectedStation))
        .sort(
          (a, b) =>
            Number(b.enabled) - Number(a.enabled) ||
            a.side.localeCompare(b.side) ||
            a.stationId.localeCompare(b.stationId) ||
            a.itemId.localeCompare(b.itemId) ||
            a.priceEach - b.priceEach,
        ),
    [marketOrders, selectedStation],
  );

  const addMarketOrder = useCallback(async () => {
    const stationId = marketDraft.stationId.trim();
    const itemId = marketDraft.itemId.trim();
    const priceEach = Number(marketDraft.priceEach);
    const quantity = Number(marketDraft.quantity);
    const tippingPoint = marketDraft.side === "sell_until" && marketDraft.tippingPoint.trim() ? Number(marketDraft.tippingPoint) : null;
    const priority = marketDraft.priority.trim() ? Number(marketDraft.priority) : 1;
    if (stationId === allStationsValue || !itemId || !Number.isFinite(priceEach) || priceEach <= 0 || !Number.isFinite(quantity) || quantity <= 0) {
      setStatus("Virtual buy/sell needs a concrete station, item, positive price, and positive quantity.");
      return;
    }
    if (!Number.isFinite(priority) || priority <= 0) {
      setStatus("Virtual buy/sell priority must be a positive number.");
      return;
    }
    if (tippingPoint !== null && (!Number.isFinite(tippingPoint) || tippingPoint <= 0)) {
      setStatus("Sell-until tipping point must be blank or a positive number.");
      return;
    }
    await updateMarketOrders([
      ...marketOrders,
      {
        id: newVirtualMarketOrderId(),
        side: marketDraft.side,
        itemId,
        stationId,
        priceEach: Math.floor(priceEach),
        quantity: Math.floor(quantity),
        ...(marketDraft.side === "sell_until" && tippingPoint !== null ? { tippingPoint: Math.floor(tippingPoint) } : {}),
        reserved: 0,
        filled: 0,
        enabled: true,
        internalOnly: marketDraft.internalOnly,
        priority,
        doForever: isUntilSide(marketDraft.side) ? marketDraft.doForever : false,
      },
    ]);
    setMarketDraft((current) => ({
      ...current,
      itemId: "",
      stationId: selectedStation === allStationsValue ? current.stationId : selectedStation,
      priceEach: "",
      quantity: "",
      tippingPoint: "",
      priority: "1",
    }));
    setStatus(null);
  }, [marketDraft, marketOrders, selectedStation, updateMarketOrders]);

  const toggleMarketOrder = useCallback(
    async (id: string) => {
      await updateMarketOrders(marketOrders.map((order) => (order.id === id ? { ...order, enabled: !order.enabled } : order)));
    },
    [marketOrders, updateMarketOrders],
  );

  const deleteMarketOrder = useCallback(
    async (id: string) => {
      await updateMarketOrders(marketOrders.filter((order) => order.id !== id));
    },
    [marketOrders, updateMarketOrders],
  );

  const fillReservedMarketOrder = useCallback(async (id: string) => {
    setMarketOrders(await fillVirtualOrder(id));
  }, []);

  const releaseReservedMarketOrder = useCallback(async (id: string) => {
    setMarketOrders(await releaseVirtualOrder(id));
  }, []);

  const addShipStockOrder = useCallback(async () => {
    if (!quartermasterShipCommissioningEnabled) {
      setStatus("Ship stock commissioning is disabled for now.");
      return;
    }
    const stationId = shipDraft.stationId.trim() || selectedStation.trim();
    const shipClassId = shipDraft.shipClassId.trim();
    const quantity = Number(shipDraft.target);
    const priority = shipDraft.priority.trim() ? Number(shipDraft.priority) : 1;
    if (!shipClassId || stationId === allStationsValue || !Number.isFinite(quantity) || quantity <= 0) {
      setStatus("Ship stock target needs a concrete station, ship class, and positive target.");
      return;
    }
    if (!Number.isFinite(priority) || priority <= 0) {
      setStatus("Ship stock priority must be a positive number.");
      return;
    }
    await updateOrders([
      ...orders,
      {
        id: newVirtualCraftOrderId("commission_until"),
        action: "commission_until",
        recipeId: shipClassId,
        itemId: shipClassId,
        stationId,
        quantity: Math.floor(quantity),
        reserved: 0,
        filled: 0,
        enabled: true,
        priority,
        facilityId: null,
        preset: null,
        doForever: true,
      },
    ]);
    setShipDraft((current) => ({
      ...current,
      shipClassId: "",
      stationId: selectedStation === allStationsValue ? current.stationId : selectedStation,
      target: "",
      priority: "1",
    }));
    setStatus(null);
  }, [orders, selectedStation, shipDraft, updateOrders]);

  const addOrder = useCallback(async () => {
    const stationId = draft.stationId.trim() || selectedStation.trim();
    const priority = draft.priority.trim() ? Number(draft.priority) : 1;
    if (!Number.isFinite(priority) || priority <= 0) {
      setStatus("Priority must be a positive number.");
      return;
    }
    const recipeId = draft.recipeId.trim();
    const recipe: RecipeCatalogEntry | undefined = recipeById.get(recipeId);
    const itemId = draft.itemId.trim() || recipe?.outputs[0]?.itemId || recipe?.outputs[0]?.id || "";
    const quantity = Number(draft.quantity);
    const facilityId = draft.facilityId.trim();
    const preset = draft.preset.trim();
    if (!recipeId || stationId === allStationsValue || !Number.isFinite(quantity) || quantity <= 0) {
      setStatus("Virtual craft needs a concrete station, recipe, and positive quantity.");
      return;
    }
    if (recipe) {
      const usableFacilities = selectedRecipeFacilities?.usableFacilities ?? [];
      const stationFacilities = usableFacilities.filter((facility) => facility.locationId === stationId);
      const facilityCandidates = stationFacilities.length > 0 ? stationFacilities : usableFacilities;
      if (facilityId) {
        const selectedFacility = facilityCandidates.find((facility) => (facility.targetId || facility.id) === facilityId);
        if (!selectedFacility) {
          setStatus("Selected facility does not support that recipe at this station.");
          return;
        }
      }
    }
    await updateOrders([
      ...orders,
      {
        id: newVirtualCraftOrderId(draft.action),
        action: draft.action,
        recipeId,
        itemId,
        stationId,
        quantity: Math.floor(quantity),
        reserved: 0,
        filled: 0,
        enabled: true,
        priority,
        facilityId: facilityId || null,
        preset: preset || null,
        doForever: draft.action === "craft_until" ? draft.doForever : false,
      },
    ]);
    setDraft((current) => ({
      ...current,
      recipeId: "",
      itemId: "",
      stationId: selectedStation === allStationsValue ? current.stationId : selectedStation,
      quantity: "",
      facilityId: "",
      preset: "",
    }));
    setStatus(null);
  }, [draft, orders, recipeById, selectedRecipeFacilities, selectedStation, updateOrders]);

  const toggleOrder = useCallback(
    async (id: string) => {
      await updateOrders(orders.map((order) => (order.id === id ? { ...order, enabled: !order.enabled } : order)));
    },
    [orders, updateOrders],
  );

  const deleteOrder = useCallback(
    async (id: string) => {
      await updateOrders(orders.filter((order) => order.id !== id));
    },
    [orders, updateOrders],
  );

  const fillReserved = useCallback(async (id: string) => {
    setOrders(await fillVirtualCraftOrder(id));
  }, []);

  const releaseReserved = useCallback(async (id: string) => {
    setOrders(await releaseVirtualCraftOrder(id));
  }, []);

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
          <div className="crafting-title">Quartermaster</div>
          <div className="crafting-meta">
            {[marketOrders.filter((order) => order.enabled).length, orders.filter((order) => order.enabled).length].reduce((sum, value) => sum + value, 0)}{" "}
            enabled orders
          </div>
        </div>
        <label className="crafting-field quartermaster-station-field">
          <span>Station</span>
          <input
            list="quartermaster-stations"
            value={selectedStation}
            onChange={(event) => setSelectedStation(event.target.value.trim() || allStationsValue)}
            placeholder="all"
          />
        </label>
        <datalist id="quartermaster-stations">
          {stationOptions.map((stationId) => (
            <option key={stationId} value={stationId} />
          ))}
        </datalist>
        <button className="session-btn" onClick={() => void refresh()} disabled={loading}>
          refresh
        </button>
      </div>

      {status && <div className="crafting-status">{status}</div>}

      <div className="quartermaster-subtabs">
        <div className="crafting-tabs" role="tablist" aria-label="Quartermaster sections">
          <button type="button" role="tab" data-active={activeTab === "market"} aria-selected={activeTab === "market"} onClick={() => setActiveTab("market")}>
            Buy/Sell
          </button>
          <button type="button" role="tab" data-active={activeTab === "ships"} aria-selected={activeTab === "ships"} onClick={() => setActiveTab("ships")}>
            Ship Stock
          </button>
          <button type="button" role="tab" data-active={activeTab === "craft"} aria-selected={activeTab === "craft"} onClick={() => setActiveTab("craft")}>
            Craft Orders
          </button>
        </div>
        <div className="crafting-meta">
          {activeTab === "market"
            ? `${visibleMarketOrders.length.toLocaleString()} buy/sell orders`
            : activeTab === "ships"
              ? `${visibleShipOrders.length.toLocaleString()} ship targets`
              : `${visibleCraftOrders.length.toLocaleString()} craft policies`}
        </div>
      </div>

      <div className="crafting-recipes-wrap">
        {activeTab === "market" && (
          <section className="crafting-quartermaster-card">
            <div className="crafting-quartermaster-form">
              <div>
                <div className="crafting-detail-title">Virtual Buy/Sell</div>
                <div className="crafting-meta">{visibleMarketOrders.length.toLocaleString()} shown</div>
              </div>
              <select
                aria-label="Virtual buy sell mode"
                value={marketDraft.side}
                onChange={(event) => {
                  const side = event.target.value as VirtualOrderSide;
                  setMarketDraft({ ...marketDraft, side, doForever: isUntilSide(side) ? marketDraft.doForever : false });
                }}
              >
                <option value="buy">buy</option>
                <option value="buy_until">buy until</option>
                <option value="sell">sell</option>
                <option value="sell_until">sell until</option>
              </select>
              <label className="crafting-field">
                <span>Item</span>
                <input value={marketDraft.itemId} onChange={(event) => setMarketDraft({ ...marketDraft, itemId: event.target.value })} placeholder="item" />
              </label>
              <label className="crafting-field quartermaster-station-field">
                <span>Station</span>
                <input
                  list="quartermaster-stations"
                  value={marketDraft.stationId}
                  onChange={(event) => setMarketDraft({ ...marketDraft, stationId: event.target.value })}
                  placeholder="station"
                />
              </label>
              <label className="crafting-field crafting-field--number">
                <span>Price</span>
                <input
                  value={marketDraft.priceEach}
                  onChange={(event) => setMarketDraft({ ...marketDraft, priceEach: event.target.value })}
                  inputMode="numeric"
                  placeholder="price"
                />
              </label>
              <label className="crafting-field crafting-field--number">
                <span>{isUntilSide(marketDraft.side) ? "Target" : "Quantity"}</span>
                <input
                  value={marketDraft.quantity}
                  onChange={(event) => setMarketDraft({ ...marketDraft, quantity: event.target.value })}
                  inputMode="numeric"
                  placeholder={isUntilSide(marketDraft.side) ? "target" : "qty"}
                />
              </label>
              {marketDraft.side === "sell_until" && (
                <label className="crafting-field crafting-field--number">
                  <span>Tip</span>
                  <input
                    value={marketDraft.tippingPoint}
                    onChange={(event) => setMarketDraft({ ...marketDraft, tippingPoint: event.target.value })}
                    inputMode="numeric"
                    placeholder="optional"
                  />
                </label>
              )}
              <label className="crafting-field crafting-field--number">
                <span>Priority</span>
                <input
                  value={marketDraft.priority}
                  onChange={(event) => setMarketDraft({ ...marketDraft, priority: event.target.value })}
                  inputMode="decimal"
                  placeholder="priority"
                />
              </label>
              <label className="crafting-toggle">
                <input
                  type="checkbox"
                  checked={marketDraft.internalOnly}
                  onChange={(event) => setMarketDraft({ ...marketDraft, internalOnly: event.target.checked })}
                />
                internal only
              </label>
              {isUntilSide(marketDraft.side) && (
                <label className="crafting-toggle">
                  <input
                    type="checkbox"
                    checked={marketDraft.doForever}
                    onChange={(event) => setMarketDraft({ ...marketDraft, doForever: event.target.checked })}
                  />
                  do forever
                </label>
              )}
              <button className="session-btn" type="button" onClick={() => void addMarketOrder()}>
                add
              </button>
            </div>
            <div className="crafting-quartermaster-table-wrap">
              <table className="economy-table economy-virtual-table crafting-quartermaster-table">
                <thead>
                  <tr>
                    <th>mode</th>
                    <th>scope</th>
                    <th>item</th>
                    <th>station</th>
                    <th>price</th>
                    <th>qty/target</th>
                    <th>tip</th>
                    <th>priority</th>
                    <th>forever</th>
                    <th>remaining</th>
                    <th>reserved</th>
                    <th>filled</th>
                    <th>state</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {visibleMarketOrders.map((order) => (
                    <tr key={order.id} data-disabled={!order.enabled}>
                      <td>{virtualOrderSideLabel(order.side)}</td>
                      <td>{order.internalOnly ? "internal" : "market"}</td>
                      <td>{order.itemId}</td>
                      <td>{order.stationId}</td>
                      <td>{formatQty(order.priceEach)}</td>
                      <td>{formatQty(order.quantity)}</td>
                      <td>{order.side === "sell_until" && order.tippingPoint ? `${formatQty(order.tippingPoint)}${order.dumping ? " dumping" : ""}` : "-"}</td>
                      <td>{formatQty(order.priority ?? 1)}</td>
                      <td>{isUntilSide(order.side) && order.doForever ? "yes" : "-"}</td>
                      <td>{virtualMarketOpenLabel(order)}</td>
                      <td>{formatQty(order.reserved ?? 0)}</td>
                      <td>{formatQty(order.filled ?? 0)}</td>
                      <td>
                        <label className="economy-checkbox economy-virtual-toggle">
                          <input type="checkbox" checked={order.enabled} onChange={() => void toggleMarketOrder(order.id)} />
                          {order.enabled ? "enabled" : "off"}
                        </label>
                      </td>
                      <td>
                        <div className="economy-row-actions">
                          <button
                            className="session-btn"
                            type="button"
                            onClick={() => void fillReservedMarketOrder(order.id)}
                            disabled={(order.reserved ?? 0) <= 0}
                          >
                            fill
                          </button>
                          <button
                            className="session-btn"
                            type="button"
                            onClick={() => void releaseReservedMarketOrder(order.id)}
                            disabled={(order.reserved ?? 0) <= 0}
                          >
                            release
                          </button>
                          <button className="session-btn" type="button" onClick={() => void deleteMarketOrder(order.id)}>
                            delete
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                  {visibleMarketOrders.length === 0 && (
                    <tr>
                      <td colSpan={14} className="crafting-empty">
                        No virtual buy/sell orders.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>
        )}

        {activeTab === "ships" && (
          <section className="crafting-quartermaster-card">
            <div className="crafting-quartermaster-form">
              <div>
                <div className="crafting-detail-title">Ship Stock Targets</div>
                <div className="crafting-meta">
                  {quartermasterShipCommissioningEnabled ? `${visibleShipOrders.length.toLocaleString()} shown` : "commissioning paused"}
                </div>
              </div>
              <label className="crafting-field">
                <span>Ship</span>
                <input
                  list="quartermaster-ships"
                  value={shipDraft.shipClassId}
                  onChange={(event) => setShipDraft({ ...shipDraft, shipClassId: event.target.value })}
                  placeholder="ship class"
                  disabled={!quartermasterShipCommissioningEnabled}
                />
              </label>
              <datalist id="quartermaster-ships">
                {ships.map((ship) => (
                  <option key={ship.id} value={ship.id}>
                    {ship.name || ship.className || ship.id}
                  </option>
                ))}
              </datalist>
              <label className="crafting-field quartermaster-station-field">
                <span>Station</span>
                <input
                  list="quartermaster-stations"
                  value={shipDraft.stationId}
                  onChange={(event) => setShipDraft({ ...shipDraft, stationId: event.target.value })}
                  placeholder="station"
                  disabled={!quartermasterShipCommissioningEnabled}
                />
              </label>
              <label className="crafting-field crafting-field--number">
                <span>Target</span>
                <input
                  value={shipDraft.target}
                  onChange={(event) => setShipDraft({ ...shipDraft, target: event.target.value })}
                  inputMode="numeric"
                  placeholder="ships"
                  disabled={!quartermasterShipCommissioningEnabled}
                />
              </label>
              <label className="crafting-field crafting-field--number">
                <span>Priority</span>
                <input
                  value={shipDraft.priority}
                  onChange={(event) => setShipDraft({ ...shipDraft, priority: event.target.value })}
                  inputMode="decimal"
                  placeholder="priority"
                  disabled={!quartermasterShipCommissioningEnabled}
                />
              </label>
              <button className="session-btn" type="button" onClick={() => void addShipStockOrder()} disabled={!quartermasterShipCommissioningEnabled}>
                add
              </button>
            </div>
            <div className="crafting-quartermaster-table-wrap">
              <table className="economy-table economy-virtual-table crafting-quartermaster-table">
                <thead>
                  <tr>
                    <th>ship</th>
                    <th>station</th>
                    <th>target</th>
                    <th>priority</th>
                    <th>remaining</th>
                    <th>reserved</th>
                    <th>state</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {visibleShipOrders.map((order) => {
                    const ship = shipById.get(order.recipeId) ?? shipById.get(order.itemId ?? "");
                    return (
                      <tr key={order.id} data-disabled={!order.enabled}>
                        <td>
                          <div>{ship?.name || ship?.className || order.recipeId}</div>
                          <div className="crafting-recipe-id">{order.recipeId}</div>
                        </td>
                        <td>{order.stationId}</td>
                        <td>{formatQty(order.quantity)}</td>
                        <td>{formatQty(order.priority ?? 1)}</td>
                        <td>{virtualCraftOpenLabel(order)}</td>
                        <td>{formatQty(order.reserved ?? 0)}</td>
                        <td>
                          <label className="economy-checkbox economy-virtual-toggle">
                            <input type="checkbox" checked={order.enabled} onChange={() => void toggleOrder(order.id)} />
                            {order.enabled ? "enabled" : "off"}
                          </label>
                        </td>
                        <td>
                          <div className="economy-row-actions">
                            <button className="session-btn" type="button" onClick={() => void releaseReserved(order.id)} disabled={(order.reserved ?? 0) <= 0}>
                              release
                            </button>
                            <button className="session-btn" type="button" onClick={() => void deleteOrder(order.id)}>
                              delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                  {visibleShipOrders.length === 0 && (
                    <tr>
                      <td colSpan={8} className="crafting-empty">
                        No ship stock targets.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>
        )}

        {activeTab === "craft" && (
          <section className="crafting-quartermaster-card">
            <div className="crafting-quartermaster-form">
              <div>
                <div className="crafting-detail-title">Virtual Craft Orders</div>
                <div className="crafting-meta">{visibleCraftOrders.length.toLocaleString()} shown</div>
              </div>
              <select
                aria-label="Quartermaster action"
                value={draft.action}
                onChange={(event) => {
                  const action = event.target.value as QuartermasterAction;
                  setDraft({ ...draft, action, doForever: action === "craft_until" || action === "commission_until" ? draft.doForever : false });
                }}
              >
                <option value="craft">craft</option>
                <option value="craft_until">craft until</option>
              </select>
              {
                <>
                  <label className="crafting-field">
                    <span>Recipe</span>
                    <input
                      list="quartermaster-recipes"
                      value={draft.recipeId}
                      onChange={(event) => {
                        const recipeId = event.target.value;
                        const recipe = recipeById.get(recipeId);
                        const itemId = draft.itemId || recipe?.outputs[0]?.itemId || recipe?.outputs[0]?.id || "";
                        setDraft({ ...draft, recipeId, itemId, facilityId: "" });
                      }}
                      placeholder="recipe"
                    />
                  </label>
                  <datalist id="quartermaster-recipes">
                    {recipes.map((recipe) => (
                      <option key={recipe.id} value={recipe.id}>
                        {recipe.name}
                      </option>
                    ))}
                  </datalist>
                  <label className="crafting-field">
                    <span>Output</span>
                    <input value={draft.itemId} onChange={(event) => setDraft({ ...draft, itemId: event.target.value })} placeholder="output item" />
                  </label>
                  <label className="crafting-field quartermaster-station-field">
                    <span>Station</span>
                    <input
                      list="quartermaster-stations"
                      value={draft.stationId}
                      onChange={(event) => setDraft({ ...draft, stationId: event.target.value, facilityId: "" })}
                      placeholder="station"
                    />
                  </label>
                  <label className="crafting-field crafting-field--number">
                    <span>{draft.action === "craft_until" ? "Target" : "Crafts"}</span>
                    <input
                      value={draft.quantity}
                      onChange={(event) => setDraft({ ...draft, quantity: event.target.value })}
                      inputMode="numeric"
                      placeholder={draft.action === "craft_until" ? "target" : "crafts"}
                    />
                  </label>
                  <label className="crafting-field">
                    <span>Facility</span>
                    <input
                      list="quartermaster-facilities"
                      value={draft.facilityId}
                      onChange={(event) => setDraft({ ...draft, facilityId: event.target.value })}
                      placeholder="optional"
                    />
                  </label>
                  <datalist id="quartermaster-facilities">
                    {facilityOptions.map((facilityId) => (
                      <option key={facilityId} value={facilityId} />
                    ))}
                  </datalist>
                  <label className="crafting-field">
                    <span>Preset</span>
                    <input value={draft.preset} onChange={(event) => setDraft({ ...draft, preset: event.target.value })} placeholder="optional" />
                  </label>
                  {(draft.action === "craft_until" || draft.action === "commission_until") && (
                    <label className="crafting-toggle">
                      <input type="checkbox" checked={draft.doForever} onChange={(event) => setDraft({ ...draft, doForever: event.target.checked })} />
                      do forever
                    </label>
                  )}
                </>
              }
              <label className="crafting-field crafting-field--number">
                <span>Priority</span>
                <input
                  value={draft.priority}
                  onChange={(event) => setDraft({ ...draft, priority: event.target.value })}
                  inputMode="decimal"
                  placeholder="priority"
                />
              </label>
              <button className="session-btn" type="button" onClick={() => void addOrder()}>
                add
              </button>
            </div>
            <div className="crafting-quartermaster-table-wrap">
              <table className="economy-table economy-virtual-table crafting-quartermaster-table">
                <thead>
                  <tr>
                    <th>mode</th>
                    <th>target</th>
                    <th>station</th>
                    <th>output</th>
                    <th>qty/floor</th>
                    <th>priority</th>
                    <th>forever</th>
                    <th>facility</th>
                    <th>preset</th>
                    <th>remaining</th>
                    <th>reserved</th>
                    <th>filled</th>
                    <th>state</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {visibleCraftOrders.map((order) => {
                    const isCreditFloor = order.action === "credit_floor";
                    return (
                      <tr key={order.id} data-disabled={!order.enabled}>
                        <td>{quartermasterActionLabel(order.action)}</td>
                        <td>
                          {isCreditFloor ? (
                            <>
                              <div>{order.squadId ?? "-"}</div>
                              <div className="crafting-recipe-id">{(order.sessionHandles ?? []).join(", ") || "no sessions"}</div>
                            </>
                          ) : (
                            <>
                              <div>{recipeById.get(order.recipeId)?.name ?? order.recipeId}</div>
                              <div className="crafting-recipe-id">{order.recipeId}</div>
                            </>
                          )}
                        </td>
                        <td>{order.stationId || allStationsValue}</td>
                        <td>{order.itemId || "-"}</td>
                        <td>{isCreditFloor ? <CreditAmount value={order.creditFloor ?? order.quantity} /> : formatQty(order.quantity)}</td>
                        <td>{formatQty(order.priority ?? 1)}</td>
                        <td>{(order.action === "craft_until" || order.action === "commission_until") && order.doForever ? "yes" : "-"}</td>
                        <td>{order.facilityId || "-"}</td>
                        <td>{order.preset || "-"}</td>
                        <td>{virtualCraftOpenLabel(order)}</td>
                        <td>{formatQty(order.reserved ?? 0)}</td>
                        <td>{formatQty(order.filled ?? 0)}</td>
                        <td>
                          <label className="economy-checkbox economy-virtual-toggle">
                            <input type="checkbox" checked={order.enabled} onChange={() => void toggleOrder(order.id)} />
                            {order.enabled ? "enabled" : "off"}
                          </label>
                        </td>
                        <td>
                          <div className="economy-row-actions">
                            <button
                              className="session-btn"
                              type="button"
                              onClick={() => void fillReserved(order.id)}
                              disabled={isCreditFloor || (order.reserved ?? 0) <= 0}
                            >
                              fill
                            </button>
                            <button
                              className="session-btn"
                              type="button"
                              onClick={() => void releaseReserved(order.id)}
                              disabled={isCreditFloor || (order.reserved ?? 0) <= 0}
                            >
                              release
                            </button>
                            <button className="session-btn" type="button" onClick={() => void deleteOrder(order.id)}>
                              delete
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                  {visibleCraftOrders.length === 0 && (
                    <tr>
                      <td colSpan={14} className="crafting-empty">
                        No virtual craft orders.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
