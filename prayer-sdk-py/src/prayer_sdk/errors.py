"""Prayer SDK exception taxonomy and retry helpers."""

from __future__ import annotations

from typing import Any


class PrayerApiError(Exception):
    def __init__(self, status: int, code: str, message: str, retryable: bool = False,
                 details: Any = None, request_id: str | None = None) -> None:
        super().__init__(message)
        self.status = status
        self.code = code
        self.retryable = retryable
        self.details = details
        self.request_id = request_id
        self.retry_after_ms = _retry_after(details)

    @classmethod
    def from_envelope(cls, status: int, body: dict[str, Any]) -> PrayerApiError:
        error = body.get("error", {})
        code = str(error.get("code", "http_error"))
        kind: type[PrayerApiError]
        if code == "lane_busy":
            kind = LaneBusyError
        elif status == 400 or code in {"validation", "bad_request"}:
            kind = PrayerValidationError
        elif status in {401, 403}:
            kind = PrayerAuthenticationError
        elif status == 404 or code == "not_found":
            kind = PrayerNotFoundError
        else:
            kind = cls
        return kind(status, code, str(error.get("message", f"Prayer API returned {status}")),
                    bool(error.get("retryable", False)), error.get("details"),
                    body.get("requestId") or body.get("request_id"))


class PrayerValidationError(PrayerApiError): pass
class PrayerAuthenticationError(PrayerApiError): pass
class PrayerNotFoundError(PrayerApiError): pass
class LaneBusyError(PrayerApiError): pass


class PrayerConnectionError(Exception):
    def __init__(self, message: str, cause: BaseException | None = None) -> None:
        super().__init__(message)
        self.cause = cause


class PrayerTimeoutError(PrayerConnectionError):
    def __init__(self, timeout_ms: float, cause: BaseException | None = None) -> None:
        super().__init__(f"Prayer API request timed out after {timeout_ms:g}ms", cause)
        self.timeout_ms = timeout_ms


class PrayerCompatibilityError(Exception): pass


def is_retryable_error(error: BaseException) -> bool:
    return isinstance(error, PrayerConnectionError) or (
        isinstance(error, PrayerApiError) and error.retryable
    )


def _retry_after(details: Any) -> float | None:
    if not isinstance(details, dict):
        return None
    value = details.get("retryAfterMs", details.get("retry_after_ms"))
    return float(value) if isinstance(value, (int, float)) and value >= 0 else None

