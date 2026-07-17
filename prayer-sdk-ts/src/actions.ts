import type { Action } from "./generated/types.js";
import type { GoTarget } from "./conveniences.js";
export const undock = (): Action => ({ type: "undock" });
export const dock = (): Action => ({ type: "dock" });
export const wait = (ticks: number): Action => ({ type: "wait", request: { ticks } });
export const go = (target: { poi: string } | { system: string } | GoTarget): Action => {
  const destination: GoTarget = "poi" in target
    ? { kind: "poi", value: target.poi }
    : "system" in target
      ? { kind: "system", value: target.system }
      : target;
  return { type: "go", request: { destination } };
};

type ActionType = Action["type"];
type RequestFor<T extends ActionType> = Action extends infer A
  ? A extends { type: infer K; request: infer R }
    ? T extends K ? R : never
    : never
  : never;
type RequestActionType = { [T in ActionType]: [RequestFor<T>] extends [never] ? never : T }[ActionType];
type UnitActionType = Exclude<ActionType, RequestActionType>;
type FullyNullableRequestActionType = "mine" | "scan" | "repair" | "refuel" | "distress_signal";

/**
 * Canonical empty requests for commands whose payload fields are all nullable.
 * The mapped type makes this registry exhaustive: adding or changing one of
 * these requests in the generated Action union fails compilation here.
 */
const nullableRequestDefaults = {
  mine: { resource: null },
  scan: { target: null },
  repair: { target: null, quantity: null, item: null },
  refuel: { target: null, quantity: null, item: null },
  distress_signal: { distress_type: null },
} satisfies Record<FullyNullableRequestActionType, Record<string, unknown>>;

const nullableRequested = <T extends FullyNullableRequestActionType>(type: T) =>
  (request: Partial<RequestFor<T>> = {}): Action => {
    const normalized: Record<string, unknown> = { ...nullableRequestDefaults[type] };
    for (const [field, value] of Object.entries(request)) {
      if (value !== undefined) normalized[field] = value;
    }
    return action(type, normalized as RequestFor<T>);
  };

export const mine = (resource?: string): Action => action("mine", { resource: resource ?? null });

/** Build any canonical prayer-actions operation with its exact request type. */
export function action<T extends RequestActionType>(type: T, request: RequestFor<T>): Action;
export function action<T extends UnitActionType>(type: T): Action;
export function action(type: ActionType, request?: unknown): Action {
  return (request === undefined ? { type } : { type, request }) as Action;
}

const requested = <T extends RequestActionType>(type: T) => (request: RequestFor<T>): Action => action(type, request);
const unit = <T extends UnitActionType>(type: T) => (): Action => action(type);

/** Complete helper catalog for the canonical prayer-actions wire shape. */
export const actions = {
  undock, dock, wait, mine, go,
  halt: unit("halt"),
  transfer: requested("transfer"), setHome: unit("set_home"), find: requested("find"), survey: unit("survey"),
  attack: requested("attack"), scan: nullableRequested("scan"), cloak: requested("cloak"), hunt: requested("hunt"), prepayTax: requested("prepay_tax"),
  acceptMission: requested("accept_mission"), abandonMission: requested("abandon_mission"), declineMission: requested("decline_mission"), completeMission: requested("complete_mission"),
  loadPassenger: requested("load_passenger"), unloadPassenger: requested("unload_passenger"), buy: requested("buy"), sell: requested("sell"),
  cancelBuy: requested("cancel_buy"), cancelSell: requested("cancel_sell"), factionCreate: requested("faction_create"), factionInvite: requested("faction_invite"),
  factionAcceptInvite: requested("faction_accept_invite"), factionKick: requested("faction_kick"), factionSetRole: requested("faction_set_role"),
  facilityBuild: requested("facility_build"), factionFacilityBuild: requested("faction_facility_build"), facilityUpgrade: requested("facility_upgrade"),
  factionFacilityUpgrade: requested("faction_facility_upgrade"), facilityDismantle: requested("facility_dismantle"), factionFacilityDismantle: requested("faction_facility_dismantle"),
  facilitySetAccess: requested("facility_set_access"), facilitySetOutputPrice: requested("facility_set_output_price"), facilitySetName: requested("facility_set_name"),
  useItem: requested("use_item"), repair: nullableRequested("repair"), repairModule: requested("repair_module"), recycle: requested("recycle"), refuel: nullableRequested("refuel"),
  selfDestruct: unit("self_destruct"), switchShip: requested("switch_ship"), renameShip: requested("rename_ship"), installMod: requested("install_mod"), uninstallMod: requested("uninstall_mod"),
  buyShip: requested("buy_ship"), buyListedShip: requested("buy_listed_ship"), commissionShip: requested("commission_ship"), sellShip: requested("sell_ship"), scrapShip: requested("scrap_ship"),
  listShipForSale: requested("list_ship_for_sale"), refitShip: unit("refit_ship"), cancelCommission: requested("cancel_commission"), supplyCommission: requested("supply_commission"),
  cancelShipListing: requested("cancel_ship_listing"), placeShipBuyOrder: requested("place_ship_buy_order"), cancelShipBuyOrder: requested("cancel_ship_buy_order"),
  sellShipToOrder: requested("sell_ship_to_order"), cancelOrder: requested("cancel_order"), modifyOrder: requested("modify_order"), craft: requested("craft"), cancelCraftJob: requested("cancel_craft_job"),
  salvageWreck: requested("salvage_wreck"), towWreck: requested("tow_wreck"), scrapWreck: unit("scrap_wreck"), sellWreck: unit("sell_wreck"), releaseWreck: unit("release_wreck"), insureShip: requested("insure_ship"),
  citizenshipApply: requested("citizenship_apply"), citizenshipWithdraw: requested("citizenship_withdraw"), citizenshipRenounce: requested("citizenship_renounce"),
  tradeOffer: requested("trade_offer"), tradeAccept: requested("trade_accept"), factionLeave: unit("faction_leave"), factionWithdrawInvite: requested("faction_withdraw_invite"),
  factionProposeAlly: requested("faction_propose_ally"), factionAcceptAlly: requested("faction_accept_ally"), factionRemoveAlly: requested("faction_remove_ally"),
  factionDeclareWar: requested("faction_declare_war"), factionProposePeace: requested("faction_propose_peace"), factionAcceptPeace: requested("faction_accept_peace"),
  factionSetEnemy: requested("faction_set_enemy"), factionRemoveEnemy: requested("faction_remove_enemy"), factionPrepayTax: requested("faction_prepay_tax"),
  factionCancelMission: requested("faction_cancel_mission"), espionage: unit("espionage"), scanPoi: requested("scan_poi"), distressSignal: nullableRequested("distress_signal"), say: requested("say"),
} as const;
