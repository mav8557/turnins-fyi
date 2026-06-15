mod rotation;
mod storage;

use std::collections::HashSet;

use gloo_net::http::Request;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use shared::{DataCentersResponse, PricesRequest, PricesResponse};

const BACKEND_URL: &str = match option_env!("BACKEND_URL") {
    Some(s) => s,
    None => "",
};

pub const ALL_JOBS: &[(u8, &str)] = &[
    (8, "CRP"),
    (9, "BSM"),
    (10, "ARM"),
    (11, "GSM"),
    (12, "LTW"),
    (13, "WVR"),
    (14, "ALC"),
    (15, "CUL"),
    (16, "MIN"),
    (17, "BTN"),
    (18, "FSH"),
];

const DEFAULT_DATACENTERS: &[&str] = &[
    "Aether", "Crystal", "Dynamis", "Primal", "Chaos", "Light", "Elemental", "Gaia", "Mana",
    "Meteor", "Materia",
];

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

#[component]
fn App() -> impl IntoView {
    let init_levels = storage::load_levels();
    let (levels, set_levels) = signal(init_levels);

    let default_dcs: Vec<String> = DEFAULT_DATACENTERS.iter().map(|s| s.to_string()).collect();
    let init_dcs = storage::load_dcs(&default_dcs);
    let (selected_dcs, set_selected_dcs) = signal::<Vec<String>>(init_dcs);

    let init_my_list = storage::load_my_list();
    let (my_list, set_my_list) = signal::<HashSet<u32>>(init_my_list.into_iter().collect());

    let (datacenters_data, set_datacenters_data) = signal(None::<DataCentersResponse>);
    let (prices_data, set_prices_data) = signal(None::<PricesResponse>);

    // Fetch /api/datacenters once on mount
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(resp) = Request::get(&format!("{BACKEND_URL}/api/datacenters"))
                .send()
                .await
            {
                if let Ok(data) = resp.json::<DataCentersResponse>().await {
                    set_datacenters_data.set(Some(data));
                }
            }
        });
    });

    // POST /api/prices whenever levels or selected_dcs change
    Effect::new(move |_| {
        let lvls = levels.get();
        let dcs = selected_dcs.get();

        let req = PricesRequest {
            crp: lvls.get(&8).copied().unwrap_or(100),
            bsm: lvls.get(&9).copied().unwrap_or(100),
            arm: lvls.get(&10).copied().unwrap_or(100),
            gsm: lvls.get(&11).copied().unwrap_or(100),
            ltw: lvls.get(&12).copied().unwrap_or(100),
            wvr: lvls.get(&13).copied().unwrap_or(100),
            alc: lvls.get(&14).copied().unwrap_or(100),
            cul: lvls.get(&15).copied().unwrap_or(100),
            min: lvls.get(&16).copied().unwrap_or(100),
            btn: lvls.get(&17).copied().unwrap_or(100),
            fsh: lvls.get(&18).copied().unwrap_or(100),
            datacenters: dcs,
        };

        spawn_local(async move {
            let request = Request::post(&format!("{BACKEND_URL}/api/prices"));
            let request = match request.json(&req) {
                Ok(r) => r,
                Err(_) => return,
            };
            match request.send().await {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<PricesResponse>().await {
                        set_prices_data.set(Some(data));
                    }
                }
                Err(_) => {}
            }
        });
    });

    // Helper closures for level mutation
    let set_level = move |cj_id: u8, new_level: u8| {
        set_levels.update(|m| {
            m.insert(cj_id, new_level);
        });
        storage::save_levels(&levels.get_untracked());
    };

    let adjust_level = move |cj_id: u8, delta: i8| {
        let cur = levels
            .get_untracked()
            .get(&cj_id)
            .copied()
            .unwrap_or(100) as i16;
        let new = (cur + delta as i16).clamp(1, 100) as u8;
        set_level(cj_id, new);
    };

    let now_unix = (js_sys::Date::now() / 1000.0) as i64;

    view! {
        <header>
            <h1>"turnins.fyi"</h1>
            <span>"FFXIV Grand Company Supply & Provisioning — Cheapest Market Listings"</span>
        </header>

        <div class="dc-select">
                {move || {
                    let dcs = datacenters_data
                        .get()
                        .map(|d| d.datacenters)
                        .unwrap_or_default();

                    let mut by_region: std::collections::BTreeMap<
                        String,
                        Vec<String>,
                    > = std::collections::BTreeMap::new();
                    for dc in dcs {
                        by_region
                            .entry(dc.region.label().to_string())
                            .or_default()
                            .push(dc.name);
                    }

                    by_region
                        .into_iter()
                        .map(|(region_label, names)| {
                            view! {
                                <div class="dc-region-group">
                                    <div class="dc-region-label">{region_label}</div>
                                    {names
                                        .into_iter()
                                        .map(|dc_name_owned| {
                                            let dc_name_for_checked = dc_name_owned.clone();
                                            let dc_name_for_change = dc_name_owned.clone();
                                            let dc_name_for_display = dc_name_owned.clone();
                                            view! {
                                                <label class="dc-checkbox">
                                                    <input
                                                        type="checkbox"
                                                        checked=move || selected_dcs.get().contains(&dc_name_for_checked)
                                                        on:change=move |e| {
                                                            let is_checked = event_target_checked(&e);
                                                            set_selected_dcs.update(|dcs| {
                                                                if is_checked {
                                                                    if !dcs.contains(&dc_name_for_change) {
                                                                        dcs.push(dc_name_for_change.clone());
                                                                    }
                                                                } else {
                                                                    dcs.retain(|d| d != &dc_name_for_change);
                                                                }
                                                            });
                                                            storage::save_dcs(
                                                                &selected_dcs.get_untracked(),
                                                            );
                                                        }
                                                    />
                                                    {dc_name_for_display}
                                                </label>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                        })
                        .collect_view()
                }}
        </div>

        <div class="my-list-panel">
            <div class="my-list-header">
                <h2>"My List"</h2>
                <button
                    on:click=move |_| {
                        set_my_list.set(HashSet::new());
                        storage::save_my_list(&[]);
                    }
                >
                    "Clear all"
                </button>
            </div>
            {move || {
                let selected_ids = my_list.get();
                if selected_ids.is_empty() {
                    view! {
                        <div class="my-list-empty">
                            "Click items below to add them here."
                        </div>
                    }
                    .into_any()
                } else {
                    let rows: Vec<_> = prices_data
                        .get()
                        .map(|p| {
                            p.jobs
                                .iter()
                                .flat_map(|j| {
                                    j.items
                                        .iter()
                                        .filter(|i| selected_ids.contains(&i.item_id))
                                        .map(move |i| {
                                            let best = i.hq_best.as_ref().or(i.nq_best.as_ref());
                                            (
                                                i.item_name.clone(),
                                                j.abbr.clone(),
                                                best.cloned(),
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    view! {
                        <div class="my-list-items">
                            {rows
                                .into_iter()
                                .map(|(name, abbr, best)| {
                                    view! {
                                        <div class="my-list-row">
                                            <span class="my-list-item-name">{name}</span>
                                            <span class="my-list-job">{abbr}</span>
                                            {match best {
                                                Some(b) => view! {
                                                    <span class="my-list-world">
                                                        {b.world_name.clone()}
                                                        " · "
                                                        {b.datacenter.clone()}
                                                    </span>
                                                    <span class="my-list-price">
                                                        {format_gil(b.price_per_unit)}
                                                        " gil"
                                                    </span>
                                                }
                                                .into_any(),
                                                None => view! {
                                                    <span class="my-list-world">"No listing"</span>
                                                }
                                                .into_any(),
                                            }}
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                    .into_any()
                }
            }}
        </div>

        <div class="main">
            {move || {
                let prices = prices_data.get();
                let last_updated = prices.as_ref().and_then(|p| {
                    p.dc_status.values().find_map(|s| s.last_updated)
                });

                let show_stale = prices.as_ref().map(|p| {
                    p.dc_status
                        .values()
                        .any(|s| s.last_updated.is_none() || s.fetching)
                });

                view! {
                    <Show when=move || last_updated.is_some()>
                        {move || {
                            let ts = last_updated.unwrap_or(0);
                            let ago_min = ((js_sys::Date::now() / 1000.0) as u64)
                                .saturating_sub(ts)
                                / 60;
                            let msg = if ago_min == 0 {
                                "Prices updated just now".to_string()
                            } else {
                                format!("Prices last updated {ago_min} min ago")
                            };
                            view! { <div class="stale-notice">{msg}</div> }
                        }}
                    </Show>

                    {if show_stale.unwrap_or(false) {
                        view! {
                            <div class="stale-notice">
                                "Fetching prices from Universalis — check back in a moment."
                            </div>
                        }
                        .into_any()
                    } else {
                        view! {}.into_any()
                    }}

                    {ALL_JOBS
                        .iter()
                        .map(|&(cj_id, abbr)| {
                            let job_result: Option<shared::JobResult> = prices_data
                                .get()
                                .and_then(|p| {
                                    p.jobs.into_iter().find(|j| j.class_job_id == cj_id)
                                });

                            let level = move || {
                                levels
                                    .get()
                                    .get(&cj_id)
                                    .copied()
                                    .unwrap_or(100)
                            };

                            view! {
                                <div class="job-panel">
                                    <div class="job-panel-header">
                                        <span class="job-abbr">{abbr}</span>
                                        <div class="level-stepper">
                                            <button on:click=move |_| adjust_level(cj_id, -1)>
                                                "−"
                                            </button>
                                            <input
                                                type="number"
                                                min="1"
                                                max="100"
                                                prop:value=move || level().to_string()
                                                on:change=move |e| {
                                                    if let Ok(n) =
                                                        event_target_value(&e).parse::<u8>()
                                                    {
                                                        set_level(cj_id, n.clamp(1, 100));
                                                    }
                                                }
                                                on:click=move |e| {
                                                    if let Some(target) = e.target() {
                                                        if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                                                            input.select();
                                                        }
                                                    }
                                                }
                                            />
                                            <button on:click=move |_| adjust_level(cj_id, 1)>
                                                "+"
                                            </button>
                                        </div>
                                    </div>

                                    {move || {
                                        match &job_result {
                                            Some(jr) if !jr.items.is_empty() => {
                                                let pool_size = jr.items.len();
                                                view! {
                                                    <div class="items-grid">
                                                        {jr.items
                                                            .iter()
                                                            .enumerate()
                                                            .map(|(idx, item)| {
                                                                let is_today =
                                                                    rotation::today_index(
                                                                        now_unix,
                                                                        pool_size,
                                                                    ) == Some(idx);
                                                                let item_id = item.item_id;
                                                                let is_selected = move || {
                                                                    my_list.get().contains(&item_id)
                                                                };

                                                                view! {
                                                                    <div
                                                                        class=move || {
                                                                            let mut class =
                                                                                String::from(
                                                                                    "item-card",
                                                                                );
                                                                            if is_today {
                                                                                class
                                                                                    .push_str(
                                                                                        " today",
                                                                                    );
                                                                            }
                                                                            class
                                                                        }
                                                                        class:selected=is_selected
                                                                        on:click=move |_| {
                                                                            set_my_list.update(|s| {
                                                                                if s.contains(&item_id) {
                                                                                    s.remove(&item_id);
                                                                                } else {
                                                                                    s.insert(item_id);
                                                                                }
                                                                            });
                                                                            let ids: Vec<_> =
                                                                                my_list
                                                                                    .get_untracked()
                                                                                    .into_iter()
                                                                                    .collect();
                                                                            storage::save_my_list(&ids);
                                                                        }
                                                                    >
                                                                        {if is_today {
                                                                            view! {
                                                                                <span class="today-badge">
                                                                                    "Today"
                                                                                </span>
                                                                            }
                                                                            .into_any()
                                                                        } else {
                                                                            view! {}.into_any()
                                                                        }}
                                                                        <div class="item-name">
                                                                            {item.item_name.clone()}
                                                                        </div>
                                                                        <div class="item-meta">
                                                                            "Requires: "
                                                                            {item.count}
                                                                            " × "
                                                                            {item.item_name.clone()}
                                                                        </div>
                                                                        <hr class="separator"/>
                                                                        {match (
                                                                            &item.hq_best,
                                                                            &item.nq_best,
                                                                        ) {
                                                                            (Some(hq), nq) => {
                                                                                let nq_line =
                                                                                    nq.as_ref()
                                                                                        .map(|n| {
                                                                                            let nq_price =
                                                                                                format_gil(
                                                                                                    n.price_per_unit,
                                                                                                );
                                                                                            let nq_world =
                                                                                                n.world_name.clone();
                                                                                            let nq_dc =
                                                                                                n.datacenter.clone();
                                                                                            view! {
                                                                                                <div class="nq-alt">
                                                                                                    "NQ: "
                                                                                                    {nq_price}
                                                                                                    " gil each · "
                                                                                                    {nq_world}
                                                                                                    " · "
                                                                                                    {nq_dc}
                                                                                                </div>
                                                                                            }
                                                                                        });
                                                                                view! {
                                                                                    <div class="price-row">
                                                                                        <div>
                                                                                            <div class="price">
                                                                                                {format_gil(hq.price_per_unit)}
                                                                                                " gil each"
                                                                                                <span class="hq-badge">
                                                                                                    "HQ"
                                                                                                </span>
                                                                                            </div>
                                                                                            <div class="price-detail">
                                                                                                "Stack: "
                                                                                                {hq.quantity}
                                                                                                " · Total: "
                                                                                                {format_gil(
                                                                                                    hq.price_per_unit
                                                                                                        * hq.quantity,
                                                                                                )}
                                                                                                " gil"
                                                                                            </div>
                                                                                            {nq_line}
                                                                                        </div>
                                                                                        <div class="world">
                                                                                            {hq.world_name.clone()}
                                                                                            " · "
                                                                                            {hq.datacenter.clone()}
                                                                                        </div>
                                                                                    </div>
                                                                                }
                                                                                .into_any()
                                                                            },
                                                                            (None, Some(nq)) => {
                                                                                view! {
                                                                                    <div class="price-row">
                                                                                        <div>
                                                                                            <div class="price nq-price">
                                                                                                {format_gil(nq.price_per_unit)}
                                                                                                " gil each"
                                                                                                <span class="nq-badge">
                                                                                                    "NQ"
                                                                                                </span>
                                                                                            </div>
                                                                                            <div class="price-detail">
                                                                                                "Stack: "
                                                                                                {nq.quantity}
                                                                                                " · Total: "
                                                                                                {format_gil(
                                                                                                    nq.price_per_unit
                                                                                                        * nq.quantity,
                                                                                                )}
                                                                                                " gil"
                                                                                            </div>
                                                                                            <div class="nq-note">
                                                                                                "No HQ listings available"
                                                                                            </div>
                                                                                        </div>
                                                                                        <div class="world">
                                                                                            {nq.world_name.clone()}
                                                                                            " · "
                                                                                            {nq.datacenter.clone()}
                                                                                        </div>
                                                                                    </div>
                                                                                }
                                                                                .into_any()
                                                                            },
                                                                            (None, None) => {
                                                                                view! {
                                                                                    <div class="no-listing">
                                                                                        "No listings available"
                                                                                    </div>
                                                                                }
                                                                                .into_any()
                                                                            },
                                                                        }}
                                                                    </div>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </div>
                                                }
                                                .into_any()
                                            },
                                            _ => {
                                                view! {
                                                    <div class="empty-pool">
                                                        "No turn-in items at this level. Try ±1 level."
                                                    </div>
                                                }
                                                .into_any()
                                            },
                                        }
                                    }}
                                </div>
                            }
                        })
                        .collect_view()}
                }
            }}
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
