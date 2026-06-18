use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::invoke;
use crate::app::{rarity_color, rarity_title, rarity_diamond};

#[component]
pub fn BossDrop(tick: ReadSignal<u32>) -> impl IntoView {
    let (snapshot, set_snapshot) = signal(None::<invoke::AddedItemsSnapshot>);

    let fetch_data = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_last_added().await;
            set_snapshot.set(data);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_data();
    });

    view! {
        <div class="status-text">
            {move || match snapshot.get() {
                Some(ref snap) => format!("Last immediate drop: {} | source={} | count={}", snap.at, snap.source, snap.items.len()),
                None => "Last immediate drop: ?".to_string(),
            }}
        </div>

        <div class="table-panel">
            <table class="data-table">
                <thead>
                    <tr>
                        <th style="width:160px">"At"</th>
                        <th style="width:120px">"Rarity"</th>
                        <th>"Item"</th>
                        <th style="width:100px">"Item ID"</th>
                        <th style="width:80px">"Count"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || snapshot.get().map(|snap| {
                        snap.items.into_iter().map(|item| {
                            let color = rarity_color(&item.rarity);
                            view! {
                                <tr>
                                    <td style="color: var(--text-dim)">{item.at}</td>
                                    <td>
                                        <span class="rarity-badge" style:color=color>
                                            <span class="diamond">{rarity_diamond(&item.rarity)}</span>
                                            {rarity_title(&item.rarity)}
                                        </span>
                                    </td>
                                    <td>{item.name}</td>
                                    <td style="color: var(--text-dim); font-family: monospace; font-size: 12px;">
                                        {format!("{:?}", item.item_id)}
                                    </td>
                                    <td>{item.count}</td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()
                    }).unwrap_or_default().into_iter().collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>

        {move || if snapshot.get().is_none() {
            view! { <div class="empty-state">"No boss drops recorded yet."</div> }.into_any()
        } else {
            view! { <span></span> }.into_any()
        }}
    }
}
