mod item_data;
mod universalis;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::{RwLock, Semaphore};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use item_data::ItemData;
use shared::{DataCenter, DataCentersResponse, ItemsResponse, Listing, PricesResponse};

// ── Constants ──────────────────────────────────────────────────────────────

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const ITEMS_JSON: &[u8] = include_bytes!("../../data/items.json");

const DATACENTERS: &[&str] = &[
    "Elemental", "Gaia", "Mana", "Meteor",     // JP
    "Aether", "Crystal", "Dynamis", "Primal",   // NA
    "Chaos", "Light",                            // EU
    "Materia",                                   // OCE
];

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Default)]
struct PriceCache {
    last_fetched: Option<Instant>,
    last_fetched_unix: Option<u64>,
    fetching: bool,
    data: HashMap<u32, Vec<Listing>>,
}

struct AppState {
    item_data: ItemData,
    /// One cache entry per DC name (lowercase).
    caches: HashMap<String, Arc<RwLock<PriceCache>>>,
    http: Client,
    /// Shared across all concurrent DC fetches to respect Universalis's 8-request limit.
    universalis_sem: Arc<Semaphore>,
}

// ── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let item_data = ItemData::load(ITEMS_JSON).expect("failed to load items.json");
    info!(
        "Loaded {} item names, {} level entries",
        item_data.all_item_ids.len(),
        item_data.pool.len()
    );

    let caches: HashMap<String, Arc<RwLock<PriceCache>>> = DATACENTERS
        .iter()
        .map(|dc| (dc.to_lowercase(), Arc::new(RwLock::new(PriceCache::default()))))
        .collect();

    let state = Arc::new(AppState {
        item_data,
        caches,
        http: Client::new(),
        universalis_sem: Arc::new(Semaphore::new(8)),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/prices", get(prices_handler))
        .route("/api/items", get(items_handler))
        .route("/api/datacenters", get(datacenters_handler))
        .route("/health", get(|| async { "ok" }))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// ── Handlers ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PricesQuery {
    dc: String,
}

async fn prices_handler(
    Query(q): Query<PricesQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let dc_key = q.dc.to_lowercase();
    let Some(cache_lock) = state.caches.get(&dc_key) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unknown datacenter"})),
        )
            .into_response();
    };

    // Check if a refresh is needed (without holding write lock during fetch)
    let needs_refresh = {
        let cache = cache_lock.read().await;
        !cache.fetching
            && cache
                .last_fetched
                .map(|t| t.elapsed() > CACHE_TTL)
                .unwrap_or(true)
    };

    if needs_refresh {
        // Mark as fetching under write lock
        {
            let mut cache = cache_lock.write().await;
            // Double-check after acquiring write lock
            if !cache.fetching
                && cache
                    .last_fetched
                    .map(|t| t.elapsed() > CACHE_TTL)
                    .unwrap_or(true)
            {
                cache.fetching = true;
            }
        }

        // Spawn background fetch
        let cache_lock2 = Arc::clone(cache_lock);
        let http = state.http.clone();
        let dc = q.dc.clone();
        let item_ids: Vec<u32> = state.item_data.all_item_ids.clone();
        let sem = Arc::clone(&state.universalis_sem);
        tokio::spawn(async move {
            info!("Starting Universalis fetch for DC {dc}");
            let new_data = universalis::fetch_all(&http, &sem, &dc, &item_ids).await;
            let mut cache = cache_lock2.write().await;
            cache.data = new_data;
            cache.last_fetched = Some(Instant::now());
            cache.last_fetched_unix = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
            cache.fetching = false;
            info!("Universalis fetch complete for DC {dc}");
        });
    }

    // Return current cache immediately (may be empty on cold start)
    let cache = cache_lock.read().await;
    Json(PricesResponse {
        last_updated: cache.last_fetched_unix,
        listings: cache.data.clone(),
    })
    .into_response()
}

#[derive(Deserialize)]
struct ItemsQuery {
    level: u8,
}

async fn items_handler(
    Query(q): Query<ItemsQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let jobs = state.item_data.jobs_at_level(q.level);
    Json(ItemsResponse { jobs }).into_response()
}

async fn datacenters_handler() -> impl IntoResponse {
    let dcs: Vec<DataCenter> = DATACENTERS
        .iter()
        .map(|dc| DataCenter { name: dc.to_string() })
        .collect();
    Json(DataCentersResponse { datacenters: dcs }).into_response()
}
