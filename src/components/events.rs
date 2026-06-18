use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn EventLog(tick: ReadSignal<u32>) -> impl IntoView {
    let (events, set_events) = signal(Vec::<invoke::StateEvent>::new());

    let fetch_data = move || {
        spawn_local(async move {
            let data = invoke::invoke_get_events().await;
            set_events.set(data);
        });
    };

    Effect::new(move |_| {
        tick.get();
        fetch_data();
    });

    view! {
        <div class="panel-header">
            <div class="panel-title">"EVENTS"</div>
        </div>

        <div class="events-list">
            {move || events.get().into_iter().rev().map(|ev| {
                view! {
                    <div class="event-line">
                        <span class="event-time">{ev.at}{"  "}</span>
                        {ev.text}
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}
