use std::collections::HashMap;

use gloo_net::http::Request;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;

use shared::{ItemsResponse, Listing, PricesResponse};

// Backend URL — override at build time with BACKEND_URL env var.
// During dev, Trunk proxies /api → localhost:8080/api (same origin = "").
const BACKEND_URL: &str = match option_env!("BACKEND_URL") {
    Some(s) => s,
    None => "",
};

const DATACENTERS: &[&str] = &[
    "Aether", "Crystal", "Dynamis", "Primal",
    "Chaos", "Light",
    "Elemental", "Gaia", "Mana", "Meteor",
    "Materia",
];

const ALL_JOBS: &[(u8, &str)] = &[
    (8, "CRP"), (9, "BSM"), (10, "ARM"), (11, "GSM"),
    (12, "LTW"), (13, "WVR"), (14, "ALC"), (15, "CUL"),
    (16, "MIN"), (17, "BTN"), (18, "FSH"),
];

// ── Storage ────────────────────────────────────────────────────────────────

fn storage_get(key: &str) -> Option<String> {
    window()?.local_storage().ok()??.get_item(key).ok()?
}

fn storage_set(key: &str, value: &str) {
    if let Some(Ok(Some(s))) = window().map(|w| w.local_storage()) {
        let _ = s.set_item(key, value);
    }
}

// ── Rotation ───────────────────────────────────────────────────────────────


// ── Helpers ────────────────────────────────────────────────────────────────

fn cheapest_hq<'a>(listings: &'a [Listing], required: u32) -> Option<&'a Listing> {
    listings
        .iter()
        .filter(|l| l.hq && l.quantity >= required)
        .min_by_key(|l| l.price_per_unit * l.quantity)
}

fn cheapest_nq<'a>(listings: &'a [Listing], required: u32) -> Option<&'a Listing> {
    listings
        .iter()
        .filter(|l| !l.hq && l.quantity >= required)
        .min_by_key(|l| l.price_per_unit * l.quantity)
}

