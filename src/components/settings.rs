use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;

const ASSET_CHECK_COOLDOWN_SECONDS: u32 = 10 * 60;

#[component]
pub fn Settings(tick: ReadSignal<u32>) -> impl IntoView {
    let (catalog, set_catalog) = signal(None::<invoke::CatalogStatus>);
    let (downloading_assets, set_downloading_assets) = signal(false);
    let (checking_assets, set_checking_assets) = signal(false);
    let (asset_check_cooldown, set_asset_check_cooldown) = signal(0u32);
    let (asset_status, set_asset_status) = signal(None::<invoke::AssetUpdateStatus>);
    let (asset_message, set_asset_message) = signal(String::new());
    let (proxy_url, set_proxy_url) = signal("http://127.0.0.1:8080".to_string());
    let (asset_manifest_url, set_asset_manifest_url) =
        signal("http://127.0.0.1:3000/assets/manifest".to_string());
    let (server_url, set_server_url) = signal("http://127.0.0.1:3000".to_string());
    let (auth_token, set_auth_token) = signal(String::new());
    let (steam_id, set_steam_id) = signal(String::new());
    let (share_claimable_rewards, set_share_claimable_rewards) = signal(false);
    let (refresh_ms, set_refresh_ms) = signal("500".to_string());
    let (log_level, set_log_level) = signal("info".to_string());
    let (include_steam_launch_options, set_include_steam_launch_options) = signal(false);
    let (steam_launch_options, set_steam_launch_options) = signal(String::new());
    let (launch_game_on_start, set_launch_game_on_start) = signal(false);
    let (steam_launch_options_prompted, set_steam_launch_options_prompted) = signal(false);
    let (use_system_proxy, set_use_system_proxy) = signal(false);
    let (sysproxy_status, set_sysproxy_status) = signal(invoke::SystemProxyStatus {
        running: false,
        pid: None,
        message: "Checking...".to_string(),
    });
    let (sysproxy_loading, set_sysproxy_loading) = signal(false);
    let (queue_filters, _set_queue_filters) = signal(HashMap::<String, String>::new());

    let fetch_sysproxy_status = move || {
        spawn_local(async move {
            let status = invoke::invoke_get_system_proxy_status().await;
            set_sysproxy_status.set(status);
        });
    };

    let fetch_catalog = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_catalog_status().await;
            set_catalog.set(data);
        });
    };

    let fetch_asset_status = move || {
        if checking_assets.get_untracked() || asset_check_cooldown.get_untracked() > 0 {
            return;
        }

        set_checking_assets.set(true);
        spawn_local(async move {
            let status = invoke::invoke_get_asset_update_status().await;
            if let Some(status) = status.clone() {
                set_asset_message.set(status.message.clone());
                if status.update_available {
                    set_downloading_assets.set(true);
                    set_asset_message.set("Update found. Downloading assets...".to_string());
                    let result = invoke::invoke_download_latest_assets().await;
                    set_asset_message.set(result.message.clone());
                    if result.ok {
                        set_catalog.set(invoke::invoke_get_catalog_status().await);
                        set_asset_status.set(invoke::invoke_get_asset_update_status().await);
                    }
                    set_downloading_assets.set(false);
                    set_checking_assets.set(false);
                    set_asset_check_cooldown.set(ASSET_CHECK_COOLDOWN_SECONDS);
                    return;
                }
            }
            set_asset_status.set(status);
            set_checking_assets.set(false);
            set_asset_check_cooldown.set(ASSET_CHECK_COOLDOWN_SECONDS);
        });
    };

    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(1000).await;
                set_asset_check_cooldown.update(|seconds| {
                    *seconds = seconds.saturating_sub(1);
                });
            }
        });
    });

    Effect::new(move |_| {
        tick.get();
        fetch_catalog();
        if use_system_proxy.get() {
            fetch_sysproxy_status();
        }
    });

    Effect::new(move |_| {
        spawn_local(async move {
            let settings = invoke::invoke_get_settings().await;
            set_refresh_ms.set(settings.refresh_ms.to_string());
            set_log_level.set(settings.log_level);
            set_proxy_url.set(settings.proxy_url);
            set_asset_manifest_url.set(settings.asset_manifest_url);
            set_server_url.set(settings.server_url);
            set_auth_token.set(settings.auth_token);
            set_steam_id.set(settings.steam_id);
            set_share_claimable_rewards.set(settings.share_claimable_rewards);
            set_include_steam_launch_options.set(settings.include_steam_launch_options);
            set_steam_launch_options.set(settings.steam_launch_options);
            set_launch_game_on_start.set(settings.launch_game_on_start);
            set_steam_launch_options_prompted.set(settings.steam_launch_options_prompted);
            set_use_system_proxy.set(settings.use_system_proxy);
            _set_queue_filters.set(settings.queue_filters);
            fetch_sysproxy_status();
            if asset_status.get_untracked().is_none() {
                fetch_asset_status();
            }
        });
    });

    let asset_check_disabled =
        move || checking_assets.get() || downloading_assets.get() || asset_check_cooldown.get() > 0;
    let asset_check_label = move || {
        if downloading_assets.get() {
            "Downloading...".to_string()
        } else if checking_assets.get() {
            "Checking...".to_string()
        } else {
            match asset_check_cooldown.get() {
                0 => "Check Assets Update".to_string(),
                seconds => format!("Check again in {:02}:{:02}", seconds / 60, seconds % 60),
            }
        }
    };

    let current_settings = move || invoke::AppSettings {
        refresh_ms: refresh_ms
            .get()
            .parse::<u32>()
            .unwrap_or(500)
            .clamp(100, 5000),
        log_level: log_level.get(),
        proxy_url: proxy_url.get(),
        asset_manifest_url: asset_manifest_url.get(),
        server_url: server_url.get(),
        auth_token: auth_token.get(),
        steam_id: steam_id.get(),
        share_claimable_rewards: share_claimable_rewards.get(),
        include_steam_launch_options: include_steam_launch_options.get(),
        steam_launch_options: steam_launch_options.get(),
        launch_game_on_start: launch_game_on_start.get(),
        steam_launch_options_prompted: steam_launch_options_prompted.get(),
        use_system_proxy: use_system_proxy.get(),
        offline_mode: false,
        queue_filters: queue_filters.get(),
    };

    let toggle_system_proxy = move || {
        if sysproxy_loading.get() {
            return;
        }
        set_sysproxy_loading.set(true);
        spawn_local(async move {
            let status = if sysproxy_status.get_untracked().running {
                invoke::invoke_stop_system_proxy().await
            } else {
                invoke::invoke_start_system_proxy().await
            };
            set_sysproxy_status.set(status);
            set_sysproxy_loading.set(false);
        });
    };

    let save_settings = move || {
        let settings = current_settings();
        spawn_local(async move {
            invoke::invoke_set_settings(settings).await;
        });
    };

    view! {
        <div class="panel-header">
            <div class="panel-title">"SETTINGS"</div>
        </div>

        <div class="settings-grid">
            <div class="settings-section">
                <div class="settings-section-title">"\u{2699} General"</div>

                <div class="settings-row">
                    <label class="settings-label">"Auto-refresh interval (ms)"</label>
                    <input type="number" prop:value=refresh_ms min="100" max="5000" step="100"
                        on:input=move |ev| {
                            set_refresh_ms.set(event_target_value(&ev));
                            save_settings();
                        }
                    />
                    <span class="settings-hint">"How often tables refresh (100-5000)"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Log level"</label>
                    <select prop:value=log_level on:change=move |ev| {
                        set_log_level.set(event_target_value(&ev));
                        save_settings();
                    }>
                        <option value="error">"Error"</option>
                        <option value="warn">"Warn"</option>
                        <option value="info">"Info"</option>
                        <option value="debug">"Debug"</option>
                        <option value="trace">"Trace"</option>
                    </select>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Use system mitmproxy"</label>
                    <label class="toggle-switch">
                        <input type="checkbox" prop:checked=use_system_proxy
                            on:change=move |ev| {
                                set_use_system_proxy.set(event_target_checked(&ev));
                                save_settings();
                            }
                        />
                        <span class="slider"></span>
                    </label>
                    <span class="settings-hint">"Disables embedded proxy. Applies immediately."</span>
                </div>

                {move || use_system_proxy.get().then(|| view! {
                    <>
                        <div class="settings-row">
                            <label class="settings-label">"System proxy status"</label>
                            <span class="settings-value">
                                {move || {
                                    let s = sysproxy_status.get();
                                    if s.running {
                                        view! {
                                            <span class="pill green">
                                                <span class="pill-dot">"\u{25cf}"</span>
                                                { format!(" Running (PID {})", s.pid.unwrap_or(0)) }
                                            </span>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <span class="pill gray">
                                                <span class="pill-dot">"\u{25cf}"</span>
                                                " Stopped"
                                            </span>
                                        }.into_any()
                                    }
                                }}
                            </span>
                        </div>
                        <div class="settings-row">
                            <label class="settings-label"></label>
                            <button class="btn-action" disabled=move || sysproxy_loading.get()
                                on:click=move |_| toggle_system_proxy()
                            >
                                {move || {
                                    if sysproxy_loading.get() {
                                        "Working..."
                                    } else if sysproxy_status.get().running {
                                        "Stop System Proxy"
                                    } else {
                                        "Start system proxy"
                                    }
                                }}
                            </button>
                            <span class="settings-hint">
                                {move || if !sysproxy_status.get().message.is_empty() { sysproxy_status.get().message } else { String::new() }}
                            </span>
                        </div>
                    </>
                })}
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{25b6} Game Launch"</div>

                <div class="settings-row">
                    <label class="settings-label">"Launch on startup"</label>
                    <label class="toggle-switch">
                        <input type="checkbox" prop:checked=launch_game_on_start
                            on:change=move |ev| {
                                set_launch_game_on_start.set(event_target_checked(&ev));
                                save_settings();
                            }
                        />
                        <span class="slider"></span>
                    </label>
                    <span class="settings-hint">"Open the game automatically when the dashboard starts"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Use launch options"</label>
                    <label class="toggle-switch">
                        <input type="checkbox" prop:checked=include_steam_launch_options
                            on:change=move |ev| {
                                set_include_steam_launch_options.set(event_target_checked(&ev));
                                save_settings();
                            }
                        />
                        <span class="slider"></span>
                    </label>
                    <span class="settings-hint">"Marks that Steam is configured manually for capture."</span>
                </div>

                <div class="settings-row column">
                    <label class="settings-label">"Steam launch options"</label>
                    <input class="settings-wide-input" type="text" prop:value=steam_launch_options
                        placeholder="OS-specific default loads automatically"
                        disabled=move || !include_steam_launch_options.get()
                        on:input=move |ev| {
                            set_steam_launch_options.set(event_target_value(&ev));
                            save_settings();
                        }
                    />
                    <span class="settings-hint">"Copy this into the game's Steam Launch Options in Steam."</span>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{1f4da} Catalog"</div>

                <div class="settings-row">
                    <label class="settings-label">"Downloaded version"</label>
                    <span class="settings-value">
                        {move || asset_status.get().and_then(|s| s.current_version).unwrap_or_else(|| "Manual / none".to_string())}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Latest version"</label>
                    <span class="settings-value">
                        {move || asset_status.get().and_then(|s| s.latest_version).unwrap_or_else(|| "--".to_string())}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Hosted status"</label>
                    <span class="settings-value">
                        {move || if asset_message.get().is_empty() { "--".to_string() } else { asset_message.get() }}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Items loaded"</label>
                    <span class="settings-value">
                        {move || catalog.get().map_or("--".to_string(), |c| c.items_count.to_string())}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Stages loaded"</label>
                    <span class="settings-value">
                        {move || catalog.get().map_or("--".to_string(), |c| c.stages_count.to_string())}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Display names"</label>
                    <span class="settings-value">
                        {move || catalog.get().map_or("--".to_string(), |c| c.display_names_count.to_string())}
                    </span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Status"</label>
                    <span class="settings-value">
                        {move || {
                            match catalog.get() {
                                Some(c) if c.valid => view! {
                                    <span class="pill green"><span class="pill-dot">"\u{25cf}"</span> " Valid"</span>
                                }.into_any(),
                                Some(_) => view! {
                                    <span class="pill gray"><span class="pill-dot">"\u{25cf}"</span> " Invalid"</span>
                                }.into_any(),
                                None => view! { <span>"--"</span> }.into_any(),
                            }
                        }}
                    </span>
                </div>

                <div class="settings-row">
                    <button class="btn-action" on:click=move |_| fetch_asset_status() disabled=asset_check_disabled>
                        <span class:spin-emoji=move || checking_assets.get() || downloading_assets.get() style:display=move || if checking_assets.get() || downloading_assets.get() { "inline-block" } else { "none" }>"\u{1f504}"</span>
                        {move || if checking_assets.get() || downloading_assets.get() { " " } else { "" }}
                        {asset_check_label}
                    </button>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{2139} About"</div>

                <div class="settings-row">
                    <label class="settings-label">"Version"</label>
                    <span class="settings-value">{env!("CARGO_PKG_VERSION")}</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Engine"</label>
                    <span class="settings-value">"Leptos + Tauri"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Rust edition"</label>
                    <span class="settings-value">"2021"</span>
                </div>
            </div>
        </div>
    }
}
