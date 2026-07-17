import { useCallback, useEffect, useMemo, useState } from "react";
import type { Module } from "@prayer/sdk/types";
import { CreditAmount, formatCredits } from "./Credits.js";
import {
  CommanderStorageRow,
  CommanderStorageView,
  CatalogEntry,
  EconomyMarketData,
  FactionGarageShipInfo,
  OwnedShipInfo,
  fetchRoutes,
  fetchVirtualOrders,
  saveVirtualOrders,
  ShipCatalogEntry,
  ShipyardData,
  ShipyardFleetData,
  StorageSessionState,
  VirtualFactionOrderInput,
} from "./api.js";
import { SessionState } from "./SessionsPanel.js";
import { jobHandleForSession, sessionMatchesJobHandle } from "./sessionIdentity.js";
import { usePrayer } from "./prayer/PrayerProvider.js";
import { useSquads } from "./prayer/useSquads.js";
import {
  selectCatalog,
  selectCommanderStorage,
  selectEconomyMarket,
  selectGalaxyMap,
  selectShipyard,
  selectShipyardFleet,
  selectStorageState,
} from "./prayer/worldSelectors.js";
import { isRecord } from "./api/decoding.js";
import { readVersionedStored, writeVersionedStored } from "./persistence.js";
import { findNearestStationPoi } from "./nearestStation.js";

type ShipyardPanelProps = {
  sessions: SessionState[];
  onChanged: () => Promise<void>;
};

type PaneMode = "ships" | "designer" | "equipped" | "loadout";
type FittingSlotType = "weapon" | "defense" | "utility";
type ModuleFilterMode = "all" | FittingSlotType;

type LoadoutRow = {
  chassisClassId: string;
  modules: string;
};

type LoadoutShipOption = {
  ship: OwnedShipInfo;
  ownerHandle: string;
};

const DSL_ARG_TOKEN_RE = /^\$?[A-Za-z_][A-Za-z0-9_-]*$|^[A-Za-z0-9_][A-Za-z0-9_-]*$/;

type ModuleFitSpec = {
  id: string;
  name: string;
  slot: FittingSlotType | null;
  cpu: number | null;
  power: number | null;
  source: Module;
};

type FittingValidation = {
  ship: ShipCatalogEntry | null;
  modules: string[];
  slotCounts: Record<FittingSlotType, number>;
  slotLimits: Record<FittingSlotType, number | null>;
  cpuUsed: number | null;
  cpuLimit: number | null;
  powerUsed: number | null;
  powerLimit: number | null;
  errors: string[];
  warnings: string[];
};

type FittedStat = {
  label: string;
  value: string;
  tone: string;
  show: boolean;
};

type ModuleStatChit = {
  label: string;
  value: string;
  tone: string;
};

type DesignerPriceRow = {
  itemId: string;
  label: string;
  quantity: number;
  orderQuantity: number;
  pricedQuantity: number;
  coverQuantity: number;
  missingQuantity: number;
  total: number;
  rawTotal: number;
  unitPrice: number | null;
  orderPriceEach: number | null;
  bestPrice: number | null;
  source: string;
};

type DesignerPriceQuote = {
  total: number;
  rows: DesignerPriceRow[];
  missingRows: DesignerPriceRow[];
  pricedAt: number;
};

type DesignerFilters = {
  query: string;
  empire: string;
  hull: string;
  shield: string;
  fuel: string;
  cargo: string;
  speed: string;
  weapon: string;
  defense: string;
  utility: string;
  cpu: string;
  power: string;
};

type LoadoutSquadOption = {
  id: string;
  name: string;
  handles: string[];
};

type LoadoutModuleSource = {
  kind: "installed" | "cargo" | "personal" | "faction";
  itemId: string;
  quantity: number;
  locationId: string;
  distance: number;
  order: number;
};

type SavedShipyardLoadout = {
  handles: string[];
  rows: Record<string, LoadoutRow>;
};

type SavedShipyardDesign = {
  id: string;
  name: string;
  row: LoadoutRow;
  savedAt: number;
};

const SHIPYARD_LOADOUTS_KEY = "prayer.shipyard.loadouts.v1";
const SHIPYARD_DESIGNS_KEY = "prayer.shipyard.designs.v1";
const SHIPYARD_VIRTUAL_ORDER_PADDING = 1.1;

const emptyDesignerFilters: DesignerFilters = {
  query: "",
  empire: "",
  hull: "",
  shield: "",
  fuel: "",
  cargo: "",
  speed: "",
  weapon: "",
  defense: "",
  utility: "",
  cpu: "",
  power: "",
};

function shipLabel(ship: OwnedShipInfo): string {
  return ship.customName || ship.className || ship.classId || ship.shipId;
}

function shipClass(ship: OwnedShipInfo): string {
  if (ship.className && ship.classId && ship.className !== ship.classId) {
    return `${ship.className} / ${ship.classId}`;
  }
  return ship.className || ship.classId || "unknown";
}

function shipLocation(ship: OwnedShipInfo): string {
  if (ship.isActive) return "Active";
  return ship.location || ship.locationBaseId || "unknown";
}

function activeYardShip(yard: ShipyardData | undefined): OwnedShipInfo | null {
  return yard?.ownedShips.find((ship) => ship.isActive) ?? null;
}

export function activeYardClassId(yard: ShipyardData | undefined): string {
  const active = activeYardShip(yard);
  if (active?.classId) return active.classId;
  const activeShip = yard?.activeShip;
  if (!activeShip) return "";
  return activeShip.class_id ?? "";
}

export function activeYardShipName(yard: ShipyardData | undefined): string {
  const active = activeYardShip(yard);
  if (active) return shipLabel(active);
  const activeShip = yard?.activeShip;
  if (!activeShip) return "";
  return activeShip.custom_name ?? activeShip.name ?? "";
}

function activeYardShipClassLabel(yard: ShipyardData | undefined): string {
  const active = activeYardShip(yard);
  if (active) return shipClass(active);
  const activeShip = yard?.activeShip;
  if (!activeShip) return "";
  return activeShip.class_name ?? activeYardClassId(yard);
}

