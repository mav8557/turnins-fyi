use std::collections::HashMap;

use serde::Deserialize;

use shared::{JobLevel, TurnInItem};

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

    /// Returns the turn-in pool for a given (level, job).
    pub fn items_for(&self, level: u8, class_job_id: u8) -> Option<&Vec<TurnInItem>> {
        self.pool.get(&(level, class_job_id))
    }

    /// Builds a JobLevel list for a given level, one entry per DoH/DoL job.
    pub fn jobs_at_level(&self, level: u8) -> Vec<JobLevel> {
        let mut result = Vec::new();
        // Jobs sorted by class_job_id ascending (8=CRP … 18=FSH)
        let mut job_ids: Vec<u8> = self.jobs.keys().copied().collect();
        job_ids.sort();
        for cj_id in job_ids {
            let meta = &self.jobs[&cj_id];
            let items = self
                .pool
                .get(&(level, cj_id))
                .cloned()
                .unwrap_or_default();
            result.push(JobLevel {
                class_job_id: cj_id,
                name: meta.name.clone(),
                abbr: meta.abbr.clone(),
                items,
            });
        }
        result
    }
}
