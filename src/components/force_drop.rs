use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn ForceDrop(tick: ReadSignal<u32>) -> impl IntoView {
    let (item_id, set_item_id) = signal(String::new());
    let (saved_id, set_saved_id) = signal(None::<i64>);
    let (saving, set_saving) = signal(false);
    let (save_message, set_save_message) = signal(String::new());
    let (restarting, set_restarting) = signal(false);
    let (restart_message, set_restart_message) = signal(String::new());
    let loaded = RwSignal::new(false);

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            let id = invoke::invoke_get_force_drop_item_id().await;
            if !loaded.get_untracked() {
                set_item_id.set(id.map(|v| v.to_string()).unwrap_or_default());
                loaded.set(true);
            }
            set_saved_id.set(id);
        });
    });

    let save_force_drop = move || {
        let val = item_id.get().trim().to_string();
        if val.is_empty() {
            set_saving.set(true);
            spawn_local(async move {
                invoke::invoke_set_force_drop_item_id(None).await;
                set_saved_id.set(None);
                set_saving.set(false);
                set_save_message.set("Force drop disabled".to_string());
            });
        } else if let Ok(id) = val.parse::<i64>() {
            set_saving.set(true);
            set_save_message.set(String::new());
            spawn_local(async move {
                invoke::invoke_set_force_drop_item_id(Some(id)).await;
                set_saved_id.set(Some(id));
                set_saving.set(false);
                set_save_message.set(format!("Force drop set to item ID {}", id));
            });
        } else {
            set_save_message.set("Invalid item ID — enter a number".to_string());
        }
    };

    let disable_force_drop = move || {
        set_saving.set(true);
        spawn_local(async move {
            invoke::invoke_set_force_drop_item_id(None).await;
            set_item_id.set(String::new());
            set_saved_id.set(None);
            set_saving.set(false);
            set_save_message.set("Force drop disabled".to_string());
        });
    };

    let restart_game = move || {
        if restarting.get() {
            return;
        }
        set_restarting.set(true);
        set_restart_message.set(String::new());
        spawn_local(async move {
            let result = invoke::invoke_restart_game().await;
            set_restart_message.set(result.message);
            set_restarting.set(false);
        });
    };

    view! {
        <div class="panel-header">
            <div class="panel-title">"FORCE DROP"</div>
        </div>
        <div class="settings-grid">
            <div class="settings-section">
                <div class="settings-section-title">"\u{1f4a5} Force Reward Item"</div>
                <div class="settings-row">
                    <label class="settings-label">"Item ID"</label>
                    <input type="number" prop:value=item_id
                        placeholder="e.g. 312171"
                        on:input=move |ev| {
                            set_item_id.set(event_target_value(&ev));
                        }
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                save_force_drop();
                            }
                        }
                    />
                </div>
                <div class="settings-row">
                    <label class="settings-label"></label>
                    <button class="btn-action" disabled=move || saving.get() || item_id.get().trim().is_empty() && saved_id.get().is_some()
                        on:click=move |_| save_force_drop()
                    >
                        {move || if saving.get() { "Saving..." } else { "Set Force Drop" }}
                    </button>
                    {move || saved_id.get().is_some().then(|| view! {
                        <button class="btn-action" disabled=saving.get()
                            on:click=move |_| disable_force_drop()
                            style="margin-left: 8px;"
                        >
                            "Disable"
                        </button>
                    })}
                    <span class="settings-hint">{save_message}</span>
                </div>
                <div class="settings-row">
                    <label class="settings-label">"Status"</label>
                    <span class="settings-value">
                        {move || {
                            let id = saved_id.get();
                            match id {
                                Some(id) => view! {
                                    <span class="pill green">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        { format!(" Active — item ID {}", id) }
                                    </span>
                                }.into_any(),
                                None => view! {
                                    <span class="pill gray">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Disabled"
                                    </span>
                                }.into_any(),
                            }
                        }}
                    </span>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{25b6}\u{fe0f} Restart Game"</div>
                <div class="settings-row column">
                    <p style="color: var(--text-dim); margin: 0 0 12px 0; line-height: 1.5;">
                        "Set a force drop item ID above, then restart the game. The next claimed chests will receive that item instead of their original reward."
                    </p>
                    <button class="btn-action" disabled=move || restarting.get()
                        on:click=move |_| restart_game()
                    >
                        {move || if restarting.get() { "Restarting..." } else { "Restart Game" }}
                    </button>
                    <span class="settings-hint">{restart_message}</span>
                </div>
            </div>
        </div>
    }
}
