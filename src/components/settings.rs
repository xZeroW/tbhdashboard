use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn Settings(tick: ReadSignal<u32>) -> impl IntoView {
    let (catalog, set_catalog) = signal(None::<invoke::CatalogStatus>);
    let (proxy_status, set_proxy_status) = signal(None::<invoke::ProxyStatus>);
    let (assets_root, set_assets_root) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (proxy_url, set_proxy_url) = signal("http://127.0.0.1:8080".to_string());
    let (refresh_ms, set_refresh_ms) = signal("500".to_string());
    let (log_level, set_log_level) = signal("info".to_string());
    let (include_steam_launch_options, set_include_steam_launch_options) = signal(false);
    let (steam_launch_options, set_steam_launch_options) = signal(String::new());
    let (launch_game_on_start, set_launch_game_on_start) = signal(false);
    let (steam_launch_options_prompted, set_steam_launch_options_prompted) = signal(false);

    let fetch_catalog = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_catalog_status().await;
            set_catalog.set(data);
        });
    };

    let fetch_assets_root = move || {
        spawn_local(async move {
            let root = invoke::invoke_get_assets_root().await;
            set_assets_root.set(root);
        });
    };

    let fetch_proxy_status = move || {
        spawn_local(async move {
            set_proxy_status.set(invoke::invoke_get_proxy_status().await);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_catalog();
        fetch_assets_root();
        fetch_proxy_status();
    });

    Effect::new(move |_| {
        spawn_local(async move {
            let settings = invoke::invoke_get_settings().await;
            set_refresh_ms.set(settings.refresh_ms.to_string());
            set_log_level.set(settings.log_level);
            set_proxy_url.set(settings.proxy_url);
            set_include_steam_launch_options.set(settings.include_steam_launch_options);
            set_steam_launch_options.set(settings.steam_launch_options);
            set_launch_game_on_start.set(settings.launch_game_on_start);
            set_steam_launch_options_prompted.set(settings.steam_launch_options_prompted);
        });
    });

    let current_settings = move || invoke::AppSettings {
        refresh_ms: refresh_ms
            .get()
            .parse::<u32>()
            .unwrap_or(500)
            .clamp(100, 5000),
        log_level: log_level.get(),
        proxy_url: proxy_url.get(),
        include_steam_launch_options: include_steam_launch_options.get(),
        steam_launch_options: steam_launch_options.get(),
        launch_game_on_start: launch_game_on_start.get(),
        steam_launch_options_prompted: steam_launch_options_prompted.get(),
    };

    let save_settings = move || {
        let settings = current_settings();
        spawn_local(async move {
            invoke::invoke_set_settings(settings).await;
        });
    };

    let pick_folder = move |_| {
        spawn_local(async move {
            if let Some(path) = invoke::invoke_browse_assets_folder().await {
                set_saving.set(true);
                invoke::invoke_set_assets_path(&path).await;
                let root = invoke::invoke_get_assets_root().await;
                set_assets_root.set(root);
                set_saving.set(false);
            }
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
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{1f310} Proxy"</div>

                <div class="settings-row">
                    <label class="settings-label">"Proxy URL"</label>
                    <input type="text" prop:value=proxy_url
                        on:input=move |ev| {
                            set_proxy_url.set(event_target_value(&ev));
                            save_settings();
                        }
                    />
                    <span class="settings-hint">"MITM proxy address"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Status"</label>
                    <div class="settings-status">
                        <span class="status-dot" class:green=move || proxy_status.get().map(|s| s.running).unwrap_or(false) class:amber=move || proxy_status.get().map(|s| s.state == "starting").unwrap_or(true) class:red=move || proxy_status.get().map(|s| !s.running && s.state != "starting").unwrap_or(false)></span>
                        <span style:color=move || {
                            match proxy_status.get() {
                                Some(s) if s.running => "var(--green)",
                                Some(s) if s.state == "starting" => "var(--amber)",
                                Some(_) => "var(--red)",
                                None => "var(--amber)",
                            }
                        }>{move || proxy_status.get().map(|s| s.message).unwrap_or_else(|| "Starting".to_string())}</span>
                    </div>
                </div>
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

                <div class="settings-row column">
                    <label class="settings-label">"Assets folder"</label>
                    <div class="settings-row-inline">
                        <span class="settings-value" style="flex: 1; word-break: break-all;">
                            {move || assets_root.get()}
                        </span>
                        <button class="btn-action" on:click=pick_folder disabled=move || saving.get()>
                            {move || if saving.get() { "Loading..." } else { "\u{1f4c2} Browse" }}
                        </button>
                    </div>
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
                    <button class="btn-action" on:click=move |_| {
                        spawn_local(async {
                            invoke::invoke_reload_catalog().await;
                        });
                    }>"\u{1f504} Reload Catalog"</button>
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
