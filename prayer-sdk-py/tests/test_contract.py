from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import ValidationError

from prayer_sdk.actions import ACTION_TYPES
from prayer_sdk.generated.api import PrayerApi
from prayer_sdk.generated.models import Action, ActionRunOutcome

ROOT = Path(__file__).resolve().parents[1]
SPEC = json.loads((ROOT.parent / "prayer-api/openapi/prayer-v1.json").read_text())


def snake(value: str) -> str:
    import re
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value).replace("-", "_").lower()


def test_every_operation_id_is_generated() -> None:
    operation_ids = {
        operation["operationId"]
        for item in SPEC["paths"].values()
        for method, operation in item.items()
        if method in {"get", "post", "put", "patch", "delete"}
    }
    assert {snake(name) for name in operation_ids} <= set(dir(PrayerApi))


def test_action_helpers_match_contract_discriminators() -> None:
    variants = SPEC["components"]["schemas"]["Action"]["oneOf"]
    discriminators = {variant["properties"]["type"]["enum"][0] for variant in variants}
    assert set(ACTION_TYPES) == discriminators


def test_tagged_unions_reject_unknown_or_incomplete_payloads() -> None:
    Action.model_validate({"type": "wait", "request": {"ticks": 1}})
    ActionRunOutcome.model_validate({"status": "failed", "action_index": 0, "message": "no"})
    with pytest.raises(ValidationError): Action.model_validate({"type": "unknown"})
    with pytest.raises(ValidationError): ActionRunOutcome.model_validate({"status": "failed"})
