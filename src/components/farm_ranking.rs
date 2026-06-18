use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::invoke;

const RARITY_OPTIONS: &[&str] = &[
    "COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY", "IMMORTAL",
    "ARCANA", "BEYOND", "CELESTIAL", "DIVINE", "COSMIC",
];

#[component]
pub fn FarmRanking(tick: ReadSignal<u32>) -> impl IntoView {
    let (rows, set_rows) = signal(Vec::<invoke::FarmRow>::new());
    let (selected_rarity, set_selected_rarity) = signal("BEYOND".to_string());
    let (min_level, set_min_level) = signal("0".to_string());
    let (max_level, set_max_level) = signal("0".to_string());
    let (clear_time, set_clear_time) = signal("0".to_string());

    let fetch_data = move || {
        let rarity = selected_rarity.get();
        let min_l = min_level.get().parse::<i32>().ok().filter(|&v| v > 0);
        let max_l = max_level.get().parse::<i32>().ok().filter(|&v| v > 0);
        let ct = clear_time.get().parse::<f64>().ok().filter(|&v| v > 0.0);
        spawn_local(async move {
            let data = invoke::invoke_get_farm_ranking(
                Some(rarity), None, None, min_l, max_l, ct
            ).await;
            set_rows.set(data);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_data();
    });

    view! {
        <div class="panel-header">
            <div class="panel-title">"FARM RANKING"</div>
        </div>

        <div class="filter-row">
            <label>"Rarity"
                <select on:change=move |ev| {
                    set_selected_rarity.set(event_target_value(&ev));
                    fetch_data();
                }>
                    {RARITY_OPTIONS.iter().map(|&r| {
                        view! { <option value=r selected={r == "BEYOND"}>{r}</option> }
                    }).collect::<Vec<_>>()}
                </select>
            </label>
            <label>"Min level"
                <input type="number" prop:value=min_level
                    on:input=move |ev| { set_min_level.set(event_target_value(&ev)); }
                    on:change=move |_| fetch_data()
                />
            </label>
            <label>"Max level"
                <input type="number" prop:value=max_level
                    on:input=move |ev| { set_max_level.set(event_target_value(&ev)); }
                    on:change=move |_| fetch_data()
                />
            </label>
            <label>"Clear sec"
                <input type="number" prop:value=clear_time step="0.1"
                    on:input=move |ev| { set_clear_time.set(event_target_value(&ev)); }
                    on:change=move |_| fetch_data()
                />
            </label>
        </div>

        <div class="table-panel">
            <table class="data-table">
                <thead>
                    <tr>
                        <th style="width:80px">"Stage"</th>
                        <th style="width:70px">"Level"</th>
                        <th style="width:110px">"Difficulty"</th>
                        <th style="width:140px">"Expected/run"</th>
                        <th style="width:140px">"Expected/hour"</th>
                        <th>"Boxes"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || rows.get().into_iter().map(|row| {
                        let boxes_str = row.boxes.iter()
                            .map(|(id, cnt)| format!("\u{1f4e6} {} x{}", id, cnt))
                            .collect::<Vec<_>>()
                            .join("  ");
                        view! {
                            <tr>
                                <td>{row.name}</td>
                                <td>{row.level}</td>
                                <td>{row.difficulty}</td>
                                <td style="font-family: monospace; font-size: 12px;">{format!("{:.8}", row.expected)}</td>
                                <td style="font-family: monospace; font-size: 12px;">
                                    {row.per_hour.map_or("-".to_string(), |h| format!("{:.6}", h))}
                                </td>
                                <td style="font-size: 12px;">{boxes_str}</td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }
}
