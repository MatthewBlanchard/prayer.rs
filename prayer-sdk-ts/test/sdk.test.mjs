import assert from "node:assert/strict";
import test from "node:test";
import {
  LaneBusyError,
  Prayer,
  PrayerCompatibilityError,
  PrayerConnectionError,
  PrayerAbortError,
  PrayerTimeoutError,
  action,
  actions,
  dock,
  go,
  mine,
  undock,
  wait,
} from "../dist/src/index.js";

function response(body, status = 200) {
  return new Response(body === undefined ? undefined : JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

function stateVersions(fleet, world, catalog) {
  return { fleet, world, map: world, resources: world, exploration: world, wildlife: world, markets: world, storage: world, facilities: world, observations: world, communications: world, factions: world, catalog };
}

test("connect negotiates v1 and constructors emit exact wire actions", async () => {
  const fetch = async () => response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
  await Prayer.connect({ baseUrl: "http://test/", fetch });
  assert.deepEqual([undock(), go({ poi: "sol" }), dock()], [
    { type: "undock" },
    { type: "go", request: { destination: { kind: "poi", value: "sol" } } },
    { type: "dock" },
  ]);
  assert.deepEqual(go({ system: "alpha" }), { type: "go", request: { destination: { kind: "system", value: "alpha" } } });
  assert.deepEqual(go({ kind: "coordinate", x: 3, y: -2 }), { type: "go", request: { destination: { kind: "coordinate", x: 3, y: -2 } } });
  assert.deepEqual(wait(3), { type: "wait", request: { ticks: 3 } });
  assert.deepEqual(mine(), { type: "mine", request: { resource: null } });
  assert.deepEqual(mine("iron"), { type: "mine", request: { resource: "iron" } });
  assert.deepEqual(actions.scan(), { type: "scan", request: { target: null } });
  assert.deepEqual(actions.scan({ target: "ship-1" }), { type: "scan", request: { target: "ship-1" } });
  assert.deepEqual(actions.repair(), { type: "repair", request: { target: null, quantity: null, item: null } });
  assert.deepEqual(actions.refuel(), { type: "refuel", request: { target: null, quantity: null, item: null } });
  assert.deepEqual(actions.refuel({ target: "ship-1" }), { type: "refuel", request: { target: "ship-1", quantity: null, item: null } });
  assert.deepEqual(actions.refuel({ target: undefined }), { type: "refuel", request: { target: null, quantity: null, item: null } });
  assert.deepEqual(actions.distressSignal(), { type: "distress_signal", request: { distress_type: null } });
});

test("route and routes use the authoritative bulk routing endpoint", async () => {
  const bodies = [];
  const fetch = async (input, init = {}) => {
    if (String(input).endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    bodies.push(JSON.parse(init.body));
    return response({ routes: bodies.at(-1).routes.map(({ from, to }) => ({ from, fromSystem: "a", to, toSystem: "b", hops: ["b"], totalJumps: 1, cost: 2, safe: bodies.at(-1).safe })) });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  assert.equal((await prayer.route("station-a", "station-b", { safe: true }))?.cost, 2);
  assert.equal((await prayer.routes([{ from: "a", to: "b" }, { from: "b", to: "a" }], { safe: false })).length, 2);
  assert.deepEqual(bodies, [
    { routes: [{ from: "station-a", to: "station-b" }], safe: true },
    { routes: [{ from: "a", to: "b" }, { from: "b", to: "a" }], safe: false },
  ]);
});

test("state conditionally refreshes and retains unchanged cached domains", async () => {
  const urls = [];
  let stateCalls = 0;
  const fleet = { bots: { bot: { id: "bot", username: "miner", state: {}, version: 4, observed_at: null, connection: "Connected" } } };
  const world = { map: { systems: [], knownPois: [] }, resources: { systemsByResource: {}, poisByResource: {} }, exploration: { exploredSystems: [], visitedPois: [], surveyedSystems: [], miningCheckedPoisByResource: {}, miningExploredSystemsByResource: {}, blacklists: {} }, wildlife: { systems: [], pois: [] }, stationMarkets: {}, storageByPlayer: {}, factionStorageByFactionPoi: {}, facilitiesByPoi: {}, ownedFacilitiesByPlayer: {}, ownedFacilitiesByFaction: {}, stationPassengers: {}, salvageByPoi: {}, agentSightings: {}, chatMessagesBySession: {}, factionBySession: {}, updatedAtUtc: "2026-01-01T00:00:00Z" };
  const catalog = { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} };
  const fetch = async (input) => {
    const url = String(input); urls.push(url);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    stateCalls += 1;
    return response(stateCalls === 1
      ? { versions: stateVersions(4, 7, "1.2.3"), fleet, world, catalog }
      : { versions: stateVersions(4, 7, "1.2.3"), fleet: null, world: null, catalog: null });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const first = await prayer.state();
  const second = await prayer.state();
  assert.equal(second.fleet, first.fleet);
  assert.equal(second.world, first.world);
  assert.equal(second.catalog, first.catalog);
  assert.match(urls.at(-1), /fleet_version=4/);
  assert.match(urls.at(-1), /world_version=7/);
  assert.match(urls.at(-1), /markets_version=7/);
  assert.match(urls.at(-1), /catalog_version=1.2.3/);
});

test("state applies keyed market deltas with structural sharing", async () => {
  let stateCalls = 0;
  const unchangedMap = { systems: [], knownPois: [] };
  const marketA = { buy_orders: {}, sell_orders: {}, observed_at_unix: 1 };
  const marketB = { buy_orders: {}, sell_orders: {}, observed_at_unix: 2 };
  const baseWorld = { map: unchangedMap, resources: { systemsByResource: {}, poisByResource: {} }, exploration: { exploredSystems: [], visitedPois: [], surveyedSystems: [], miningCheckedPoisByResource: {}, miningExploredSystemsByResource: {}, blacklists: {} }, wildlife: { systems: [], pois: [] }, stationMarkets: { a: marketA, stable: marketA, removed: marketA }, storageByPlayer: {}, factionStorageByFactionPoi: {}, facilitiesByPoi: {}, ownedFacilitiesByPlayer: {}, ownedFacilitiesByFaction: {}, stationPassengers: {}, salvageByPoi: {}, agentSightings: {}, chatMessagesBySession: {}, factionBySession: {}, updatedAtUtc: "2026-01-01T00:00:00Z" };
  const fetch = async (input) => {
    if (String(input).endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    stateCalls += 1;
    return response(stateCalls === 1
      ? { versions: stateVersions(1, 1, "1"), fleet: { bots: {} }, world: baseWorld, catalog: { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} } }
      : { versions: stateVersions(1, 2, "1"), fleet: null, world: { stationMarkets: null, stationMarketDelta: { baseVersion: 1, upsert: { a: marketB, added: marketB }, remove: ["removed"] }, updatedAtUtc: "2026-01-01T00:00:01Z" }, catalog: null });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const first = await prayer.state();
  const second = await prayer.state();
  assert.notEqual(second.world, first.world);
  assert.notEqual(second.world.stationMarkets, first.world.stationMarkets);
  assert.equal(second.world.stationMarkets.stable, first.world.stationMarkets.stable);
  assert.deepEqual(second.world.stationMarkets.a, marketB);
  assert.deepEqual(second.world.stationMarkets.added, marketB);
  assert.equal(second.world.stationMarkets.removed, undefined);
  assert.deepEqual(first.world.stationMarkets.removed, marketA);
});

test("state recovers from a mismatched market delta with a full snapshot", async () => {
  let stateCalls = 0;
  const world = { map: { systems: [], knownPois: [] }, resources: { systemsByResource: {}, poisByResource: {} }, exploration: { exploredSystems: [], visitedPois: [], surveyedSystems: [], miningCheckedPoisByResource: {}, miningExploredSystemsByResource: {}, blacklists: {} }, wildlife: { systems: [], pois: [] }, stationMarkets: { a: { buy_orders: {}, sell_orders: {}, observed_at_unix: 1 } }, storageByPlayer: {}, factionStorageByFactionPoi: {}, facilitiesByPoi: {}, ownedFacilitiesByPlayer: {}, ownedFacilitiesByFaction: {}, stationPassengers: {}, salvageByPoi: {}, agentSightings: {}, chatMessagesBySession: {}, factionBySession: {}, updatedAtUtc: "2026-01-01T00:00:00Z" };
  const fetch = async (input) => {
    if (String(input).endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    stateCalls += 1;
    if (stateCalls === 1) return response({ versions: stateVersions(1, 1, "1"), fleet: { bots: {} }, world, catalog: { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} } });
    if (stateCalls === 2) return response({ versions: stateVersions(1, 2, "1"), fleet: null, world: { stationMarkets: null, stationMarketDelta: { baseVersion: 99, upsert: {}, remove: [] } }, catalog: null });
    return response({ versions: stateVersions(1, 2, "1"), fleet: { bots: {} }, world: { ...world, updatedAtUtc: "2026-01-01T00:00:01Z" }, catalog: { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} } });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  await prayer.state();
  const recovered = await prayer.state();
  assert.equal(stateCalls, 3);
  assert.equal(recovered.versions.world, 2);
});

test("consumer mutation cannot corrupt the internal state cache", async () => {
  let stateCalls = 0;
  const market = { buy_orders: {}, sell_orders: {}, observed_at_unix: 1 };
  const world = { map: { systems: [], knownPois: [] }, resources: { systemsByResource: {}, poisByResource: {} }, exploration: { exploredSystems: [], visitedPois: [], surveyedSystems: [], miningCheckedPoisByResource: {}, miningExploredSystemsByResource: {}, blacklists: {} }, wildlife: { systems: [], pois: [] }, stationMarkets: { station: market }, storageByPlayer: {}, factionStorageByFactionPoi: {}, facilitiesByPoi: {}, ownedFacilitiesByPlayer: {}, ownedFacilitiesByFaction: {}, stationPassengers: {}, salvageByPoi: {}, agentSightings: {}, chatMessagesBySession: {}, factionBySession: {}, updatedAtUtc: "2026-01-01T00:00:00Z" };
  const fetch = async (input) => {
    if (String(input).endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    stateCalls += 1;
    return response(stateCalls === 1
      ? { versions: stateVersions(1, 1, "1"), fleet: { bots: {} }, world, catalog: { itemsById: {}, shipsById: {}, recipesById: {}, facilitiesById: {}, skillsById: {} } }
      : { versions: stateVersions(1, 1, "1"), fleet: null, world: null, catalog: null });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const first = await prayer.state();
  assert.throws(() => delete first.world.stationMarkets.station, TypeError);
  assert.throws(() => { first.versions.world = 999; }, TypeError);
  const second = await prayer.state();
  assert.deepEqual(second.world.stationMarkets.station, market);
  assert.equal(second.versions.world, 1);
});

test("complete action helper catalog emits canonical wire actions", () => {
  const expected = [
    "undock", "dock", "wait", "mine", "go", "halt", "transfer", "setHome", "find", "survey", "attack", "scan", "cloak", "hunt", "prepayTax",
    "acceptMission", "abandonMission", "declineMission", "completeMission", "loadPassenger", "unloadPassenger", "buy", "sell", "cancelBuy", "cancelSell",
    "factionCreate", "factionInvite", "factionAcceptInvite", "factionKick", "factionSetRole", "facilityBuild", "factionFacilityBuild", "facilityUpgrade",
    "factionFacilityUpgrade", "facilityDismantle", "factionFacilityDismantle", "facilitySetAccess", "facilitySetOutputPrice", "facilitySetName", "useItem",
    "repair", "repairModule", "recycle", "refuel", "selfDestruct", "switchShip", "renameShip", "installMod", "uninstallMod", "buyShip", "buyListedShip",
    "commissionShip", "sellShip", "scrapShip", "listShipForSale", "refitShip", "cancelCommission", "supplyCommission", "cancelShipListing", "placeShipBuyOrder",
    "cancelShipBuyOrder", "sellShipToOrder", "cancelOrder", "modifyOrder", "craft", "cancelCraftJob", "salvageWreck", "towWreck", "scrapWreck", "sellWreck",
    "releaseWreck", "insureShip", "citizenshipApply", "citizenshipWithdraw", "citizenshipRenounce", "tradeOffer", "tradeAccept", "factionLeave",
    "factionWithdrawInvite", "factionProposeAlly", "factionAcceptAlly", "factionRemoveAlly", "factionDeclareWar", "factionProposePeace", "factionAcceptPeace",
    "factionSetEnemy", "factionRemoveEnemy", "factionPrepayTax", "factionCancelMission", "espionage", "scanPoi", "distressSignal", "say",
  ];
  assert.deepEqual(Object.keys(actions), expected);
  assert.deepEqual(actions.attack({ target_id: "pirate" }), { type: "attack", request: { target_id: "pirate" } });
  assert.deepEqual(actions.transfer({ subject: { kind: "all_cargo" }, from: { kind: "cargo" }, to: { kind: "storage" } }), {
    type: "transfer", request: { subject: { kind: "all_cargo" }, from: { kind: "cargo" }, to: { kind: "storage" } },
  });
  assert.deepEqual(actions.selfDestruct(), { type: "self_destruct" });
  assert.deepEqual(action("scan_poi", { poi_id: "poi-1" }), { type: "scan_poi", request: { poi_id: "poi-1" } });
});

test("connect rejects an incompatible API major", async () => {
  const fetch = async () => response({ apiVersion: "2.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
  await assert.rejects(() => Prayer.connect({ baseUrl: "http://test/", fetch }), PrayerCompatibilityError);
});

test("transport maps lane_busy into the specialized structured error", async () => {
  const fetch = async (input) => String(input).endsWith("api/v1/meta")
    ? response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] })
    : response({ error: { code: "lane_busy", message: "busy", retryable: false, details: { runId: "other" } }, requestId: "req" }, 409);
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  await assert.rejects(() => prayer.bot("miner"), (error) => {
    assert.ok(error instanceof LaneBusyError); assert.equal(error.status, 409); assert.equal(error.requestId, "req");
    assert.deepEqual(error.details, { runId: "other" }); return true;
  });
});

test("transport adds bearer authentication and caller headers", async () => {
  let observed;
  const fetch = async (_input, init) => {
    observed = new Headers(init.headers);
    return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
  };
  await Prayer.connect({ baseUrl: "http://test/", token: "secret", headers: { "x-client": "sdk-test" }, fetch });
  assert.equal(observed.get("authorization"), "Bearer secret"); assert.equal(observed.get("x-client"), "sdk-test");
});

test("network failures become PrayerConnectionError", async () => {
  const fetch = async () => { throw new TypeError("offline"); };
  await assert.rejects(() => Prayer.connect({ baseUrl: "http://test/", fetch }), PrayerConnectionError);
});

test("default browser fetch is invoked with the global receiver", async () => {
  const originalFetch = globalThis.fetch;
  let receiver;
  globalThis.fetch = function () {
    receiver = this;
    return Promise.resolve(response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] }));
  };
  try {
    await Prayer.connect({ baseUrl: "http://test/" });
    assert.equal(receiver, globalThis);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("request timeout aborts an unresponsive fetch", async () => {
  const fetch = async (_input, init) => new Promise((_resolve, reject) => {
    init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
  });
  await assert.rejects(() => Prayer.connect({ baseUrl: "http://test/", fetch, timeoutMs: 5 }), PrayerTimeoutError);
});

test("caller abort remains distinct from timeout and transport failure", async () => {
  const controller = new AbortController();
  const fetch = async (_input, init) => new Promise((_resolve, reject) => {
    init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
  });
  const pending = Prayer.connect({ baseUrl: "http://test/", fetch, signal: controller.signal });
  controller.abort("caller stopped");
  await assert.rejects(() => pending, PrayerAbortError);
});

test("high-level starts generate, validate, preserve, and expose idempotency keys", async () => {
  const keys = [];
  const fetch = async (input, init = {}) => {
    const url = String(input);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.endsWith("api/v1/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    keys.push(new Headers(init.headers).get("idempotency-key"));
    return response({ runId: `run-${keys.length}`, botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" }, 202);
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const bot = await prayer.bot("miner");
  const generated = await bot.start(wait(1));
  assert.match(generated.idempotencyKey, /^[0-9a-f-]{36}$/);
  const first = await bot.start(wait(1), { idempotencyKey: " durable-key " });
  const repeated = await bot.start(wait(1), { idempotencyKey: "durable-key" });
  assert.equal(first.idempotencyKey, "durable-key");
  assert.equal(repeated.idempotencyKey, "durable-key");
  assert.deepEqual(keys.slice(1), ["durable-key", "durable-key"]);
  const before = keys.length;
  await assert.rejects(() => bot.start(wait(1), { idempotencyKey: "  " }), TypeError);
  assert.equal(keys.length, before);
});

test("run wait polls without cancelling and explicit cancel is separate", async () => {
  const methods = [];
  const statuses = [];
  let statusCalls = 0;
  const fetch = async (input, init = {}) => {
    const url = String(input); methods.push([url, init.method ?? "GET"]);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.endsWith("api/v1/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    if (url.endsWith("action-runs")) return response({ runId: "run", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" }, 202);
    if (url.endsWith("/cancel")) return response({ runId: "run", botId: "bot", status: "cancelled", runVersion: 2, prayerlang: "wait 1;" });
    statusCalls += 1;
    return response({ runId: "run", botId: "bot", status: statusCalls > 1 ? "succeeded" : "running", runVersion: statusCalls + 1, prayerlang: "wait 1;" });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const bot = await prayer.bot("miner"); const run = await bot.start([{ type: "wait", ticks: 1 }]);
  assert.equal((await run.wait({ pollMs: 0, onStatus: (status) => statuses.push(status.status) })).status, "succeeded");
  assert.deepEqual(statuses, ["running", "succeeded"]);
  assert.equal(run.isTerminal, true); assert.equal(run.succeeded, true); assert.equal(run.cancellationKind, undefined);
  await run.cancel("done"); assert.equal(methods.filter(([url]) => url.endsWith("/cancel")).length, 1);
});

test("execute waits for the action run and returns its terminal response", async () => {
  let statusCalls = 0;
  const fetch = async (input) => {
    const url = String(input);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.endsWith("api/v1/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    if (url.endsWith("action-runs")) return response({ runId: "run", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" }, 202);
    statusCalls += 1;
    return response({ runId: "run", botId: "bot", status: "succeeded", runVersion: 2, prayerlang: "wait 1;", outcome: { completed: 1 } });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const bot = await prayer.bot("miner");
  const outcome = await bot.execute([wait(1)], { pollMs: 0 });
  assert.equal(outcome.status, "succeeded");
  assert.deepEqual(outcome.outcome, { completed: 1 });
  assert.equal(statusCalls, 1);
});

test("action submission sends idempotency key and exact action body", async () => {
  let submission;
  const fetch = async (input, init = {}) => {
    const url = String(input);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.endsWith("api/v1/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    submission = { headers: new Headers(init.headers), body: JSON.parse(init.body) };
    return response({ runId: "run", botId: "bot", status: "running", runVersion: 1, prayerlang: "dock;" }, 202);
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch }); const bot = await prayer.bot("miner");
  await bot.start([wait(1)], { idempotencyKey: "key-1" });
  assert.equal(submission.headers.get("idempotency-key"), "key-1");
  assert.deepEqual(submission.body, { actions: [{ type: "wait", request: { ticks: 1 } }] });
});

test("script submission, status, and cancellation use script resources", async () => {
  const paths = [];
  const fetch = async (input, init = {}) => {
    const url = new URL(String(input)); paths.push([url.pathname, init.method ?? "GET"]);
    if (url.pathname.endsWith("/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.pathname.endsWith("/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    if (url.pathname.endsWith("/script-runs")) return response({ runId: "script", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" }, 202);
    if (url.pathname.endsWith("/cancel")) return response({ runId: "script", botId: "bot", status: "cancelled", runVersion: 2, prayerlang: "wait 1;" });
    return response({ runId: "script", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch }); const bot = await prayer.bot("miner");
  const run = await bot.startScript("wait 1;", { idempotencyKey: "script-key" }); await run.status(); await run.cancel("stop");
  assert.ok(paths.some(([path]) => path.endsWith("/script-runs/script"))); assert.ok(paths.some(([path]) => path.endsWith("/script-runs/script/cancel")));
});

test("existing action and script runs can be recovered as high-level handles", async () => {
  const paths = [];
  const fetch = async (input) => {
    const url = new URL(String(input)); paths.push(url.pathname);
    if (url.pathname.endsWith("/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.pathname.endsWith("/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    const runId = url.pathname.endsWith("/script-runs/script-1") ? "script-1" : "action-1";
    return response({ runId, botId: "bot", status: "succeeded", runVersion: 2, prayerlang: "wait 1;", outcome: { recovered: true } });
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const bot = await prayer.bot("miner");
  const actionRun = await bot.actionRun("action-1");
  const scriptRun = await bot.scriptRun("script-1");
  assert.equal(actionRun.id, "action-1");
  assert.equal(scriptRun.id, "script-1");
  assert.deepEqual((await actionRun.status()).outcome, { recovered: true });
  assert.deepEqual((await scriptRun.status()).outcome, { recovered: true });
  assert.ok(paths.includes("/api/v1/bots/bot/action-runs/action-1"));
  assert.ok(paths.includes("/api/v1/bots/bot/script-runs/script-1"));
});

test("run polling removes abort listeners after each completed delay", async () => {
  let statusCalls = 0;
  const fetch = async (input) => {
    const url = String(input);
    if (url.endsWith("api/v1/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.endsWith("api/v1/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    if (url.endsWith("action-runs")) return response({ runId: "run", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 1;" }, 202);
    statusCalls += 1;
    return response({ runId: "run", botId: "bot", status: statusCalls === 3 ? "succeeded" : "running", runVersion: statusCalls + 1, prayerlang: "wait 1;" });
  };
  const controller = new AbortController();
  const add = controller.signal.addEventListener.bind(controller.signal);
  const remove = controller.signal.removeEventListener.bind(controller.signal);
  let added = 0; let removed = 0;
  controller.signal.addEventListener = (...args) => { added += 1; return add(...args); };
  controller.signal.removeEventListener = (...args) => { removed += 1; return remove(...args); };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch });
  const bot = await prayer.bot("miner");
  await (await bot.start([wait(1)])).wait({ signal: controller.signal, pollMs: 0 });
  assert.equal(added, 3);
  assert.equal(removed, 3);
});

test("aborting a local wait never sends server cancellation", async () => {
  const paths = [];
  const fetch = async (input) => {
    const url = new URL(String(input)); paths.push(url.pathname);
    if (url.pathname.endsWith("/meta")) return response({ apiVersion: "1.0", serverVersion: "test", actionSchemaVersion: 5, capabilities: [] });
    if (url.pathname.endsWith("/bots/miner")) return response({ botId: "bot", name: "miner", connection: "connected", stateVersion: 1, observedAt: null });
    return response({ runId: "run", botId: "bot", status: "running", runVersion: 1, prayerlang: "wait 10;" }, url.pathname.endsWith("/action-runs") ? 202 : 200);
  };
  const prayer = await Prayer.connect({ baseUrl: "http://test/", fetch }); const bot = await prayer.bot("miner"); const run = await bot.start([wait(10)]);
  const controller = new AbortController(); controller.abort(new Error("local stop"));
  await assert.rejects(() => run.wait({ signal: controller.signal, pollMs: 0 }), /local stop/);
  assert.equal(paths.filter((path) => path.endsWith("/cancel")).length, 0);
});
