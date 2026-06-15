use std::collections::HashMap;

use serde::Deserialize;

use shared::TurnInItem;

/// Mirrors the structure of data/items.json.
#[derive(Debug, Deserialize)]
struct RawItems {
    item_names: HashMap<String, String>,
    jobs: Vec<RawJob>,
    levels: HashMap<String, HashMap<String, Vec<RawTurnIn>>>,
}

#[derive(Debug, Deserialize)]
struct RawJob {
    class_job_id: u8,
    name: String,
    abbr: String,
}

#[derive(Debug, Deserialize)]
struct RawTurnIn {
    item_id: u32,
    count: u32,
}

/// Parsed and indexed supply duty data.
pub struct ItemData {
    /// job_id → job metadata
    pub jobs: HashMap<u8, JobMeta>,
    /// (level, job_id) → pool of items
    pub pool: HashMap<(u8, u8), Vec<TurnInItem>>,
    /// All unique item IDs referenced across all levels.
    pub all_item_ids: Vec<u32>,
}

pub struct JobMeta {
    pub name: String,
    pub abbr: String,
}

impl ItemData {
    pub fn load(json_bytes: &[u8]) -> anyhow::Result<Self> {
        let raw: RawItems = serde_json::from_slice(json_bytes)?;

        let jobs: HashMap<u8, JobMeta> = raw
            .jobs
            .into_iter()
            .map(|j| {
                (
                    j.class_job_id,
                    JobMeta {
                        name: j.name,
                        abbr: j.abbr,
                    },
                )
            })
            .collect();

        let mut pool: HashMap<(u8, u8), Vec<TurnInItem>> = HashMap::new();
        let mut all_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for (level_str, classes) in &raw.levels {
            let level: u8 = level_str.parse()?;
            for (cj_str, items) in classes {
                let cj_id: u8 = cj_str.parse()?;
                let parsed: Vec<TurnInItem> = items
                    .iter()
                    .map(|i| {
                        all_ids.insert(i.item_id);
                        TurnInItem {
                            item_id: i.item_id,
                            item_name: raw
                                .item_names
                                .get(&i.item_id.to_string())
                                .cloned()
                                .unwrap_or_else(|| format!("Item #{}", i.item_id)),
                            count: i.count,
                        }
                    })
                    .collect();
                pool.insert((level, cj_id), parsed);
            }
        }

        let mut all_item_ids: Vec<u32> = all_ids.into_iter().collect();
        all_item_ids.sort();

        Ok(ItemData {
            jobs,
            pool,
            all_item_ids,
        })
    }

    /// Returns the turn-in pool for a given (level, job) as a slice.
    pub fn pool_for(&self, class_job_id: u8, level: u8) -> &[TurnInItem] {
        self.pool
            .get(&(level, class_job_id))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// For each of the 11 jobs, pairs job metadata with the pool for a given (clamped) level.
    pub fn job_pools<'a>(&'a self, levels: &[(u8, u8); 11]) -> Vec<(u8, &'a JobMeta, u8, &'a [TurnInItem])> {
        levels
            .iter()
            .map(|&(cj_id, level)| {
                let level = level.clamp(1, 100);
                (cj_id, &self.jobs[&cj_id], level, self.pool_for(cj_id, level))
            })
            .collect()
    }
}
