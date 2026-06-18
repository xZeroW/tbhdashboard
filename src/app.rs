use leptos::prelude::*;
use leptos::task::spawn_local;
use crate::components;

pub fn rarity_class(rarity: &str) -> &'static str {
    match rarity {
        "COMMON" => "rarity-common",
        "UNCOMMON" => "rarity-uncommon",
        "RARE" => "rarity-rare",
        "EPIC" => "rarity-epic",
        "LEGENDARY" => "rarity-legendary",
        "IMMORTAL" => "rarity-immortal",
        "ARCANA" => "rarity-arcana",
        "BEYOND" => "rarity-beyond",
        "CELESTIAL" => "rarity-celestial",
        "DIVINE" => "rarity-divine",
        "COSMIC" => "rarity-cosmic",
        _ => "",
    }
}

pub fn rarity_color(rarity: &str) -> &'static str {
    match rarity {
        "COMMON" => "#9ca3af",
        "UNCOMMON" => "#22c55e",
        "RARE" => "#3b82f6",
        "EPIC" => "#a855f7",
        "LEGENDARY" => "#f59e0b",
        "IMMORTAL" => "#ef4444",
        "ARCANA" => "#ec4899",
        "BEYOND" => "#06b6d4",
        "CELESTIAL" => "#22d3ee",
        "DIVINE" => "#ffffff",
        "COSMIC" => "#f43f5e",
        _ => "#e2e8f0",
    }
}

pub fn rarity_title(rarity: &str) -> String {
    match rarity {
        "COMMON" => "Common".into(),
        "UNCOMMON" => "Uncommon".into(),
        "RARE" => "Rare".into(),
        "EPIC" => "Epic".into(),
        "LEGENDARY" => "Legendary".into(),
        "IMMORTAL" => "Immortal".into(),
        "ARCANA" => "Arcana".into(),
        "BEYOND" => "Beyond".into(),
        "CELESTIAL" => "Celestial".into(),
        "DIVINE" => "Divine".into(),
        "COSMIC" => "Cosmic".into(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper = first.to_uppercase();
                    let mut s: String = upper.collect();
                    for c in chars {
                        for lc in c.to_lowercase() { s.push(lc); }
                    }
                    s
                }
            }
        }
    }
}

pub fn rarity_diamond(_rarity: &str) -> &'static str { "\u{2666}" }

pub fn chest_emoji(box_label: &str) -> &'static str {
    if box_label.contains("Stage") { "\u{1f3f0}\u{fe0f}" }
    else if box_label.contains("Boss") { "\u{1f409}" }
    else { "\u{1f4e6}" }
}

pub fn reward_emoji(rarity: &str) -> &'static str {
    match rarity {
        "COMMON" => "\u{1f7e3}",
        "UNCOMMON" => "\u{2705}",
        "RARE" => "\u{1f48e}",
        "EPIC" => "\u{1f7e4}",
        "LEGENDARY" => "\u{2b50}",
        "IMMORTAL" => "\u{2764}\u{fe0f}",
        "ARCANA" => "\u{1f480}",
        "BEYOND" => "\u{26a1}",
        "CELESTIAL" => "\u{2728}",
        "DIVINE" => "\u{1f451}",
        "COSMIC" => "\u{1f525}",
        _ => "\u{2753}",
    }
}

#[derive(Clone, PartialEq)]
pub enum Tab {
    ChestQueue,
    RerollPreview,
    BossDrop,
    FarmRanking,
    Events,
    Settings,
}

const TAB_DEFS: &[(Tab, &str, &str)] = &[
    (Tab::ChestQueue, "Queue", "\u{1f4e6}"),
    (Tab::RerollPreview, "Reroll", "\u{1f3b2}"),
    (Tab::BossDrop, "Boss", "\u{1f479}"),
    (Tab::FarmRanking, "Farming", "\u{1f33e}"),
    (Tab::Events, "Events", "\u{1f4dc}"),
    (Tab::Settings, "Settings", "\u{1f527}"),
];

fn tab_title(tab: &Tab) -> &'static str {
    TAB_DEFS.iter().find(|(t, _, _)| *t == *tab).map(|(_, l, _)| *l).unwrap_or("")
}

fn cs(classes: &str) -> String { classes.to_string() }

