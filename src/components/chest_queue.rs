use crate::app::{
    chest_emoji, rarity_class, rarity_color, rarity_diamond, rarity_title, reward_emoji,
};
use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::{BTreeSet, HashMap};

#[component]
pub fn ChestQueue(tick: ReadSignal<u32>) -> impl IntoView {
    let (rows, set_rows) = signal(Vec::<invoke::ChestRow>::new());
    let (_summary, set_summary) = signal(HashMap::<String, usize>::new());
    let (show_claimable_only, set_show_claimable_only) = signal(false);
    let (show_claimed, set_show_claimed) = signal(false);
    let (freeze_queue, set_freeze_queue) = signal(false);
    let (rarity_options, set_rarity_options) = signal(Vec::<String>::new());
    let (filter_cats, set_filter_cats) = signal(HashMap::<String, String>::new());

    Effect::new(move |_| {
        spawn_local(async move {
            let options = invoke::invoke_get_rarity_order().await;
            if !options.is_empty() {
                set_rarity_options.set(options);
            }
        });
    });

    Effect::new(move |_| {
        spawn_local(async move {
            set_freeze_queue.set(invoke::invoke_get_freeze_queue().await);
        });
    });

    let fetch_data = move || {
        let include_claimed = show_claimed.get();
        spawn_local(async move {
            let data = invoke::invoke_get_chest_rows(include_claimed).await;
            set_rows.set(data);
            let sum = invoke::invoke_get_box_summary().await;
            set_summary.set(sum);
        });
    };

    Effect::new(move |_| {
        tick.get();
        show_claimed.track();
        fetch_data();
    });

    let categories = move || {
        let set: BTreeSet<String> = rows
            .get()
            .iter()
            .map(|r| {
                if r.slot.is_empty() {
                    r.kind.clone()
                } else {
                    r.slot.clone()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        set.into_iter().collect::<Vec<_>>()
    };

    Effect::new(move |_| {
        let cats = categories();
        let mut filters = filter_cats.get();
        let mut changed = false;
        for cat in &cats {
            if !filters.contains_key(cat) {
                filters.insert(cat.clone(), "ALL".to_string());
                changed = true;
            }
        }
        if changed {
            set_filter_cats.set(filters);
        }
    });

    let filtered_rows = move || {
        let only_claimable = show_claimable_only.get();
        let show_claimed = show_claimed.get();
        let filters = filter_cats.get();
        let options = rarity_options.get();
        rows.get()
            .into_iter()
            .filter(|r| {
                if r.is_get && !show_claimed {
                    return false;
                }
                if only_claimable && r.remaining > 0.0 {
                    return false;
                }
                let cat = if r.slot.is_empty() { &r.kind } else { &r.slot };
                let filt = filters
                    .get(cat.as_str())
                    .map(String::as_str)
                    .unwrap_or("ALL");
                if filt == "ALL" {
                    return true;
                }
                match options.iter().position(|x| x == filt) {
                    Some(min_idx) => options
                        .iter()
                        .position(|x| x == &r.rarity)
                        .map(|ri| ri >= min_idx)
                        .unwrap_or(false),
                    None => false,
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="panel-header">
            <div class="panel-title">"CHEST QUEUE"</div>
            <div class="toggle-row">
                <span>"Claimable Only"</span>
                <label class="toggle-switch">
                    <input type="checkbox" prop:checked=show_claimable_only on:change=move |ev| {
                        set_show_claimable_only.set(event_target_checked(&ev));
                    }/>
                    <span class="slider"></span>
                </label>
                <span class="toggle-sep"></span>
                <span>"Show Claimed"</span>
                <label class="toggle-switch">
                    <input type="checkbox" prop:checked=show_claimed on:change=move |ev| {
                        set_show_claimed.set(event_target_checked(&ev));
                    }/>
                    <span class="slider"></span>
                </label>
                <span class="toggle-sep"></span>
                <span>"Freeze Queue"</span>
                <label class="toggle-switch">
                    <input type="checkbox" prop:checked=freeze_queue on:change=move |ev| {
                        let enabled = event_target_checked(&ev);
                        set_freeze_queue.set(enabled);
                        spawn_local(async move {
                            let applied = invoke::invoke_set_freeze_queue(enabled).await;
                            set_freeze_queue.set(applied);
                        });
                    }/>
                    <span class="slider"></span>
                </label>
            </div>
        </div>

        <details class="filter-details" open>
            <summary class="filter-summary">"FILTERS"</summary>
            <div class="filter-row">
                <For each=categories key=|cat| cat.clone() let(cat)>
                    <label>
                        {cat.clone()}
                        <select on:change=move |ev| {
                            let v = event_target_value(&ev);
                            let c = cat.clone();
                            set_filter_cats.update(|f| { f.insert(c, v); });
                        }>
                            <option value="ALL">"ALL"</option>
                            {rarity_options.get().into_iter().map(|r| {
                                let opt = r.clone();
                                view! { <option value=opt>{r}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                    </label>
                </For>
            </div>
        </details>

        <div class="table-panel">
            <table class="data-table">
                <thead>
                    <tr>
                        <th style="width:40px">"#"</th>
                        <th style="width:220px">"Type"</th>
                        <th style="width:110px">"Status"</th>
                        <th style="width:150px">"Unlock Time"</th>
                        <th>"Reward"</th>
                        <th style="width:120px">"Rarity"</th>
                        <th style="width:60px">"Open"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || filtered_rows().into_iter().enumerate().map(|(index, row)| {
                        let box_label = row.box_label.clone();
                        let box_icon = chest_emoji(&box_label);
                        let rarity = row.rarity.clone();
                        let reward_icon = reward_emoji(&rarity);
                        let reward_name = row.name.clone();
                        let rarity_badge_class = format!("rarity-badge {}", rarity_class(&rarity));
                        let rarity_badge_color = rarity_color(&rarity);
                        let rarity_badge_diamond = rarity_diamond(&rarity);
                        let rarity_badge_title = rarity_title(&rarity);
                        let remaining = row.remaining;
                        let unlock_width = format!("{}%", ((86400.0 - remaining) / 86400.0 * 100.0).min(100.0));
                        let is_get = row.is_get;
                        let key_for_open = row.key.clone().unwrap_or_default();
                        view! {
                            <tr>
                                <td style="color: var(--text-dim)">{index + 1}</td>
                                <td>
                                    <div class="cell-type">
                                        <span class="type-icon">{box_icon}</span>
                                        <span>{box_label}</span>
                                    </div>
                                </td>
                                <td>
                                    {if is_get {
                                        view! { <span class="pill purple"><span class="pill-dot">"\u{2714}\u{fe0f}"</span> " Claimed"</span> }.into_any()
                                    } else if remaining <= 0.0 {
                                        view! { <span class="pill green"><span class="pill-dot">"\u{25cf}"</span> " Claimable"</span> }.into_any()
                                    } else {
                                        view! { <span class="pill gray"><span class="pill-dot">"\u{1f512}"</span> " Waiting"</span> }.into_any()
                                    }}
                                </td>
                                <td>
                                    <div class="unlock-cell">
                                        <div class="unlock-bar">
                                            <div class="unlock-bar-fill" style:width=unlock_width></div>
                                        </div>
                                        {if remaining <= 0.0 {
                                            view! { <span class="unlock-time">"--"</span> }.into_any()
                                        } else {
                                            let secs = remaining as i64;
                                            let h = secs / 3600;
                                            let m = (secs % 3600) / 60;
                                            let s = secs % 60;
                                            view! { <span class="unlock-time">{format!("{:02}:{:02}:{:02}", h, m, s)}</span> }.into_any()
                                        }}
                                    </div>
                                </td>
                                <td>
                                    <div class="cell-reward">
                                        <span class="rw-icon">{reward_icon}</span>
                                        <span class="rw-name">{reward_name}</span>
                                    </div>
                                </td>
                                <td>
                                    <span class=rarity_badge_class style:color=rarity_badge_color>
                                        <span class="diamond">{rarity_badge_diamond}</span>
                                        {rarity_badge_title}
                                    </span>
                                </td>
                                <td>
                                    <button class="btn-open" on:click=move |_| {
                                        let k = key_for_open.clone();
                                        spawn_local(async move {
                                            crate::invoke::invoke_mark_opened(&k).await;
                                        });
                                    }>"\u{2714}\u{fe0f}"</button>
                                </td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }
}
