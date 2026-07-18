#!/usr/bin/env python3
"""Generate the policy-free Python wire models and endpoint client."""

from __future__ import annotations

import json
import keyword
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SPEC = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "prayer-api/openapi/prayer-v1.json"
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "prayer-sdk-py/src/prayer_sdk/generated"
SCHEMA_NAMES: set[str] = set()


def snake(value: str) -> str:
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value).replace("-", "_").lower()
    return value + "_" if keyword.iskeyword(value) else value


def ref_name(ref: str) -> str:
    return ref.rsplit("/", 1)[-1]


def py_type(schema: dict[str, Any] | None) -> str:
    if not schema or schema is True:
        return "Any"
    if schema is False:
        return "Any"
    if "$ref" in schema:
        name = ref_name(schema["$ref"])
        return name if name in SCHEMA_NAMES else "Any"
    if "const" in schema:
        return f"Literal[{schema['const']!r}]"
    if "enum" in schema:
        return "Literal[" + ", ".join(repr(x) for x in schema["enum"]) + "]"
    variants = schema.get("oneOf") or schema.get("anyOf")
    if variants:
        return " | ".join(py_type(x) for x in variants)
    if schema.get("allOf"):
        # Current contract uses allOf only as a composition of records. A named
        # schema validates the aggregate fields below; inline composition is rare.
        return " | ".join(py_type(x) for x in schema["allOf"])
    kind = schema.get("type")
    if isinstance(kind, list):
        return " | ".join(py_type({**schema, "type": x}) for x in kind)
    if kind == "array":
        return f"tuple[{py_type(schema.get('items'))}, ...]"
    if kind == "object" or "properties" in schema or "additionalProperties" in schema:
        additional = schema.get("additionalProperties")
        if not schema.get("properties"):
            return f"FrozenDict[str, {py_type(additional)}]" if isinstance(additional, dict) else "FrozenDict[str, Any]"
        return "FrozenDict[str, Any]"
    if kind == "string":
        if schema.get("format") == "date-time":
            return "datetime"
        if schema.get("format") == "uuid":
            return "UUID"
        return "str"
    if kind == "integer":
        return "int"
    if kind == "number":
        return "float"
    if kind == "boolean":
        return "bool"
    if kind == "null":
        return "None"
    return "Any"


def model_source(spec: dict[str, Any]) -> str:
    global SCHEMA_NAMES
    SCHEMA_NAMES = set(spec["components"]["schemas"])
    lines = [
        "# AUTO-GENERATED from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.",
        "from __future__ import annotations",
        "",
        "from collections.abc import Iterator, Mapping",
        "from datetime import datetime",
        "from types import MappingProxyType",
        "from typing import Any, Generic, Literal, TypeVar",
        "from uuid import UUID",
        "",
        "from pydantic import BaseModel, ConfigDict, Field, RootModel",
        "",
        "V = TypeVar('V')",
        "class FrozenDict(Mapping[str, V], Generic[V]):",
        "    def __init__(self, value: Mapping[str, V] | None = None): self._data = MappingProxyType(dict(value or {}))",
        "    def __getitem__(self, key: str) -> V: return self._data[key]",
        "    def __iter__(self) -> Iterator[str]: return iter(self._data)",
        "    def __len__(self) -> int: return len(self._data)",
        "    def __repr__(self) -> str: return repr(dict(self._data))",
        "    @classmethod",
        "    def __get_pydantic_core_schema__(cls, source: Any, handler: Any) -> Any:",
        "        from pydantic_core import core_schema",
        "        args = getattr(source, '__args__', (str, Any))",
        "        values = handler.generate_schema(args[1])",
        "        return core_schema.no_info_after_validator_function(cls, core_schema.dict_schema(core_schema.str_schema(), values))",
        "",
        "class WireModel(BaseModel):",
        "    model_config = ConfigDict(populate_by_name=True, frozen=True, extra='forbid')",
        "",
    ]
    schemas = spec["components"]["schemas"]

    def emit_object(class_name: str, schema: dict[str, Any]) -> None:
        properties = schema.get("properties", {})
        nested: dict[str, str] = {}
        for wire, prop in properties.items():
            if not isinstance(prop, dict):
                continue
            if (prop.get("type") == "object" or "properties" in prop) and prop.get("properties"):
                nested_name = class_name + "".join(part.title() for part in snake(wire).rstrip("_").split("_"))
                emit_object(nested_name, prop)
                nested[wire] = nested_name
        lines.append(f"class {class_name}(WireModel):")
        required = set(schema.get("required", []))
        if not properties:
            lines.append("    pass")
        for wire, prop in properties.items():
            attr = snake(wire)
            annotation = nested.get(wire, py_type(prop if isinstance(prop, dict) else None))
            default = "..." if wire in required else "None"
            if wire not in required and "None" not in annotation:
                annotation += " | None"
            alias = f", alias={wire!r}" if attr != wire else ""
            lines.append(f"    {attr}: {annotation} = Field({default}{alias})")
        lines.append("")

    for name, schema in schemas.items():
        properties = schema.get("properties")
        if properties is not None and not any(k in schema for k in ("oneOf", "anyOf", "allOf")):
            emit_object(name, schema)
        elif schema.get("oneOf") and all(
            variant.get("type") == "object" or variant.get("properties")
            for variant in schema["oneOf"]
        ):
            variants = []
            for index, variant in enumerate(schema["oneOf"], 1):
                variant_name = f"{name}Variant{index}"
                emit_object(variant_name, variant)
                variants.append(variant_name)
            lines.extend([
                f"class {name}(RootModel[{(' | '.join(variants))!r}]):",
                "    model_config = ConfigDict(frozen=True)", "",
            ])
        else:
            lines.extend([f"class {name}(RootModel[{py_type(schema)!r}]):", "    model_config = ConfigDict(frozen=True)", "",])
    lines.extend([f"{name}.model_rebuild()" for name in schemas])
    return "\n".join(lines) + "\n"


