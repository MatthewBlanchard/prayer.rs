import assert from "node:assert/strict";
import test from "node:test";
import type { FleetEntry, GalaxyCatalog } from "@prayer/sdk/types";
import { activeYardClassId, activeYardShipName, collectModuleCatalog, validateFitting } from "../src/client/ShipyardPanel.js";
import { selectCatalog, selectShipyard } from "../src/client/prayer/worldSelectors.js";

const catalog = {
  itemsById: {
    laser: {
      id: "laser",
      name: "Pulse Laser",
      description: "A laser",
      type: "weapon",
      type_id: "laser",
      slot: "weapon",
      size: 1,
      base_value: 100,
      cpu_usage: 2,
      power_usage: 3,
      damage: 5,
    },
  },
  shipsById: {
    scout: {
      id: "scout",
      name: "Scout",
      class: "scout",
      default_modules: ["laser"],
      weapon_slots: 1,
      defense_slots: 0,
      utility_slots: 0,
      cpu_capacity: 5,
      power_capacity: 5,
    },
  },
  recipesById: {},
  facilitiesById: {},
  skillsById: {},
} satisfies GalaxyCatalog;

const fleetEntry = {
  id: "bot-1",
  username: "Ada",
  version: 4,
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
    location: { system_id: "sol", poi_id: "earth", docked_at: "earth" },
    max_fuel: 10,
    mission_complete: {},
    missions: { active: [], active_details: [], available: [], available_details: [] },
    modules: [{ type_id: "laser", name: "Pulse Laser", slot: "weapon" }],
    observation_nearby: {},
    own_buy_orders: [],
    own_sell_orders: [],
    owned_ship_details: [{ ship_id: "ship-1", class_id: "scout", class_name: "Scout", custom_name: "Swift", is_active: true }],
    passengers: {
      aboard: [],
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
    ship: { id: "ship-1", class_id: "scout", class_name: "Scout", custom_name: "Swift" },
    skills: {},
  },
} satisfies FleetEntry;

test("Designer and Loadout use generated module and chassis contracts", () => {
  const projected = selectCatalog(catalog)!;
  const modules = collectModuleCatalog(projected);
  assert.deepEqual([...modules.ids], ["laser"]);
  const fitting = validateFitting("scout", ["laser"], projected.ships, modules.specs);
  assert.deepEqual(fitting.errors, []);
  assert.equal(fitting.cpuUsed, 2);
  assert.equal(fitting.powerUsed, 3);
});

test("Equipped reads the generated active ship projection", () => {
  const yard = selectShipyard(fleetEntry)!;
  assert.equal(activeYardClassId(yard), "scout");
  assert.equal(activeYardShipName(yard), "Swift");
  assert.deepEqual(yard.installedModules, ["laser"]);
});
