import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import { Prayer } from "../dist/src/index.js";

test("Node client reads bounded bot resources", async () => {
  const entry = {
    id: "bot-1", username: "Test Bot", version: 7, observed_at: null, connection: "Connected",
    state: { location: { system_id: "sol", poi_id: "sol" } },
  };
  const server = createServer((request, response) => {
    response.setHeader("content-type", "application/json");
    if (request.url === "/api/v1/meta") {
      response.end(JSON.stringify({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: ["conditional_state"] }));
    } else if (request.url === "/api/v1/bots") {
      response.end(JSON.stringify([{ botId: entry.id, name: entry.username, connection: "connected", stateVersion: entry.version, observedAt: entry.observed_at }]));
    } else if (request.url === `/api/v1/bots/${entry.id}`) {
      response.end(JSON.stringify({ botId: entry.id, name: entry.username, connection: "connected", stateVersion: entry.version, observedAt: entry.observed_at }));
    } else if (request.url?.startsWith("/api/v1/state")) {
      response.end(JSON.stringify({
        versions: { fleet: entry.version, world: 1, map: 1, resources: 1, wildlife: 1, markets: 1, storage: 1, facilities: 1, observations: 1, communications: 1, factions: 1, catalog: "test" },
        fleet: { bots: { [entry.id]: entry } },
        world: { map: { systems: [], knownPois: [] }, resources: { systemsByResource: {}, poisByResource: {} }, wildlife: { systems: [], pois: [] }, stationMarkets: {}, storageByPlayer: {}, factionStorageByFactionPoi: {}, facilitiesByPoi: {}, ownedFacilitiesByPlayer: {}, ownedFacilitiesByFaction: {}, stationPassengers: {}, salvageByPoi: {}, agentSightings: {}, chatMessagesBySession: {}, factionBySession: {}, updatedAtUtc: "2026-01-01T00:00:00Z" },
        catalog: { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} },
      }));
    } else {
      response.statusCode = 404; response.end(JSON.stringify({ error: { code: "not_found", message: "missing", retryable: false }, requestId: "test" }));
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const address = server.address();
    assert.equal(typeof address, "object");
    const prayer = await Prayer.connect({ baseUrl: `http://127.0.0.1:${address.port}` });
    const bots = await prayer.bots();
    const snapshot = await (await prayer.bot(bots[0].botId)).state();
    assert.equal(snapshot.id, entry.id);
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
});
