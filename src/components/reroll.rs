use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::invoke;
use crate::app::{rarity_color, rarity_title, rarity_diamond, chest_emoji, reward_emoji};

const RARITY_OPTIONS: &[&str] = &[
    "COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "IMMORTAL",
    "ARCANA", "BEYOND", "CELESTIAL", "DIVINE", "COSMIC",
];

#[component]
pub fn RerollPreview(tick: ReadSignal<u32>) -> impl IntoView {
    let (rows, set_rows) = signal(Vec::<invoke::ChestRow>::new());
    let (reroll_info, set_reroll_info) = signal(None::<invoke::ProcessBoxInfo>);
    let (filter_rarity, set_filter_rarity) = signal("ALL".to_string());

    let fetch_data = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_chest_rows(false).await;
            set_rows.set(data);
            let pb = invoke::invoke_get_last_processbox().await;
            set_reroll_info.set(pb);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_data();
    });

    let filtered_rows = move || {
        let filt = filter_rarity.get();
        let min_idx = RARITY_OPTIONS.iter().position(|&r| r == filt);
        let mut display: Vec<invoke::ChestRow> = rows.get()
            .into_iter()
            .filter(|r| {
                filt == "ALL" || match min_idx {
                    Some(mi) => RARITY_OPTIONS.iter().position(|&x| x == r.rarity).map(|ri| ri >= mi).unwrap_or(false),
                    None => false,
                }
            })
            .collect();
        display.sort_by(|a, b| a.remaining.partial_cmp(&b.remaining).unwrap_or(std::cmp::Ordering::Equal));
        display
    };

    view! {
        <div class="panel-header">
            <div class="panel-title">"REROLL PREVIEW"</div>
        </div>

        <div class="status-text warning">
            "Use this after entering Act Boss to judge the generated queue."
        </div>

        <div class="status-text">
            {move || match reroll_info.get() {
                Some(ref pb) => format!("Last reset: {} | reset={}", pb.at, pb.is_reset),
                None => "Last reset: ?".to_string(),
            }}
        </div>

        <div class="filter-row">
            <label style="color: #e2e8f0; font-size: 13px;">"Filter rarity"
                <select style="appearance: none; -webkit-appearance: none; -moz-appearance: none; background: #1a2230; color: #ffffff; font-weight: 600; border: 1px solid #3b82f6; border-radius: 6px; padding: 6px 28px 6px 12px; font-size: 13px; outline: none; cursor: pointer;" on:change=move |ev| {
                    set_filter_rarity.set(event_target_value(&ev));
                }>
                    <option value="ALL" style="background: #131921; color: #ffffff;">"ALL"</option>
                    {RARITY_OPTIONS.iter().map(|&r| {
                        view! { <option value=r style="background: #131921; color: #ffffff;">{r}</option> }
                    }).collect::<Vec<_>>()}
                </select>
            </label>
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
                        let color = rarity_color(&row.rarity);
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
                                    {if is_ready {
                                        view! { <span class="pill green"><span class="pill-dot">"\u{25cf}"</span> " Claimable"</span> }.into_any()
                                    } else {
                                        view! { <span class="pill gray"><span class="pill-dot">"\u{1f512}"</span> " Waiting"</span> }.into_any()
                                    }}
                                </td>
                                <td>
                                    <span class="unlock-time">{remaining_display}</span>
                                </td>
                                <td>
                                    <div class="cell-reward">
                                        <span class="rw-icon">{re}</span>
                                        <span class="rw-name">{row.name}</span>
                                    </div>
                                </td>
                                <td>
                                    <span class="rarity-badge" style:color=color>
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
