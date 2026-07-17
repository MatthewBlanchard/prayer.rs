import assert from "node:assert/strict";
import test from "node:test";
import type { FleetEntry, GalaxyCatalog, GalaxyExploration, GalaxyMap, GalaxyResources, GalaxyWildlife } from "@prayer/sdk";
import {
  selectCatalog,
  selectCharacterSkills,
  selectCommanderStorage,
  selectEconomyMarket,
  selectFacilities,
  selectGalaxyExploration,
  selectGalaxyMap,
  selectGalaxyResources,
  selectGameChatMessages,
  selectMarketArbitrage,
  selectPassengerStates,
  selectSalvageStates,
  selectShipyard,
  selectShipyardFleet,
  selectSocialBots,
  selectStorageState,
  selectWildlifeStates,
} from "../src/client/prayer/worldSelectors.js";

const map: GalaxyMap = {
  systems: [
    { id: "sol", empire: "terran", isStronghold: true, x: 10, y: 20, connections: ["alpha"], pois: [{ id: "sol-belt", x: 1, y: 2 }] },
    { id: "alpha", empire: "", isStronghold: false, x: 30, y: 40, connections: ["sol"], pois: [] },
  ],
  knownPois: [
    {
      id: "sol-belt",
      systemId: "sol",
      name: "Sol Belt",
      type: "asteroid_field",
      x: 1,
      y: 2,
      hasBase: false,
      baseId: null,
      baseName: null,
      lastSeenUtc: "2026-01-01T00:00:00Z",
    },
  ],
};
const resources: GalaxyResources = { poisByResource: { iron: ["sol-belt"] }, systemsByResource: { iron: ["sol"] } };
const exploration: GalaxyExploration = {
  exploredSystems: ["sol"],
  visitedPois: ["sol-belt"],
  surveyedSystems: ["sol"],
  miningCheckedPoisByResource: {},
  miningExploredSystemsByResource: {},
  blacklists: {},
};
const catalog = {
  itemsById: { iron: { id: "iron", name: "Iron", base_value: 10, category: "ore", description: "Iron ore", size: 1, stackable: true, tradeable: true } },
  shipsById: {},
  recipesById: {},
  facilitiesById: {},
  skillsById: {},
} satisfies GalaxyCatalog;
const creature = {
  creatureId: "c1",
  species: "manta",
  name: "Void Manta",
  role: "predator",
  hull: 8,
  maxHull: 10,
  inCombat: false,
  systemId: "sol",
  poiId: "sol-belt",
  observedAtUnix: 123,
};
const wildlife: GalaxyWildlife = {
  systems: [
    {
      systemId: "sol",
      creatureCount: 1,
      species: [{ species: "manta", name: "Void Manta", role: "predator", count: 1 }],
      pois: ["sol-belt"],
      observedAtUnix: 123,
    },
  ],
  pois: [{ systemId: "sol", poiId: "sol-belt", creatureCount: 1, observedAtUnix: 123, creatures: [creature] }],
};
const fleet = [
  {
    id: "bot1",
    username: "silver",
    version: 3,
    observed_at: null,
    connection: "Connected",
    state: {
      active_commissions: [],
      cargo: {},
      cargo_capacity: 10,
      cargo_items: [],
      cargo_pct: 0,
      cargo_used: 0,
      crafting_queue: [],
      fuel: 10,
      fuel_pct: 100,
      in_battle: false,
      installed_modules: [],
      last_mined: {},
      last_stored: {},
      location: { system_id: "sol", poi_id: "sol-belt" },
      max_fuel: 10,
      mission_complete: {},
      missions: { active: [], active_details: [], available: [], available_details: [] },
      modules: [],
      observation_nearby: {},
      own_buy_orders: [],
      own_sell_orders: [],
      owned_ship_details: [],
      passengers: {
        aboard: [],
        aboard_count: 0,
        business_berths: { current: 0, max: 0 },
        business_berths_raw: "0/0",
        economy_berths: { current: 0, max: 0 },
        economy_berths_raw: "0/0",
        first_berths: { current: 0, max: 0 },
        first_berths_raw: "0/0",
      },
      player: { id: "player-1" },
      script_mined_by_item: {},
      script_stored_by_item: {},
      ship: {},
      skills: {},
    },
  },
] satisfies FleetEntry[];

