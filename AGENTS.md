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
1. On first request the backend returns empty listings and spawns a background Universalis fetch.
2. Universalis data is cached in memory with a 15-minute TTL. The VM sleeps between requests (fly.io auto-stop), so cold starts serve stale/empty data and refresh on the next client poll.
3. The frontend fetches `/api/items?level={level}` for the supply pool and `/api/prices?dc={dc}` for live listings, then computes today's rotation index and the cheapest satisfying listing client-side.

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

# Supply pool for level 100 jobs
curl "http://localhost:8080/api/items?level=100" | python3 -m json.tool

# Trigger a price fetch and read results (first call is cold)
curl "http://localhost:8080/api/prices?dc=Aether"
sleep 10
curl "http://localhost:8080/api/prices?dc=Aether" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print('Items with listings:', sum(1 for v in d['listings'].values() if v))
print('Last updated:', d['last_updated'])
"
```

### Checking today's rotation

The frontend highlights which pool item is today's turn-in using:

```
supply_day = (unix_timestamp_seconds - 72000) / 86400
item_index = supply_day % pool_size
```

To compute it manually and verify against in-game:

```bash
python3 -c "
import time
supply_day = (int(time.time()) - 20*3600) // 86400
print(f'Supply day: {supply_day}')
for n in [1, 2, 3]:
    print(f'  pool of {n} → today index {supply_day % n}')
"
```

If the highlighted item doesn't match what's shown in-game, adjust `ROTATION_EPOCH_DAYS` in `frontend/src/main.rs` and redeploy the frontend.

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

## Universalis rate limits

The backend respects Universalis's 8-concurrent-request limit via `tokio::sync::Semaphore`. Item IDs are chunked into batches of 100 per request. A full refresh fetches all ~2500 items across ~26 requests in roughly 4 parallel rounds.