function formatFilterLabel(value: string): string {
  return value.replace(/[_-]+/g, " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function splitModules(value: string): string[] {
  return value
    .split(/[\n,]+/g)
    .map((item) => item.trim())
    .filter(Boolean);
}

function moduleInputSlots(value: string): string[] {
  if (!value.trim()) return [""];
  if (!value.includes("\n") && value.includes(",")) return splitModules(value);
  return value.split(/\n/g);
}

function catalogEntryIsModule(entry: CatalogEntry): boolean {
  const typeName = entry.typeName.trim().toLowerCase();
  const category = entry.category.trim().toLowerCase();
  const slot = "slot" in entry.source ? normalizeSlot(entry.source.slot) : null;
  return (
    typeName === "module" ||
    typeName === "weapon" ||
    typeName === "defense" ||
    typeName === "utility" ||
    category === "module" ||
    category === "modules" ||
    category.endsWith("_module") ||
    category.endsWith(" modules") ||
    slot !== null
  );
}

function moduleString(module: Module, keys: Array<keyof Module>): string {
  for (const key of keys) {
    const value = module[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return "";
}

function moduleNumber(module: Module, keys: Array<keyof Module>): number | null {
  for (const key of keys) {
    const value = module[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (typeof value === "string" && value.trim() && Number.isFinite(Number(value))) return Number(value);
  }
  return null;
}

function normalizeSlot(value: string): FittingSlotType | null {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[_\s-]+/g, "");
  if (normalized.includes("weapon") || normalized === "high") return "weapon";
  if (normalized.includes("defense") || normalized.includes("defence") || normalized === "mid") return "defense";
  if (normalized.includes("utility") || normalized === "low") return "utility";
  return null;
}

function moduleFitSpec(entry: CatalogEntry): ModuleFitSpec {
  if (!("slot" in entry.source)) throw new Error(`${entry.id} is not a module`);
  return {
    id: entry.id,
    name: entry.name || entry.id,
    slot: normalizeSlot(entry.source.slot),
    cpu: entry.source.cpu_usage,
    power: entry.source.power_usage,
    source: entry.source,
  };
}

function chassisLabel(ship: ShipCatalogEntry): string {
  const pieces = [ship.name || ship.id, ship.className || ship.category, ship.tier == null ? "" : `tier ${ship.tier}`].filter(Boolean);
  return pieces.join(" / ");
}

function chassisMatches(ship: OwnedShipInfo, chassisId: string): boolean {
  const needle = chassisId.trim().toLowerCase();
  if (!needle) return false;
  return [ship.classId, ship.className, ship.shipId].some((value) => value.trim().toLowerCase() === needle);
}

function findCatalogShip(ships: ShipCatalogEntry[], classId: string): ShipCatalogEntry | null {
  const needle = classId.trim().toLowerCase();
  if (!needle) return null;
  return ships.find((ship) => [ship.id, ship.classId, ship.className, ship.name].some((value) => value.trim().toLowerCase() === needle)) ?? null;
}

function moduleSummary(moduleId: string, specs: Record<string, ModuleFitSpec>, names: Record<string, string>): string {
  const spec = specs[moduleId];
  const label = spec?.name || names[moduleId] || moduleId;
  if (!spec) return label;
  const slot = spec.slot ? formatFilterLabel(spec.slot) : "unknown slot";
  const cpu = spec.cpu == null ? "CPU ?" : `CPU ${spec.cpu}`;
  const power = spec.power == null ? "power ?" : `power ${spec.power}`;
  const effects = moduleEffectSummary(spec);
  return `${label} / ${slot} / ${cpu} / ${power}${effects ? ` / ${effects}` : ""}`;
}

function compactEffectValue(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return value.map(compactEffectValue).filter(Boolean).join(", ");
  if (typeof value === "object") {
    return Object.entries(value)
      .map(([key, item]) => {
        const formatted = compactEffectValue(item);
        return formatted ? `${formatFilterLabel(key)} ${formatted}` : "";
      })
      .filter(Boolean)
      .join(", ");
  }
  return "";
}

function formatDps(value: number): string {
  if (Number.isInteger(value)) return value.toLocaleString();
  return value.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

function moduleEffectSummary(spec: ModuleFitSpec | undefined): string {
  if (!spec) return "";
  const module = spec.source;
  const effects: string[] = [];
  const addNumber = (keys: Array<keyof Module>, label: string, prefix = "+") => {
    const value = moduleNumber(module, keys);
    if (value != null && value !== 0) effects.push(`${label} ${value > 0 ? prefix : ""}${value}`);
  };
  const addText = (keys: Array<keyof Module>, label: string) => {
    const value = moduleString(module, keys);
    if (value) effects.push(`${label} ${value}`);
  };

  addNumber(["damage"], "Damage", "");
  addText(["damage_type"], "type");
  addNumber(["reach"], "Reach", "");
  addNumber(["cooldown"], "cooldown", "");
  const damage = module.damage ?? null;
  const cooldown = module.cooldown ?? null;
  if (damage != null && cooldown != null && cooldown > 0) {
    effects.push(`DPS ${formatDps(damage / cooldown)}`);
  }
  addNumber(["magazine_size"], "mag", "");
  addNumber(["shield_bonus"], "Shield");
  addNumber(["armor_bonus"], "Armor");
  addNumber(["hull_bonus"], "Hull");
  addNumber(["cargo_bonus"], "Cargo");
  addNumber(["max_fuel_bonus"], "Fuel");
  addNumber(["speed_bonus"], "Speed");
  addNumber(["speed_penalty"], "Speed");
  addNumber(["fuel_efficiency"], "Fuel efficiency");
  addNumber(["cpu_bonus"], "CPU");
  addNumber(["power_bonus"], "Power");
  addNumber(["damage_reduction"], "Damage reduction");
  addNumber(["armor_repair_rate"], "Armor repair");
  addNumber(["shield_recharge_bonus"], "Shield recharge");
  addNumber(["mining_power"], "Mining");
  addNumber(["survey_power"], "Survey");
  addNumber(["scanner_power"], "Scanner");
  addNumber(["remote_repair_power"], "Remote repair");
  addNumber(["cloak_strength"], "Cloak");
  addNumber(["webify_strength"], "Webify");
  addNumber(["warp_stabilization"], "Warp stab");
  addNumber(["disruptor_power"], "Disrupt");
  addNumber(["scramble_power"], "Scramble");
  addNumber(["tracking_bonus"], "Tracking");
  addNumber(["accuracy_bonus"], "Accuracy");
  addNumber(["signature_bonus"], "Signature");
  addNumber(["shield_bypass_bonus"], "Shield bypass");
  addNumber(["armor_bypass_bonus"], "Armor bypass");
  addNumber(["drone_capacity"], "Drones", "");
  addNumber(["drone_bandwidth"], "Bandwidth", "");
  addNumber(["passenger_economy_berths"], "Economy berths", "");
  addNumber(["passenger_business_berths"], "Business berths", "");
  addNumber(["passenger_first_berths"], "First berths", "");

  const resistance = compactEffectValue(module.resistance_bonus);
  if (resistance) effects.push(`Resist ${resistance}`);
  addText(["special"], "Special");

  return effects.slice(0, 6).join(" / ");
}

function signedModuleValue(value: number, prefix = "+"): string {
  return `${value > 0 ? prefix : ""}${formatCatalogValue(value)}`;
}

function moduleStatChits(spec: ModuleFitSpec | undefined): ModuleStatChit[] {
  if (!spec) {
    return [
      { label: "Slot", value: "unknown", tone: "slots" },
      { label: "CPU", value: "?", tone: "cpu" },
      { label: "Power", value: "?", tone: "power" },
    ];
  }

  const module = spec.source;
  const chits: ModuleStatChit[] = [
    { label: "Slot", value: spec.slot ? formatFilterLabel(spec.slot) : "unknown", tone: "slots" },
    { label: "CPU", value: spec.cpu == null ? "?" : formatCatalogValue(spec.cpu), tone: "cpu" },
    { label: "Power", value: spec.power == null ? "?" : formatCatalogValue(spec.power), tone: "power" },
  ];

  const addNumber = (keys: Array<keyof Module>, label: string, tone: string, prefix = "+") => {
    const value = moduleNumber(module, keys);
    if (value != null && value !== 0) chits.push({ label, value: signedModuleValue(value, prefix), tone });
  };
  const addText = (keys: Array<keyof Module>, label: string, tone: string) => {
    const value = moduleString(module, keys);
    if (value) chits.push({ label, value, tone });
  };

  addNumber(["damage"], "Damage", "weapon", "");
  addText(["damage_type"], "Type", "weapon");
  addNumber(["reach"], "Reach", "weapon", "");
  addNumber(["cooldown"], "Cooldown", "weapon", "");
  const damage = module.damage ?? null;
  const cooldown = module.cooldown ?? null;
  if (damage != null && cooldown != null && cooldown > 0) {
    chits.push({ label: "DPS", value: formatDps(damage / cooldown), tone: "weapon" });
  }
  addNumber(["magazine_size"], "Mag", "weapon", "");
  addNumber(["tracking_bonus"], "Tracking", "weapon");
  addNumber(["accuracy_bonus"], "Accuracy", "weapon");
  addNumber(["shield_bypass_bonus"], "Shield bypass", "weapon");
  addNumber(["armor_bypass_bonus"], "Armor bypass", "weapon");
  addNumber(["shield_bonus"], "Shield", "defense");
  addNumber(["armor_bonus"], "Armor", "defense");
  addNumber(["hull_bonus"], "Hull", "defense");
  addNumber(["damage_reduction"], "DR", "defense");
  addNumber(["armor_repair_rate"], "Armor repair", "defense");
  addNumber(["shield_recharge_bonus"], "Shield regen", "defense");
  addNumber(["cargo_bonus"], "Cargo", "cargo");
  addNumber(["max_fuel_bonus"], "Fuel", "cargo");
  addNumber(["speed_bonus"], "Speed", "cargo");
  addNumber(["speed_penalty"], "Speed", "cargo");
  addNumber(["fuel_efficiency"], "Fuel efficiency", "cargo");
  addNumber(["cpu_bonus"], "CPU bonus", "cpu");
  addNumber(["power_bonus"], "Power bonus", "power");
  addNumber(["mining_power"], "Mining", "utility", "");
  addNumber(["survey_power"], "Survey", "utility", "");
  addNumber(["scanner_power"], "Scanner", "utility", "");
  addNumber(["remote_repair_power"], "Remote repair", "utility", "");
  addNumber(["cloak_strength"], "Cloak", "utility", "");
  addNumber(["webify_strength"], "Webify", "utility", "");
  addNumber(["warp_stabilization"], "Warp stab", "utility", "");
  addNumber(["disruptor_power"], "Disrupt", "utility", "");
  addNumber(["scramble_power"], "Scramble", "utility", "");
  addNumber(["signature_bonus"], "Signature", "utility");
  addNumber(["drone_capacity"], "Drones", "utility", "");
  addNumber(["drone_bandwidth"], "Bandwidth", "utility", "");
  addNumber(["passenger_economy_berths"], "Economy berths", "cargo", "");
  addNumber(["passenger_business_berths"], "Business berths", "cargo", "");
  addNumber(["passenger_first_berths"], "First berths", "cargo", "");

  const resistance = compactEffectValue(module.resistance_bonus);
  if (resistance) chits.push({ label: "Resist", value: resistance, tone: "defense" });
  addText(["special"], "Special", "utility");

  return chits;
}

function sumModuleNumbers(modules: string[], specs: Record<string, ModuleFitSpec>, keys: Array<keyof Module>): number {
  return modules.reduce((sum, moduleId) => {
    const spec = specs[moduleId];
    const value = spec ? moduleNumber(spec.source, keys) : null;
    return sum + (value ?? 0);
  }, 0);
}

function addMaybe(base: number | null, bonus: number): number | null {
  if (base == null && bonus === 0) return null;
  return (base ?? 0) + bonus;
}

function fittedStats(ship: ShipCatalogEntry | null, modules: string[], specs: Record<string, ModuleFitSpec>, validation: FittingValidation): FittedStat[] {
  if (!ship) return [];
  const hull = addMaybe(shipHullValue(ship), sumModuleNumbers(modules, specs, ["hull_bonus"]) - sumModuleNumbers(modules, specs, ["hull_penalty"]));
  const shield = addMaybe(shipShieldValue(ship), sumModuleNumbers(modules, specs, ["shield_bonus"]));
  const shieldRegen = addMaybe(ship.source.base_shield_recharge ?? null, sumModuleNumbers(modules, specs, ["shield_recharge_bonus"]));
  const armor = addMaybe(ship.source.base_armor ?? null, sumModuleNumbers(modules, specs, ["armor_bonus"]));
  const fuel = addMaybe(shipFuelCapacity(ship), sumModuleNumbers(modules, specs, ["max_fuel_bonus"]));
  const cargo = addMaybe(shipCargoValue(ship), sumModuleNumbers(modules, specs, ["cargo_bonus"]));
  const speed = addMaybe(
    shipSpeedValue(ship),
    sumModuleNumbers(modules, specs, ["speed_bonus"]) - sumModuleNumbers(modules, specs, ["speed_penalty", "tow_speed_penalty"]),
  );
  const drones = sumModuleNumbers(modules, specs, ["drone_capacity"]);
  const bandwidth = sumModuleNumbers(modules, specs, ["drone_bandwidth"]);
  const passengers = sumModuleNumbers(modules, specs, ["passenger_economy_berths", "passenger_business_berths", "passenger_first_berths"]);
  const mining = sumModuleNumbers(modules, specs, ["mining_power"]);
  const survey = sumModuleNumbers(modules, specs, ["survey_power"]);
  const scanner = sumModuleNumbers(modules, specs, ["scanner_power"]);
  const cloak = sumModuleNumbers(modules, specs, ["cloak_strength"]);
  const damageReduction = sumModuleNumbers(modules, specs, ["damage_reduction"]);
  const warp = sumModuleNumbers(modules, specs, ["warp_stabilization"]);

  return [
    { label: "Hull", value: formatCatalogValue(hull), tone: "hull", show: true },
    { label: "Shield", value: formatCatalogValue(shield), tone: "shield", show: true },
    { label: "Shield regen", value: formatCatalogValue(shieldRegen), tone: "shield", show: true },
    { label: "Armor", value: formatCatalogValue(armor), tone: "hull", show: armor != null },
    { label: "Fuel", value: formatCatalogValue(fuel), tone: "fuel", show: true },
    { label: "Cargo", value: formatCatalogValue(cargo), tone: "cargo", show: true },
    { label: "Speed", value: formatCatalogValue(speed), tone: "fuel", show: true },
    { label: "Weapon", value: `${validation.slotCounts.weapon}/${validation.slotLimits.weapon ?? "?"}`, tone: "slots", show: true },
    { label: "Defense", value: `${validation.slotCounts.defense}/${validation.slotLimits.defense ?? "?"}`, tone: "slots", show: true },
    { label: "Utility", value: `${validation.slotCounts.utility}/${validation.slotLimits.utility ?? "?"}`, tone: "slots", show: true },
    { label: "CPU", value: formatUsedLimit(validation.cpuUsed, validation.cpuLimit), tone: "cpu", show: true },
    { label: "Power", value: formatUsedLimit(validation.powerUsed, validation.powerLimit), tone: "power", show: true },
    { label: "Drones", value: bandwidth ? `${drones}/${bandwidth}` : formatCatalogValue(drones), tone: "slots", show: drones > 0 || bandwidth > 0 },
    { label: "Passengers", value: formatCatalogValue(passengers), tone: "cargo", show: passengers > 0 },
    { label: "Mining", value: formatCatalogValue(mining), tone: "slots", show: mining > 0 },
    { label: "Survey", value: formatCatalogValue(survey), tone: "slots", show: survey > 0 },
    { label: "Scanner", value: formatCatalogValue(scanner), tone: "slots", show: scanner > 0 },
    { label: "Cloak", value: formatCatalogValue(cloak), tone: "cpu", show: cloak > 0 },
    { label: "DR", value: formatCatalogValue(damageReduction), tone: "shield", show: damageReduction > 0 },
    { label: "Warp stab", value: formatCatalogValue(warp), tone: "power", show: warp > 0 },
  ];
}

function emptySlotCounts(): Record<FittingSlotType, number> {
  return { weapon: 0, defense: 0, utility: 0 };
}

export function validateFitting(chassisClassId: string, modules: string[], ships: ShipCatalogEntry[], specs: Record<string, ModuleFitSpec>): FittingValidation {
  const ship = findCatalogShip(ships, chassisClassId);
  const slotCounts = emptySlotCounts();
  const slotLimits = {
    weapon: ship?.weaponSlots ?? null,
    defense: ship?.defenseSlots ?? null,
    utility: ship?.utilitySlots ?? null,
  };
  const errors: string[] = [];
  const warnings: string[] = [];
  let cpuUsed = 0;
  let powerUsed = 0;
  let cpuKnown = true;
  let powerKnown = true;
  let cpuCapacityBonus = 0;
  let powerCapacityBonus = 0;

  if (!chassisClassId.trim()) {
    errors.push("choose a ship chassis");
  } else if (!ship) {
    errors.push(`unknown chassis ${chassisClassId}`);
  }

  for (const moduleId of modules) {
    const spec = specs[moduleId];
    if (!spec) {
      errors.push(`unknown module ${moduleId}`);
      continue;
    }
    if (!spec.slot) {
      errors.push(`${moduleId} has no catalog slot type`);
    } else {
      slotCounts[spec.slot] += 1;
    }
    if (spec.cpu == null) {
      cpuKnown = false;
      warnings.push(`${moduleId} has no CPU usage in catalog`);
    } else {
      cpuUsed += spec.cpu;
    }
    if (spec.power == null) {
      powerKnown = false;
      warnings.push(`${moduleId} has no power usage in catalog`);
    } else {
      powerUsed += spec.power;
    }
    cpuCapacityBonus += spec.source.cpu_bonus ?? 0;
    powerCapacityBonus += spec.source.power_bonus ?? 0;
  }

  for (const slot of ["weapon", "defense", "utility"] as FittingSlotType[]) {
    const limit = slotLimits[slot];
    if (limit == null) {
      if (ship) warnings.push(`${formatFilterLabel(slot)} slot capacity is missing from catalog`);
      continue;
    }
    if (slotCounts[slot] > limit) {
      errors.push(`${formatFilterLabel(slot)} slots ${slotCounts[slot]}/${limit}`);
    }
  }

  const cpuLimit = addMaybe(ship?.cpuCapacity ?? null, cpuCapacityBonus);
  const powerLimit = addMaybe(ship?.powerCapacity ?? null, powerCapacityBonus);

  if (cpuLimit == null) {
    if (ship) warnings.push("CPU capacity is missing from catalog");
  } else if (cpuKnown && cpuUsed > cpuLimit) {
    errors.push(`CPU ${cpuUsed}/${cpuLimit}`);
  }

  if (powerLimit == null) {
    if (ship) warnings.push("Power capacity is missing from catalog");
  } else if (powerKnown && powerUsed > powerLimit) {
    errors.push(`power ${powerUsed}/${powerLimit}`);
  }

  return {
    ship,
    modules,
    slotCounts,
    slotLimits,
    cpuUsed: cpuKnown ? cpuUsed : null,
    cpuLimit,
    powerUsed: powerKnown ? powerUsed : null,
    powerLimit,
    errors,
    warnings,
  };
}

function formatUsedLimit(used: number | null, limit: number | null): string {
  return `${used == null ? "?" : used.toLocaleString()}/${limit == null ? "?" : limit.toLocaleString()}`;
}

function formatCatalogValue(value: number | null | undefined): string {
  return value == null ? "?" : value.toLocaleString();
}

function shipEmpire(ship: ShipCatalogEntry): string {
  return ship.source.faction ?? "unaffiliated";
}

function shipHullValue(ship: ShipCatalogEntry): number | null {
  return ship.hull ?? ship.baseHull;
}

function shipShieldValue(ship: ShipCatalogEntry): number | null {
  return ship.shield ?? ship.baseShield;
}

function shipFuelCapacity(ship: ShipCatalogEntry): number | null {
  return ship.source.base_fuel ?? null;
}

function shipCargoValue(ship: ShipCatalogEntry): number | null {
  return ship.cargo ?? ship.cargoCapacity;
}

function shipSpeedValue(ship: ShipCatalogEntry): number | null {
  return ship.speed ?? ship.baseSpeed;
}

function chassisCompactStats(ship: ShipCatalogEntry): Array<{ label: string; value: string; tone: string }> {
  return [
    { label: "Empire", value: formatFilterLabel(shipEmpire(ship)), tone: "empire" },
    { label: "Hull", value: formatCatalogValue(shipHullValue(ship)), tone: "hull" },
    { label: "Shield", value: formatCatalogValue(shipShieldValue(ship)), tone: "shield" },
    { label: "Fuel", value: formatCatalogValue(shipFuelCapacity(ship)), tone: "fuel" },
    { label: "Cargo", value: formatCatalogValue(shipCargoValue(ship)), tone: "cargo" },
    { label: "Speed", value: formatCatalogValue(shipSpeedValue(ship)), tone: "fuel" },
    {
      label: "Slots",
      value: `W${formatCatalogValue(ship.weaponSlots)} D${formatCatalogValue(ship.defenseSlots)} U${formatCatalogValue(ship.utilitySlots)}`,
      tone: "slots",
    },
    { label: "CPU", value: formatCatalogValue(ship.cpuCapacity), tone: "cpu" },
    { label: "Power", value: formatCatalogValue(ship.powerCapacity), tone: "power" },
  ];
}

function minimumFilterPass(value: number | null, minimum: string): boolean {
  const trimmed = minimum.trim();
  if (!trimmed) return true;
  const required = Number(trimmed);
  if (!Number.isFinite(required)) return true;
  return value != null && value >= required;
}

function shipMatchesDesignerFilters(ship: ShipCatalogEntry, filters: DesignerFilters): boolean {
  const needle = filters.query.trim().toLowerCase();
  if (needle && ![ship.id, ship.name, ship.className, ship.classId, ship.category, shipEmpire(ship)].some((value) => value.toLowerCase().includes(needle)))
    return false;
  if (filters.empire && shipEmpire(ship) !== filters.empire) return false;
  return (
    minimumFilterPass(shipHullValue(ship), filters.hull) &&
    minimumFilterPass(shipShieldValue(ship), filters.shield) &&
    minimumFilterPass(shipFuelCapacity(ship), filters.fuel) &&
    minimumFilterPass(shipCargoValue(ship), filters.cargo) &&
    minimumFilterPass(shipSpeedValue(ship), filters.speed) &&
    minimumFilterPass(ship.weaponSlots, filters.weapon) &&
    minimumFilterPass(ship.defenseSlots, filters.defense) &&
    minimumFilterPass(ship.utilitySlots, filters.utility) &&
    minimumFilterPass(ship.cpuCapacity, filters.cpu) &&
    minimumFilterPass(ship.powerCapacity, filters.power)
  );
}

function moduleMatchesDesignerFilters(
  moduleId: string,
  specs: Record<string, ModuleFitSpec>,
  names: Record<string, string>,
  query: string,
  slotFilter: ModuleFilterMode,
): boolean {
  const spec = specs[moduleId];
  if (slotFilter !== "all" && spec?.slot !== slotFilter) return false;
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [moduleId, names[moduleId], spec?.name, spec?.slot ?? "", moduleEffectSummary(spec)].some((value) => (value ?? "").toLowerCase().includes(needle));
}

function moduleAddLimitReasons(
  chassisId: string,
  modules: string[],
  moduleId: string,
  ships: ShipCatalogEntry[],
  specs: Record<string, ModuleFitSpec>,
): string[] {
  const preview = validateFitting(chassisId, [...modules, moduleId], ships, specs);
  return preview.errors.filter(
    (error) =>
      error.startsWith("Weapon slots") ||
      error.startsWith("Defense slots") ||
      error.startsWith("Utility slots") ||
      error.startsWith("CPU ") ||
      error.startsWith("power "),
  );
}

function addDemand(demand: Map<string, { quantity: number; label: string }>, itemId: string, quantity: number, label?: string) {
  const normalized = itemId.trim();
  if (!normalized || !Number.isFinite(quantity) || quantity <= 0) return;
  const existing = demand.get(normalized);
  if (existing) {
    existing.quantity += quantity;
    if (label && existing.label === normalized) existing.label = label;
  } else {
    demand.set(normalized, { quantity, label: label || normalized });
  }
}

function designerDemand(
  ship: ShipCatalogEntry | null,
  modules: string[],
  moduleNames: Record<string, string>,
): Map<string, { quantity: number; label: string }> {
  const demand = new Map<string, { quantity: number; label: string }>();
  if (ship) {
    for (const [itemId, quantity] of Object.entries(ship.materials)) {
      addDemand(demand, itemId, quantity, formatFilterLabel(itemId));
    }
    if (Object.keys(ship.materials).length === 0) {
      for (const ingredient of ship.ingredients.length ? ship.ingredients : ship.inputs) {
        const itemId = ingredient.itemId || ingredient.id || ingredient.item || ingredient.name;
        const quantity = ingredient.quantity ?? ingredient.amount ?? ingredient.count ?? 0;
        addDemand(demand, itemId, quantity, ingredient.name || ingredient.item || itemId);
      }
    }
  }
  for (const moduleId of modules) {
    addDemand(demand, moduleId, 1, moduleNames[moduleId] || moduleId);
  }
  return demand;
}

function factionStorageAtStation(commanderStorage: CommanderStorageView | null, stationId: string): Map<string, number> {
  const available = new Map<string, number>();
  const normalizedStation = normalizeLocationKey(stationId);
  if (!normalizedStation) return available;
  for (const row of commanderStorage?.rows ?? []) {
    if (row.sourceKind !== "faction" || row.quantity <= 0 || !row.itemId) continue;
    const rowLocation = normalizeLocationKey(row.locationId);
    const rowName = normalizeLocationKey(row.locationName);
    if (rowLocation !== normalizedStation && rowName !== normalizedStation) continue;
    available.set(row.itemId, (available.get(row.itemId) ?? 0) + row.quantity);
  }
  return available;
}

function subtractAvailableDemand(
  demand: Map<string, { quantity: number; label: string }>,
  available: Map<string, number>,
): Map<string, { quantity: number; label: string }> {
  const net = new Map<string, { quantity: number; label: string }>();
  for (const [itemId, entry] of demand.entries()) {
    const remaining = Math.max(0, entry.quantity - (available.get(itemId) ?? 0));
    if (remaining > 0) {
      net.set(itemId, { ...entry, quantity: remaining });
    }
  }
  return net;
}

function priceItemFromMarket(itemId: string, label: string, quantity: number, market: EconomyMarketData): DesignerPriceRow {
  const asks = Object.values(market.marketsByStation)
    .flatMap((station) =>
      (station.sellOrders[itemId] ?? []).map((order) => ({
        stationId: station.stationId,
        priceEach: order.price_each,
        quantity: order.quantity,
      })),
    )
    .filter((order) => order.quantity > 0 && Number.isFinite(order.priceEach))
    .sort((a, b) => a.priceEach - b.priceEach || a.stationId.localeCompare(b.stationId));

  const requestedQuantity = quantity;
  const coverTarget = quantity * 2;
  let remaining = requestedQuantity;
  let coverRemaining = coverTarget;
  let requestedTotal = 0;
  let coverTotal = 0;
  let pricedQuantity = 0;
  let coverQuantity = 0;
  const stations = new Set<string>();
  for (const ask of asks) {
    if (coverRemaining <= 0) break;
    const coverTake = Math.min(coverRemaining, ask.quantity);
    coverTotal += coverTake * ask.priceEach;
    coverQuantity += coverTake;
    coverRemaining -= coverTake;

    if (remaining > 0) {
      const requestedTake = Math.min(remaining, ask.quantity);
      requestedTotal += requestedTake * ask.priceEach;
      pricedQuantity += requestedTake;
      remaining -= requestedTake;
    }
    stations.add(ask.stationId);
  }
  const unitPrice = coverQuantity > 0 ? coverTotal / coverQuantity : null;
  const orderQuantity = Math.ceil(requestedQuantity);
  const orderPriceEach = unitPrice == null ? null : Math.ceil(unitPrice * SHIPYARD_VIRTUAL_ORDER_PADDING);
  const rawTotal = unitPrice == null ? requestedTotal : unitPrice * requestedQuantity;
  const estimatedTotal = orderPriceEach == null ? Math.ceil(rawTotal * SHIPYARD_VIRTUAL_ORDER_PADDING) : orderPriceEach * orderQuantity;

  return {
    itemId,
    label,
    quantity: requestedQuantity,
    orderQuantity,
    pricedQuantity,
    coverQuantity,
    missingQuantity: Math.max(0, remaining),
    total: estimatedTotal,
    rawTotal,
    unitPrice,
    orderPriceEach,
    bestPrice: asks[0]?.priceEach ?? null,
    source: stations.size ? `${formatDps(coverQuantity)} cover / ${stations.size} station${stations.size === 1 ? "" : "s"}` : "no sell orders",
  };
}

function newShipyardVirtualOrderId(): string {
  const random = Math.random().toString(36).slice(2, 8);
  return `shipyard_${Date.now().toString(36)}_${random}`;
}

function mergeShipyardVirtualBuyOrders(existing: VirtualFactionOrderInput[], stationId: string, rows: DesignerPriceRow[]): VirtualFactionOrderInput[] {
  const next = [...existing];
  for (const row of rows) {
    if (row.orderPriceEach == null || row.orderQuantity <= 0) continue;
    const current = next.find(
      (order) => order.side === "buy" && order.stationId === stationId && order.itemId === row.itemId && order.id.startsWith("shipyard_"),
    );
    if (current) {
      current.quantity = Math.max(current.quantity, row.orderQuantity);
      current.priceEach = Math.max(current.priceEach, row.orderPriceEach);
      current.enabled = true;
      current.internalOnly = true;
      current.priority = Math.max(current.priority ?? 1, 1);
      continue;
    }
    next.push({
      id: newShipyardVirtualOrderId(),
      side: "buy",
      itemId: row.itemId,
      stationId,
      priceEach: row.orderPriceEach,
      quantity: row.orderQuantity,
      reserved: 0,
      filled: 0,
      enabled: true,
      internalOnly: true,
      priority: 1,
      doForever: false,
    });
  }
  return next;
}

function priceDesignerParts(
  ship: ShipCatalogEntry | null,
  modules: string[],
  moduleNames: Record<string, string>,
  market: EconomyMarketData,
): DesignerPriceQuote {
  return priceDemandParts(designerDemand(ship, modules, moduleNames), market);
}

function priceDemandParts(demand: Map<string, { quantity: number; label: string }>, market: EconomyMarketData): DesignerPriceQuote {
  const rows = Array.from(demand.entries())
    .map(([itemId, entry]) => priceItemFromMarket(itemId, entry.label, entry.quantity, market))
    .sort((a, b) => b.total - a.total || a.label.localeCompare(b.label) || a.itemId.localeCompare(b.itemId));
  return {
    total: rows.reduce((sum, row) => sum + row.total, 0),
    rows,
    missingRows: rows.filter((row) => row.missingQuantity > 0),
    pricedAt: Date.now(),
  };
}

function missingLoadoutModules(
  storage: StorageSessionState | null | undefined,
  modules: string[],
  handle: string,
  commanderStorage: CommanderStorageView | null,
  yard: ShipyardData | undefined,
): string[] {
  const sourcesByModule = new Map<string, LoadoutModuleSource[]>();
  const missing: string[] = [];
  for (const moduleId of modules) {
    let sources = sourcesByModule.get(moduleId);
    if (!sources) {
      sources = loadoutModuleSources(handle, moduleId, storage ?? undefined, commanderStorage, {}, yard);
      sourcesByModule.set(moduleId, sources);
    }
    const source = sources.find((candidate) => candidate.quantity > 0);
    if (!source) {
      missing.push(moduleId);
      continue;
    }
    source.quantity -= 1;
  }
  return missing;
}

function rowJumpDistance(row: CommanderStorageRow, routeDistances: Record<string, number>): number {
  const routed = routeDistances[row.locationId];
  if (typeof routed === "number" && Number.isFinite(routed)) return routed;
  const jumps = row.details?.["jumps"];
  return typeof jumps === "number" && Number.isFinite(jumps) ? jumps : Number.MAX_SAFE_INTEGER;
}

function personalStorageRowMatches(row: CommanderStorageRow, handle: string, storage: StorageSessionState | undefined): boolean {
  return row.sourceKind === "personal" && (row.observedBy.includes(handle) || (!!storage?.username && row.ownerName === storage.username));
}

function factionStorageRowMatches(row: CommanderStorageRow, storage: StorageSessionState | undefined): boolean {
  return row.sourceKind === "faction" && (!storage?.factionId || row.ownerId === storage.factionId || row.ownerName === storage.factionId);
}

function loadoutModuleSources(
  handle: string,
  moduleId: string,
  storage: StorageSessionState | undefined,
  commanderStorage: CommanderStorageView | null,
  routeDistances: Record<string, number>,
  yard: ShipyardData | undefined,
): LoadoutModuleSource[] {
  const sources: LoadoutModuleSource[] = [];
  let order = 0;
  const installedQuantity = yard?.installedModules.filter((installed) => installed === moduleId).length ?? 0;
  if (installedQuantity > 0) {
    sources.push({
      kind: "installed",
      itemId: moduleId,
      quantity: installedQuantity,
      locationId: "installed",
      distance: -2,
      order: order++,
    });
  }

  const cargoQuantity = storage?.cargo[moduleId] ?? 0;
  if (cargoQuantity > 0) {
    sources.push({
      kind: "cargo",
      itemId: moduleId,
      quantity: cargoQuantity,
      locationId: "cargo",
      distance: -1,
      order: order++,
    });
  }

  for (const row of commanderStorage?.rows ?? []) {
    if (row.itemId !== moduleId || row.quantity <= 0 || !row.locationId || row.locationId === "shared") continue;
    if (personalStorageRowMatches(row, handle, storage)) {
      sources.push({
        kind: "personal",
        itemId: moduleId,
        quantity: row.quantity,
        locationId: row.locationId,
        distance: rowJumpDistance(row, routeDistances),
        order: order++,
      });
    } else if (factionStorageRowMatches(row, storage)) {
      sources.push({
        kind: "faction",
        itemId: moduleId,
        quantity: row.quantity,
        locationId: row.locationId,
        distance: rowJumpDistance(row, routeDistances),
        order: order++,
      });
    }
  }

  if (!commanderStorage) {
    for (const [poiId, items] of Object.entries(storage?.storageByPoi ?? {})) {
      const quantity = items[moduleId] ?? 0;
      if (quantity <= 0) continue;
      sources.push({
        kind: "personal",
        itemId: moduleId,
        quantity,
        locationId: poiId,
        distance: routeDistances[poiId] ?? Number.MAX_SAFE_INTEGER,
        order: order++,
      });
    }
  }

  return sources.sort((a, b) => a.distance - b.distance || a.kind.localeCompare(b.kind) || a.locationId.localeCompare(b.locationId) || a.order - b.order);
}

export function collectModuleCatalog(catalog: { items: CatalogEntry[]; ships: ShipCatalogEntry[] } | null | undefined): {
  ids: Set<string>;
  names: Record<string, string>;
  specs: Record<string, ModuleFitSpec>;
} {
  const ids = new Set<string>();
  const names: Record<string, string> = {};
  const specs: Record<string, ModuleFitSpec> = {};
  const items = catalog?.items ?? [];
  const itemsById = new Map(items.map((item) => [item.id, item]));
  for (const item of items) {
    if (!catalogEntryIsModule(item)) continue;
    ids.add(item.id);
    names[item.id] = item.name || item.id;
    specs[item.id] = moduleFitSpec(item);
  }
  for (const ship of catalog?.ships ?? []) {
    for (const moduleId of ship.defaultModules) {
      if (!moduleId) continue;
      ids.add(moduleId);
      const item = itemsById.get(moduleId);
      if (item && !specs[moduleId]) {
        names[moduleId] = item.name || moduleId;
        specs[moduleId] = moduleFitSpec(item);
      } else if (!names[moduleId]) {
        names[moduleId] = moduleId;
      }
    }
  }
  const categoryCounts = items.reduce<Record<string, number>>((counts, item) => {
    const key = item.category || "(empty)";
    counts[key] = (counts[key] ?? 0) + 1;
    return counts;
  }, {});
  const cargoExpander = items.find((item) => item.id === "cargo_expander_iii");
  if (ids.size === 0 || cargoExpander) {
    console.warn("[shipyard-loadout] module catalog telemetry", {
      itemCount: items.length,
      shipCount: catalog?.ships.length ?? 0,
      moduleCount: ids.size,
      categoryCounts,
      sampleItems: items.slice(0, 8).map((item) => ({
        id: item.id,
        name: item.name,
        category: item.category,
        typeName: item.typeName,
        className: item.className,
        classId: item.classId,
        sourceKeys: Object.keys(item.source),
      })),
      cargoExpander: cargoExpander
        ? {
            id: cargoExpander.id,
            name: cargoExpander.name,
            category: cargoExpander.category,
            typeName: cargoExpander.typeName,
            className: cargoExpander.className,
            classId: cargoExpander.classId,
            sourceKeys: Object.keys(cargoExpander.source),
            source: cargoExpander.source,
          }
        : null,
    });
  }
  return { ids, names, specs };
}

function loadoutCatalogTelemetryStatus(
  catalog: { items: CatalogEntry[]; ships: ShipCatalogEntry[] } | null | undefined,
  modules: { ids: Set<string>; names: Record<string, string> },
): string | null {
  if (!catalog) return "Catalog unavailable; loadout module list is empty.";
  if (modules.ids.size > 0) return null;
  const cargoExpander = catalog.items.find((item) => item.id === "cargo_expander_iii");
  const cargoDetail = cargoExpander
    ? ` cargo_expander_iii type=${cargoExpander.typeName || "(empty)"} category=${cargoExpander.category || "(empty)"}.`
    : " cargo_expander_iii was not present.";
  return `Catalog parsed ${catalog.items.length} items but 0 modules.${cargoDetail}`;
}

function moduleDisplayLabel(moduleId: string, names: Record<string, string>): string {
  const name = names[moduleId]?.trim();
  return name && name !== moduleId ? `${name} / ${moduleId}` : moduleId;
}

function loadoutModuleAvailableCount(
  handle: string,
  storage: StorageSessionState | undefined,
  moduleId: string,
  commanderStorage: CommanderStorageView | null,
  yard: ShipyardData | undefined,
): number {
  if (!moduleId) return 0;
  return loadoutModuleSources(handle, moduleId, storage, commanderStorage, {}, yard).reduce((sum, source) => sum + source.quantity, 0);
}

function normalizeLocationKey(value: string | null | undefined): string {
  return (value ?? "").trim().toLowerCase();
}

function shipAtCurrentBase(ship: OwnedShipInfo, yard: ShipyardData | undefined): boolean {
  if (!yard || ship.isActive || ship.listingId) return false;
  const currentBaseId = normalizeLocationKey(yard.currentBaseId);
  if (currentBaseId) {
    return normalizeLocationKey(ship.locationBaseId) === currentBaseId;
  }

  const currentBaseName = normalizeLocationKey(yard.currentBaseName);
  if (currentBaseName) {
    return [ship.location, ship.locationBaseId].some((value) => normalizeLocationKey(value) === currentBaseName);
  }

  return true;
}

function emptyLoadoutRow(): LoadoutRow {
  return { chassisClassId: "", modules: "" };
}

function normalizeLoadoutRow(row: LoadoutRow): LoadoutRow {
  return {
    chassisClassId: row.chassisClassId.trim(),
    modules: splitModules(row.modules).join("\n"),
  };
}

function loadoutRowsEqual(left: LoadoutRow, right: LoadoutRow): boolean {
  const normalizedLeft = normalizeLoadoutRow(left);
  const normalizedRight = normalizeLoadoutRow(right);
  return normalizedLeft.chassisClassId === normalizedRight.chassisClassId && normalizedLeft.modules === normalizedRight.modules;
}

function readSavedShipyardLoadouts(): Record<string, SavedShipyardLoadout> {
  return readVersionedStored(
    SHIPYARD_LOADOUTS_KEY,
    1,
    (parsed) => {
      if (!isRecord(parsed)) return null;
      const loadouts: Record<string, SavedShipyardLoadout> = {};
      for (const [squadId, value] of Object.entries(parsed)) {
        if (!isRecord(value)) return null;
        const source = value;
        if (!Array.isArray(source["handles"]) || !source["handles"].every((item) => typeof item === "string" && item.trim().length > 0)) return null;
        const handles = source["handles"];
        const rowsSource = source["rows"];
        if (!isRecord(rowsSource)) return null;
        const rows: Record<string, LoadoutRow> = {};
        for (const [handle, rowValue] of Object.entries(rowsSource)) {
          if (!isRecord(rowValue) || typeof rowValue["chassisClassId"] !== "string" || typeof rowValue["modules"] !== "string") return null;
          rows[handle] = { chassisClassId: rowValue["chassisClassId"], modules: rowValue["modules"] };
        }
        loadouts[squadId] = { handles, rows };
      }
      return loadouts;
    },
    {},
  );
}

function saveShipyardLoadout(squad: LoadoutSquadOption, rowsByHandle: Record<string, LoadoutRow>): boolean {
  try {
    const loadouts = readSavedShipyardLoadouts();
    const rows: Record<string, LoadoutRow> = {};
    for (const handle of squad.handles) {
      const row = rowsByHandle[handle] ?? emptyLoadoutRow();
      rows[handle] = {
        chassisClassId: row.chassisClassId.trim(),
        modules: row.modules.trim(),
      };
    }
    loadouts[squad.id] = { handles: squad.handles, rows };
    return writeVersionedStored(SHIPYARD_LOADOUTS_KEY, 1, loadouts);
  } catch {
    return false;
  }
}

function readSavedShipyardDesigns(): SavedShipyardDesign[] {
  return readVersionedStored(
    SHIPYARD_DESIGNS_KEY,
    1,
    (parsed) => {
      if (!Array.isArray(parsed)) return null;
      const designs: SavedShipyardDesign[] = [];
      for (const value of parsed) {
        if (!isRecord(value) || !isRecord(value["row"])) return null;
        const row = value["row"];
        if (
          typeof value["id"] !== "string" ||
          !value["id"].trim() ||
          typeof value["name"] !== "string" ||
          !value["name"].trim() ||
          typeof row["chassisClassId"] !== "string" ||
          !row["chassisClassId"].trim() ||
          typeof row["modules"] !== "string" ||
          typeof value["savedAt"] !== "number" ||
          !Number.isFinite(value["savedAt"])
        )
          return null;
        designs.push({
          id: value["id"].trim(),
          name: value["name"].trim(),
          row: { chassisClassId: row["chassisClassId"], modules: row["modules"] },
          savedAt: value["savedAt"],
        });
      }
      return designs.sort((a, b) => b.savedAt - a.savedAt || a.name.localeCompare(b.name));
    },
    [],
  );
}

function writeSavedShipyardDesigns(designs: SavedShipyardDesign[]): boolean {
  try {
    return writeVersionedStored(SHIPYARD_DESIGNS_KEY, 1, designs);
  } catch {
    return false;
  }
}

function isFactionGarageShip(ship: OwnedShipInfo): boolean {
  return ship.isGaraged || ship.ownerKind === "faction_garage";
}

function factionGarageShipHasStation(ship: OwnedShipInfo): boolean {
  return !isFactionGarageShip(ship) || Boolean(ship.locationBaseId);
}

function fleetGarageShipAsFleetRow(ship: FactionGarageShipInfo): OwnedShipInfo {
  const owner = ship.factionTag || ship.factionId || "faction garage";
  return {
    owner,
    ownerHandle: ship.ownerHandle || owner,
    shipId: ship.shipId,
    classId: ship.classId,
    location: ship.baseName || ship.systemName || "Faction garage",
    locationBaseId: ship.baseId,
    ownerKind: "faction_garage",
    ownerId: ship.depositorId,
    ownerName: ship.depositorName,
    factionId: ship.factionId,
    factionTag: ship.factionTag,
    active: false,
    isActive: false,
    isGaraged: true,
    className: ship.className,
    customName: ship.customName,
    fuel: "",
    hull: "",
    cargoUsed: null,
    modules: null,
    listingId: "",
    listingBaseId: "",
    listingPrice: null,
  };
}

function buildFleetLoadoutShipOptions(fleetData: ShipyardFleetData | null): LoadoutShipOption[] {
  return (fleetData?.ships ?? [])
    .filter((ship) => ship.shipId && !ship.listingId)
    .map((ship) => ({
      ship,
      ownerHandle: ship.ownerHandle || ship.ownerName || ship.owner || "unknown",
    }))
    .sort(
      (a, b) =>
        Number(isFactionGarageShip(b.ship)) - Number(isFactionGarageShip(a.ship)) ||
        Number(a.ship.isActive) - Number(b.ship.isActive) ||
        a.ownerHandle.localeCompare(b.ownerHandle) ||
        shipClass(a.ship).localeCompare(shipClass(b.ship)) ||
        shipLabel(a.ship).localeCompare(shipLabel(b.ship)),
    );
}

function loadoutOptionsForHandle(options: LoadoutShipOption[], handle: string): LoadoutShipOption[] {
  return options.filter((option) => isFactionGarageShip(option.ship) || option.ownerHandle === handle);
}

export default function ShipyardPanel({ sessions, onChanged }: ShipyardPanelProps) {
  const squads = useSquads();
  const prayer = usePrayer();
  const galaxyMap = selectGalaxyMap(prayer.galaxyMap);
  const readCatalog = useCallback(async (_handle?: string) => selectCatalog(prayer.catalog), [prayer.catalog]);
  const readEconomyMarket = useCallback(
    async (_handle?: string, _fresh?: boolean) => selectEconomyMarket(prayer.stationMarkets, prayer.galaxyMap),
    [prayer.galaxyMap, prayer.stationMarkets],
  );
  const readShipyard = useCallback(
    async (handle: string) => {
      const bot = prayer.fleet.find((candidate) => candidate.id === handle || candidate.username === handle) ?? null;
      const yard = selectShipyard(bot);
      if (!yard) throw new Error(`Shipyard state is unavailable for ${handle}.`);
      return yard;
    },
    [prayer.fleet],
  );
  const readShipyardFleet = useCallback(async () => selectShipyardFleet(prayer.fleet), [prayer.fleet]);
  const readStorage = useCallback(
    async (handle: string) => {
      const bot = prayer.fleet.find((candidate) => candidate.id === handle || candidate.username === handle) ?? null;
      return selectStorageState(bot, prayer.storageByPlayer, prayer.factionStorageByFactionPoi);
    },
    [prayer.factionStorageByFactionPoi, prayer.fleet, prayer.storageByPlayer],
  );
  const readCommanderStorage = useCallback(
    async () => selectCommanderStorage(prayer.fleet, prayer.galaxyMap, prayer.storageByPlayer, prayer.factionStorageByFactionPoi, prayer.factionBySession),
    [prayer.factionBySession, prayer.factionStorageByFactionPoi, prayer.fleet, prayer.galaxyMap, prayer.storageByPlayer],
  );
  const executeShipyardScript = useCallback(
    async (handle: string, script: string, _maxSteps: number) => {
      const bot = await prayer.bot(handle);
      const run = await bot.startScript(script, { idempotencyKey: crypto.randomUUID() });
      const terminal = await run.wait();
      if (terminal.status !== "succeeded") throw new Error(`Shipyard run ${run.id} ${terminal.status}.`);
      await prayer.refresh();
      return terminal;
    },
    [prayer],
  );
  const [pane, setPane] = useState<PaneMode>("ships");
  const [fleetData, setFleetData] = useState<ShipyardFleetData | null>(null);
  const [fleetLoading, setFleetLoading] = useState(false);
  const [designerChassisId, setDesignerChassisId] = useState("");
  const [designerModules, setDesignerModules] = useState("");
  const [designerFilters, setDesignerFilters] = useState<DesignerFilters>(emptyDesignerFilters);
  const [designerSaveName, setDesignerSaveName] = useState("");
  const [savedDesigns, setSavedDesigns] = useState<SavedShipyardDesign[]>(() => readSavedShipyardDesigns());
  const [selectedDesignId, setSelectedDesignId] = useState("");
  const [moduleSearch, setModuleSearch] = useState("");
  const [moduleFilter, setModuleFilter] = useState<ModuleFilterMode>("all");
  const [selectedSquadId, setSelectedSquadId] = useState("");
  const [loadoutRows, setLoadoutRows] = useState<Record<string, LoadoutRow>>({});
  const [loadoutShipyards, setLoadoutShipyards] = useState<Record<string, ShipyardData>>({});
  const [loadoutStorage, setLoadoutStorage] = useState<Record<string, StorageSessionState>>({});
  const [loadoutCommanderStorage, setLoadoutCommanderStorage] = useState<CommanderStorageView | null>(null);
  const [loadoutChassis, setLoadoutChassis] = useState<ShipCatalogEntry[]>([]);
  const [loadoutModuleIds, setLoadoutModuleIds] = useState<Set<string>>(new Set());
  const [loadoutModuleNames, setLoadoutModuleNames] = useState<Record<string, string>>({});
  const [loadoutModuleSpecs, setLoadoutModuleSpecs] = useState<Record<string, ModuleFitSpec>>({});
  const [equippedShipyards, setEquippedShipyards] = useState<Record<string, ShipyardData>>({});
  const [equippedChassis, setEquippedChassis] = useState<ShipCatalogEntry[]>([]);
  const [equippedModuleIds, setEquippedModuleIds] = useState<Set<string>>(new Set());
  const [equippedModuleNames, setEquippedModuleNames] = useState<Record<string, string>>({});
  const [equippedModuleSpecs, setEquippedModuleSpecs] = useState<Record<string, ModuleFitSpec>>({});
  const [equippedLoading, setEquippedLoading] = useState(false);
  const [selectedEquippedHandle, setSelectedEquippedHandle] = useState("");
  const [designerChassis, setDesignerChassis] = useState<ShipCatalogEntry[]>([]);
  const [designerModuleIds, setDesignerModuleIds] = useState<Set<string>>(new Set());
  const [designerModuleNames, setDesignerModuleNames] = useState<Record<string, string>>({});
  const [designerModuleSpecs, setDesignerModuleSpecs] = useState<Record<string, ModuleFitSpec>>({});
  const [designerPriceQuote, setDesignerPriceQuote] = useState<DesignerPriceQuote | null>(null);
  const [designerMarket, setDesignerMarket] = useState<EconomyMarketData | null>(null);
  const [designerVirtualStation, setDesignerVirtualStation] = useState("");
  const [loadoutLoading, setLoadoutLoading] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const loading = false;

  const refreshShipyardFleet = useCallback(
    async (showLoading = true) => {
      if (showLoading) setFleetLoading(true);
      try {
        const next = await readShipyardFleet();
        setFleetData(next);
        if (showLoading) setStatus(null);
      } catch (err) {
        if (showLoading) setStatus(err instanceof Error ? err.message : String(err));
      } finally {
        if (showLoading) setFleetLoading(false);
      }
    },
    [readShipyardFleet],
  );

  const loadDesignerCatalog = useCallback(
    async (handle: string) => {
      if (!handle) {
        setDesignerModuleIds(new Set());
        setDesignerModuleNames({});
        setDesignerModuleSpecs({});
        setDesignerChassis([]);
        return;
      }
      try {
        const catalog = await readCatalog(handle);
        const modules = collectModuleCatalog(catalog);
        setDesignerModuleIds(modules.ids);
        setDesignerModuleNames(modules.names);
        setDesignerModuleSpecs(modules.specs);
        setDesignerChassis(catalog?.ships ?? []);
      } catch {
        setDesignerModuleIds(new Set());
        setDesignerModuleNames({});
        setDesignerModuleSpecs({});
        setDesignerChassis([]);
      }
    },
    [readCatalog],
  );

  useEffect(() => {
    if (pane === "ships") {
      void refreshShipyardFleet();
    }
  }, [pane, refreshShipyardFleet]);

  const designerCatalogHandle = sessions[0]?.sessionHandle ?? "";

  useEffect(() => {
    if (pane === "designer") {
      void loadDesignerCatalog(designerCatalogHandle);
    }
  }, [pane, designerCatalogHandle, loadDesignerCatalog]);

  const loadEquippedView = useCallback(async () => {
    if (!sessions.length) {
      setEquippedShipyards({});
      setEquippedChassis([]);
      setEquippedModuleIds(new Set());
      setEquippedModuleNames({});
      setEquippedModuleSpecs({});
      return;
    }
    setEquippedLoading(true);
    try {
      const handles = sessions.map((session) => session.sessionHandle);
      const [shipyardEntries, catalog] = await Promise.all([
        Promise.all(handles.map(async (handle) => [handle, await readShipyard(handle)] as const)),
        readCatalog(handles[0]).catch(() => null),
      ]);
      const modules = collectModuleCatalog(catalog);
      setEquippedShipyards(Object.fromEntries(shipyardEntries));
      setEquippedChassis(catalog?.ships ?? []);
      setEquippedModuleIds(modules.ids);
      setEquippedModuleNames(modules.names);
      setEquippedModuleSpecs(modules.specs);
      setStatus(loadoutCatalogTelemetryStatus(catalog, modules));
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setEquippedLoading(false);
    }
  }, [readCatalog, readShipyard, sessions]);

  useEffect(() => {
    if (pane === "equipped") {
      void loadEquippedView();
    }
  }, [pane, loadEquippedView]);

  useEffect(() => {
    if (pane !== "designer" || !designerCatalogHandle) return;
    let alive = true;
    readEconomyMarket(designerCatalogHandle, false)
      .then((market) => {
        if (!alive) return;
        setDesignerMarket(market);
      })
      .catch(() => {
        if (alive) setDesignerMarket(null);
      });
    return () => {
      alive = false;
    };
  }, [pane, designerCatalogHandle, readEconomyMarket]);

  useEffect(() => {
    setDesignerPriceQuote(null);
  }, [designerChassisId, designerModules]);

  const squadOptions = useMemo<LoadoutSquadOption[]>(() => {
    const sessionByIdentity = new Map(
      sessions.flatMap((session) => [
        [session.botId, session] as const,
        [session.sessionHandle, session] as const,
        ...(session.username ? ([[session.username, session]] as const) : []),
      ]),
    );
    const assigned = new Set<string>();
    const squadRows = squads.flatMap((squad) => {
      const handles = squad.botIds.flatMap((identity) => {
        const session = sessionByIdentity.get(identity);
        if (!session || assigned.has(session.botId)) return [];
        assigned.add(session.botId);
        return [jobHandleForSession(session)];
      });
      return handles.length ? [{ id: squad.id, name: squad.name, handles }] : [];
    });
    const unassigned = sessions
      .filter((session) => !assigned.has(session.botId))
      .sort((left, right) => jobHandleForSession(left).localeCompare(jobHandleForSession(right)))
      .map(jobHandleForSession);
    const fleetHandles = [...squadRows.flatMap((squad) => squad.handles), ...unassigned];
    if (!fleetHandles.length) return [];
    return [
      { id: "__fleet__", name: "Fleet (by squad)", handles: fleetHandles },
      ...squadRows,
      ...(unassigned.length ? [{ id: "__unassigned__", name: "Unassigned", handles: unassigned }] : []),
    ];
  }, [sessions, squads]);

  useEffect(() => {
    if (!squadOptions.length) {
      setSelectedSquadId("");
      return;
    }
    if (!squadOptions.some((option) => option.id === selectedSquadId)) {
      setSelectedSquadId(squadOptions[0]!.id);
    }
  }, [selectedSquadId, squadOptions]);

  const selectedSquad = useMemo(() => squadOptions.find((option) => option.id === selectedSquadId) ?? null, [selectedSquadId, squadOptions]);
  const selectedSquadHandles = selectedSquad?.handles ?? [];
  const selectedSquadHandlesKey = selectedSquadHandles.join("\n");
  const runningLoadoutHandles = useMemo(
    () => new Set(sessions.filter((session) => session.runningScript?.isRunning === true).map((session) => session.sessionHandle)),
    [sessions],
  );

  useEffect(() => {
    if (pane !== "loadout" || selectedSquadHandles.length === 0) {
      setLoadoutLoading(false);
      return;
    }
    let alive = true;
    const handles = selectedSquadHandles;
    setLoadoutLoading(true);
    let commanderStorageError: string | null = null;
    Promise.all([
      Promise.all(handles.map(async (handle) => [handle, await readShipyard(handle)] as const)),
      Promise.all(handles.map(async (handle) => [handle, await readStorage(handle)] as const)),
      readShipyardFleet(),
      readCatalog(handles[0]).catch(() => null),
      readCommanderStorage().catch((err) => {
        commanderStorageError = err instanceof Error ? err.message : String(err);
        return null;
      }),
    ])
      .then(([shipyardEntries, storageEntries, fleet, catalog, commanderStorage]) => {
        if (!alive) return;
        setLoadoutShipyards(Object.fromEntries(shipyardEntries));
        setFleetData(fleet);
        setLoadoutStorage(Object.fromEntries(storageEntries.filter((entry): entry is readonly [string, StorageSessionState] => entry[1] !== null)));
        setLoadoutCommanderStorage(commanderStorage);
        setLoadoutChassis(catalog?.ships ?? []);
        const modules = collectModuleCatalog(catalog);
        setLoadoutModuleIds(modules.ids);
        setLoadoutModuleNames(modules.names);
        setLoadoutModuleSpecs(modules.specs);
        const catalogStatus = loadoutCatalogTelemetryStatus(catalog, modules);
        setStatus(
          catalogStatus ?? (commanderStorageError ? `Commander storage unavailable; faction module sourcing is incomplete: ${commanderStorageError}` : null),
        );
      })
      .catch((err) => {
        if (!alive) return;
        setStatus(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (alive) setLoadoutLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [pane, readCatalog, readCommanderStorage, readShipyard, readShipyardFleet, readStorage, selectedSquadId, selectedSquadHandlesKey]);

  useEffect(() => {
    if (selectedSquadHandles.length === 0) return;
    const handles = selectedSquadHandles;
    const savedRows = selectedSquadId ? (readSavedShipyardLoadouts()[selectedSquadId]?.rows ?? {}) : {};
    setLoadoutRows((prev) => {
      const next: Record<string, LoadoutRow> = {};
      for (const handle of handles) {
        next[handle] = savedRows[handle] ?? prev[handle] ?? emptyLoadoutRow();
      }
      return next;
    });
  }, [selectedSquadId, selectedSquadHandlesKey]);

  useEffect(() => {
    if (!designerChassis.length) {
      setDesignerChassisId("");
      setDesignerModules("");
      return;
    }
    if (!designerChassisId || !findCatalogShip(designerChassis, designerChassisId)) {
      const first = designerChassis[0]!;
      setDesignerChassisId(first.id);
      setDesignerModules(first.defaultModules.join("\n"));
    }
  }, [designerChassis, designerChassisId]);

  useEffect(() => {
    if (!savedDesigns.length) {
      setSelectedDesignId("");
      return;
    }
    if (!savedDesigns.some((design) => design.id === selectedDesignId)) {
      setSelectedDesignId(savedDesigns[0]!.id);
    }
  }, [savedDesigns, selectedDesignId]);

  const designerValidation = useMemo(
    () => validateFitting(designerChassisId, splitModules(designerModules), designerChassis, designerModuleSpecs),
    [designerChassisId, designerModules, designerChassis, designerModuleSpecs],
  );
  const designerCurrentModules = useMemo(() => splitModules(designerModules), [designerModules]);
  const designerModuleSlots = moduleInputSlots(designerModules);
  const designerFittedStats = useMemo(
    () => fittedStats(designerValidation.ship, designerCurrentModules, designerModuleSpecs, designerValidation),
    [designerCurrentModules, designerModuleSpecs, designerValidation],
  );
  const designerFilteredChassis = useMemo(
    () => designerChassis.filter((ship) => shipMatchesDesignerFilters(ship, designerFilters)),
    [designerChassis, designerFilters],
  );
  const designerEmpireOptions = useMemo(
    () => Array.from(new Set(designerChassis.map(shipEmpire))).sort((a, b) => formatFilterLabel(a).localeCompare(formatFilterLabel(b))),
    [designerChassis],
  );
  const designerFilterActive = Object.values(designerFilters).some((value) => value.trim().length > 0);
  const designerMeta = `${designerFilteredChassis.length}/${designerChassis.length} catalog ships / ${designerModuleIds.size} modules`;
  const designerFilteredModuleIds = useMemo(
    () =>
      Array.from(designerModuleIds)
        .filter((id) => moduleMatchesDesignerFilters(id, designerModuleSpecs, designerModuleNames, moduleSearch, moduleFilter))
        .sort((a, b) => {
          const aBlocked = moduleAddLimitReasons(designerChassisId, designerCurrentModules, a, designerChassis, designerModuleSpecs).length > 0;
          const bBlocked = moduleAddLimitReasons(designerChassisId, designerCurrentModules, b, designerChassis, designerModuleSpecs).length > 0;
          return (
            Number(aBlocked) - Number(bBlocked) ||
            moduleDisplayLabel(a, designerModuleNames).localeCompare(moduleDisplayLabel(b, designerModuleNames)) ||
            a.localeCompare(b)
          );
        }),
    [designerChassis, designerChassisId, designerCurrentModules, designerModuleIds, designerModuleNames, designerModuleSpecs, moduleFilter, moduleSearch],
  );
  const selectedSavedDesign = useMemo(() => savedDesigns.find((design) => design.id === selectedDesignId) ?? null, [savedDesigns, selectedDesignId]);
  const designerStationOptions = useMemo(
    () =>
      Object.entries(designerMarket?.marketsByStation ?? {})
        .map(([stationKey, stationMarket]) => stationMarket.stationId || stationKey)
        .filter(Boolean)
        .sort((a, b) => a.localeCompare(b)),
    [designerMarket],
  );
  const fleetShipOptions = useMemo<LoadoutShipOption[]>(() => {
    const options = [
      ...(fleetData?.ownedShips ?? []).map((ship) => ({ ship, ownerHandle: ship.ownerHandle || ship.ownerName || ship.owner || "unknown" })),
      ...(fleetData?.factionGarageShips ?? []).map((ship) => ({
        ship: fleetGarageShipAsFleetRow(ship),
        ownerHandle: ship.ownerHandle || ship.factionTag || ship.factionId || "faction garage",
      })),
    ];
    return options.sort(
      (a, b) =>
        Number(isFactionGarageShip(a.ship)) - Number(isFactionGarageShip(b.ship)) ||
        a.ownerHandle.localeCompare(b.ownerHandle) ||
        Number(a.ship.isActive) - Number(b.ship.isActive) ||
        shipClass(a.ship).localeCompare(shipClass(b.ship)) ||
        shipLabel(a.ship).localeCompare(shipLabel(b.ship)),
    );
  }, [fleetData]);
  const fleetStoredCount = fleetShipOptions.filter((option) => !option.ship.isActive && !option.ship.isGaraged).length;
  const fleetGarageCount = fleetShipOptions.filter((option) => option.ship.isGaraged).length;
  const fleetListedCount = fleetShipOptions.filter((option) => option.ship.listingId).length;
  const fleetObservedMeta =
    fleetData?.sessionsObserved == null || fleetData?.sessionsTotal == null ? "" : ` / ${fleetData.sessionsObserved}/${fleetData.sessionsTotal} sessions`;
  const shipsMeta = `${fleetShipOptions.length} ships / ${fleetStoredCount} stored / ${fleetGarageCount} garage / ${fleetListedCount} listed${fleetObservedMeta}`;
  const equippedRows = useMemo(
    () =>
      sessions.map((session) => {
        const yard = equippedShipyards[session.sessionHandle];
        const activeShip = activeYardShip(yard);
        const classId = activeShip?.classId || activeYardClassId(yard);
        const modules = yard?.installedModules ?? [];
        const validation = validateFitting(classId, modules, equippedChassis, equippedModuleSpecs);
        return {
          handle: session.sessionHandle,
          yard,
          activeShip,
          activeShipName: activeYardShipName(yard),
          activeShipClassLabel: activeYardShipClassLabel(yard),
          classId,
          modules,
          validation,
          stats: fittedStats(validation.ship, modules, equippedModuleSpecs, validation).filter((stat) => stat.show),
        };
      }),
    [equippedChassis, equippedModuleSpecs, equippedShipyards, sessions],
  );
  const equippedActiveCount = equippedRows.filter((row) => row.activeShip || row.activeShipName || row.classId).length;
  const equippedMeta = `${equippedActiveCount}/${sessions.length} active ships / ${equippedModuleIds.size} modules`;
  const selectedEquippedRow = equippedRows.find((row) => row.handle === selectedEquippedHandle) ?? equippedRows[0] ?? null;

  useEffect(() => {
    if (pane !== "equipped") return;
    if (!equippedRows.length) {
      setSelectedEquippedHandle("");
      return;
    }
    if (!equippedRows.some((row) => row.handle === selectedEquippedHandle)) {
      setSelectedEquippedHandle(equippedRows[0]!.handle);
    }
  }, [pane, equippedRows, selectedEquippedHandle]);

  const loadoutShipOptions = useMemo<LoadoutShipOption[]>(() => buildFleetLoadoutShipOptions(fleetData), [fleetData]);

  const loadoutWarnings = useMemo(() => {
    const warnings = new Map<string, string[]>();
    for (const handle of selectedSquadHandles) {
      const row = loadoutRows[handle] ?? { chassisClassId: "", modules: "" };
      const rowWarnings: string[] = [];
      const modules = splitModules(row.modules);
      const missingModules = missingLoadoutModules(loadoutStorage[handle], modules, handle, loadoutCommanderStorage, loadoutShipyards[handle]);
      if (missingModules.length) {
        rowWarnings.push(`can't source modules ${missingModules.join(", ")}`);
      }
      const invalidModules = modules.filter((moduleId) => !loadoutModuleIds.has(moduleId));
      if (invalidModules.length) {
        rowWarnings.push(`unknown modules ${invalidModules.join(", ")}`);
      }
      const current = loadoutShipyards[handle]?.ownedShips.find((ship) => ship.isActive);
      const fitting = validateFitting(row.chassisClassId || current?.classId || "", modules, loadoutChassis, loadoutModuleSpecs);
      rowWarnings.push(...fitting.errors, ...fitting.warnings);
      warnings.set(handle, rowWarnings);
    }
    return warnings;
  }, [loadoutChassis, loadoutCommanderStorage, loadoutModuleIds, loadoutModuleSpecs, loadoutRows, loadoutShipyards, loadoutStorage, selectedSquadHandlesKey]);

  const loadoutWarningCount = useMemo(() => Array.from(loadoutWarnings.values()).reduce((sum, row) => sum + row.length, 0), [loadoutWarnings]);
  const loadoutMeta = `${selectedSquad?.handles.length ?? 0} pilots / ${loadoutShipOptions.length} ships / ${loadoutModuleIds.size} modules / ${loadoutWarningCount} warnings`;

  function setDesignerFilter<K extends keyof DesignerFilters>(key: K, value: DesignerFilters[K]) {
    setDesignerFilters((prev) => ({ ...prev, [key]: value }));
  }

  function selectDesignerChassis(chassisId: string) {
    setDesignerChassisId(chassisId);
    const ship = findCatalogShip(designerChassis, chassisId);
    setDesignerModules(ship?.defaultModules.join("\n") ?? "");
    if (ship && !designerSaveName.trim()) {
      setDesignerSaveName(ship.name || ship.id);
    }
  }

  function saveCurrentDesign() {
    const ship = designerValidation.ship;
    if (!ship) {
      setStatus("Choose a chassis before saving a design.");
      return;
    }
    const name = designerSaveName.trim() || ship.name || ship.id;
    const design: SavedShipyardDesign = {
      id: `${Date.now()}:${ship.id}`,
      name,
      row: {
        chassisClassId: ship.id,
        modules: splitModules(designerModules).join("\n"),
      },
      savedAt: Date.now(),
    };
    const next = [design, ...savedDesigns.filter((item) => item.name.toLowerCase() !== name.toLowerCase())];
    if (!writeSavedShipyardDesigns(next)) {
      setStatus("Could not save ship design.");
      return;
    }
    setSavedDesigns(next);
    setSelectedDesignId(design.id);
    setDesignerSaveName(name);
    setStatus(`Saved design ${name}.`);
  }

  function loadDesignIntoDesigner(design: SavedShipyardDesign | null) {
    if (!design) return;
    setDesignerChassisId(design.row.chassisClassId);
    setDesignerModules(design.row.modules);
    setDesignerSaveName(design.name);
    setSelectedDesignId(design.id);
    setStatus(`Loaded design ${design.name}.`);
  }

  function deleteSelectedDesign() {
    if (!selectedSavedDesign) return;
    const next = savedDesigns.filter((design) => design.id !== selectedSavedDesign.id);
    if (!writeSavedShipyardDesigns(next)) {
      setStatus("Could not delete ship design.");
      return;
    }
    setSavedDesigns(next);
    setStatus(`Deleted design ${selectedSavedDesign.name}.`);
  }

  async function priceCurrentDesign() {
    const ship = designerValidation.ship;
    if (!ship) {
      setStatus("Choose a chassis before pricing a design.");
      return;
    }
    const modules = splitModules(designerModules);
    const demand = designerDemand(ship, modules, designerModuleNames);
    if (demand.size === 0) {
      setStatus("This design has no commission materials or modules to price.");
      setDesignerPriceQuote(null);
      return;
    }
    setBusyKey("designer-price");
    try {
      setStatus("Pricing design against live market snapshots...");
      const market = await readEconomyMarket(designerCatalogHandle, true);
      setDesignerMarket(market);
      const quote = priceDesignerParts(ship, modules, designerModuleNames, market);
      setDesignerPriceQuote(quote);
      const missing = quote.missingRows.length
        ? ` Missing depth for ${quote.missingRows.map((row) => `${row.itemId} x${row.missingQuantity}`).join(" / ")}.`
        : "";
      setStatus(`Priced ${quote.rows.length} part types at ${formatCredits(quote.total)}.${missing}`);
    } catch (err) {
      setDesignerPriceQuote(null);
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  }

  async function addDesignVirtualOrders() {
    const stationId = designerVirtualStation.trim();
    const ship = designerValidation.ship;
    if (!ship) {
      setStatus("Choose a chassis before creating virtual orders.");
      return;
    }
    if (!stationId) {
      setStatus("Choose a station for the internal virtual buys.");
      return;
    }
    const modules = splitModules(designerModules);
    setBusyKey("designer-virtual-orders");
    try {
      setStatus("Creating internal virtual buys with 10% padded budget...");
      const market = designerMarket ?? (await readEconomyMarket(designerCatalogHandle, false));
      setDesignerMarket(market);
      const commanderStorage = await readCommanderStorage().catch(() => null);
      const demand = designerDemand(ship, modules, designerModuleNames);
      const available = factionStorageAtStation(commanderStorage, stationId);
      const netDemand = subtractAvailableDemand(demand, available);
      const coveredCount = Array.from(demand.entries()).filter(([itemId, entry]) => Math.min(entry.quantity, available.get(itemId) ?? 0) > 0).length;
      const coveredQuantity = Array.from(demand.entries()).reduce((sum, [itemId, entry]) => {
        return sum + Math.min(entry.quantity, available.get(itemId) ?? 0);
      }, 0);
      const quote = priceDemandParts(netDemand, market);
      setDesignerPriceQuote(quote);
      const orderableRows = quote.rows.filter((row) => row.orderPriceEach != null && row.orderQuantity > 0);
      if (!orderableRows.length) {
        setStatus(
          netDemand.size === 0
            ? `No virtual orders needed; faction storage at ${stationId} already covers this design.`
            : `No market-priced remaining parts are available to create internal virtual buys.${coveredQuantity > 0 ? ` Faction storage covered ${formatDps(coveredQuantity)} units.` : ""}`,
        );
        return;
      }
      const existing = await fetchVirtualOrders();
      const saved = await saveVirtualOrders(mergeShipyardVirtualBuyOrders(existing, stationId, orderableRows));
      const totalBudget = orderableRows.reduce((sum, row) => sum + row.total, 0);
      const covered =
        coveredQuantity > 0 ? ` Faction storage covered ${formatDps(coveredQuantity)} units across ${coveredCount.toLocaleString()} item types.` : "";
      setStatus(
        `Added or updated ${orderableRows.length} internal virtual buys at ${stationId} with exact quantities and 10% padded budget (${formatCredits(totalBudget)}).${covered} ${saved.length.toLocaleString()} total virtual orders.`,
      );
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  }

  function selectedDesignForId(designId: string): SavedShipyardDesign | null {
    return savedDesigns.find((design) => design.id === designId) ?? null;
  }

  function savedDesignForLoadoutRow(row: LoadoutRow): SavedShipyardDesign | null {
    const normalized = normalizeLoadoutRow(row);
    if (!normalized.chassisClassId && !normalized.modules) return null;
    return savedDesigns.find((design) => loadoutRowsEqual(design.row, row)) ?? null;
  }

  function assignDesignToLoadoutRow(handle: string, designId: string) {
    const design = selectedDesignForId(designId);
    if (!design) {
      setLoadoutRow(handle, emptyLoadoutRow());
      setStatus(`Cleared assigned design for ${handle}.`);
      return;
    }
    setLoadoutRow(handle, design.row);
    setStatus(`Assigned ${design.name} to ${handle}.`);
  }

  function assignSelectedDesignToLoadoutRow(handle: string) {
    if (!selectedSavedDesign) {
      setStatus("Select a saved design first.");
      return;
    }
    setLoadoutRow(handle, selectedSavedDesign.row);
    setStatus(`Assigned ${selectedSavedDesign.name} to ${handle}.`);
  }

  function assignSelectedDesignToSquad() {
    if (!selectedSquad) {
      setStatus("Select a squad.");
      return;
    }
    if (!selectedSavedDesign) {
      setStatus("Select a saved design first.");
      return;
    }
    setLoadoutRows((prev) => {
      const next = { ...prev };
      for (const handle of selectedSquad.handles) {
        next[handle] = selectedSavedDesign.row;
      }
      return next;
    });
    setStatus(`Assigned ${selectedSavedDesign.name} to ${selectedSquad.handles.length} pilots.`);
  }

  function saveCurrentSquadLoadout() {
    if (!selectedSquad) {
      setStatus("Select a squad.");
      return false;
    }
    if (!saveShipyardLoadout(selectedSquad, loadoutRows)) {
      setStatus("Could not save squad loadout.");
      return false;
    }
    setStatus(`Saved loadout for ${selectedSquad.name}.`);
    return true;
  }

  function setLoadoutRow(handle: string, patch: Partial<LoadoutRow>) {
    setLoadoutRows((prev) => ({
      ...prev,
      [handle]: {
        ...(prev[handle] ?? { chassisClassId: "", modules: "" }),
        ...patch,
      },
    }));
  }

  function setDesignerModule(index: number, value: string) {
    const modules = moduleInputSlots(designerModules);
    modules[index] = value;
    setDesignerModules(modules.join("\n"));
  }

  function addDesignerModule() {
    setDesignerModules([...moduleInputSlots(designerModules), ""].join("\n"));
  }

  function removeDesignerModule(index: number) {
    const modules = moduleInputSlots(designerModules);
    modules.splice(index, 1);
    setDesignerModules((modules.length ? modules : [""]).join("\n"));
  }

  async function refreshLoadoutSquad(handles: string[]) {
    let commanderStorageError: string | null = null;
    const [shipyardEntries, storageEntries, fleet, catalog, commanderStorage] = await Promise.all([
      Promise.all(handles.map(async (handle) => [handle, await readShipyard(handle)] as const)),
      Promise.all(handles.map(async (handle) => [handle, await readStorage(handle)] as const)),
      readShipyardFleet(),
      handles[0] ? readCatalog(handles[0]).catch(() => null) : Promise.resolve(null),
      readCommanderStorage().catch((err) => {
        commanderStorageError = err instanceof Error ? err.message : String(err);
        return null;
      }),
    ]);
    setLoadoutShipyards(Object.fromEntries(shipyardEntries));
    setFleetData(fleet);
    setLoadoutStorage(Object.fromEntries(storageEntries.filter((entry): entry is readonly [string, StorageSessionState] => entry[1] !== null)));
    setLoadoutCommanderStorage(commanderStorage);
    setLoadoutChassis(catalog?.ships ?? []);
    const modules = collectModuleCatalog(catalog);
    setLoadoutModuleIds(modules.ids);
    setLoadoutModuleNames(modules.names);
    setLoadoutModuleSpecs(modules.specs);
    const catalogStatus = loadoutCatalogTelemetryStatus(catalog, modules);
    setStatus(
      catalogStatus ?? (commanderStorageError ? `Commander storage unavailable; faction module sourcing is incomplete: ${commanderStorageError}` : null),
    );
  }

  function goTargetForShip(ship: OwnedShipInfo): string {
    if (ship.isActive) return "";
    if (isFactionGarageShip(ship)) {
      return ship.locationBaseId || (DSL_ARG_TOKEN_RE.test(ship.location) ? ship.location : "");
    }
    return ship.locationBaseId || (DSL_ARG_TOKEN_RE.test(ship.location) ? ship.location : "");
  }

  function dslGoTarget(target: string): string {
    const trimmed = target.trim();
    if (!trimmed) {
      throw new Error("Cannot travel to loadout location because the station id is missing.");
    }
    if (!DSL_ARG_TOKEN_RE.test(trimmed)) {
      throw new Error(`Cannot travel to loadout location "${trimmed}" because it is a display name, not a station id.`);
    }
    return trimmed;
  }

  function pushGoBlock(steps: string[], target: string) {
    const dslTarget = dslGoTarget(target);
    steps.push(`go ${dslTarget};`);
    steps.push("refuel;");
  }

  function pushNearestStationBlock(steps: string[], stationPoi: string) {
    steps.push(`go ${dslGoTarget(stationPoi)};`);
    steps.push("refuel;");
  }

  async function loadoutRouteDistancesFor(
    handle: string,
    modules: string[],
    commanderStorage: CommanderStorageView | null,
    freshStorage?: StorageSessionState | null,
    origin?: string,
  ): Promise<Record<string, number>> {
    const storage = freshStorage ?? loadoutStorage[handle];
    const moduleSet = new Set(modules);
    const targets = new Set<string>();

    for (const row of commanderStorage?.rows ?? []) {
      if (!moduleSet.has(row.itemId) || row.quantity <= 0 || !row.locationId || row.locationId === "shared") continue;
      if (personalStorageRowMatches(row, handle, storage) || factionStorageRowMatches(row, storage)) {
        targets.add(row.locationId);
      }
    }

    if (!commanderStorage) {
      for (const [poiId, items] of Object.entries(storage?.storageByPoi ?? {})) {
        if (Array.from(moduleSet).some((moduleId) => (items[moduleId] ?? 0) > 0)) {
          targets.add(poiId);
        }
      }
    }

    const currentSystem = origin ?? sessions.find((session) => sessionMatchesJobHandle(session, handle))?.location.system ?? "";
    const targetList = Array.from(targets);
    const routes = currentSystem
      ? await fetchRoutes(
          targetList.map((target) => ({ from: currentSystem, to: target })),
          true,
        )
      : [];
    return Object.fromEntries(routes.flatMap((route, index) => (route ? [[targetList[index]!, route.cost] as const] : [])));
  }

  function moduleInstallSteps(
    modules: string[],
    storage: StorageSessionState | undefined,
    yard: ShipyardData | undefined,
    handle: string,
    commanderStorage: CommanderStorageView | null,
    routeDistances: Record<string, number>,
  ): string[] {
    const steps: string[] = [];
    const sourcesByModule = new Map<string, LoadoutModuleSource[]>();
    const desiredCounts = new Map<string, number>();
    for (const moduleId of modules) {
      desiredCounts.set(moduleId, (desiredCounts.get(moduleId) ?? 0) + 1);
    }

    const installedToKeep = new Map<string, number>();
    for (const moduleId of yard?.installedModules ?? []) {
      const desired = desiredCounts.get(moduleId) ?? 0;
      const kept = installedToKeep.get(moduleId) ?? 0;
      if (kept < desired) {
        installedToKeep.set(moduleId, kept + 1);
        continue;
      }
      steps.push(`uninstall_mod ${JSON.stringify(moduleId)};`);
      steps.push(`transfer ${JSON.stringify(moduleId)} 1 from cargo to storage;`);
    }

    for (const moduleId of modules) {
      let sources = sourcesByModule.get(moduleId);
      if (!sources) {
        sources = loadoutModuleSources(handle, moduleId, storage, commanderStorage, routeDistances, yard);
        const kept = installedToKeep.get(moduleId) ?? 0;
        for (const source of sources) {
          if (source.kind !== "installed") continue;
          source.quantity = Math.min(source.quantity, kept);
        }
        sourcesByModule.set(moduleId, sources);
      }

      const source = sources.find((candidate) => candidate.quantity > 0);
      if (!source) continue;
      source.quantity -= 1;
      if (source.kind === "installed") {
        continue;
      }

      if (source.kind !== "cargo") {
        pushGoBlock(steps, source.locationId);
        if (source.kind === "personal") {
          steps.push(`transfer ${JSON.stringify(moduleId)} 1 from storage to cargo;`);
        } else {
          steps.push(`transfer ${JSON.stringify(moduleId)} 1 from faction to cargo;`);
        }
      }
      steps.push(`install_mod ${JSON.stringify(moduleId)};`);
    }

    return steps;
  }

  function buildChassisLoadoutScript(option: LoadoutShipOption | null): string {
    const steps: string[] = [];
    if (option && !option.ship.isActive) {
      const target = goTargetForShip(option.ship);
      pushGoBlock(steps, target);
      steps.push(`switch_ship ${JSON.stringify(option.ship.shipId)};`);
    }
    return steps.join("\n");
  }

  function buildModuleLoadoutScript(
    handle: string,
    modules: string[],
    storage: StorageSessionState | undefined,
    yard: ShipyardData | undefined,
    commanderStorage: CommanderStorageView | null,
    routeDistances: Record<string, number>,
    nearestStationPoi: string | null,
  ): string {
    const steps: string[] = [];
    if (modules.length > 0 && yard?.docked !== true && steps.length === 0) {
      if (!nearestStationPoi) throw new Error("No reachable station POI is known for this loadout.");
      pushNearestStationBlock(steps, nearestStationPoi);
    }
    steps.push(...moduleInstallSteps(modules, storage, yard, handle, commanderStorage, routeDistances));
    return steps.join("\n");
  }

  async function applyLoadout() {
    if (!selectedSquad) {
      setStatus("Select a squad.");
      return;
    }
    const skippedRunning = selectedSquad.handles.filter((handle) => runningLoadoutHandles.has(handle));
    const activeHandles = selectedSquad.handles.filter((handle) => !runningLoadoutHandles.has(handle));
    const skippedRunningMessage = skippedRunning.length ? ` Skipped running scripts: ${skippedRunning.join(" / ")}.` : "";
    const freshFleet = await readShipyardFleet().catch(() => fleetData);
    if (freshFleet) setFleetData(freshFleet);
    const freshLoadoutShipOptions = buildFleetLoadoutShipOptions(freshFleet);
    console.warn("[shipyard-loadout] fleet candidates", {
      mergedShips: freshFleet?.ships.length ?? 0,
      ownedShips: freshFleet?.ownedShips.length ?? 0,
      factionGarageShips: freshFleet?.factionGarageShips.length ?? 0,
      loadoutOptions: freshLoadoutShipOptions.length,
      runnerOptions: freshLoadoutShipOptions
        .filter((option) => chassisMatches(option.ship, "runner"))
        .map((option) => ({
          shipId: option.ship.shipId,
          classId: option.ship.classId,
          className: option.ship.className,
          ownerHandle: option.ownerHandle,
          isGarage: isFactionGarageShip(option.ship),
          locationBaseId: option.ship.locationBaseId,
          location: option.ship.location,
        }))
        .slice(0, 20),
    });
    const desired = activeHandles.map((handle) => ({
      handle,
      row: loadoutRows[handle] ?? emptyLoadoutRow(),
      option: null as LoadoutShipOption | null,
      modules: splitModules((loadoutRows[handle] ?? emptyLoadoutRow()).modules),
    }));
    const assignedShipIds = new Set<string>();
    for (const assignment of desired) {
      if (!assignment.row.chassisClassId || assignment.option) continue;
      const options = loadoutOptionsForHandle(freshLoadoutShipOptions, assignment.handle);
      const stored = options.find(
        (entry) => !entry.ship.isActive && !assignedShipIds.has(entry.ship.shipId) && chassisMatches(entry.ship, assignment.row.chassisClassId),
      );
      if (stored) {
        assignment.option = stored;
        assignedShipIds.add(stored.ship.shipId);
        continue;
      }
    }
    const unresolvedChassis = desired
      .filter((assignment) => assignment.row.chassisClassId && !assignment.option)
      .map((assignment) => `${assignment.handle}: ${assignment.row.chassisClassId}`);
    if (unresolvedChassis.length) {
      console.warn("[shipyard-loadout] unresolved chassis", {
        unresolvedChassis,
        desired: desired
          .filter((assignment) => assignment.row.chassisClassId && !assignment.option)
          .map((assignment) => {
            const options = loadoutOptionsForHandle(freshLoadoutShipOptions, assignment.handle);
            const matching = options.filter((entry) => chassisMatches(entry.ship, assignment.row.chassisClassId));
            return {
              handle: assignment.handle,
              chassisClassId: assignment.row.chassisClassId,
              optionCount: options.length,
              matchingCount: matching.length,
              matching: matching.slice(0, 20).map((entry) => ({
                shipId: entry.ship.shipId,
                classId: entry.ship.classId,
                className: entry.ship.className,
                isActive: entry.ship.isActive,
                isGarage: isFactionGarageShip(entry.ship),
                hasStation: factionGarageShipHasStation(entry.ship),
                atCurrentBase: shipAtCurrentBase(entry.ship, loadoutShipyards[assignment.handle]),
                switchTarget: goTargetForShip(entry.ship),
                locationBaseId: entry.ship.locationBaseId,
                location: entry.ship.location,
              })),
            };
          }),
      });
    }
    const assignments = desired.filter(
      (assignment) =>
        (!assignment.row.chassisClassId || assignment.option) && ((assignment.option && !assignment.option.ship.isActive) || assignment.modules.length > 0),
    );

    if (!assignments.length) {
      setStatus(
        unresolvedChassis.length
          ? `Skipped unavailable chassis: ${unresolvedChassis.join(" / ")}.${skippedRunningMessage}`
          : skippedRunning.length
            ? `No available squad members to load out.${skippedRunningMessage}`
            : "Select at least one chassis or module loadout.",
      );
      return;
    }

    setBusyKey("loadout");
    const saved = saveCurrentSquadLoadout();
    try {
      setStatus(`Applying loadout for ${assignments.length} squad members...`);
      const chassisResults = await Promise.allSettled(
        assignments.map(async (assignment) => {
          const { handle, option } = assignment;
          const script = buildChassisLoadoutScript(option);
          if (!script.trim()) return handle;
          const maxSteps = Math.max(2, script.split(";").length + 2);
          await executeShipyardScript(handle, script, maxSteps);
          return handle;
        }),
      );
      const chassisFailures = chassisResults
        .map((result, index) => ({ result, handle: assignments[index]?.handle ?? "unknown" }))
        .filter((entry): entry is { result: PromiseRejectedResult; handle: string } => entry.result.status === "rejected");
      if (chassisFailures.length) {
        const message = chassisFailures
          .map(({ handle, result }) => `${handle}: ${result.reason instanceof Error ? result.reason.message : String(result.reason)}`)
          .join(" / ");
        throw new Error(message);
      }

      const [freshShipyardEntries, freshStorageEntries, freshCommanderStorage] = await Promise.all([
        Promise.all(assignments.map(async ({ handle }) => [handle, await readShipyard(handle)] as const)),
        Promise.all(assignments.map(async ({ handle }) => [handle, await readStorage(handle)] as const)),
        readCommanderStorage().catch(() => loadoutCommanderStorage),
      ]);
      const freshShipyards = Object.fromEntries(freshShipyardEntries);
      const freshStorage = Object.fromEntries(freshStorageEntries);

      const moduleWarnings = assignments.flatMap(({ handle, modules }) => {
        const current = freshShipyards[handle]?.ownedShips.find((ship) => ship.isActive);
        const row = loadoutRows[handle] ?? emptyLoadoutRow();
        const fitting = validateFitting(row.chassisClassId || current?.classId || "", modules, loadoutChassis, loadoutModuleSpecs);
        const missingModules = missingLoadoutModules(freshStorage[handle], modules, handle, freshCommanderStorage, freshShipyards[handle]);
        const invalidModules = modules.filter((moduleId) => !loadoutModuleIds.has(moduleId));
        const warnings = [
          ...fitting.errors,
          ...(missingModules.length ? [`can't source modules ${missingModules.join(", ")}`] : []),
          ...(invalidModules.length ? [`unknown modules ${invalidModules.join(", ")}`] : []),
        ];
        return warnings.length ? [`${handle}: ${warnings.join("; ")}`] : [];
      });
      if (moduleWarnings.length) {
        throw new Error(`Loadout is not settable after chassis swap: ${moduleWarnings.join(" / ")}`);
      }

      const moduleResults = await Promise.allSettled(
        assignments.map(async (assignment) => {
          const { handle, modules } = assignment;
          const yard = freshShipyards[handle];
          const storage = freshStorage[handle];
          const routeDistances = await loadoutRouteDistancesFor(handle, modules, freshCommanderStorage, storage);
          const origin = sessions.find((session) => sessionMatchesJobHandle(session, handle))?.location.system ?? "";
          const nearestStationPoi = yard?.docked === true ? null : await findNearestStationPoi(galaxyMap, origin);
          const script = buildModuleLoadoutScript(handle, modules, storage ?? undefined, yard, freshCommanderStorage, routeDistances, nearestStationPoi);
          if (!script.trim()) return handle;
          const maxSteps = Math.max(2, script.split(";").length + 2);
          await executeShipyardScript(handle, script, maxSteps);
          return handle;
        }),
      );
      const failures = moduleResults
        .map((result, index) => ({ result, handle: assignments[index]?.handle ?? "unknown" }))
        .filter((entry): entry is { result: PromiseRejectedResult; handle: string } => entry.result.status === "rejected");
      if (failures.length) {
        const message = failures
          .map(({ handle, result }) => `${handle}: ${result.reason instanceof Error ? result.reason.message : String(result.reason)}`)
          .join(" / ");
        throw new Error(message);
      }
      const skipped = unresolvedChassis.length ? ` Skipped unavailable chassis: ${unresolvedChassis.join(" / ")}.` : "";
      setStatus(`Applied loadout for ${assignments.length} squad members${saved ? " and saved it." : "."}${skipped}${skippedRunningMessage}`);
      await Promise.all([refreshLoadoutSquad(selectedSquad.handles), refreshShipyardFleet(false), onChanged()]);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKey(null);
    }
  }

  if (!sessions.length) {
    return (
      <div className="shipyard-panel">
        <div className="shipyard-empty">No registered sessions.</div>
      </div>
    );
  }

  return (
    <div className="shipyard-panel">
      <div className="shipyard-toolbar">
        <div>
          <div className="shipyard-title">Shipyard</div>
          <div className="shipyard-meta">
            {pane === "designer" ? designerMeta : pane === "equipped" ? equippedMeta : pane === "ships" ? shipsMeta : loadoutMeta}
          </div>
        </div>
        <div className="shipyard-pane-switch" role="tablist" aria-label="Shipyard panes">
          {(["ships", "designer", "equipped", "loadout"] as PaneMode[]).map((mode) => (
            <button key={mode} type="button" data-active={pane === mode} onClick={() => setPane(mode)}>
              {mode === "ships" ? "Ships" : mode === "designer" ? "Designer" : mode === "equipped" ? "Equipped" : "Loadout"}
            </button>
          ))}
        </div>
      </div>

      {status && <div className="shipyard-status">{status}</div>}

      {pane === "ships" ? (
        <>
          <div className="shipyard-controls shipyard-loadout-controls">
            <button className="session-btn" onClick={() => void refreshShipyardFleet()} disabled={fleetLoading || Boolean(busyKey)}>
              refresh
            </button>
            <div className="shipyard-filter-count">{shipsMeta}</div>
          </div>

          <div className="shipyard-table-wrap shipyard-ships-wrap">
            <table className="shipyard-table">
              <thead>
                <tr>
                  <th>owner</th>
                  <th>ship</th>
                  <th>class</th>
                  <th>location</th>
                  <th>mods</th>
                </tr>
              </thead>
              <tbody>
                {fleetShipOptions.map((option) => (
                  <tr key={`${option.ownerHandle}:${option.ship.shipId}`} data-active={option.ship.isActive}>
                    <td>{option.ownerHandle}</td>
                    <td>
                      <div className="shipyard-ship-name">{shipLabel(option.ship)}</div>
                      <div className="shipyard-ship-id">{option.ship.shipId}</div>
                    </td>
                    <td>{shipClass(option.ship)}</td>
                    <td>{shipLocation(option.ship)}</td>
                    <td>{option.ship.modules ?? "-"}</td>
                  </tr>
                ))}
                {fleetLoading && (
                  <tr>
                    <td colSpan={5} className="shipyard-empty">
                      Loading ships...
                    </td>
                  </tr>
                )}
                {!fleetLoading && fleetShipOptions.length === 0 && (
                  <tr>
                    <td colSpan={5} className="shipyard-empty">
                      No ships loaded.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </>
      ) : pane === "equipped" ? (
        <>
          <div className="shipyard-controls shipyard-loadout-controls">
            <button className="session-btn" onClick={() => void loadEquippedView()} disabled={equippedLoading || Boolean(busyKey)}>
              refresh
            </button>
            <div className="shipyard-filter-count">{equippedMeta}</div>
          </div>

          <div className="shipyard-designer shipyard-equipped">
            <div className="shipyard-designer-layout">
              <aside className="shipyard-chassis-list shipyard-equipped-list">
                <div className="shipyard-fit-label">Active ships</div>
                <div className="shipyard-chassis-scroll shipyard-equipped-scroll">
                  {equippedRows.map((row) => (
                    <button
                      type="button"
                      key={row.handle}
                      className="shipyard-chassis-option"
                      data-active={selectedEquippedRow?.handle === row.handle}
                      onClick={() => setSelectedEquippedHandle(row.handle)}
                      disabled={equippedLoading}
                    >
                      <span className="shipyard-ship-name">{row.handle}</span>
                      <span className="shipyard-ship-id">{row.activeShipName || "No active ship loaded"}</span>
                      <span className="shipyard-chassis-statline">
                        <span className="shipyard-chassis-stat" data-tone="empire">
                          <span>Class</span>
                          <strong>{row.validation.ship?.name || row.activeShipClassLabel || row.classId || "?"}</strong>
                        </span>
                        <span className="shipyard-chassis-stat" data-tone="slots">
                          <span>Mods</span>
                          <strong>{row.modules.length.toLocaleString()}</strong>
                        </span>
                      </span>
                      {(row.validation.errors.length > 0 || row.validation.warnings.length > 0) && (
                        <span className="shipyard-ship-id">{[...row.validation.errors, ...row.validation.warnings].join(" / ")}</span>
                      )}
                    </button>
                  ))}
                  {equippedLoading && <div className="shipyard-fit-empty">Loading equipped ships...</div>}
                  {!equippedLoading && equippedRows.length === 0 && <div className="shipyard-fit-empty">No sessions loaded.</div>}
                </div>
              </aside>
              <div className="shipyard-designer-main">
                <div className="shipyard-designer-summary">
                  <div className="shipyard-designer-selected">
                    <div className="shipyard-fit-label">Equipped chassis</div>
                    <div className="shipyard-ship-name">
                      {selectedEquippedRow?.validation.ship?.name ||
                        selectedEquippedRow?.activeShip?.className ||
                        selectedEquippedRow?.activeShipClassLabel ||
                        selectedEquippedRow?.classId ||
                        "No ship selected"}
                    </div>
                    <div className="shipyard-ship-id">
                      {selectedEquippedRow?.activeShip || selectedEquippedRow?.activeShipName || selectedEquippedRow?.classId
                        ? `${selectedEquippedRow.activeShipName || "Active ship"} / ${selectedEquippedRow.activeShipClassLabel || selectedEquippedRow.classId || "unknown class"}${selectedEquippedRow.activeShip ? ` / ${shipLocation(selectedEquippedRow.activeShip)}` : ""}`
                        : selectedEquippedRow
                          ? "Active ship data was not available for this session"
                          : "Load shipyard data to inspect equipped modules"}
                    </div>
                  </div>
                </div>
                {selectedEquippedRow && selectedEquippedRow.stats.length > 0 && (
                  <div className="shipyard-fitted-stats" aria-label={`${selectedEquippedRow.handle} equipped totals`}>
                    {selectedEquippedRow.stats.map((stat) => (
                      <div className="shipyard-fitted-stat" key={stat.label} data-tone={stat.tone}>
                        <span>{stat.label}</span>
                        <strong>{stat.value}</strong>
                      </div>
                    ))}
                  </div>
                )}
                {selectedEquippedRow && (selectedEquippedRow.validation.errors.length > 0 || selectedEquippedRow.validation.warnings.length > 0) && (
                  <div className="shipyard-designer-messages" data-error={selectedEquippedRow.validation.errors.length > 0}>
                    {[...selectedEquippedRow.validation.errors, ...selectedEquippedRow.validation.warnings].join(" / ")}
                  </div>
                )}
                <div className="shipyard-designer-body">
                  <div className="shipyard-designer-modules">
                    <div className="shipyard-fit-label">Equipped modules</div>
                    <div className="shipyard-loadout-module-list">
                      {selectedEquippedRow && selectedEquippedRow.modules.length === 0 && (
                        <div className="shipyard-fit-empty">No equipped modules reported.</div>
                      )}
                      {selectedEquippedRow?.modules.map((moduleId, index) => {
                        const slot = equippedModuleSpecs[moduleId]?.slot ?? "unknown";
                        return (
                          <div className="shipyard-fit-row" key={`equipped:${selectedEquippedRow.handle}:${index}:${moduleId}`} data-slot={slot}>
                            <div>
                              <div className="shipyard-fit-name">
                                {equippedModuleNames[moduleId] ?? selectedEquippedRow.yard?.installedModuleNames[moduleId] ?? moduleId}
                              </div>
                              <div className="shipyard-ship-id">
                                {moduleSummary(moduleId, equippedModuleSpecs, { ...selectedEquippedRow.yard?.installedModuleNames, ...equippedModuleNames })}
                              </div>
                            </div>
                            <span className="shipyard-loadout-module-count">{formatFilterLabel(slot)}</span>
                          </div>
                        );
                      })}
                      {!selectedEquippedRow && <div className="shipyard-fit-empty">Select a ship to inspect.</div>}
                    </div>
                  </div>
                  <div className="shipyard-fit-column shipyard-designer-catalog shipyard-equipped-catalog">
                    <div className="shipyard-module-toolbar">
                      <div className="shipyard-fit-label">Module catalog</div>
                      <div className="shipyard-filter-count">{equippedModuleIds.size.toLocaleString()} modules</div>
                    </div>
                    <div className="shipyard-fit-list">
                      {Array.from(equippedModuleIds)
                        .sort(
                          (a, b) => moduleDisplayLabel(a, equippedModuleNames).localeCompare(moduleDisplayLabel(b, equippedModuleNames)) || a.localeCompare(b),
                        )
                        .map((moduleId) => {
                          const slot = equippedModuleSpecs[moduleId]?.slot ?? "unknown";
                          return (
                            <div className="shipyard-fit-row" key={moduleId} data-slot={slot}>
                              <div>
                                <div className="shipyard-fit-name">{equippedModuleNames[moduleId] ?? moduleId}</div>
                                <div className="shipyard-ship-id">{moduleSummary(moduleId, equippedModuleSpecs, equippedModuleNames)}</div>
                              </div>
                            </div>
                          );
                        })}
                      {equippedModuleIds.size === 0 && <div className="shipyard-fit-empty">No module catalog loaded.</div>}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </>
      ) : pane === "loadout" ? (
        <>
          <div className="shipyard-controls shipyard-loadout-controls">
            <label>
              <span>Squad</span>
              <select value={selectedSquadId} onChange={(event) => setSelectedSquadId(event.target.value)}>
                {squadOptions.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Design</span>
              <select value={selectedDesignId} onChange={(event) => setSelectedDesignId(event.target.value)}>
                {savedDesigns.length === 0 && <option value="">No saved designs</option>}
                {savedDesigns.map((design) => (
                  <option key={design.id} value={design.id}>
                    {design.name}
                  </option>
                ))}
              </select>
            </label>
            <button
              className="session-btn"
              onClick={() => assignSelectedDesignToSquad()}
              disabled={!selectedSquad || !selectedSavedDesign || loadoutLoading || Boolean(busyKey)}
              title="Assign the selected saved design to every pilot in this squad"
            >
              assign to squad
            </button>
            <button
              className="session-btn"
              onClick={() => selectedSquad && void refreshLoadoutSquad(selectedSquad.handles)}
              disabled={!selectedSquad || loadoutLoading || Boolean(busyKey)}
            >
              refresh
            </button>
            <button
              className="session-btn"
              onClick={() => saveCurrentSquadLoadout()}
              disabled={!selectedSquad || loadoutLoading || Boolean(busyKey)}
              title="Save these assigned designs without applying them"
            >
              save
            </button>
            <button className="session-btn" onClick={() => void applyLoadout()} disabled={!selectedSquad || loadoutLoading || Boolean(busyKey)}>
              apply loadout
            </button>
            <div className="shipyard-filter-count">
              {selectedSquad?.handles.length ?? 0} pilots / {loadoutShipOptions.length} ships / {loadoutModuleIds.size} modules / {loadoutWarningCount} warnings
            </div>
          </div>

          <div className="shipyard-loadout-wrap">
            <div className="shipyard-loadout-grid">
              <div className="shipyard-loadout-head">pilot</div>
              <div className="shipyard-loadout-head">assigned design</div>
              <div className="shipyard-loadout-head">designer fit</div>
              {(selectedSquad?.handles ?? []).map((handle) => {
                const row = loadoutRows[handle] ?? { chassisClassId: "", modules: "" };
                const current = loadoutShipyards[handle]?.ownedShips.find((ship) => ship.isActive);
                const warnings = loadoutWarnings.get(handle) ?? [];
                const assignedDesign = savedDesignForLoadoutRow(row);
                const assignedDesignValue = assignedDesign?.id ?? (row.chassisClassId.trim() || splitModules(row.modules).length ? "__custom" : "");
                const modules = splitModules(row.modules);
                const fitting = validateFitting(row.chassisClassId, modules, loadoutChassis, loadoutModuleSpecs);
                const stats = fittedStats(fitting.ship, modules, loadoutModuleSpecs, fitting).filter((stat) => stat.show);
                return (
                  <div className="shipyard-loadout-row" key={handle} data-warning={warnings.length > 0}>
                    <div className="shipyard-loadout-pilot">
                      <div className="shipyard-ship-name">{handle}</div>
                      <div className="shipyard-ship-id">{current ? shipLabel(current) : "no active ship loaded"}</div>
                      <div className="shipyard-loadout-row-actions">
                        <button
                          type="button"
                          className="session-btn"
                          onClick={() => assignSelectedDesignToLoadoutRow(handle)}
                          disabled={loadoutLoading || Boolean(busyKey) || !selectedSavedDesign}
                          title={selectedSavedDesign ? `Assign ${selectedSavedDesign.name} to this pilot` : "Select a saved design"}
                        >
                          assign selected
                        </button>
                        <button
                          type="button"
                          className="session-btn"
                          onClick={() => setLoadoutRow(handle, emptyLoadoutRow())}
                          disabled={loadoutLoading || Boolean(busyKey) || (!row.chassisClassId && !row.modules)}
                          title="Clear assigned design"
                        >
                          clear
                        </button>
                      </div>
                      {warnings.length > 0 && <div className="shipyard-loadout-warning">{warnings.join("; ")}</div>}
                    </div>
                    <div className="shipyard-loadout-design">
                      <select
                        value={assignedDesignValue}
                        onChange={(event) => assignDesignToLoadoutRow(handle, event.target.value)}
                        disabled={loadoutLoading || Boolean(busyKey)}
                        title="Assign a saved ship designer design"
                      >
                        <option value="">No design</option>
                        {assignedDesignValue === "__custom" && (
                          <option value="__custom" disabled>
                            Unsaved legacy design
                          </option>
                        )}
                        {savedDesigns.map((design) => (
                          <option key={design.id} value={design.id}>
                            {design.name}
                          </option>
                        ))}
                      </select>
                      <div className="shipyard-designer-selected">
                        <div className="shipyard-fit-label">Selected chassis</div>
                        <div className="shipyard-ship-name">{fitting.ship?.name || row.chassisClassId || "No design assigned"}</div>
                        <div className="shipyard-ship-id">
                          {fitting.ship ? chassisLabel(fitting.ship) : assignedDesign ? assignedDesign.name : "Assign a saved design from Ship designer"}
                        </div>
                      </div>
                      {stats.length > 0 && (
                        <div className="shipyard-fitted-stats shipyard-loadout-stats" aria-label={`${handle} fitted totals`}>
                          {stats.map((stat) => (
                            <div className="shipyard-fitted-stat" key={stat.label} data-tone={stat.tone}>
                              <span>{stat.label}</span>
                              <strong>{stat.value}</strong>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                    <div className="shipyard-designer-modules shipyard-loadout-fit">
                      <div className="shipyard-fit-label">Designed modules</div>
                      <div className="shipyard-loadout-module-list">
                        {modules.length === 0 && <div className="shipyard-fit-empty">No modules assigned.</div>}
                        {modules.map((moduleId, index) => {
                          const slot = loadoutModuleSpecs[moduleId]?.slot ?? "unknown";
                          const quantity = loadoutModuleAvailableCount(
                            handle,
                            loadoutStorage[handle],
                            moduleId,
                            loadoutCommanderStorage,
                            loadoutShipyards[handle],
                          );
                          return (
                            <div className="shipyard-fit-row" key={`${handle}:module:${index}:${moduleId}`} data-slot={slot}>
                              <div>
                                <div className="shipyard-fit-name">{loadoutModuleNames[moduleId] ?? moduleId}</div>
                                <div className="shipyard-ship-id">{moduleSummary(moduleId, loadoutModuleSpecs, loadoutModuleNames)}</div>
                              </div>
                              <span className="shipyard-loadout-module-count" title="Available to this pilot">
                                {quantity.toLocaleString()}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  </div>
                );
              })}
              {loadoutLoading && <div className="shipyard-loadout-empty">Loading squad shipyards...</div>}
              {!loadoutLoading && selectedSquad && selectedSquad.handles.length === 0 && <div className="shipyard-loadout-empty">No pilots in this squad.</div>}
              {!loadoutLoading && !selectedSquad && <div className="shipyard-loadout-empty">No squads available.</div>}
            </div>
          </div>
        </>
      ) : (
        <>
          <datalist id="shipyard-designer-modules">
            {Array.from(designerModuleIds)
              .map((id) => ({ id, label: moduleDisplayLabel(id, designerModuleNames) }))
              .sort((a, b) => a.label.localeCompare(b.label) || a.id.localeCompare(b.id))
              .map((module) => (
                <option key={module.id} value={module.id}>
                  {module.label}
                </option>
              ))}
          </datalist>
          <datalist id="shipyard-designer-stations">
            {designerStationOptions.map((stationId) => (
              <option key={stationId} value={stationId} />
            ))}
          </datalist>
          <div className="shipyard-designer">
            <div className="shipyard-designer-layout">
              <aside className="shipyard-chassis-list">
                <div className="shipyard-fit-label">Chassis</div>
                <div className="shipyard-chassis-filters">
                  <input
                    className="shipyard-filter-wide"
                    value={designerFilters.query}
                    onChange={(event) => setDesignerFilter("query", event.target.value)}
                    placeholder="search ships"
                    disabled={loading || Boolean(busyKey)}
                    title="Search chassis"
                  />
                  <select
                    value={designerFilters.empire}
                    onChange={(event) => setDesignerFilter("empire", event.target.value)}
                    disabled={loading || Boolean(busyKey)}
                    title="Filter by empire"
                  >
                    <option value="">All empires</option>
                    {designerEmpireOptions.map((empire) => (
                      <option key={empire} value={empire}>
                        {formatFilterLabel(empire)}
                      </option>
                    ))}
                  </select>
                  {(
                    [
                      ["hull", "Hull"],
                      ["shield", "Shield"],
                      ["fuel", "Fuel"],
                      ["cargo", "Cargo"],
                      ["speed", "Speed"],
                      ["weapon", "W slots"],
                      ["defense", "D slots"],
                      ["utility", "U slots"],
                      ["cpu", "CPU"],
                      ["power", "Power"],
                    ] as Array<[keyof DesignerFilters, string]>
                  ).map(([key, label]) => (
                    <input
                      key={key}
                      type="number"
                      min="0"
                      inputMode="numeric"
                      value={designerFilters[key]}
                      onChange={(event) => setDesignerFilter(key, event.target.value)}
                      placeholder={label}
                      disabled={loading || Boolean(busyKey)}
                      title={`Minimum ${label}`}
                    />
                  ))}
                  <button
                    type="button"
                    className="session-btn"
                    onClick={() => setDesignerFilters(emptyDesignerFilters)}
                    disabled={!designerFilterActive || loading || Boolean(busyKey)}
                    title="Clear chassis filters"
                  >
                    clear
                  </button>
                </div>
                <div className="shipyard-chassis-scroll">
                  {designerFilteredChassis.map((ship) => {
                    const stats = chassisCompactStats(ship);
                    return (
                      <button
                        type="button"
                        key={ship.id}
                        className="shipyard-chassis-option"
                        data-active={designerChassisId === ship.id}
                        onClick={() => selectDesignerChassis(ship.id)}
                        disabled={loading || Boolean(busyKey)}
                      >
                        <span className="shipyard-ship-name">{ship.name || ship.id}</span>
                        <span className="shipyard-ship-id">
                          {ship.className || ship.category || ship.id}
                          {ship.tier == null ? "" : ` / tier ${ship.tier}`}
                        </span>
                        <span className="shipyard-chassis-statline">
                          {stats.slice(0, 5).map((stat) => (
                            <span key={stat.label} className="shipyard-chassis-stat" data-tone={stat.tone}>
                              <span>{stat.label}</span>
                              <strong>{stat.value}</strong>
                            </span>
                          ))}
                        </span>
                        <span className="shipyard-chassis-statline">
                          {stats.slice(5).map((stat) => (
                            <span key={stat.label} className="shipyard-chassis-stat" data-tone={stat.tone}>
                              <span>{stat.label}</span>
                              <strong>{stat.value}</strong>
                            </span>
                          ))}
                        </span>
                      </button>
                    );
                  })}
                  {designerFilteredChassis.length === 0 && (
                    <div className="shipyard-fit-empty">{designerChassis.length === 0 ? "No ship catalog loaded." : "No ships match these filters."}</div>
                  )}
                </div>
              </aside>
              <div className="shipyard-designer-main">
                <div className="shipyard-designer-summary">
                  <div className="shipyard-designer-selected">
                    <div className="shipyard-fit-label">Selected chassis</div>
                    <div className="shipyard-ship-name">{designerValidation.ship?.name || designerValidation.ship?.id || "No chassis selected"}</div>
                    <div className="shipyard-ship-id">{designerValidation.ship ? chassisLabel(designerValidation.ship) : "Load catalog to begin"}</div>
                  </div>
                  <div className="shipyard-designer-actions">
                    <input
                      value={designerSaveName}
                      onChange={(event) => setDesignerSaveName(event.target.value)}
                      placeholder="design name"
                      disabled={loading || Boolean(busyKey)}
                      title="Saved design name"
                    />
                    <button
                      className="session-btn"
                      onClick={() => {
                        void loadDesignerCatalog(designerCatalogHandle);
                      }}
                      disabled={loading || Boolean(busyKey)}
                    >
                      refresh
                    </button>
                    <button
                      className="session-btn"
                      onClick={() => {
                        const ship = designerValidation.ship;
                        if (ship) setDesignerModules(ship.defaultModules.join("\n"));
                      }}
                      disabled={!designerValidation.ship || Boolean(busyKey)}
                      title="Restore the catalog default modules for this ship"
                    >
                      default fit
                    </button>
                    <button
                      className="session-btn"
                      onClick={() => void priceCurrentDesign()}
                      disabled={!designerValidation.ship || !designerCatalogHandle || Boolean(busyKey)}
                      title="Price commission materials and designed modules from known market sell orders"
                    >
                      price
                    </button>
                    <input
                      list="shipyard-designer-stations"
                      value={designerVirtualStation}
                      onChange={(event) => setDesignerVirtualStation(event.target.value)}
                      placeholder="station"
                      disabled={loading || Boolean(busyKey)}
                      title="Station for generated internal virtual buys"
                    />
                    <button
                      className="session-btn"
                      onClick={() => void addDesignVirtualOrders()}
                      disabled={!designerValidation.ship || !designerVirtualStation.trim() || Boolean(busyKey)}
                      title="Create or update internal virtual buys with exact quantities and 10% padded budget"
                    >
                      virtual orders
                    </button>
                    <button
                      className="session-btn"
                      onClick={() => saveCurrentDesign()}
                      disabled={!designerValidation.ship || Boolean(busyKey)}
                      title="Save this chassis and module design"
                    >
                      save
                    </button>
                    <select
                      value={selectedDesignId}
                      onChange={(event) => setSelectedDesignId(event.target.value)}
                      disabled={savedDesigns.length === 0 || loading || Boolean(busyKey)}
                      title="Saved designs"
                    >
                      {savedDesigns.length === 0 && <option value="">No saved designs</option>}
                      {savedDesigns.map((design) => (
                        <option key={design.id} value={design.id}>
                          {design.name}
                        </option>
                      ))}
                    </select>
                    <button
                      className="session-btn"
                      onClick={() => loadDesignIntoDesigner(selectedSavedDesign)}
                      disabled={!selectedSavedDesign || loading || Boolean(busyKey)}
                      title="Load saved design into designer"
                    >
                      load
                    </button>
                    <button
                      className="session-btn session-btn--danger"
                      onClick={() => deleteSelectedDesign()}
                      disabled={!selectedSavedDesign || loading || Boolean(busyKey)}
                      title="Delete selected saved design"
                    >
                      delete
                    </button>
                  </div>
                </div>
                {designerFittedStats.length > 0 && (
                  <div className="shipyard-fitted-stats" aria-label="Fitted totals">
                    {designerFittedStats
                      .filter((stat) => stat.show)
                      .map((stat) => (
                        <div className="shipyard-fitted-stat" key={stat.label} data-tone={stat.tone}>
                          <span>{stat.label}</span>
                          <strong>{stat.value}</strong>
                        </div>
                      ))}
                  </div>
                )}
                {designerPriceQuote && (
                  <div className="shipyard-price-quote" aria-label="Market price quote">
                    <div className="shipyard-price-total">
                      <span>Market parts total</span>
                      <strong>
                        <CreditAmount value={designerPriceQuote.total} fallback="?" />
                      </strong>
                      <em>{designerPriceQuote.missingRows.length ? `${designerPriceQuote.missingRows.length} short` : "all priced"}</em>
                    </div>
                    <div className="shipyard-price-rows">
                      {designerPriceQuote.rows.slice(0, 8).map((row) => (
                        <div className="shipyard-price-row" key={row.itemId} data-missing={row.missingQuantity > 0}>
                          <span>{row.label}</span>
                          <span>x{row.orderQuantity.toLocaleString()}</span>
                          <CreditAmount value={row.total} fallback="?" />
                          <span>
                            {row.orderPriceEach == null ? "-" : `${row.orderPriceEach.toLocaleString()} ea`}
                            {" / "}
                            {row.unitPrice == null ? "-" : `${formatDps(row.unitPrice)} avg`}
                            {" / best "}
                            {row.bestPrice == null ? "-" : row.bestPrice.toLocaleString()}
                            {" / "}
                            {row.source}
                          </span>
                        </div>
                      ))}
                    </div>
                    {designerPriceQuote.rows.length > 8 && <div className="shipyard-price-more">+{designerPriceQuote.rows.length - 8} more part types</div>}
                  </div>
                )}
                {(designerValidation.errors.length > 0 || designerValidation.warnings.length > 0) && (
                  <div className="shipyard-designer-messages" data-error={designerValidation.errors.length > 0}>
                    {[...designerValidation.errors, ...designerValidation.warnings].join(" / ")}
                  </div>
                )}
                <div className="shipyard-designer-body">
                  <div className="shipyard-designer-modules">
                    <div className="shipyard-fit-label">Designed modules</div>
                    <div className="shipyard-loadout-module-list">
                      {designerModuleSlots.map((moduleId, index) => {
                        const slot = designerModuleSpecs[moduleId]?.slot ?? "unknown";
                        if (!moduleId) {
                          return (
                            <div className="shipyard-module-card shipyard-module-card--empty" key={`designer:${index}`}>
                              <input
                                list="shipyard-designer-modules"
                                value={moduleId}
                                onChange={(event) => setDesignerModule(index, event.target.value)}
                                placeholder="module id"
                                disabled={loading || Boolean(busyKey)}
                              />
                              <button
                                type="button"
                                className="session-btn shipyard-loadout-remove"
                                onClick={() => removeDesignerModule(index)}
                                disabled={loading || Boolean(busyKey) || designerModuleSlots.length === 1}
                                title="Remove module slot"
                              >
                                -
                              </button>
                            </div>
                          );
                        }
                        const chits = moduleStatChits(designerModuleSpecs[moduleId]);
                        return (
                          <div className="shipyard-module-card" key={`designer:${index}`} data-slot={slot}>
                            <div className="shipyard-module-card-head">
                              <div>
                                <div className="shipyard-fit-name">{designerModuleNames[moduleId] ?? moduleId}</div>
                                <div className="shipyard-ship-id">{moduleId}</div>
                              </div>
                              <button
                                type="button"
                                className="session-btn session-btn--danger"
                                onClick={() => removeDesignerModule(index)}
                                disabled={loading || Boolean(busyKey) || (designerModuleSlots.length === 1 && !moduleId)}
                                title="Remove module slot"
                              >
                                remove
                              </button>
                            </div>
                            <div className="shipyard-module-chits">
                              {chits.map((chit, chitIndex) => (
                                <span className="shipyard-module-chit" data-tone={chit.tone} key={`${chit.label}:${chit.value}:${chitIndex}`}>
                                  <span>{chit.label}</span>
                                  <strong>{chit.value}</strong>
                                </span>
                              ))}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                    <button
                      type="button"
                      className="session-btn"
                      onClick={() => addDesignerModule()}
                      disabled={loading || Boolean(busyKey)}
                      title="Add module slot"
                    >
                      +
                    </button>
                  </div>
                  <div className="shipyard-fit-column shipyard-designer-catalog">
                    <div className="shipyard-module-toolbar">
                      <div className="shipyard-fit-label">Module catalog</div>
                      <input
                        value={moduleSearch}
                        onChange={(event) => setModuleSearch(event.target.value)}
                        placeholder="search modules"
                        disabled={loading || Boolean(busyKey)}
                      />
                      <div className="shipyard-module-filter" role="tablist" aria-label="Module type filters">
                        {(["all", "weapon", "defense", "utility"] as ModuleFilterMode[]).map((mode) => (
                          <button
                            key={mode}
                            type="button"
                            data-active={moduleFilter === mode}
                            onClick={() => setModuleFilter(mode)}
                            disabled={loading || Boolean(busyKey)}
                          >
                            {mode}
                          </button>
                        ))}
                      </div>
                    </div>
                    <div className="shipyard-fit-list">
                      {designerFilteredModuleIds.map((moduleId) => {
                        const slot = designerModuleSpecs[moduleId]?.slot ?? "unknown";
                        const chits = moduleStatChits(designerModuleSpecs[moduleId]);
                        const limitReasons = moduleAddLimitReasons(designerChassisId, designerCurrentModules, moduleId, designerChassis, designerModuleSpecs);
                        const limitBlocked = limitReasons.length > 0;
                        return (
                          <div
                            className="shipyard-module-card"
                            key={moduleId}
                            data-slot={slot}
                            data-disabled={limitBlocked}
                            title={limitBlocked ? limitReasons.join("; ") : undefined}
                          >
                            <div className="shipyard-module-card-head">
                              <div>
                                <div className="shipyard-fit-name">{designerModuleNames[moduleId] ?? moduleId}</div>
                                <div className="shipyard-ship-id">{moduleId}</div>
                              </div>
                              <button
                                className="session-btn"
                                onClick={() => setDesignerModules([...splitModules(designerModules), moduleId].join("\n"))}
                                disabled={Boolean(busyKey) || limitBlocked}
                                title={limitBlocked ? limitReasons.join("; ") : "Add this module to the designed loadout"}
                              >
                                add
                              </button>
                            </div>
                            <div className="shipyard-module-chits">
                              {chits.map((chit, chitIndex) => (
                                <span className="shipyard-module-chit" data-tone={chit.tone} key={`${chit.label}:${chit.value}:${chitIndex}`}>
                                  <span>{chit.label}</span>
                                  <strong>{chit.value}</strong>
                                </span>
                              ))}
                            </div>
                          </div>
                        );
                      })}
                      {designerFilteredModuleIds.length === 0 && (
                        <div className="shipyard-fit-empty">
                          {designerModuleIds.size === 0 ? "No module catalog loaded." : "No modules match these filters."}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
