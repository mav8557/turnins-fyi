use std::collections::HashMap;
use web_sys::window;

pub fn storage_get(key: &str) -> Option<String> {
    window()?
        .local_storage()
        .ok()?
        .and_then(|s| s.get_item(key).ok())?
}

pub fn storage_set(key: &str, value: &str) {
    if let Some(Ok(Some(s))) = window().map(|w| w.local_storage()) {
        let _ = s.set_item(key, value);
    }
}

const LEVELS_KEY: &str = "job_levels";
const DCS_KEY: &str = "datacenters";
const MY_LIST_KEY: &str = "my_list";

const DEFAULT_LEVEL: u8 = 1;

/// class_job_id -> level, defaulting all 11 jobs to DEFAULT_LEVEL if not present.
pub fn load_levels() -> HashMap<u8, u8> {
    storage_get(LEVELS_KEY)
        .and_then(|s| serde_json::from_str::<HashMap<u8, u8>>(&s).ok())
        .map(|mut m| {
            for &(cj_id, _) in crate::ALL_JOBS {
                m.entry(cj_id).or_insert(DEFAULT_LEVEL);
            }
            m
        })
        .unwrap_or_else(|| {
            crate::ALL_JOBS
                .iter()
                .map(|&(cj_id, _)| (cj_id, DEFAULT_LEVEL))
                .collect()
        })
}

pub fn save_levels(levels: &HashMap<u8, u8>) {
    if let Ok(s) = serde_json::to_string(levels) {
        storage_set(LEVELS_KEY, &s);
    }
}

/// Selected DC names.
pub fn load_dcs(default: &[String]) -> Vec<String> {
    storage_get(DCS_KEY)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_else(|| default.to_vec())
}

pub fn save_dcs(dcs: &[String]) {
    if let Ok(s) = serde_json::to_string(dcs) {
        storage_set(DCS_KEY, &s);
    }
}

/// My List item ids.
pub fn load_my_list() -> Vec<u32> {
    storage_get(MY_LIST_KEY)
        .and_then(|s| serde_json::from_str::<Vec<u32>>(&s).ok())
        .unwrap_or_default()
}

pub fn save_my_list(ids: &[u32]) {
    if let Ok(s) = serde_json::to_string(ids) {
        storage_set(MY_LIST_KEY, &s);
    }
}