test("bounded galaxy resources pass directly to view selectors", () => {
  assert.equal(selectGalaxyMap(map)?.systems[0].id, "sol");
  assert.deepEqual(selectGalaxyExploration(exploration)?.visitedPois, ["sol-belt"]);
  assert.deepEqual(selectGalaxyResources(resources), resources);
});

test("projects catalog and wildlife without aggregate state", () => {
  assert.equal(selectCatalog(catalog)?.items[0].name, "Iron");
  assert.equal(selectWildlifeStates(fleet, wildlife)[0].state.nearbyCreatures[0].creatureId, "c1");
});

test("projects character skills from canonical fleet and catalog facts", () => {
  const skilled = [
    { ...fleet[0], state: { ...fleet[0].state, skills: { mining: { level: 3, max_level: 10, xp: 20, next_level_xp: 40 } } } },
  ] satisfies FleetEntry[];
  const skillCatalog = {
    ...catalog,
    skillsById: { mining: { id: "mining", name: "Mining", category: "industry", description: "Mining", max_level: 10 } },
  } satisfies GalaxyCatalog;
  const state = selectCharacterSkills(skilled[0], skillCatalog);
  assert.equal(state?.sessionId, "bot1");
  assert.deepEqual(state?.skills[0], {
    id: "mining",
    name: "Mining",
    category: "industry",
    level: 3,
    maxLevel: 10,
    xp: 20,
    nextLevelXp: 40,
  });
});

test("projects current and owned facilities from canonical world facts", () => {
  const facilityFleet = [{ ...fleet[0], state: { ...fleet[0].state, player: { id: "player-1", faction_id: "faction-1" } } }] satisfies FleetEntry[];
  const facilityCatalog = {
    ...catalog,
    facilitiesById: {
      mine: { id: "mine", name: "Mine", category: "industry", level: 1, build_cost: 100, build_time: 5, build_materials: [{ item_id: "iron", quantity: 2 }] },
    },
  } satisfies GalaxyCatalog;
  const state = selectFacilities(
    facilityFleet[0],
    map,
    facilityCatalog,
    {
      "sol-belt": {
        observed_at_unix: 10,
        current: {
          station_facilities: [{ facility_id: "station-1", type: "mine", name: "Station Mine" }],
          public_facilities: [{ facility_id: "public-1", type: "mine", name: "Public Mine", production: { public: true } }],
        },
      },
    },
    { "player-1": { facilities: [{ facility_id: "mine-1", type: "mine", custom_name: "Ada Mine", owner_id: "player-1", production: { public: true } }] } },
    {
      "faction-1": {
        faction_id: "faction-1",
        total_rent_per_cycle: 12,
        arrears_owed: 3,
        facilities: [{ facility_id: "faction-mine", type: "mine", faction_id: "faction-1" }],
      },
    },
  );
  assert.equal(state?.current[0].facilityId, "station-1");
  assert.equal(state?.publicFacilities[0].facilityId, "public-1");
  assert.equal(state?.owned[0].name, "Ada Mine");
  assert.equal(state?.factionOwned[0].ownerKind, "faction");
  assert.equal(state?.factionRentPerCycle, 12);
  assert.deepEqual(state?.types[0].requiredItems, { iron: 2 });
});

test("projects shipyard, fleet, and storage from canonical aggregate facts", () => {
  const shipyardFleet = [
    {
      ...fleet[0],
      state: {
        ...fleet[0].state,
        player: { id: "player-1", faction_id: "faction-1" },
        ship: { id: "ship-1" },
        installed_modules: ["cargo_rack"],
        modules: [],
        owned_ship_details: [{ ship_id: "ship-1", class_id: "runner", is_active: true }],
        active_commissions: [{ commission_id: "commission-1", ship_class_id: "runner", status: "building", materials_provided: false }],
      },
    },
  ] satisfies FleetEntry[];
  const yard = selectShipyard(shipyardFleet[0]);
  assert.equal(yard?.ownedShips[0].shipId, "ship-1");
  assert.deepEqual(yard?.installedModules, ["cargo_rack"]);
  assert.equal(yard?.factionGarage.ships.length, 0);
  assert.equal(yard?.inProgressCommissions[0].commissionId, "commission-1");
  const projectedFleet = selectShipyardFleet(shipyardFleet);
  assert.equal(projectedFleet.ownedShips.length, 1);
  assert.equal(projectedFleet.factionGarageShips.length, 0);
  const storage = selectStorageState(shipyardFleet[0], { "player-1": { "sol-belt": { cargo_rack: 2 } } }, { "faction-1": { "sol-belt": { armor: 3 } } });
  assert.equal(storage?.storageByPoi["sol-belt"].cargo_rack, 2);
  assert.equal(storage?.factionStorage.armor, 3);
});

