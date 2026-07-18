# Python SDK generation decision

The Python SDK uses a small repository-owned generator over
`prayer-api/openapi/prayer-v1.json`, matching the existing TypeScript strategy.

During the implementation spike, general-purpose generators were rejected as a
repository dependency: deterministic generation would require an additional locked
toolchain, while the contract's inline tagged unions and recursive state maps still
needed project-specific handling. The checked-in generator handles aliases, frozen
Pydantic models, recursive references, inline union variants, endpoint paths, query
encoding, headers, and stable ordering without network access or generated patches.

Regression fixtures cover `Action` tagged variants, terminal outcomes, exact JSON
aliases, recursive state payloads, and percent-encoded path parameters. Generated
modules contain wire mechanics only and never import the handwritten facade.

