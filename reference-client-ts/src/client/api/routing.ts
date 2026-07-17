import type { AuthoritativeRoute, RouteRequest } from "@prayer/sdk";
import { fetchWithTimeout } from "./http.js";
import { isRecord } from "./decoding.js";

function decodeRoute(value: unknown): AuthoritativeRoute | null {
  if (
    !isRecord(value) ||
    typeof value.cost !== "number" ||
    typeof value.from !== "string" ||
    typeof value.fromSystem !== "string" ||
    !Array.isArray(value.hops) ||
    !value.hops.every((hop) => typeof hop === "string") ||
    typeof value.safe !== "boolean" ||
    typeof value.to !== "string" ||
    typeof value.toSystem !== "string" ||
    typeof value.totalJumps !== "number"
  )
    return null;
  return {
    cost: value.cost,
    from: value.from,
    fromSystem: value.fromSystem,
    hops: value.hops,
    safe: value.safe,
    to: value.to,
    toSystem: value.toSystem,
    totalJumps: value.totalJumps,
  };
}

export async function fetchRoutes(routes: readonly RouteRequest[], safe = true, signal?: AbortSignal): Promise<Array<AuthoritativeRoute | null>> {
  const response = await fetchWithTimeout("/api/routes", 5_000, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ routes, safe }),
    signal,
  });
  if (!response.ok) throw new Error(`Route lookup failed (${response.status})`);
  const body: unknown = await response.json();
  if (!isRecord(body)) throw new Error("Route lookup returned an invalid response");
  if (!Array.isArray(body.routes)) throw new Error("Route lookup returned invalid routes");
  const decodedRoutes: Array<AuthoritativeRoute | null> = [];
  for (const value of body.routes) {
    if (value === null) {
      decodedRoutes.push(null);
      continue;
    }
    const route = decodeRoute(value);
    if (!route) throw new Error("Route lookup returned invalid routes");
    decodedRoutes.push(route);
  }
  return decodedRoutes;
}
