use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One market board listing returned by Universalis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub price_per_unit: u32,
    pub quantity: u32,
    pub world_name: String,
    pub hq: bool,
}

/// Response from GET /api/prices?dc={dc}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricesResponse {
    /// Unix timestamp (seconds) of the last successful Universalis fetch, if any.
    pub last_updated: Option<u64>,
    /// item_id → listings sorted by price_per_unit ascending.
    pub listings: HashMap<u32, Vec<Listing>>,
}

/// One supply turn-in item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnInItem {
    pub item_id: u32,
    pub item_name: String,
    pub count: u32,
}

/// One job's turn-in pool at a specific level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLevel {
    pub class_job_id: u8,
    pub name: String,
    pub abbr: String,
    pub items: Vec<TurnInItem>,
}

/// Response from GET /api/items?level={level}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemsResponse {
    pub jobs: Vec<JobLevel>,
}

/// Info about an available datacenter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCenter {
    pub name: String,
}

/// Response from GET /api/datacenters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCentersResponse {
    pub datacenters: Vec<DataCenter>,
}
