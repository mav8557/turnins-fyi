use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use tokio::sync::{RwLock, Semaphore};
use tracing::info;

use crate::universalis;
use shared::{BestListing, DcStatus, JobResult, PricesRequest, PricesResponse, PricedItem};

/// Reads a snapshot of cached listings for just `needed_item_ids`, tagged with
/// their datacenter. Returns item_id -> Vec<(dc_name, Listing)>.
pub async fn snapshot_listings(
    caches: &HashMap<String, Arc<RwLock<crate::PriceCache>>>,
    needed_item_ids: &HashSet<u32>,
    requested_dcs: &[(String, String)], // (lowercase_key, canonical_name)
) -> HashMap<u32, Vec<(String, universalis::Listing)>> {
    let mut out: HashMap<u32, Vec<(String, universalis::Listing)>> = HashMap::new();
    for (dc_key, canonical_dc) in requested_dcs {
        if let Some(cache_lock) = caches.get(dc_key) {
            let cache = cache_lock.read().await;
            for &item_id in needed_item_ids {
                if let Some(listings) = cache.data.get(&item_id) {
                    out.entry(item_id)
                        .or_default()
                        .extend(listings.iter().cloned().map(|l| (canonical_dc.clone(), l)));
                }
            }
        }
    }
    out
}

/// Computes the cheapest HQ and NQ listing (by price_per_unit * quantity)
/// that satisfies the required count, across the provided (dc, Listing) pairs.
pub fn best_listings(
    listings: &[(String, universalis::Listing)],
    count: u32,
) -> (Option<BestListing>, Option<BestListing>) {
    let hq_best = listings
        .iter()
        .filter(|(_, l)| l.hq && l.quantity >= count)
        .min_by_key(|(_, l)| l.price_per_unit * l.quantity)
        .map(|(dc, l)| BestListing {
            price_per_unit: l.price_per_unit,
            quantity: l.quantity,
            world_name: l.world_name.clone(),
            datacenter: dc.clone(),
        });
    let nq_best = listings
        .iter()
        .filter(|(_, l)| !l.hq && l.quantity >= count)
        .min_by_key(|(_, l)| l.price_per_unit * l.quantity)
        .map(|(dc, l)| BestListing {
            price_per_unit: l.price_per_unit,
            quantity: l.quantity,
            world_name: l.world_name.clone(),
            datacenter: dc.clone(),
        });
    (hq_best, nq_best)
}

// ── Background refresh ───────────────────────────────────────────────────

const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15 * 60;

/// How often each datacenter's cache is refreshed in the background.
/// Configurable via the `REFRESH_INTERVAL_SECS` env var.
pub(crate) fn refresh_interval() -> Duration {
    std::env::var("REFRESH_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_REFRESH_INTERVAL_SECS))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Fetches fresh listings for `dc` and merges them into the cache. Items
/// missing from the new data (e.g. a failed chunk) keep their previously
/// cached listings instead of being wiped.
pub(crate) async fn refresh_dc(
    http: &Client,
    sem: &Arc<Semaphore>,
    item_ids: &[u32],
    cache_lock: &Arc<RwLock<crate::PriceCache>>,
    dc: &str,
) {
    {
        cache_lock.write().await.fetching = true;
    }
    info!("Starting Universalis refresh for DC {dc}");
    let new_data = universalis::fetch_all(http, sem, dc, item_ids).await;
    let mut cache = cache_lock.write().await;
    cache.data.extend(new_data);
    cache.last_fetched_unix = Some(now_unix());
    cache.fetching = false;
    info!("Universalis refresh complete for DC {dc}");
}

/// Orchestrates the full response: job pools -> needed items -> bootstrap
/// fetch if needed -> snapshot -> best listings per item -> response with
/// dc_status.
pub async fn build_response(
    state: &crate::AppState,
    req: &PricesRequest,
    requested_dcs: &[(String, String)], // (lowercase_key, canonical_name)
) -> PricesResponse {
    // Get job pools with clamped levels
    let job_pools = state.item_data.job_pools(&req.job_levels());

    // Collect unique item IDs needed across all job pools
    let mut needed_item_ids: HashSet<u32> = HashSet::new();
    for (_, _, _, pool) in &job_pools {
        for item in *pool {
            needed_item_ids.insert(item.item_id);
        }
    }

    // Bootstrap: if a requested DC has never been fetched yet (e.g. right
    // after a cold start), kick off an immediate fetch instead of waiting
    // for its staggered slot in the periodic refresh loop.
    for (dc_key, canonical_dc) in requested_dcs {
        if let Some(cache_lock) = state.caches.get(dc_key) {
            let needs_refresh = {
                let cache = cache_lock.read().await;
                !cache.fetching && cache.last_fetched_unix.is_none()
            };
            if needs_refresh {
                cache_lock.write().await.fetching = true;
                let http = state.http.clone();
                let sem = Arc::clone(&state.universalis_sem);
                let ids = state.item_data.all_item_ids.clone();
                let cache_lock = Arc::clone(cache_lock);
                let dc = canonical_dc.clone();
                tokio::spawn(async move {
                    refresh_dc(&http, &sem, &ids, &cache_lock, &dc).await;
                });
            }
        }
    }

    // Snapshot listings for needed items across requested DCs
    let listings_snapshot = snapshot_listings(&state.caches, &needed_item_ids, requested_dcs).await;

    // Build JobResult for each job
    let jobs: Vec<JobResult> = job_pools
        .into_iter()
        .map(|(cj_id, meta, level, pool)| {
            let items: Vec<PricedItem> = pool
                .iter()
                .map(|item| {
                    let listings = listings_snapshot
                        .get(&item.item_id)
                        .map(|l| l.as_slice())
                        .unwrap_or(&[]);
                    let (hq_best, nq_best) = best_listings(listings, item.count);
                    PricedItem {
                        item_id: item.item_id,
                        item_name: item.item_name.clone(),
                        count: item.count,
                        hq_best,
                        nq_best,
                    }
                })
                .collect();
            JobResult {
                class_job_id: cj_id,
                name: meta.name.clone(),
                abbr: meta.abbr.clone(),
                level,
                items,
            }
        })
        .collect();

    // Build dc_status from each requested DC's cache
    let mut dc_status: HashMap<String, DcStatus> = HashMap::new();
    for (_, canonical_dc) in requested_dcs {
        let status = if let Some(cache_lock) = state.caches.get(&canonical_dc.to_lowercase()) {
            let cache = cache_lock.read().await;
            DcStatus {
                last_updated: cache.last_fetched_unix,
                fetching: cache.fetching,
            }
        } else {
            DcStatus {
                last_updated: None,
                fetching: false,
            }
        };
        dc_status.insert(canonical_dc.clone(), status);
    }

    PricesResponse { jobs, dc_status }
}
