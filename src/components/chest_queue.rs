use crate::app::{
    chest_emoji, rarity_class, rarity_color, rarity_diamond, rarity_title, reward_emoji,
};
use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn ChestQueue(tick: ReadSignal<u32>) -> impl IntoView {
    let (rows, set_rows) = signal(Vec::<invoke::ChestRow>::new());
    let (_summary, set_summary) = signal(std::collections::HashMap::<String, usize>::new());
    let (filter_tab, set_filter_tab) = signal("ALL".to_string());
    let (show_claimable_only, set_show_claimable_only) = signal(false);
    let (show_claimed, set_show_claimed) = signal(false);

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

    let filtered_rows = move || {
        let tab = filter_tab.get();
        let only_claimable = show_claimable_only.get();
        let show_claimed = show_claimed.get();
        rows.get()
            .into_iter()
            .filter(|r| {
                if r.is_get && !show_claimed {
                    return false;
                }
                if only_claimable && r.remaining > 0.0 {
                    return false;
                }
                match tab.as_str() {
                    "Common" => r.box_label.contains("Common"),
                    "Stage" => r.box_label.contains("Stage"),
                    "Boss" => r.box_label.contains("Boss"),
                    _ => true,
                }
            })
            .collect::<Vec<_>>()
    };

    let _stats = move || {
        let all = rows.get();
        let total = all.len();
        let ready = all.iter().filter(|r| r.remaining <= 0.0).count();
        let waiting = total - ready;
        (total, ready, waiting)
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
            </div>
        </div>

        <div class="filter-tabs">
            {["ALL", "Common", "Stage", "Boss"].iter().map(|&tab| {
                let t = tab.to_string();
                let is_active = move || filter_tab.get() == t;
                view! {
                    <button
                        class="filter-tab"
                        class:active=is_active
                        on:click=move |_| set_filter_tab.set(tab.to_string())
                    >{tab}</button>
                }
            }).collect::<Vec<_>>()}
        </div>

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
                    {move || filtered_rows().into_iter().enumerate().map(|(i, row)| {
                        let is_ready = row.remaining <= 0.0;
                        let remaining_display = if is_ready {
                            "--".to_string()
                        } else {
                            let secs = row.remaining as i64;
                            let h = secs / 3600;
                            let m = (secs % 3600) / 60;
                            let s = secs % 60;
                            format!("{:02}:{:02}:{:02}", h, m, s)
                        };
                        let progress = if is_ready {
                            100.0
                        } else {
                            ((86400.0 - row.remaining) / 86400.0 * 100.0).min(100.0)
                        };
                        let color = rarity_color(&row.rarity);
                        let badge_class = format!("rarity-badge {}", rarity_class(&row.rarity));
                        let ce = chest_emoji(&row.box_label);
                        let re = reward_emoji(&row.rarity);
                        let key_for_open = row.key.clone().unwrap_or_default();
                        view! {
                            <tr>
                                <td style="color: var(--text-dim)">{i + 1}</td>
                                <td>
                                    <div class="cell-type">
                                        <span class="type-icon">{ce}</span>
                                        <span>{row.box_label}</span>
                                    </div>
                                </td>
                                <td>
                                    {if row.is_get {
                                        view! { <span class="pill purple"><span class="pill-dot">"\u{2714}\u{fe0f}"</span> " Claimed"</span> }.into_any()
                                    } else if is_ready {
                                        view! { <span class="pill green"><span class="pill-dot">"\u{25cf}"</span> " Claimable"</span> }.into_any()
                                    } else {
                                        view! { <span class="pill gray"><span class="pill-dot">"\u{1f512}"</span> " Waiting"</span> }.into_any()
                                    }}
                                </td>
                                <td>
                                    <div class="unlock-cell">
                                        <div class="unlock-bar">
                                            <div class="unlock-bar-fill" style:width={format!("{}%", progress)}></div>
                                        </div>
                                        <span class="unlock-time">{remaining_display}</span>
                                    </div>
                                </td>
                                <td>
                                    <div class="cell-reward">
                                        <span class="rw-icon">{re}</span>
                                        <span class="rw-name">{row.name}</span>
                                    </div>
                                </td>
                                <td>
                                    <span class=badge_class style:color=color>
                                        <span class="diamond">{rarity_diamond(&row.rarity)}</span>
                                        {rarity_title(&row.rarity)}
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