fn format_gil(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

// ── App ────────────────────────────────────────────────────────────────────

#[component]
fn App() -> impl IntoView {
    let init_dc = storage_get("dc").unwrap_or_else(|| "Aether".to_string());
    let init_level: u8 = storage_get("level").and_then(|s| s.parse().ok()).unwrap_or(100);
    let init_job: u8 = storage_get("job").and_then(|s| s.parse().ok()).unwrap_or(8);

    let (dc, set_dc) = signal(init_dc);
    let (level, set_level) = signal(init_level);
    let (selected_job, set_selected_job) = signal(init_job);

    // Async data via spawn_local + signals (avoids Send requirement on gloo-net futures)
    let (items_data, set_items_data) = signal(None::<ItemsResponse>);
    let (prices_data, set_prices_data) = signal(None::<PricesResponse>);

    // Re-fetch items when level changes
    Effect::new(move |_| {
        let lvl = level.get();
        let url = format!("{BACKEND_URL}/api/items?level={lvl}");
        spawn_local(async move {
            if let Ok(resp) = Request::get(&url).send().await {
                if let Ok(data) = resp.json::<ItemsResponse>().await {
                    set_items_data.set(Some(data));
                }
            }
        });
    });

    // Re-fetch prices when DC changes
    Effect::new(move |_| {
        let dc_val = dc.get();
        let url = format!("{BACKEND_URL}/api/prices?dc={dc_val}");
        spawn_local(async move {
            if let Ok(resp) = Request::get(&url).send().await {
                if let Ok(data) = resp.json::<PricesResponse>().await {
                    set_prices_data.set(Some(data));
                }
            }
        });
    });

    view! {
        <header>
            <h1>"turnins.fyi"</h1>
            <span>"FFXIV Grand Company Supply & Provisioning — Cheapest Market Listings"</span>
        </header>

        <div class="controls">
            <label>
                "Datacenter "
                <select on:change=move |e| {
                    let val = event_target_value(&e);
                    storage_set("dc", &val);
                    set_dc.set(val);
                }>
                    {DATACENTERS.iter().map(|&name| {
                        let selected = move || name == dc.get().as_str();
                        view! { <option selected=selected>{name}</option> }
                    }).collect_view()}
                </select>
            </label>
            <label>
                "Class Level "
                <input
                    type="number" min="1" max="100"
                    prop:value=move || level.get().to_string()
                    on:change=move |e| {
                        if let Ok(n) = event_target_value(&e).parse::<u8>() {
                            let v = n.clamp(1, 100);
                            storage_set("level", &v.to_string());
                            set_level.set(v);
                        }
                    }
                />
            </label>
        </div>

        <div class="job-tabs">
            {ALL_JOBS.iter().map(|&(cj_id, abbr)| {
                view! {
                    <button
                        class=move || if selected_job.get() == cj_id { "active" } else { "" }
                        on:click=move |_| {
                            storage_set("job", &cj_id.to_string());
                            set_selected_job.set(cj_id);
                        }
                    >
                        {abbr}
                    </button>
                }
            }).collect_view()}
        </div>

        <div class="main">
            {move || {
                let listings_map: HashMap<u32, Vec<Listing>> = prices_data.get()
                    .map(|p| p.listings)
                    .unwrap_or_default();

                let last_updated = prices_data.get().and_then(|p| p.last_updated);

                let job_items: Vec<shared::TurnInItem> = items_data.get()
                    .and_then(|r| r.jobs.into_iter().find(|j| j.class_job_id == selected_job.get()))
                    .map(|j| j.items)
                    .unwrap_or_default();

                view! {
                    <Show when=move || last_updated.is_some()>
                        {move || {
                            let ts = last_updated.unwrap_or(0);
                            let ago_min = ((js_sys::Date::now() / 1000.0) as u64).saturating_sub(ts) / 60;
                            let msg = if ago_min == 0 { "Prices updated just now".to_string() }
                                      else { format!("Prices last updated {ago_min} min ago") };
                            view! { <div class="stale-notice">{msg}</div> }
                        }}
                    </Show>

                    {if listings_map.is_empty() {
                        view! {
                            <div class="stale-notice">
                                "Fetching prices from Universalis — check back in a moment."
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}

                    <div class="items-grid">
                        {job_items.into_iter().map(|item| {
                            let listings = listings_map.get(&item.item_id).cloned().unwrap_or_default();
                            let hq_best = cheapest_hq(&listings, item.count).cloned();
                            let nq_best = cheapest_nq(&listings, item.count).cloned();

                            view! {
                                <div class="item-card">
                                    <div class="item-name">
                                        {item.item_name.clone()}
                                    </div>
                                    <div class="item-meta">"Requires: "{item.count}" × "{item.item_name.clone()}</div>
                                    <hr class="separator"/>
                                    {match (hq_best, nq_best) {
                                        (Some(hq), nq) => {
                                            let nq_line = nq.map(|n| {
                                                let nq_price = format_gil(n.price_per_unit);
                                                let nq_world = n.world_name.clone();
                                                view! {
                                                    <div class="nq-alt">
                                                        "NQ: "{nq_price}" gil each · "{nq_world}
                                                    </div>
                                                }
                                            });
                                            view! {
                                                <div class="price-row">
                                                    <div>
                                                        <div class="price">
                                                            {format_gil(hq.price_per_unit)}" gil each"
                                                            <span class="hq-badge">"HQ"</span>
                                                        </div>
                                                        <div class="price-detail">
                                                            "Stack: "{hq.quantity}
                                                            " · Total: "{format_gil(hq.price_per_unit * hq.quantity)}" gil"
                                                        </div>
                                                        {nq_line}
                                                    </div>
                                                    <div class="world">{hq.world_name}</div>
                                                </div>
                                            }.into_any()
                                        },
                                        (None, Some(nq)) => view! {
                                            <div class="price-row">
                                                <div>
                                                    <div class="price nq-price">
                                                        {format_gil(nq.price_per_unit)}" gil each"
                                                        <span class="nq-badge">"NQ"</span>
                                                    </div>
                                                    <div class="price-detail">
                                                        "Stack: "{nq.quantity}
                                                        " · Total: "{format_gil(nq.price_per_unit * nq.quantity)}" gil"
                                                    </div>
                                                    <div class="nq-note">"No HQ listings available"</div>
                                                </div>
                                                <div class="world">{nq.world_name}</div>
                                            </div>
                                        }.into_any(),
                                        (None, None) => view! {
                                            <div class="no-listing">
                                                {if listings.is_empty() { "No price data yet" }
                                                 else { "No single stack with enough quantity" }}
                                            </div>
                                        }.into_any(),
                                    }}
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }
            }}
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
