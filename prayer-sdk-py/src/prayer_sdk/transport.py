"""HTTP lifecycle, serialization, and error handling."""

from __future__ import annotations

import asyncio
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

import httpx
from pydantic import BaseModel

from .errors import PrayerApiError, PrayerConnectionError, PrayerTimeoutError


@dataclass(frozen=True, slots=True)
class RequestOptions:
    timeout: float | httpx.Timeout | None = None


class Transport:
    def __init__(self, base_url: str, *, token: str | None = None,
                 headers: Mapping[str, str] | None = None,
                 timeout: float | httpx.Timeout = 30.0,
                 client: httpx.AsyncClient | None = None,
                 close_client: bool | None = None) -> None:
        self.base_url = base_url.rstrip("/") + "/"
        self.timeout = timeout
        self._closed = False
        self._owns_client = client is None if close_client is None else close_client
        defaults = dict(headers or {})
        defaults["accept"] = "application/json"
        if token:
            defaults["authorization"] = f"Bearer {token}"
        self.client = client or httpx.AsyncClient(headers=defaults)
        self._headers = defaults if client is not None else {}

    async def request(self, method: str, path: str, *, query: Mapping[str, Any] | None = None,
                      json: Any = None, headers: Mapping[str, Any] | None = None,
                      options: RequestOptions | None = None) -> Any:
        request_headers = dict(self._headers)
        request_headers.update({k: str(v) for k, v in (headers or {}).items() if v is not None})
        key = next((v for k, v in request_headers.items() if k.lower() == "idempotency-key"), None)
        if key is not None and not key.strip():
            raise ValueError("Idempotency-Key must not be blank")
        params = {k: v for k, v in (query or {}).items() if v is not None}
        payload = _wire(json)
        timeout = options.timeout if options and options.timeout is not None else self.timeout
        try:
            response = await self.client.request(method, self.base_url + path, params=params,
                                                 json=payload, headers=request_headers,
                                                 timeout=timeout)
        except asyncio.CancelledError:
            raise
        except httpx.TimeoutException as error:
            milliseconds = float(timeout) * 1000 if isinstance(timeout, (int, float)) else 30_000
            raise PrayerTimeoutError(milliseconds, error) from error
        except httpx.HTTPError as error:
            raise PrayerConnectionError(f"Prayer API {method} {path} failed: {error}", error) from error
        if response.status_code == 204:
            return None
        content_type = response.headers.get("content-type", "")
        body: Any = None
        if response.content:
            if "json" not in content_type:
                raise PrayerConnectionError(
                    f"Prayer API {method} {path} returned non-JSON content type {content_type!r}"
                )
            try:
                body = response.json()
            except ValueError as error:
                raise PrayerConnectionError(f"Prayer API {method} {path} returned malformed JSON", error) from error
        if response.is_error:
            if isinstance(body, dict) and isinstance(body.get("error"), dict):
                raise PrayerApiError.from_envelope(response.status_code, body)
            raise PrayerApiError(response.status_code, "http_error",
                                 f"Prayer API returned {response.status_code}",
                                 response.status_code >= 500)
        if body is None:
            raise PrayerConnectionError(f"Prayer API {method} {path} returned an empty success body")
        return body

    async def aclose(self) -> None:
        if self._closed:
            return
        self._closed = True
        if self._owns_client:
            await self.client.aclose()


def _wire(value: Any) -> Any:
    if isinstance(value, BaseModel):
        return value.model_dump(by_alias=True, exclude_none=False)
    if isinstance(value, tuple):
        return [_wire(item) for item in value]
    if isinstance(value, list):
        return [_wire(item) for item in value]
    if isinstance(value, Mapping):
        return {key: _wire(item) for key, item in value.items()}
    return value
