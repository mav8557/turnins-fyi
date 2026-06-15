use std::{collections::HashMap, sync::Arc};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tracing::{error, info};

/// One market board listing returned by Universalis.
#[derive(Debug, Clone)]
pub struct Listing {
    pub price_per_unit: u32,
    pub quantity: u32,
    pub world_name: String,
    pub hq: bool,
}

/// Fetches market listings from Universalis for all given item IDs in a datacenter.
/// `sem` is a shared semaphore (8 permits) that spans all concurrent DC fetches.
/// Returns a map of item_id → listings sorted by price_per_unit ascending.
pub async fn fetch_all(
    client: &Client,
    sem: &Arc<Semaphore>,
    dc: &str,
    item_ids: &[u32],
) -> HashMap<u32, Vec<Listing>> {
    let chunks: Vec<Vec<u32>> = item_ids.chunks(100).map(|c| c.to_vec()).collect();

    let mut handles = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let permit = sem.clone().acquire_owned().await.expect("semaphore closed");
        let client = client.clone();
        let dc = dc.to_string();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            fetch_chunk(&client, &dc, &chunk).await
        }));
    }

    let mut result: HashMap<u32, Vec<Listing>> = HashMap::new();
    for handle in handles {
        match handle.await {
            Ok(chunk_map) => result.extend(chunk_map),
            Err(e) => error!("Universalis task panicked: {e}"),
        }
    }
    result
}

async fn fetch_chunk(
    client: &Client,
    dc: &str,
    item_ids: &[u32],
) -> HashMap<u32, Vec<Listing>> {
    let ids_str: String = item_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // listings=20 caps memory per item; entries=0 skips sale history.
    let url = format!(
        "https://universalis.app/api/v2/{dc}/{ids_str}?listings=20&entries=0"
    );

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Universalis request failed: {e}");
            return HashMap::new();
        }
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to parse Universalis response: {e}");
            return HashMap::new();
        }
    };

    // Multi-item: { items: { "id": { itemID, listings: [...] } } }
    // Single-item: { itemID: N, listings: [...] }
    let mut map: HashMap<u32, Vec<Listing>> = HashMap::new();

    if let Some(items_obj) = body.get("items").and_then(|v| v.as_object()) {
        for (_, item_val) in items_obj {
            if let Ok(item) = serde_json::from_value::<UniversalisItem>(item_val.clone()) {
                map.insert(item.item_id, parse_listings(item.listings));
            }
        }
    } else if let Ok(item) = serde_json::from_value::<UniversalisItem>(body) {
        map.insert(item.item_id, parse_listings(item.listings));
    }

    info!("Fetched {} items for DC {dc}", map.len());
    map
}

fn parse_listings(raw: Vec<UniversalisListing>) -> Vec<Listing> {
    let mut listings: Vec<Listing> = raw
        .into_iter()
        .map(|l| Listing {
            price_per_unit: l.price_per_unit,
            quantity: l.quantity,
            world_name: l.world_name,
            hq: l.hq,
        })
        .collect();
    listings.sort_by_key(|l| l.price_per_unit);
    listings
}

// ── Universalis response types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct UniversalisItem {
    #[serde(rename = "itemID")]
    item_id: u32,
    #[serde(default)]
    listings: Vec<UniversalisListing>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniversalisListing {
    price_per_unit: u32,
    quantity: u32,
    world_name: String,
    hq: bool,
}
