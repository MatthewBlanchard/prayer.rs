#!/usr/bin/env python3
"""Extract Spacemolt API calls from prayer API logs.

Writes:
  - a CSV of one row per API response/call
  - a Markdown summary grouped by inferred session/player
"""

from __future__ import annotations

import argparse
import csv
import json
import re
from collections import Counter, defaultdict, deque
from datetime import datetime, timezone
from pathlib import Path


ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")
LINE_RE = re.compile(r"^(?P<ts>\S+)\s+\S+\s+(?P<target>.+?):\s+(?P<msg>.*)$")
FIELD_RE = re.compile(r'(\w+)=(?:"((?:[^"\\]|\\.)*)"|([^\s]+))')
CTX_RE = re.compile(r"facility catalog path: built fetch context session=(?P<session>.+?) session_last_catalog_version=")
REQ_RE = re.compile(r"spacemolt api request\b")
RESP_RE = re.compile(r"spacemolt api response\b")
COMMAND_RE = re.compile(
    r"prayer_api::service: (?P<bot>.+?) - (?P<command>.+?) id=(?P<runtime_id>[0-9a-f-]+)"
)
HYDRATE_RE = re.compile(r"startup session hydration: .*id=(?P<runtime_id>[0-9a-f-]+) label=(?P<label>.+?)(?:\s+\w+=|$)")
UPSTREAM_START_RE = re.compile(
    r"upstream api call start id=(?P<runtime_id>[0-9a-f-]+) command=(?P<command>\S+) api_action=(?P<api_action>\S+)"
)


def strip_ansi(line: str) -> str:
    return ANSI_RE.sub("", line.rstrip("\n"))


def parse_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for match in FIELD_RE.finditer(text):
        key = match.group(1)
        value = match.group(2) if match.group(2) is not None else match.group(3)
        if value is not None:
            fields[key] = bytes(value, "utf-8").decode("unicode_escape")
    return fields


def parse_json_field(text: str, field: str) -> object | None:
    marker = f"{field}="
    start = text.find(marker)
    if start < 0:
        return None
    raw = text[start + len(marker) :].strip()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return None


def preview(value: object | None, limit: int = 220) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        text = value
    else:
        text = json.dumps(value, separators=(",", ":"), ensure_ascii=False)
    text = text.replace("\n", "\\n")
    if len(text) > limit:
        return text[: limit - 1] + "…"
    return text


def username_from_response(body: object) -> str:
    if not isinstance(body, dict):
        return ""
    structured = body.get("structuredContent")
    if isinstance(structured, dict):
        player = structured.get("player")
        if isinstance(player, dict) and isinstance(player.get("username"), str):
            return player["username"]
    result = body.get("result")
    if isinstance(result, str):
        first = result.splitlines()[0] if result else ""
        match = re.match(r"(.+?)\s+\[", first)
        if match:
            return match.group(1)
    return ""


