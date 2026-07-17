import { fetchWithTimeout } from "./http.js";
import { decodeArray, decodeResponse, isRecord, isStringArray } from "./decoding.js";
import type { VirtualFactionOrderInput, VirtualCraftOrderInput } from "./types.js";

function optionalNumber(value: unknown): value is number | null | undefined {
  return value === undefined || value === null || (typeof value === "number" && Number.isFinite(value));
}

function optionalBoolean(value: unknown): value is boolean | undefined {
  return value === undefined || typeof value === "boolean";
}

function decodeFactionOrderSide(value: unknown): VirtualFactionOrderInput["side"] | null {
  switch (value) {
    case "buy":
    case "sell":
    case "buy_until":
    case "sell_until":
      return value;
    default:
      return null;
  }
}

function decodeCraftOrderAction(value: unknown): VirtualCraftOrderInput["action"] | null {
  switch (value) {
    case "craft":
    case "craft_until":
    case "commission_until":
    case "credit_floor":
      return value;
    default:
      return null;
  }
}

function decodeFactionOrder(value: unknown): VirtualFactionOrderInput | null {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.itemId !== "string" ||
    typeof value.stationId !== "string" ||
    typeof value.priceEach !== "number" ||
    typeof value.quantity !== "number" ||
    typeof value.enabled !== "boolean" ||
    !optionalNumber(value.tippingPoint) ||
    !optionalNumber(value.reserved) ||
    !optionalNumber(value.filled) ||
    !optionalNumber(value.priority) ||
    !optionalBoolean(value.dumping) ||
    !optionalBoolean(value.internalOnly) ||
    !optionalBoolean(value.doForever)
  )
    return null;
  const side = decodeFactionOrderSide(value.side);
  if (!side) return null;
  return {
    id: value.id,
    side,
    itemId: value.itemId,
    stationId: value.stationId,
    priceEach: value.priceEach,
    quantity: value.quantity,
    enabled: value.enabled,
    ...(value.tippingPoint !== undefined ? { tippingPoint: value.tippingPoint } : {}),
    ...(value.dumping !== undefined ? { dumping: value.dumping } : {}),
    ...(value.internalOnly !== undefined ? { internalOnly: value.internalOnly } : {}),
    ...(value.reserved !== undefined ? { reserved: value.reserved ?? 0 } : {}),
    ...(value.filled !== undefined ? { filled: value.filled ?? 0 } : {}),
    ...(value.priority !== undefined ? { priority: value.priority ?? 0 } : {}),
    ...(value.doForever !== undefined ? { doForever: value.doForever } : {}),
  };
}

function decodeCraftOrder(value: unknown): VirtualCraftOrderInput | null {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.recipeId !== "string" ||
    typeof value.stationId !== "string" ||
    typeof value.quantity !== "number" ||
    typeof value.enabled !== "boolean" ||
    (value.itemId !== undefined && typeof value.itemId !== "string") ||
    !optionalNumber(value.reserved) ||
    !optionalNumber(value.filled) ||
    !optionalNumber(value.priority) ||
    !optionalNumber(value.creditFloor) ||
    !optionalBoolean(value.doForever) ||
    (value.facilityId !== undefined && value.facilityId !== null && typeof value.facilityId !== "string") ||
    (value.preset !== undefined && value.preset !== null && typeof value.preset !== "string") ||
    (value.squadId !== undefined && value.squadId !== null && typeof value.squadId !== "string") ||
    (value.sessionHandles !== undefined && !isStringArray(value.sessionHandles))
  )
    return null;
  const action = decodeCraftOrderAction(value.action);
  if (!action) return null;
  return {
    id: value.id,
    action,
    recipeId: value.recipeId,
    stationId: value.stationId,
    quantity: value.quantity,
    enabled: value.enabled,
    ...(typeof value.itemId === "string" ? { itemId: value.itemId } : {}),
    ...(value.facilityId !== undefined ? { facilityId: value.facilityId } : {}),
    ...(value.preset !== undefined ? { preset: value.preset } : {}),
    ...(value.squadId !== undefined ? { squadId: value.squadId } : {}),
    ...(value.sessionHandles !== undefined ? { sessionHandles: value.sessionHandles } : {}),
    ...(value.creditFloor !== undefined ? { creditFloor: value.creditFloor } : {}),
    ...(value.reserved !== undefined ? { reserved: value.reserved ?? 0 } : {}),
    ...(value.filled !== undefined ? { filled: value.filled ?? 0 } : {}),
    ...(value.priority !== undefined ? { priority: value.priority ?? 0 } : {}),
    ...(value.doForever !== undefined ? { doForever: value.doForever } : {}),
  };
}

function decodeOrders<T>(value: unknown, decode: (entry: unknown) => T | null): T[] | null {
  return isRecord(value) ? decodeArray(value.orders ?? [], decode) : null;
}

async function requestFactionOrders(url: string, init?: RequestInit): Promise<VirtualFactionOrderInput[]> {
  return decodeResponse(await fetchWithTimeout(url, 5_000, init), (value) => decodeOrders(value, decodeFactionOrder));
}

async function requestCraftOrders(url: string, init?: RequestInit): Promise<VirtualCraftOrderInput[]> {
  return decodeResponse(await fetchWithTimeout(url, 5_000, init), (value) => decodeOrders(value, decodeCraftOrder));
}

const jsonPut = (body: unknown): RequestInit => ({ method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
const jsonPost = (body: unknown): RequestInit => ({ method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });

export const fetchVirtualOrders = () => requestFactionOrders("/api/virtual-orders");
export const saveVirtualOrders = (orders: VirtualFactionOrderInput[]) => requestFactionOrders("/api/virtual-orders", jsonPut({ orders }));
export const reserveVirtualOrders = (uses: Array<{ orderId: string; quantity: number }>) =>
  requestFactionOrders("/api/virtual-orders/reserve", jsonPost({ uses }));
export const fillVirtualOrder = (id: string) => requestFactionOrders(`/api/virtual-orders/${encodeURIComponent(id)}/fill`, { method: "POST" });
export const releaseVirtualOrder = (id: string) => requestFactionOrders(`/api/virtual-orders/${encodeURIComponent(id)}/release`, { method: "POST" });
export const fetchVirtualCraftOrders = () => requestCraftOrders("/api/virtual-craft-orders");
export const saveVirtualCraftOrders = (orders: VirtualCraftOrderInput[]) => requestCraftOrders("/api/virtual-craft-orders", jsonPut({ orders }));
export const reserveVirtualCraftOrders = (uses: Array<{ orderId: string; quantity: number }>) =>
  requestCraftOrders("/api/virtual-craft-orders/reserve", jsonPost({ uses }));
export const fillVirtualCraftOrder = (id: string) => requestCraftOrders(`/api/virtual-craft-orders/${encodeURIComponent(id)}/fill`, { method: "POST" });
export const releaseVirtualCraftOrder = (id: string) => requestCraftOrders(`/api/virtual-craft-orders/${encodeURIComponent(id)}/release`, { method: "POST" });
