import { useMemo, useState } from "react";
import type { EconomyMarketData } from "./api.js";
import type { SessionState } from "./SessionsPanel.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { selectEconomyMarket } from "./prayer/worldSelectors.js";

type Side = "ask" | "bid";
type Row = { key: string; itemId: string; stationId: string; stationLabel: string; priceEach: number; quantity: number; myQuantity: number | null; observedAtUnix: number | null };
const limit = 500;
const number = (value: string) => value.trim() && Number.isFinite(Number(value)) ? Number(value) : null;
const age = (value: number | null) => value ? `${Math.max(0, Math.floor((Date.now() / 1000 - value) / 60))}m` : "unknown";
function marketRows(data: EconomyMarketData): Record<Side, Row[]> {
  const rows: Record<Side, Row[]> = { ask: [], bid: [] };
  for (const [stationKey, market] of Object.entries(data.marketsByStation)) {
    const stationId = market.stationId || stationKey; const stationLabel = market.stationName || market.poiId || stationId;
    for (const [itemId, orders] of Object.entries(market.sellOrders)) orders.forEach((order, index) => rows.ask.push({ key: `ask:${stationId}:${itemId}:${order.price_each}:${order.quantity}:${index}`, itemId, stationId, stationLabel, priceEach: order.price_each, quantity: order.quantity, myQuantity: order.my_quantity ?? null, observedAtUnix: market.observedAtUnix }));
    for (const [itemId, orders] of Object.entries(market.buyOrders)) orders.forEach((order, index) => rows.bid.push({ key: `bid:${stationId}:${itemId}:${order.price_each}:${order.quantity}:${index}`, itemId, stationId, stationLabel, priceEach: order.price_each, quantity: order.quantity, myQuantity: order.my_quantity ?? null, observedAtUnix: market.observedAtUnix }));
  }
  rows.ask.sort((a, b) => a.itemId.localeCompare(b.itemId) || a.priceEach - b.priceEach || a.stationId.localeCompare(b.stationId) || b.quantity - a.quantity);
  rows.bid.sort((a, b) => a.itemId.localeCompare(b.itemId) || b.priceEach - a.priceEach || a.stationId.localeCompare(b.stationId) || b.quantity - a.quantity);
  return rows;
}
function Orders({ rows, side, total }: { rows: Row[]; side: Side; total: number }) {
  return <section className={`economy-order-pane economy-order-pane--${side}`}><div className="economy-order-pane-head"><div className="economy-card-title">{side === "ask" ? "Asks" : "Buys"}</div><div className="economy-order-count">{rows.length < total ? `${rows.length.toLocaleString()} of ${total.toLocaleString()} orders` : `${total.toLocaleString()} orders`}</div></div><div className="economy-table-wrap"><table className="economy-table"><thead><tr><th>item</th><th>station</th><th>price</th><th>qty</th><th>age</th></tr></thead><tbody>{rows.map((row) => <tr key={row.key}><td>{row.itemId}</td><td title={row.stationId}>{row.stationLabel}{row.myQuantity !== null && row.myQuantity > 0 && <div className="economy-subtle">own {row.myQuantity.toLocaleString()}</div>}</td><td>{row.priceEach.toLocaleString()}</td><td>{row.quantity.toLocaleString()}</td><td>{age(row.observedAtUnix)}</td></tr>)}{!rows.length && <tr><td colSpan={5} className="economy-empty">No matching {side === "ask" ? "ask" : "buy"} orders.</td></tr>}</tbody></table></div></section>;
}
export default function EconomyPanel({ sessions }: { sessions: SessionState[] }) {
  const prayer = usePrayer(); const market = useMemo(() => selectEconomyMarket(prayer.stationMarkets, prayer.galaxyMap), [prayer.galaxyMap, prayer.stationMarkets]);
  const loadingMarket = prayer.connection === "connecting";
  const [query, setQuery] = useState(""); const [station, setStation] = useState(""); const [minPrice, setMinPrice] = useState(""); const [maxPrice, setMaxPrice] = useState(""); const [minQuantity, setMinQuantity] = useState("");
  const stations = useMemo(() => Object.values(market.marketsByStation).map((entry) => ({ id: entry.stationId, label: entry.stationName || entry.poiId || entry.stationId })).sort((a, b) => a.label.localeCompare(b.label)), [market]);
  const rows = useMemo(() => { const all = marketRows(market); const needle = query.trim().toLowerCase(); const low = number(minPrice); const high = number(maxPrice); const qty = number(minQuantity); const matches = (row: Row) => (!needle || row.itemId.toLowerCase().includes(needle) || row.stationLabel.toLowerCase().includes(needle)) && (!station || row.stationId === station) && (low === null || row.priceEach >= low) && (high === null || row.priceEach <= high) && (qty === null || row.quantity >= qty); const asks = all.ask.filter(matches); const bids = all.bid.filter(matches); return { ask: asks.slice(0, limit), bid: bids.slice(0, limit), askTotal: asks.length, bidTotal: bids.length }; }, [market, query, station, minPrice, maxPrice, minQuantity]);
  if (!sessions.length) return <div className="economy-panel"><div className="economy-empty">No registered sessions.</div></div>;
  return <div className="economy-panel"><div className="economy-toolbar"><div><div className="economy-title">Economy</div><div className="economy-meta">Shared market memory</div></div></div><div className="economy-body"><section className="economy-card economy-market-card"><div className="economy-market-controls"><div className="economy-card-title">Market search</div><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="item or station"/><select value={station} onChange={(event) => setStation(event.target.value)}><option value="">all stations</option>{stations.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select><input className="economy-number-filter" value={minPrice} onChange={(event) => setMinPrice(event.target.value)} inputMode="numeric" placeholder="min price"/><input className="economy-number-filter" value={maxPrice} onChange={(event) => setMaxPrice(event.target.value)} inputMode="numeric" placeholder="max price"/><input className="economy-number-filter" value={minQuantity} onChange={(event) => setMinQuantity(event.target.value)} inputMode="numeric" placeholder="min qty"/><button className="session-btn" onClick={() => void prayer.refresh()} disabled={loadingMarket}>refresh</button></div><div className="economy-order-books">{!loadingMarket && rows.askTotal === 0 && rows.bidTotal === 0 ? <div className="economy-empty">No matching market observations.</div> : <><Orders rows={rows.ask} side="ask" total={rows.askTotal}/><Orders rows={rows.bid} side="bid" total={rows.bidTotal}/></>}</div></section></div></div>;
}