test("projects owned ships from the canonical Rust BotState wire shape", () => {
  const shipyardFleet = [
    {
      ...fleet[0],
      state: {
        ...fleet[0].state,
        ship: { id: "ship-1" },
        player: { id: "player-1" },
        installed_modules: [],
        modules: [],
        active_commissions: [],
        owned_ship_details: [
          { ship_id: "ship-1", class_id: "runner", is_active: true, location: "Active ship" },
          { ship_id: "ship-2", class_id: "hauler", is_active: false, location: "Earth Station" },
        ],
      },
    },
  ] satisfies FleetEntry[];

  const yard = selectShipyard(shipyardFleet[0]);
  assert.deepEqual(
    yard?.ownedShips.map((ship) => ship.shipId),
    ["ship-1", "ship-2"],
  );
  const projectedFleet = selectShipyardFleet(shipyardFleet);
  assert.equal(projectedFleet.ownedShips.length, 2);
});

test("projects passenger boards from the canonical fleet snapshot", () => {
  const passengerFleet = [
    {
      ...fleet[0],
      state: {
        ...fleet[0].state,
        passengers: {
          aboard_count: 1,
          economy_berths: { current: 1, max: 4 },
          business_berths: { current: 0, max: 0 },
          first_berths: { current: 0, max: 0 },
          aboard: [{ citizen_id: "ada", name: "Ada", class: "economy", destination: "alpha" }],
          station: "sol-belt",
          waiting_count: 1,
          waiting: [{ citizen_id: "grace", name: "Grace", class: "business", destination: "alpha" }],
        },
      },
    },
  ] satisfies FleetEntry[];

  const result = selectPassengerStates(passengerFleet, map, {
    "sol-belt": {
      station: "sol-belt",
      waiting_count: 1,
      waiting: [{ citizen_id: "grace", name: "Grace", class: "business", destination: "alpha" }],
      economy_berths: { current: 0, max: 0 },
      business_berths: { current: 0, max: 0 },
      first_berths: { current: 0, max: 0 },
    },
  })[0];
  assert.equal(result.state?.currentPoiName, "Sol Belt");
  assert.equal(result.state?.aboard[0].name, "Ada");
  assert.equal(result.state?.waiting[0].className, "business");
});

test("projects canonical salvage world memory", () => {
  const result = selectSalvageStates(fleet, {
    "sol-belt": {
      visible_lootables: [
        { id: "wreck-1", kind: "wreck", poi_id: "sol-belt", system_id: "sol", cargo: [{ item_id: "iron", name: "Iron", quantity: 2 }], modules: [] },
      ],
      lootables_by_poi: {},
      last_seen_poi: "sol-belt",
      last_seen_system: "sol",
      observed_at_unix: 100,
    },
  })[0];
  assert.equal(result.state.visibleLootables[0].id, "wreck-1");
  assert.equal(result.state.visibleLootables[0].cargo[0].itemId, "iron");
});

test("projects canonical social sightings", () => {
  const rows = selectSocialBots(
    {
      contact: {
        contact: { player_id: "p2", username: "Grace", faction_id: "f1", in_combat: false },
        last_seen_system: "alpha",
        first_seen_unix: 10,
        last_seen_unix: 20,
        times_seen: 2,
      },
    },
    fleet,
  );
  assert.equal(rows.find((row) => row.playerId === "p2")?.username, "Grace");
  assert.equal(rows.find((row) => row.playerId === "p2")?.lastSeenSystem, "alpha");
});

test("projects canonical cached chat messages", () => {
  const rows = selectGameChatMessages(
    [{ id: "m1", channel: "system", sender_id: "p2", sender: "Grace", content: "Hello", timestamp_utc: "2026-01-01T00:00:00Z", system_id: "sol" }],
    "silver",
  );
  assert.deepEqual(rows[0], {
    id: "m1",
    channel: "system",
    senderId: "p2",
    sender: "Grace",
    content: "Hello",
    timestampUtc: "2026-01-01T00:00:00Z",
    systemId: "sol",
    poiId: null,
    factionId: null,
    targetId: null,
    targetName: null,
    empireOfficial: false,
    sessionHandle: "silver",
  });
});

