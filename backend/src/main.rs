mod item_data;
mod pricing;
mod universalis;

use std::{
    collections::HashMap,
    sync::Arc,
};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use tokio::sync::{RwLock, Semaphore};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use item_data::ItemData;
use shared::{DataCenter, DataCentersResponse, PricesRequest, Region};

// ── Constants ──────────────────────────────────────────────────────────────

const ITEMS_JSON: &[u8] = include_bytes!("../../data/items.json");

const DATACENTERS: &[(&str, Region)] = &[
    ("Elemental", Region::JP),
    ("Gaia", Region::JP),
    ("Mana", Region::JP),
    ("Meteor", Region::JP),
    ("Aether", Region::NA),
    ("Crystal", Region::NA),
    ("Dynamis", Region::NA),
    ("Primal", Region::NA),
    ("Chaos", Region::EU),
    ("Light", Region::EU),
    ("Materia", Region::OCE),
];

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub(crate) struct PriceCache {
    pub(crate) last_fetched_unix: Option<u64>,
    pub(crate) fetching: bool,
    pub(crate) data: HashMap<u32, Vec<universalis::Listing>>,
}

pub(crate) struct AppState {
    pub(crate) item_data: ItemData,
    /// One cache entry per DC name (lowercase).
    pub(crate) caches: HashMap<String, Arc<RwLock<PriceCache>>>,
    pub(crate) http: Client,
    /// Shared across all concurrent DC fetches to respect Universalis's 8-request limit.
    pub(crate) universalis_sem: Arc<Semaphore>,
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
        .map(|(dc, _)| (dc.to_lowercase(), Arc::new(RwLock::new(PriceCache::default()))))
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
        .route("/api/prices", post(prices_handler))
        .route("/api/datacenters", get(datacenters_handler))
        .route("/health", get(|| async { "ok" }))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// ── Handlers ───────────────────────────────────────────────────────────────

async fn prices_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PricesRequest>,
) -> impl IntoResponse {
    // Validate datacenters
    let mut requested_dcs: Vec<(String, String)> = Vec::with_capacity(req.datacenters.len());
    for dc in &req.datacenters {
        let key = dc.to_lowercase();
        if !state.caches.contains_key(&key) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Unknown datacenter: {dc}")})),
            )
                .into_response();
        }
        requested_dcs.push((key, dc.clone()));
    }

    // Build and return response
    let response = pricing::build_response(&state, &req, &requested_dcs).await;
    Json(response).into_response()
}

async fn datacenters_handler() -> impl IntoResponse {
    let dcs: Vec<DataCenter> = DATACENTERS
        .iter()
        .map(|(name, region)| DataCenter {
            name: name.to_string(),
            region: *region,
        })
        .collect();
    Json(DataCentersResponse { datacenters: dcs }).into_response()
}
