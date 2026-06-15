# turnins.fyi

An FFXIV Grand Company Supply & Provisioning price helper. The site shows the cheapest market board listings for daily supply turn-in items — accounting for required stack sizes, which plain Universalis lookups ignore.

**The core insight:** Universalis sorts by price-per-unit, but listings often have large stacks. If a turn-in only needs 1 or 3 items you can't split a stack of 99, so the cheapest-per-unit listing may not be buyable. This app finds the cheapest *single listing whose stack size satisfies the required quantity*.

## Architecture

Rust workspace with three crates:

| Crate | Purpose | Deployed to |
|---|---|---|
| `backend/` | Axum HTTP server, caches Universalis data | fly.io |
| `frontend/` | Leptos WASM SPA | GitHub Pages (turnins.fyi) |
| `shared/` | Shared API types (serde structs) | — |

**Data flow:**
1. The frontend sends one `POST /api/prices` with each job's level (1-100, per-job) and a list of datacenters to search. On first request, the backend returns empty best-listings and spawns a background Universalis fetch for each requested DC.
2. Universalis data is cached in memory per-DC with a 15-minute TTL. The VM sleeps between requests (fly.io auto-stop), so cold starts serve stale/empty data and refresh on the next client poll.
3. The backend computes the cheapest HQ/NQ listing across the requested DCs for each item in the requested jobs' pools, then returns per-job pools with best-listings resolved (tagged with DC and world). The frontend computes today's rotation index and renders the pools, and can highlight items for "My List" selection.

