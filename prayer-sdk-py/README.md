# Prayer SDK for Python

An asynchronous, typed HTTP client for Prayer API v1. It requires Python 3.11–3.13.

```python
from prayer_sdk import Prayer
from prayer_sdk.actions import dock, go, refuel

prayer = await Prayer.connect("http://127.0.0.1:7777", token="...")
bot = await prayer.bot("my-miner")
result = await bot.execute([go(poi="sol_central"), dock(), refuel()])
await prayer.aclose()  # once, at application shutdown
```

`Prayer`, `Bot`, `ActionRun`, and `ScriptRun` are long-lived async handles. Run-level
failure is returned as outcome data; HTTP and compatibility failures raise the
exceptions exported from `prayer_sdk`. Reattach after a restart with
`bot.action_run(run_id)` or `bot.script_run(run_id)`. Reusing the same idempotency
key safely identifies a repeated submission whose first response was uncertain.

The complete generated endpoint client is available as `prayer.advanced.api`, and
wire models are public from `prayer_sdk.generated.models`.

Development is networkless after dependencies are installed:

```console
python scripts/generate.py
python scripts/check_generated.py
pytest
ruff check src scripts tests
```