def extract(log_path: Path) -> list[dict[str, str]]:
    pending: dict[tuple[str, str, str], deque[dict[str, str]]] = defaultdict(deque)
    session_names: dict[str, str] = {}
    rows: list[dict[str, str]] = []
    current_context = ""
    current_command_hint = ""
    runtime_names: dict[str, str] = {}
    last_command_by_runtime: dict[str, str] = {}

    with log_path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_no, raw_line in enumerate(handle, 1):
            line = strip_ansi(raw_line)
            match = LINE_RE.match(line)
            if not match:
                continue
            ts = match.group("ts")
            msg = match.group("msg")

            ctx = CTX_RE.search(line)
            if ctx:
                current_context = ctx.group("session")
                current_command_hint = ""

            hydrate = HYDRATE_RE.search(line)
            if hydrate:
                runtime_names[hydrate.group("runtime_id")] = hydrate.group("label")

            command = COMMAND_RE.search(line)
            if command:
                runtime_id = command.group("runtime_id")
                runtime_names[runtime_id] = command.group("bot")
                current_context = command.group("bot")
                current_command_hint = command.group("command")
                last_command_by_runtime[runtime_id] = command.group("command")

            upstream = UPSTREAM_START_RE.search(line)
            if upstream:
                runtime_id = upstream.group("runtime_id")
                if runtime_id in runtime_names:
                    current_context = runtime_names[runtime_id]
                current_command_hint = last_command_by_runtime.get(runtime_id, upstream.group("command"))

            if REQ_RE.search(msg):
                fields = parse_fields(msg)
                key = (
                    fields.get("requested_utc", ""),
                    fields.get("api_action", ""),
                    fields.get("path", ""),
                )
                pending[key].append(
                    {
                        "request_line": str(line_no),
                        "request_ts": ts,
                        "requested_utc": fields.get("requested_utc", ""),
                        "method": fields.get("method", ""),
                        "api_action": fields.get("api_action", ""),
                        "path": fields.get("path", ""),
                        "rate_bucket": fields.get("rate_bucket", ""),
                        "payload": preview(parse_json_field(msg, "payload"), 500),
                        "context_session": current_context,
                        "command_hint": current_command_hint,
                    }
                )
                continue

            if RESP_RE.search(msg):
                fields = parse_fields(msg)
                key = (
                    fields.get("requested_utc", ""),
                    fields.get("api_action", ""),
                    fields.get("path", ""),
                )
                req = pending[key].popleft() if pending.get(key) else {}
                body = parse_json_field(msg, "response")
                body_dict = body if isinstance(body, dict) else {}
                session = body_dict.get("session") if isinstance(body_dict, dict) else {}
                session_id = fields.get("session_id", "")
                if not session_id:
                    session_id = session.get("id", "") if isinstance(session, dict) else ""
                player_id = fields.get("player_id", "")
                if not player_id:
                    player_id = session.get("player_id", "") if isinstance(session, dict) else ""
                player_name = fields.get("player_name", "") or username_from_response(body)
                if session_id and player_name:
                    session_names[session_id] = player_name
                if not player_name and session_id:
                    player_name = session_names.get(session_id, "")
                if not player_name:
                    player_name = req.get("context_session", "")

                rows.append(
                    {
                        "player_name": player_name,
                        "session_id": session_id,
                        "player_id": player_id,
                        "request_line": req.get("request_line", ""),
                        "response_line": str(line_no),
                        "request_ts": req.get("request_ts", ""),
                        "response_ts": ts,
                        "requested_utc": fields.get("requested_utc", req.get("requested_utc", "")),
                        "responded_utc": fields.get("responded_utc", ""),
                        "method": req.get("method", "") or fields.get("method", ""),
                        "api_action": fields.get("api_action", req.get("api_action", "")),
                        "path": fields.get("path", req.get("path", "")),
                        "status": fields.get("status", ""),
                        "rate_bucket": fields.get("rate_bucket", req.get("rate_bucket", "")),
                        "context_session": req.get("context_session", ""),
                        "command_hint": req.get("command_hint", ""),
                        "payload": req.get("payload", "") or fields.get("payload", ""),
                        "result_preview": fields.get("result_preview", "")
                        or preview(body_dict.get("result") if isinstance(body_dict, dict) else body, 320),
                        "error_preview": fields.get("error_preview", "")
                        or preview(body_dict.get("error") if isinstance(body_dict, dict) else None, 320),
                    }
                )

    return rows


def write_summary(rows: list[dict[str, str]], path: Path) -> None:
    by_player: dict[str, list[dict[str, str]]] = defaultdict(list)
    for row in rows:
        by_player[row["player_name"] or "(unknown)"].append(row)

    lines = ["# API Call Summary", ""]
    lines.append(f"Total calls: {len(rows)}")
    lines.append("")

    for player in sorted(by_player):
        group = by_player[player]
        statuses = Counter(row["status"] or "?" for row in group)
        actions = Counter(row["api_action"] or "?" for row in group)
        failures = [row for row in group if row["status"] and row["status"] != "200"]
        first = group[0]["response_ts"]
        last = group[-1]["response_ts"]
        lines.append(f"## {player}")
        lines.append(f"- Calls: {len(group)}")
        lines.append(f"- Window: {first} to {last}")
        lines.append(f"- Statuses: {', '.join(f'{k}={v}' for k, v in sorted(statuses.items()))}")
        lines.append(
            "- Top actions: "
            + ", ".join(f"{action}={count}" for action, count in actions.most_common(8))
        )
        if failures:
            lines.append("- Non-200 calls:")
            for row in failures[:12]:
                lines.append(
                    f"  - line {row['response_line']}: {row['api_action']} status={row['status']} {row['error_preview']}"
                )
        lines.append("")

    path.write_text("\n".join(lines), encoding="utf-8")