**Item data** (`data/items.json`) is pre-compiled from [TeamCraft's gc-supply.json](https://github.com/ffxiv-teamcraft/ffxiv-teamcraft) augmented with item names from XIVAPI. It covers all 11 DoH/DoL jobs across all 100 levels.

## Running locally

```bash
# Build and start the backend (port 8080)
cargo run --release --package backend

# In a second terminal, serve the frontend with hot-reload
cd frontend
trunk serve   # proxies /api/* → localhost:8080 automatically
# open http://localhost:8080 (or whatever trunk prints)
```

### Testing the backend endpoints

```bash
# Health check
curl http://localhost:8080/health

# Datacenters with regions
curl http://localhost:8080/api/datacenters | python3 -m json.tool

# Combined prices request (first call per DC is cold -- empty bests until cache warms)
curl -X POST http://localhost:8080/api/prices \
  -H "Content-Type: application/json" \
  -d '{
    "CRP": 100, "BSM": 100, "ARM": 100, "GSM": 100, "LTW": 100,
    "WVR": 100, "ALC": 100, "CUL": 100, "MIN": 100, "BTN": 100, "FSH": 100,
    "datacenters": ["Aether", "Crystal"]
  }' | python3 -m json.tool

# Empty-pool level example (level 48 has no MIN pool)
curl -X POST http://localhost:8080/api/prices \
  -H "Content-Type: application/json" \
  -d '{
    "CRP": 48, "BSM": 48, "ARM": 48, "GSM": 48, "LTW": 48,
    "WVR": 48, "ALC": 48, "CUL": 48, "MIN": 48, "BTN": 48, "FSH": 48,
    "datacenters": ["Aether"]
  }' | python3 -c "
import json, sys
d = json.load(sys.stdin)
for j in d['jobs']:
    print(j['abbr'], 'level', j['level'], '-> items:', len(j['items']))
"

# Empty datacenters -- graceful all-None
curl -X POST http://localhost:8080/api/prices \
  -H "Content-Type: application/json" \
  -d '{
    "CRP": 100, "BSM": 100, "ARM": 100, "GSM": 100, "LTW": 100,
    "WVR": 100, "ALC": 100, "CUL": 100, "MIN": 100, "BTN": 100, "FSH": 100,
    "datacenters": []
  }' | python3 -m json.tool

sleep 10  # allow background fetch to populate cache
# re-run the combined prices request above to see hq_best/nq_best populated
```

### Checking today's rotation

The frontend highlights which pool item is today's turn-in using the rotation formula in `frontend/src/rotation.rs`:

```
supply_day = (unix_timestamp_seconds - 72000) / 86400 + ROTATION_EPOCH_DAYS
item_index = supply_day % pool_size
```

To compute it manually and verify against in-game:

```bash
python3 -c "
import time
supply_day = (int(time.time()) - 72000) // 86400
print(f'Supply day: {supply_day}')
for n in [1, 2, 3]:
    print(f'  pool of {n} → today index {supply_day % n}')
"
```

If the highlighted item doesn't match what's shown in-game, adjust `ROTATION_EPOCH_DAYS` in `frontend/src/rotation.rs` and redeploy the frontend.

As a quick workaround while a fix is pending deploy, use the per-job level ±1 stepper buttons in the UI to adjust to a level where the visible pool items match what you see in-game.

## Regenerating item data

If item data needs to be refreshed (new FFXIV patch with new turn-in items):

```bash
cd data
# Download the latest source from TeamCraft
curl -s "https://raw.githubusercontent.com/ffxiv-teamcraft/ffxiv-teamcraft/master/libs/data/src/lib/json/gc-supply.json" -o gc_supply_raw.json
# Rebuild items.json (fetches names from XIVAPI — takes ~10s)
python3 gen_items.py
```

Commit the updated `data/items.json`. The backend embeds it at compile time (`include_bytes!`).

## Deployment

### Backend (fly.io)

First-time setup:
```bash
flyctl launch --no-deploy   # keep app name "turnins-fyi" or update fly.toml + the GitHub Action
flyctl deploy
```

The GitHub Action (`.github/workflows/deploy-backend.yml`) deploys automatically on pushes to `main` that touch `backend/`, `shared/`, `data/items.json`, `Dockerfile`, or `fly.toml`. It requires a `FLY_API_TOKEN` repository secret.

### Frontend (GitHub Pages)

The GitHub Action (`.github/workflows/deploy-frontend.yml`) builds the WASM bundle with `trunk build --release` (setting `BACKEND_URL=https://turnins-fyi.fly.dev`) and pushes to the `gh-pages` branch.

Enable GitHub Pages in repo settings (source: `gh-pages` branch). Point the `turnins.fyi` DNS CNAME to `<your-github-username>.github.io`.

To build the frontend manually:
```bash
cd frontend
BACKEND_URL=https://turnins-fyi.fly.dev trunk build --release
# output in frontend/dist/
```

## Future work

- **Switch from OpenSSL to rustls** — the backend currently depends on `openssl-sys`, which requires `pkg-config` and `libssl-dev` in the Docker build. Enabling the `rustls-tls` feature on `reqwest` (and disabling `default-features`) would remove the native OpenSSL dependency and simplify the Dockerfile.
- **Implement dependency caching in CI** - the frontend builds in particular were taking pretty long (10 minutes)
- **Lock down CORS in production** — the backend currently allows any origin (`CorsLayer::new().allow_origin(Any)`). Read an `ALLOWED_ORIGIN` env var at startup: if set use `AllowOrigin::exact(...)`, otherwise fall back to `Any` for local dev. Set `ALLOWED_ORIGIN=https://turnins.fyi` in `fly.toml` under `[env]`.
- **Support multiple small listings** - Show optional smaller listings - can be cheaper than a single-stack option, or the user might only need 1 or 2 of an item to complete the stack. Should do this for HQ and NQ.
- **List generation** - Selection mechanism ("My List") is implemented as a flat list. Future: group selected items by world/datacenter and compute an optimized shopping route (start at one DC/world, visit all items there, then teleport to the next DC/world, etc.). 
- **Fix data retrieval** - It should be an every 15-minute job on the backend to scrape everything, ideally configured in a constant or environment variable

## Universalis rate limits

The backend respects Universalis's 8-concurrent-request limit via `tokio::sync::Semaphore`. Item IDs are chunked into batches of 100 per request. A full refresh fetches all ~2500 items across ~26 requests in roughly 4 parallel rounds.