def api_source(spec: dict[str, Any]) -> str:
    operations = []
    for path, item in spec["paths"].items():
        path_params = item.get("parameters", [])
        for method, operation in item.items():
            if method not in {"get", "post", "put", "patch", "delete"}:
                continue
            operations.append((operation["operationId"], method.upper(), path, path_params + operation.get("parameters", []), operation))
    lines = [
        "# AUTO-GENERATED from prayer-api/openapi/prayer-v1.json. DO NOT EDIT.",
        "from __future__ import annotations", "", "from typing import Any", "from urllib.parse import quote", "",
        "from ..transport import RequestOptions, Transport", "", "class PrayerApi:",
        "    def __init__(self, transport: Transport) -> None: self._transport = transport", "",
    ]
    for op_id, method, path, params, operation in operations:
        name = snake(op_id)
        path_ps = [p for p in params if p.get("in") == "path"]
        query_ps = [p for p in params if p.get("in") == "query"]
        header_ps = [p for p in params if p.get("in") == "header"]
        body = operation.get("requestBody")
        args = [snake(p["name"]) + ": Any" for p in path_ps]
        if body:
            args.append("body: Any")
        args.extend(snake(p["name"]) + (": Any" if p.get("required") else ": Any = None") for p in query_ps + header_ps)
        args.append("options: RequestOptions | None = None")
        lines.append(f"    async def {name}(self, {', '.join(args)}) -> Any:")
        rendered = repr(path.lstrip("/"))
        for p in path_ps:
            wire, attr = p["name"], snake(p["name"])
            rendered = rendered.replace("{" + wire + "}", "{" + attr + "}")
        if path_ps:
            lines.append(f"        path = {rendered}.format(" + ", ".join(f"{snake(p['name'])}=quote(str({snake(p['name'])}), safe='')" for p in path_ps) + ")")
        else:
            lines.append(f"        path = {rendered}")
        if query_ps:
            pairs = ", ".join(f"{p['name']!r}: {snake(p['name'])}" for p in query_ps)
            lines.append(f"        query = {{{pairs}}}")
        else:
            lines.append("        query = None")
        headers = ", ".join(f"{p['name']!r}: {snake(p['name'])}" for p in header_ps)
        body_expr = "body" if body else "None"
        lines.append(f"        return await self._transport.request({method!r}, path, query=query, json={body_expr}, headers={{{headers}}}, options=options)")
        lines.append("")
    return "\n".join(lines)


def main() -> None:
    spec = json.loads(SPEC.read_text())
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "models.py").write_text(model_source(spec))
    (OUT / "api.py").write_text(api_source(spec))


if __name__ == "__main__":
    main()
