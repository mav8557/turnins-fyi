use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ── Request ────────────────────────────────────────────────────────────────

/// POST body for /api/prices. Field names match in-game job abbreviations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PricesRequest {
    pub crp: u8,
    pub bsm: u8,
    pub arm: u8,
    pub gsm: u8,
    pub ltw: u8,
    pub wvr: u8,
    pub alc: u8,
    pub cul: u8,
    pub min: u8,
    pub btn: u8,
    pub fsh: u8,
    #[serde(rename = "datacenters")]
    pub datacenters: Vec<String>,
}

impl PricesRequest {
    /// Returns (class_job_id, level) pairs for all 11 jobs, ordered 8..=18.
    pub fn job_levels(&self) -> [(u8, u8); 11] {
        [
            (8, self.crp),
            (9, self.bsm),
            (10, self.arm),
            (11, self.gsm),
            (12, self.ltw),
            (13, self.wvr),
            (14, self.alc),
            (15, self.cul),
            (16, self.min),
            (17, self.btn),
            (18, self.fsh),
        ]
    }
}

// ── Response types ─────────────────────────────────────────────────────────

/// One market board listing after aggregation, with its source datacenter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestListing {
    pub price_per_unit: u32,
    pub quantity: u32,
    pub world_name: String,
    pub datacenter: String,
}

/// One turn-in item with its best HQ/NQ listings resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricedItem {
    pub item_id: u32,
    pub item_name: String,
    pub count: u32,
    pub hq_best: Option<BestListing>,
    pub nq_best: Option<BestListing>,
}

/// One job's turn-in pool at a requested level, with prices resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub class_job_id: u8,
    pub name: String,
    pub abbr: String,
    pub level: u8,
    pub items: Vec<PricedItem>,
}

/// Per-datacenter cache status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcStatus {
    pub last_updated: Option<u64>,
    pub fetching: bool,
}

/// Response from POST /api/prices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricesResponse {
    pub jobs: Vec<JobResult>,
    /// Keyed by datacenter name.
    pub dc_status: HashMap<String, DcStatus>,
}

// ── Item data (kept for backend internal use) ──────────────────────────────

/// One supply turn-in item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnInItem {
    pub item_id: u32,
    pub item_name: String,
    pub count: u32,
}

// ── Datacenters ────────────────────────────────────────────────────────────

/// Available datacenter regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    NA,
    EU,
    JP,
    OCE,
}

impl Region {
    pub fn label(&self) -> &'static str {
        match self {
            Region::NA => "North America",
            Region::EU => "Europe",
            Region::JP => "Japan",
            Region::OCE => "Oceania",
        }
    }
}

/// Info about an available datacenter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCenter {
    pub name: String,
    pub region: Region,
}

/// Response from GET /api/datacenters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCentersResponse {
    pub datacenters: Vec<DataCenter>,
}
