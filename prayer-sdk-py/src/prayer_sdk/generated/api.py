# AUTO-GENERATED from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.
from __future__ import annotations

from typing import Any
from urllib.parse import quote

from ..transport import RequestOptions, Transport

class PrayerApi:
    def __init__(self, transport: Transport) -> None: self._transport = transport

    async def list_market_movements(self, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements'
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def reserve_market_movement(self, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/reservations'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def complete_market_movement(self, movement_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/complete'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def fail_market_movement(self, movement_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/fail'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def get_market_movement_health(self, movement_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/health'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def reconcile_market_movement(self, movement_id: Any, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/reconcile'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def release_market_movement(self, movement_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/release'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def start_market_movement(self, movement_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/market-movements/{movement_id}/start'.format(movement_id=quote(str(movement_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def list_virtual_craft_orders(self, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-craft-orders'
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def create_virtual_craft_orders(self, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-craft-orders'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def reserve_virtual_craft_orders(self, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-craft-orders/reservations'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def fill_virtual_craft_order(self, order_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-craft-orders/{order_id}/fills'.format(order_id=quote(str(order_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def release_virtual_craft_order(self, order_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-craft-orders/{order_id}/reservation'.format(order_id=quote(str(order_id), safe=''))
        query = None
        return await self._transport.request('DELETE', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def list_virtual_orders(self, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-orders'
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def create_virtual_orders(self, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-orders'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def reserve_virtual_orders(self, body: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-orders/reservations'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def fill_virtual_order(self, order_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-orders/{order_id}/fills'.format(order_id=quote(str(order_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def release_virtual_order(self, order_id: Any, idempotency_key: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/admin/virtual-orders/{order_id}/reservation'.format(order_id=quote(str(order_id), safe=''))
        query = None
        return await self._transport.request('DELETE', path, query=query, json=None, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def list_bots(self, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots'
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def register_bot(self, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/register'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def get_bot(self, bot_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def execute_action_override(self, bot_id: Any, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/action-overrides'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def start_action_run(self, bot_id: Any, body: Any, idempotency_key: Any = None, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/action-runs'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def get_action_run(self, bot_id: Any, run_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/action-runs/{run_id}'.format(bot_id=quote(str(bot_id), safe=''), run_id=quote(str(run_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def cancel_action_run(self, bot_id: Any, run_id: Any, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/action-runs/{run_id}/cancel'.format(bot_id=quote(str(bot_id), safe=''), run_id=quote(str(run_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def halt_bot(self, bot_id: Any, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/halt'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def get_bot_queue(self, bot_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/queue'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def get_bot_normal_queue(self, bot_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/queue/normal'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def get_bot_override_queue(self, bot_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/queue/override'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def execute_script_override(self, bot_id: Any, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/script-overrides'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def start_script_run(self, bot_id: Any, body: Any, idempotency_key: Any = None, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/script-runs'.format(bot_id=quote(str(bot_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={'Idempotency-Key': idempotency_key}, options=options)

    async def get_script_run(self, bot_id: Any, run_id: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/script-runs/{run_id}'.format(bot_id=quote(str(bot_id), safe=''), run_id=quote(str(run_id), safe=''))
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def cancel_script_run(self, bot_id: Any, run_id: Any, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/bots/{bot_id}/script-runs/{run_id}/cancel'.format(bot_id=quote(str(bot_id), safe=''), run_id=quote(str(run_id), safe=''))
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def get_meta(self, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/meta'
        query = None
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)

    async def select_routes(self, body: Any, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/routes'
        query = None
        return await self._transport.request('POST', path, query=query, json=body, headers={}, options=options)

    async def get_state(self, fleet_version: Any = None, world_version: Any = None, map_version: Any = None, resources_version: Any = None, wildlife_version: Any = None, markets_version: Any = None, storage_version: Any = None, facilities_version: Any = None, observations_version: Any = None, communications_version: Any = None, factions_version: Any = None, catalog_version: Any = None, options: RequestOptions | None = None) -> Any:
        path = 'api/v1/state'
        query = {'fleet_version': fleet_version, 'world_version': world_version, 'map_version': map_version, 'resources_version': resources_version, 'wildlife_version': wildlife_version, 'markets_version': markets_version, 'storage_version': storage_version, 'facilities_version': facilities_version, 'observations_version': observations_version, 'communications_version': communications_version, 'factions_version': factions_version, 'catalog_version': catalog_version}
        return await self._transport.request('GET', path, query=query, json=None, headers={}, options=options)
