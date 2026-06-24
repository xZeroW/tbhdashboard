use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn ExtraFeatures(tick: ReadSignal<u32>) -> impl IntoView {
    let (enabled, set_enabled) = signal(false);
    let (multiplier, set_multiplier) = signal(2.0f32);
    let (game_running, set_game_running) = signal(false);
    let (addresses_found, set_addresses_found) = signal(0usize);
    let (last_write_ok, set_last_write_ok) = signal(false);
    let (last_verify_ok, set_last_verify_ok) = signal(false);
    let (status_message, set_status_message) = signal(String::new());

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            let state = invoke::invoke_get_speedhack_state().await;
            set_enabled.set(state.enabled);
            set_multiplier.set(state.multiplier);
            set_game_running.set(state.game_running);
            set_addresses_found.set(state.addresses_found);
            set_last_write_ok.set(state.last_write_ok);
            set_last_verify_ok.set(state.last_verify_ok);
        });
    });

    let toggle_speedhack = move || {
        let new_val = !enabled.get();
        set_enabled.set(new_val);
        let mult = multiplier.get();
        set_status_message.set(String::new());
        spawn_local(async move {
            invoke::invoke_set_speedhack_enabled(new_val).await;
            if new_val {
                set_status_message.set(format!("Speedhack active — {}x speed", mult));
            } else {
                set_status_message.set("Speedhack disabled — values restored".to_string());
            }
        });
    };

    let update_multiplier = move |val: f32| {
        let clamped = val.clamp(0.1, 10.0);
        set_multiplier.set(clamped);
        let active = enabled.get();
        spawn_local(async move {
            invoke::invoke_set_speedhack_multiplier(clamped).await;
            if active {
                set_status_message.set(format!("Speedhack active — {}x speed", clamped));
            }
        });
    };

    let verify = move || {
        set_status_message.set("Verifying...".to_string());
        spawn_local(async move {
            let state = invoke::invoke_verify_speedhack().await;
            set_game_running.set(state.game_running);
            set_addresses_found.set(state.addresses_found);
            set_last_write_ok.set(state.last_write_ok);
            set_last_verify_ok.set(state.last_verify_ok);
            set_enabled.set(state.enabled);
            set_multiplier.set(state.multiplier);
            if state.game_running && state.addresses_found > 0 && state.last_verify_ok {
                set_status_message.set(format!(
                    "✓ Patch verified — {} addresses, write confirmed",
                    state.addresses_found
                ));
            } else if state.game_running && state.addresses_found == 0 {
                set_status_message.set(
                    "✗ Game running but no writable addresses found — patch may not work"
                        .to_string(),
                );
            } else if !state.game_running {
                set_status_message
                    .set("✗ Game process not found — start the game first".to_string());
            } else {
                set_status_message
                    .set("✗ Patch verification failed — some addresses did not match".to_string());
            }
        });
    };

    view! {
        <div class="panel-header">
            <div class="panel-title">"EXTRA FEATURES"</div>
        </div>
        <div class="settings-grid">
            <div class="settings-section">
                <div class="settings-section-title">"\u{26a1} Speedhack"</div>

                <div class="settings-row">
                    <label class="settings-label">"Enable Speedhack"</label>
                    <label class="toggle-switch">
                        <input type="checkbox"
                            prop:checked=enabled
                            on:change=move |_| toggle_speedhack()
                        />
                        <span class="slider"></span>
                    </label>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Speed Multiplier"</label>
                    <div class="slider-container">
                        <input type="range"
                            min="0.1"
                            max="10.0"
                            step="0.1"
                            prop:value=move || multiplier.get().to_string()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<f32>() {
                                    update_multiplier(v);
                                }
                            }
                            style="width: 200px;"
                        />
                        <span class="settings-value" style="min-width: 60px; text-align: center;">
                            {move || format!("{:.1}x", multiplier.get())}
                        </span>
                    </div>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"State"</label>
                    <span class="settings-value">
                        {move || {
                            if enabled.get() {
                                view! {
                                    <span class="pill green">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        { format!(" Active — {:.1}x", multiplier.get()) }
                                    </span>
                                }.into_any()
                            } else {
                                view! {
                                    <span class="pill gray">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Disabled"
                                    </span>
                                }.into_any()
                            }
                        }}
                    </span>
                </div>

                <div class="settings-row" style="border-top: 1px solid var(--border); padding-top: 8px; margin-top: 4px;">
                    <label class="settings-label">"Game"</label>
                    <span class="settings-value">
                        {move || {
                            if game_running.get() {
                                view! {
                                    <span class="pill green">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Running"
                                    </span>
                                }.into_any()
                            } else {
                                view! {
                                    <span class="pill red">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Not Running"
                                    </span>
                                }.into_any()
                            }
                        }}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Addresses"</label>
                    <span class="settings-value">
                        {move || {
                            let n = addresses_found.get();
                            if n > 0 {
                                view! {
                                    <span class="pill green">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        { format!(" {} found", n) }
                                    </span>
                                }.into_any()
                            } else if game_running.get() {
                                view! {
                                    <span class="pill amber">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Scanning..."
                                    </span>
                                }.into_any()
                            } else {
                                view! {
                                    <span class="pill gray">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " N/A"
                                    </span>
                                }.into_any()
                            }
                        }}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Write"</label>
                    <span class="settings-value">
                        {move || {
                            if !enabled.get() || addresses_found.get() == 0 {
                                view! {
                                    <span class="pill gray">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Idle"
                                    </span>
                                }.into_any()
                            } else if last_write_ok.get() && last_verify_ok.get() {
                                view! {
                                    <span class="pill green">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " OK (verified)"
                                    </span>
                                }.into_any()
                            } else if last_write_ok.get() {
                                view! {
                                    <span class="pill amber">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Written (unverified)"
                                    </span>
                                }.into_any()
                            } else {
                                view! {
                                    <span class="pill red">
                                        <span class="pill-dot">"\u{25cf}"</span>
                                        " Failed"
                                    </span>
                                }.into_any()
                            }
                        }}
                    </span>
                </div>

                <div class="settings-row" style="gap: 12px;">
                    <label class="settings-label">"Verify"</label>
                    <button class="btn-action"
                        on:click=move |_| verify()
                    >
                        "Rescan & Verify"
                    </button>
                </div>

                <div class="settings-row">
                    <label class="settings-label"></label>
                    <span class="settings-hint">{status_message}</span>
                </div>

                <div class="settings-row column" style="margin-top: 12px;">
                    <p style="color: var(--text-dim); margin: 0; line-height: 1.5; font-size: 13px;">
                        "Speedhack modifies the game's time scale by scanning the process memory. "
                        "Use the status indicators above to confirm the patch is working. "
                        "The game must be running for this to work."
                    </p>
                </div>
            </div>
        </div>
    }
}