test("treats omitted empty passenger arrays as empty lists", () => {
  const emptyPassengerFleet = [
    {
      ...fleet[0],
      state: {
        ...fleet[0].state,
        passengers: {
          economy_berths: { current: 0, max: 0 },
          business_berths: { current: 0, max: 0 },
          first_berths: { current: 0, max: 0 },
        },
      },
    },
  ] satisfies FleetEntry[];

  const state = selectPassengerStates(emptyPassengerFleet, map)[0].state;
  assert.deepEqual(state?.aboard, []);
  assert.deepEqual(state?.waiting, []);
});

test("composes commander storage rows from canonical fleet and world facts", () => {
  const storageFleet = [
    {
      ...fleet[0],
      state: {
        ...fleet[0].state,
        cargo: { iron: 2 },
        player: { id: "player-1", username: "silver", credits: 50, faction_id: "faction-1" },
      },
    },
  ] satisfies FleetEntry[];
  const view = selectCommanderStorage(
    storageFleet,
    map,
    { "player-1": { "sol-belt": { copper: 3 } } },
    { "faction-1": { "sol-belt": { water: 4 } } },
    {
      silver: {
        id: "faction-1",
        name: "Solar Union",
        tag: "SOL",
        leader_id: "player-1",
        leader_username: "silver",
        member_count: 1,
        treasury: 750,
        is_member: true,
        description: "",
        primary_color: "",
        secondary_color: "",
        members: [],
        roles: [],
      },
    },
  );

  assert.equal(view.sessionsObserved, 1);
  assert.deepEqual(
    view.rows.map((row) => [row.sourceKind, row.itemId, row.quantity]),
    [
      ["cargo", "iron", 2],
      ["financial", "credits", 50],
      ["financial", "credits", 750],
      ["personal", "copper", 3],
      ["faction", "water", 4],
    ],
  );
  const treasury = view.rows.find((row) => row.details?.["kind"] === "faction_treasury");
  assert.equal(treasury?.ownerName, "Solar Union");
  assert.deepEqual(treasury?.observedBy, ["silver"]);
});

test("projects market books and global price aggregates from canonical facts", () => {
  const market = selectEconomyMarket(
    {
      "sol-belt": {
        buy_orders: { iron: [{ price_each: 8, quantity: 5 }] },
        sell_orders: {
          iron: [
            { price_each: 10, quantity: 2 },
            { price_each: 12, quantity: 4 },
          ],
        },
        observed_at_unix: 100,
      },
      alpha: {
        buy_orders: { iron: [{ price_each: 9, quantity: 5 }] },
        sell_orders: { iron: [{ price_each: 14, quantity: 1 }] },
        observed_at_unix: 200,
      },
    },
    map,
  );

  assert.equal(market.marketsByStation["sol-belt"].stationName, "Sol Belt");
  assert.equal(market.globalMedianBuyPrices.iron, 8.5);
  assert.equal(market.globalMedianSellPrices.iron, 12);
  assert.equal(market.globalWeightedMidPrices.iron, 12);
});

test("builds market arbitrage packages entirely from canonical snapshots", () => {
  const arbitrageMap = {
    ...map,
    knownPois: [
      ...map.knownPois,
      {
        id: "alpha-station",
        systemId: "alpha",
        name: "Alpha Station",
        type: "station",
        x: 0,
        y: 0,
        hasBase: true,
        baseId: "alpha-station",
        baseName: "Alpha Station",
        lastSeenUtc: "2026-01-01T00:00:00Z",
      },
    ],
  };
  const result = selectMarketArbitrage(
    {
      "sol-belt": { buy_orders: {}, sell_orders: { iron: [{ price_each: 10, quantity: 8 }] }, observed_at_unix: null },
      "alpha-station": { buy_orders: { iron: [{ price_each: 16, quantity: 6 }] }, sell_orders: {}, observed_at_unix: null },
    },
    arbitrageMap,
    { ...catalog, itemsById: { iron: { id: "iron", size: 2 } } },
    "sol",
    true,
    {
      minMargin: 0.1,
      minDepthCoverage: 1,
      maxUnits: 10,
      limit: 20,
      routeCosts: new Map([
        ["sol\0sol", 0],
        ["sol\0alpha", 1],
      ]),
    },
  );

  assert.equal(result.packages.length, 1);
  assert.equal(result.packages[0].jumpsBuyToSell, 1);
  assert.equal(result.packages[0].deals[0].quantity, 5);
  assert.equal(result.packages[0].totalProfit, 30);
});
