use crate::invoke;
use js_sys::Date;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsValue;

fn method_pill_class(method: &str) -> &'static str {
    match method {
        "GET" => "pill green",
        "POST" => "pill amber",
        _ => "pill gray",
    }
}

fn pretty_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty body>".to_string();
    }

    serde_json::from_str::<serde_json::Value>(trimmed)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_string())
}

fn local_time(at: &str) -> String {
    let millis = Date::parse(at);
    if millis.is_nan() {
        return at.to_string();
    }

    let date = Date::new(&JsValue::from_f64(millis));
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}

#[component]
pub fn RequestHistory(tick: ReadSignal<u32>) -> impl IntoView {
    let (requests, set_requests) = signal(Vec::<invoke::RequestLogEntry>::new());
    let (selected, set_selected) = signal(None::<invoke::RequestLogEntry>);

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            let history = invoke::invoke_get_request_history().await;
            set_requests.set(history);
        });
    });

    let clear_history = move |_| {
        spawn_local(async move {
            if invoke::invoke_clear_request_history().await {
                set_requests.set(Vec::new());
                set_selected.set(None);
            }
        });
    };

    view! {
        <div class="panel-header">
            <div>
                <div class="panel-title">"REQUEST HISTORY"</div>
                <div style="color: var(--text-dim); font-size: 13px; margin-top: 4px;">
                    "Recent captured thebackend.io requests with request bodies. Newest first."
                </div>
            </div>
            <button class="btn-action" on:click=clear_history>"Clear"</button>
        </div>

        <div class="table-panel">
            <table class="data-table">
                <thead>
                    <tr>
                        <th style="width: 165px">"Time"</th>
                        <th style="width: 90px">"Method"</th>
                        <th>"Path"</th>
                        <th style="width: 120px">"Body"</th>
                    </tr>
                </thead>
                <tbody>
                    <Show
                        when=move || !requests.get().is_empty()
                        fallback=move || view! {
                            <tr><td colspan="4" style="color: var(--text-dim);">"No captured requests yet."</td></tr>
                        }
                    >
                        <For
                            each=move || requests.get()
                            key=|req| format!("{}:{}", req.at, req.source)
                            let(req)
                        >
                            <tr style="cursor: pointer;" on:click=move |_| set_selected.set(Some(req.clone()))>
                                <td style="color: var(--text-dim); font-family: var(--font-mono); font-size: 12px;">{local_time(&req.at)}</td>
                                <td><span class=method_pill_class(&req.method)>{req.method.clone()}</span></td>
                                <td style="font-family: var(--font-mono); font-size: 12px; overflow-wrap: anywhere;">{req.path.clone()}</td>
                                <td>{format!("{} bytes", req.body_bytes)}</td>
                            </tr>
                        </For>
                    </Show>
                </tbody>
            </table>
        </div>

        <Show when=move || selected.get().is_some()>
            {move || selected.get().map(|req| {
                let formatted_body = pretty_body(&req.body);
                view! {
                <div class="table-panel" style="margin-top: 16px; padding: 16px;">
                    <div class="panel-head">
                        <div>
                            <div class="panel-title" style="font-size: 16px;">{req.source.clone()}</div>
                            <div style="color: var(--text-dim); font-family: var(--font-mono); font-size: 12px; margin-top: 4px; overflow-wrap: anywhere;">
                                {format!("{} | {} | {}", local_time(&req.at), req.content_type, req.host)}
                            </div>
                        </div>
                    </div>
                    <pre style="margin-top: 14px; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 420px; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; padding: 12px; color: var(--text); font-family: var(--font-mono); font-size: 12px; line-height: 1.5;">{formatted_body}</pre>
                </div>
            }})}
        </Show>
    }
}
