use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::invoke;

#[component]
pub fn Settings(tick: ReadSignal<u32>) -> impl IntoView {
    let (catalog, set_catalog) = signal(None::<invoke::CatalogStatus>);

    let fetch_catalog = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_catalog_status().await;
            set_catalog.set(data);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_catalog();
    });

    let (proxy_url, set_proxy_url) = signal("http://127.0.0.1:8080".to_string());
    let (refresh_ms, set_refresh_ms) = signal("500".to_string());
    let (_log_level, set_log_level) = signal("info".to_string());

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
                        on:input=move |ev| { set_refresh_ms.set(event_target_value(&ev)); }
                    />
                    <span class="settings-hint">"How often tables refresh (100-5000)"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Log level"</label>
                    <select on:change=move |ev| {
                        set_log_level.set(event_target_value(&ev));
                    }>
                        <option value="error">"Error"</option>
                        <option value="warn">"Warn"</option>
                        <option value="info" selected>"Info"</option>
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
                        on:input=move |ev| { set_proxy_url.set(event_target_value(&ev)); }
                    />
                    <span class="settings-hint">"MITM proxy address"</span>
                </div>

                <div class="settings-row">
                    <label class="settings-label">"Status"</label>
                    <div class="settings-status">
                        <span class="status-dot green"></span>
                        <span>"Connected"</span>
                    </div>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"\u{1f4da} Catalog"</div>

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
