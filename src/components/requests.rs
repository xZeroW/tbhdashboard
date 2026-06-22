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

fn decode_nested_result(v: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = v {
        if let Some(serde_json::Value::String(s)) = map.get("result") {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                map.insert("result".to_string(), parsed);
            }
        }
        for val in map.values_mut() {
            decode_nested_result(val);
        }
    } else if let serde_json::Value::Array(arr) = v {
        for val in arr.iter_mut() {
            decode_nested_result(val);
        }
    }
}

fn pretty_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(mut value) => {
            decode_nested_result(&mut value);
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string())
        }
        Err(_) => body.to_string(),
    }
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
    let (selected_key, set_selected_key) = signal(None::<String>);

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            let history = invoke::invoke_get_request_history().await;
            set_requests.set(history);
        });
    });

    let selected = move || {
        let key = selected_key.get()?;
        requests
            .get()
            .into_iter()
            .find(|r| format!("{}:{}", r.at, r.source) == key)
    };

    let clear_history = move |_| {
        spawn_local(async move {
            if invoke::invoke_clear_request_history().await {
                set_requests.set(Vec::new());
                set_selected_key.set(None);
            }
        });
    };

    view! {
        <div class="panel-header">
            <div>
                <div class="panel-title">"REQUEST HISTORY"</div>
                <div style="color: var(--text-dim); font-size: 13px; margin-top: 4px;">
                    "Captured thebackend.io requests and responses."
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
                            <tr style="cursor: pointer;" on:click=move |_| set_selected_key.set(Some(format!("{}:{}", req.at, req.source)))>
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

        <Show when=move || selected().is_some()>
            {move || selected().map(|req| {
                let has_req = !req.body.trim().is_empty();
                let has_res = !req.response_body.trim().is_empty();
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

                    <Show when=move || has_req>
                        <div style="margin-top: 14px; font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px;">"Request"</div>
                        <pre style="margin-top: 6px; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 320px; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; padding: 12px; color: var(--text); font-family: var(--font-mono); font-size: 12px; line-height: 1.5;">{pretty_body(&req.body)}</pre>
                    </Show>

                    <Show when=move || has_res>
                        <div style="margin-top: 16px; font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px;">"Response ({})" {format!("{} bytes", req.response_body_bytes)}</div>
                        <pre style="margin-top: 6px; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 420px; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; padding: 12px; color: var(--text); font-family: var(--font-mono); font-size: 12px; line-height: 1.5;">{pretty_body(&req.response_body)}</pre>
                    </Show>
                </div>
            }})}
        </Show>
    }
}