def parse_ts_minute(value: str) -> str:
    if not value:
        return ""
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return value[:16]
    parsed = parsed.astimezone(timezone.utc).replace(second=0, microsecond=0)
    return parsed.isoformat().replace("+00:00", "Z")


def bucket_kind(rate_bucket: str) -> str:
    if rate_bucket.endswith("_mutation"):
        return "mutation"
    if rate_bucket.endswith("_query"):
        return "query"
    return rate_bucket or "unknown"


def write_rate_outputs(rows: list[dict[str, str]], csv_path: Path, summary_path: Path) -> None:
    counts: Counter[tuple[str, str, str]] = Counter()
    by_action: dict[tuple[str, str], list[str]] = defaultdict(list)

    for row in rows:
        minute = parse_ts_minute(row["response_ts"] or row["responded_utc"])
        kind = bucket_kind(row["rate_bucket"])
        action = row["api_action"] or "(unknown)"
        if not minute:
            continue
        counts[(minute, kind, action)] += 1
        by_action[(kind, action)].append(minute)

    csv_path.parent.mkdir(parents=True, exist_ok=True)
    with csv_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["minute_utc", "bucket_kind", "api_action", "calls"])
        writer.writeheader()
        for (minute, kind, action), calls in sorted(counts.items()):
            writer.writerow(
                {
                    "minute_utc": minute,
                    "bucket_kind": kind,
                    "api_action": action,
                    "calls": calls,
                }
            )

    lines = ["# API Call Rates", ""]
    lines.append("| bucket | api_action | calls | minutes active | avg calls/min | peak calls/min | peak minute |")
    lines.append("|---|---:|---:|---:|---:|---:|---|")

    summary_rows = []
    for (kind, action), minutes in by_action.items():
        per_minute = Counter(minutes)
        total = sum(per_minute.values())
        active_minutes = len(per_minute)
        average = total / active_minutes if active_minutes else 0.0
        peak_minute, peak = max(per_minute.items(), key=lambda item: (item[1], item[0]))
        summary_rows.append((kind, action, total, active_minutes, average, peak, peak_minute))

    for kind, action, total, active_minutes, average, peak, peak_minute in sorted(
        summary_rows, key=lambda row: (row[0], -row[4], row[1])
    ):
        lines.append(
            f"| {kind} | `{action}` | {total} | {active_minutes} | {average:.2f} | {peak} | {peak_minute} |"
        )

    summary_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", type=Path)
    parser.add_argument("--csv", type=Path, default=Path("logs/api-calls.csv"))
    parser.add_argument("--summary", type=Path, default=Path("logs/api-calls-summary.md"))
    parser.add_argument("--rates-csv", type=Path, default=Path("logs/api-call-rates.csv"))
    parser.add_argument("--rates-summary", type=Path, default=Path("logs/api-call-rates-summary.md"))
    args = parser.parse_args()

    rows = extract(args.log)
    args.csv.parent.mkdir(parents=True, exist_ok=True)
    args.summary.parent.mkdir(parents=True, exist_ok=True)

    fields = [
        "player_name",
        "session_id",
        "player_id",
        "request_line",
        "response_line",
        "request_ts",
        "response_ts",
        "requested_utc",
        "responded_utc",
        "method",
        "api_action",
        "path",
        "status",
        "rate_bucket",
        "context_session",
        "command_hint",
        "payload",
        "result_preview",
        "error_preview",
    ]
    with args.csv.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)

    write_summary(rows, args.summary)
    write_rate_outputs(rows, args.rates_csv, args.rates_summary)
    print(f"wrote {len(rows)} calls to {args.csv}")
    print(f"wrote summary to {args.summary}")
    print(f"wrote rates to {args.rates_csv}")
    print(f"wrote rates summary to {args.rates_summary}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
