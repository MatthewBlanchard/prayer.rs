"""Exact-wire action constructors."""

from __future__ import annotations

from collections.abc import Mapping
from types import MappingProxyType
from typing import Any

Action = Mapping[str, Any]


def action(type: str, request: Mapping[str, Any] | None = None, *, include_request: bool | None = None) -> Action:
    value: dict[str, Any] = {"type": type}
    if request is not None or include_request:
        value["request"] = MappingProxyType(dict(request or {}))
    return MappingProxyType(value)


def undock() -> Action: return action("undock")
def dock() -> Action: return action("dock")
def wait(ticks: int) -> Action: return action("wait", {"ticks": ticks})
def mine(resource: str | None = None) -> Action: return action("mine", {"resource": resource})
def go(*, poi: str | None = None, system: str | None = None,
       kind: str | None = None, value: str | None = None) -> Action:
    choices = sum(x is not None for x in (poi, system, kind))
    if choices != 1:
        raise ValueError("go requires exactly one of poi, system, or kind/value")
    destination = {"kind": "poi", "value": poi} if poi is not None else (
        {"kind": "system", "value": system} if system is not None else {"kind": kind, "value": value}
    )
    return action("go", {"destination": destination})


_UNIT = {"halt", "set_home", "survey", "self_destruct", "refit_ship", "scrap_wreck",
         "sell_wreck", "release_wreck", "faction_leave", "espionage"}
_NULLABLE = {
    "scan": {"target": None}, "repair": {"target": None, "quantity": None, "item": None},
    "refuel": {"target": None, "quantity": None, "item": None},
    "distress_signal": {"distress_type": None},
}


def _named_helper(type: str):
    if type in _UNIT:
        return lambda: action(type)
    if type in _NULLABLE:
        return lambda **request: action(type, {**_NULLABLE[type], **request})
    return lambda request=None, **fields: action(type, request if request is not None else fields)


ACTION_TYPES = (
    "undock dock wait mine go halt transfer set_home find survey attack scan cloak hunt prepay_tax "
    "accept_mission abandon_mission decline_mission complete_mission load_passenger unload_passenger buy sell "
    "cancel_buy cancel_sell faction_create faction_invite faction_accept_invite faction_kick faction_set_role found_station "
    "facility_build faction_facility_build facility_upgrade faction_facility_upgrade facility_dismantle "
    "faction_facility_dismantle facility_set_access facility_set_output_price facility_set_name use_item repair "
    "repair_module recycle refuel self_destruct switch_ship rename_ship install_mod uninstall_mod buy_ship "
    "buy_listed_ship commission_ship sell_ship scrap_ship list_ship_for_sale refit_ship cancel_commission "
    "supply_commission cancel_ship_listing place_ship_buy_order cancel_ship_buy_order sell_ship_to_order "
    "cancel_order modify_order craft cancel_craft_job salvage_wreck tow_wreck scrap_wreck sell_wreck release_wreck "
    "insure_ship citizenship_apply citizenship_withdraw citizenship_renounce trade_offer trade_accept faction_leave "
    "faction_withdraw_invite faction_propose_ally faction_accept_ally faction_remove_ally faction_declare_war "
    "faction_propose_peace faction_accept_peace faction_set_enemy faction_remove_enemy faction_prepay_tax "
    "faction_cancel_mission espionage scan_poi distress_signal say"
).split()

for _type in ACTION_TYPES:
    if _type not in {"undock", "dock", "wait", "mine", "go"}:
        globals()[_type] = _named_helper(_type)

actions = MappingProxyType({name: globals()[name] for name in ACTION_TYPES})
__all__ = ["ACTION_TYPES", "Action", "action", "actions", *ACTION_TYPES]