#[component]
pub fn App() -> impl IntoView {
    let (active_tab, set_active_tab) = signal(Tab::ChestQueue);
    let (tick, set_tick) = signal(0u32);

    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(500).await;
            set_tick.update(|n| *n += 1);
        }
    });

    let _view_title = move || tab_title(&active_tab.get());

    view! {
        <div class="app-root">
            <aside class="sidebar">
                <div class="sidebar-logo">
                    <img class="logo-img" src="logo.png" alt="TaskBarHero"/>
                    <div class="logo-title">"TASKBAR"</div>
                    <div class="logo-title">"HERO"</div>
                    <div class="logo-subtitle">"\u{2014} DASHBOARD \u{2014}"</div>
                </div>
                <nav>
                    {TAB_DEFS.iter().map(|(tab, label, icon)| {
                        let t1 = tab.clone();
                        let t2 = tab.clone();
                        let l = label.to_string();
                        let ic = icon.to_string();
                        let is_active = move || active_tab.get() == t1;
                        view! {
                            <button
                                class:active=is_active
                                on:click=move |_| set_active_tab.set(t2.clone())
                            >
                                <span class="nav-icon">{ic}</span>
                                {l}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </nav>
                <div class="sidebar-spacer"></div>
                <div class="sidebar-version">{concat!("v", env!("CARGO_PKG_VERSION"))}</div>
            </aside>

            <div class="main-area">
                <div class="header-bar">
                    <div class="header-status">
                        <span class="header-label">"Proxy:"</span>
                        <span class=cs("status-dot green")></span>
                        <span class="header-val">"Running"</span>
                    </div>
                    <div class="header-status">
                        <span class="header-label">"Game Data:"</span>
                        <span class="header-val">"OK"</span>
                    </div>
                    <div class="header-status">
                        <span class="header-label">"Last Updated:"</span>
                        <span class="header-val amber">"--:--:--"</span>
                    </div>
                    <div class="header-icon" title="Refresh" style="font-size: 24px;">"\u{21bb}"</div>
                    <div class="header-icon" title="Settings" style="cursor: pointer;" on:click=move |_| set_active_tab.set(Tab::Settings)>"\u{1f527}"</div>
                </div>

                <div class="stat-cards">
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f4e6}"</div>
                        <div class="card-info">
                            <div class="card-label">"COMMON CHESTS"</div>
                            <div class="card-value" style="color: var(--text)">"--"</div>
                            <div class=cs("card-sub dim")>"In Queue"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f3f0}\u{fe0f}"</div>
                        <div class="card-info">
                            <div class="card-label">"STAGE CHESTS"</div>
                            <div class="card-value" style="color: var(--text)">"--"</div>
                            <div class=cs("card-sub dim")>"In Queue"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{2705}"</div>
                        <div class="card-info">
                            <div class="card-label">"CLAIMABLE"</div>
                            <div class="card-value" style="color: var(--green)">"--"</div>
                            <div class="card-sub">"Claim now!"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{23f3}"</div>
                        <div class="card-info">
                            <div class="card-label">"NEXT UNLOCK"</div>
                            <div class="card-value" style="color: var(--purple)">"--:--:--"</div>
                            <div class=cs("card-sub dim")>"No waiting chest"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f451}"</div>
                        <div class="card-info">
                            <div class="card-label">"BEST RARITY"</div>
                            <div class="card-value" style="color: var(--red)">"--"</div>
                            <div class=cs("card-sub dim")>"In Queue"</div>
                        </div>
                    </div>
                </div>

                <div class="content-area">
                    <div style:display={move || if active_tab.get() == Tab::ChestQueue { "block" } else { "none" }}>
                        <components::chest_queue::ChestQueue tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::RerollPreview { "block" } else { "none" }}>
                        <components::reroll::RerollPreview tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::BossDrop { "block" } else { "none" }}>
                        <components::boss_drop::BossDrop tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::FarmRanking { "block" } else { "none" }}>
                        <components::farm_ranking::FarmRanking tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::Events { "block" } else { "none" }}>
                        <components::events::EventLog tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::Settings { "block" } else { "none" }}>
                        <components::settings::Settings tick/>
                    </div>
                </div>
            </div>
        </div>
    }
}
