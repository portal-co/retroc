# Agent Guide — retroc

retroc is a retro-computing toolchain. It transitively depends on **speet** (via moond)
and **wasm-blitz** (via speet's `speet-recompile` crate).

Dependency chain: retroc → moond → speet → speet-recompile → wasm-blitz

## Compression-aware logging

This workspace transitively depends on **speet** and **wasm-blitz**, both of which
support structured verbose logging.
Set the following env vars before running any tool or test:

| Variable | Effect |
|---|---|
| `PORTAL_LOG_JSON=1` | speet and wasm-blitz emit structured NDJSON events; compresses ~3–5× better under token compression proxies. |
| `PORTAL_LOG_BATCH=1` | Group events by phase into single JSON arrays. |

When a token compression proxy is active, more verbose logging is net-cheaper than
terse output — the proxy reclaims the token cost.

These variables have no effect when unset and do not change program correctness.
