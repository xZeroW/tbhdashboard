use crate::components;
use crate::invoke;
use leptos::prelude::*;
use leptos::task::spawn_local;

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
                        for lc in c.to_lowercase() {
                            s.push(lc);
                        }
                    }
                    s
                }
            }
        }
    }
}

pub fn rarity_diamond(_rarity: &str) -> &'static str {
    "\u{2666}"
}

fn rarity_rank(rarity: &str) -> usize {
    match rarity {
        "COMMON" => 1,
        "UNCOMMON" => 2,
        "RARE" => 3,
        "EPIC" => 4,
        "LEGENDARY" => 5,
        "IMMORTAL" => 6,
        "ARCANA" => 7,
        "BEYOND" => 8,
        "CELESTIAL" => 9,
        "DIVINE" => 10,
        "COSMIC" => 11,
        _ => 0,
    }
}

fn format_remaining(seconds: f64) -> String {
    if seconds <= 0.0 {
        return "--".to_string();
    }

    let secs = seconds.ceil() as i64;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn chest_emoji(box_label: &str) -> &'static str {
    if box_label.contains("Stage") {
        "\u{1f3f0}\u{fe0f}"
    } else if box_label.contains("Boss") {
        "\u{1f409}"
    } else {
        "\u{1f4e6}"
    }
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
    TAB_DEFS
        .iter()
        .find(|(t, _, _)| *t == *tab)
        .map(|(_, l, _)| *l)
        .unwrap_or("")
}

fn cs(classes: &str) -> String {
    classes.to_string()
}

#[component]
pub fn App() -> impl IntoView {
    let (active_tab, set_active_tab) = signal(Tab::ChestQueue);
    let (tick, set_tick) = signal(0u32);
    let (proxy_status, set_proxy_status) = signal(None::<invoke::ProxyStatus>);
    let (chest_rows, set_chest_rows) = signal(Vec::<invoke::ChestRow>::new());
    let (launching_game, set_launching_game) = signal(false);
    let (launch_status, set_launch_status) = signal(None::<invoke::LaunchGameResult>);

    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(500).await;
            set_tick.update(|n| *n += 1);
        }
    });

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            set_proxy_status.set(invoke::invoke_get_proxy_status().await);
        });
    });

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            set_chest_rows.set(invoke::invoke_get_chest_rows(false).await);
        });
    });

    let _view_title = move || tab_title(&active_tab.get());
    let common_chests = move || {
        chest_rows
            .get()
            .iter()
            .filter(|row| row.box_label.contains("Common"))
            .count()
    };
    let stage_chests = move || {
        chest_rows
            .get()
            .iter()
            .filter(|row| row.box_label.contains("Stage"))
            .count()
    };
    let claimable_chests = move || {
        chest_rows
            .get()
            .iter()
            .filter(|row| row.remaining <= 0.0)
            .count()
    };
    let next_unlock = move || {
        chest_rows
            .get()
            .iter()
            .filter(|row| row.remaining > 0.0)
            .map(|row| row.remaining)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(format_remaining)
            .unwrap_or_else(|| "--:--:--".to_string())
    };
    let next_unlock_sub = move || {
        chest_rows
            .get()
            .iter()
            .filter(|row| row.remaining > 0.0)
            .min_by(|a, b| {
                a.remaining
                    .partial_cmp(&b.remaining)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|row| row.box_label.clone())
            .unwrap_or_else(|| "No waiting chest".to_string())
    };
    let best_rarity = move || {
        chest_rows
            .get()
            .iter()
            .max_by_key(|row| rarity_rank(&row.rarity))
            .map(|row| rarity_title(&row.rarity))
            .unwrap_or_else(|| "--".to_string())
    };
    let best_rarity_color = move || {
        chest_rows
            .get()
            .iter()
            .max_by_key(|row| rarity_rank(&row.rarity))
            .map(|row| rarity_color(&row.rarity))
            .unwrap_or("var(--red)")
    };
    let best_rarity_sub = move || {
        chest_rows
            .get()
            .iter()
            .max_by_key(|row| rarity_rank(&row.rarity))
            .map(|row| row.name.clone())
            .unwrap_or_else(|| "In Queue".to_string())
    };

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
                        <span class="status-dot" class:green=move || proxy_status.get().map(|s| s.running).unwrap_or(false) class:amber=move || proxy_status.get().map(|s| s.state == "starting").unwrap_or(true) class:red=move || proxy_status.get().map(|s| !s.running && s.state != "starting").unwrap_or(false)></span>
                        <span class="header-val" style:color=move || {
                            match proxy_status.get() {
                                Some(s) if s.running => "var(--green)",
                                Some(s) if s.state == "starting" => "var(--amber)",
                                Some(_) => "var(--red)",
                                None => "var(--amber)",
                            }
                        }>{move || proxy_status.get().map(|s| s.message).unwrap_or_else(|| "Starting".to_string())}</span>
                    </div>
                    <div class="header-status">
                        <span class="header-label">"Game Data:"</span>
                        <span class="header-val">"OK"</span>
                    </div>
                    <div class="header-status">
                        <span class="header-label">"Last Updated:"</span>
                        <span class="header-val amber">"--:--:--"</span>
                    </div>
                    <div class="header-status" style:display=move || if launch_status.get().is_some() { "flex" } else { "none" }>
                        <span class="header-label">"Game:"</span>
                        <span class="header-val" style:color=move || {
                            match launch_status.get() {
                                Some(result) if result.ok => "var(--green)",
                                Some(_) => "var(--red)",
                                None => "var(--text-dim)",
                            }
                        }>{move || launch_status.get().map(|result| result.message).unwrap_or_default()}</span>
                    </div>
                    <div class="header-icon disabled" title="Auto-refresh is enabled" style="font-size: 24px;">"\u{21bb}"</div>
                    <div class="header-icon" title="Settings" style="cursor: pointer;" on:click=move |_| set_active_tab.set(Tab::Settings)>"\u{1f527}"</div>
                    <button class="header-icon play-button" title="Run Game" disabled=move || launching_game.get() on:click=move |_| {
                        if launching_game.get() {
                            return;
                        }
                        set_launching_game.set(true);
                        set_launch_status.set(None);
                        spawn_local(async move {
                            let result = invoke::invoke_launch_game().await;
                            set_launch_status.set(Some(result));
                            set_launching_game.set(false);
                        });
                    }>
                        {move || if launching_game.get() { "..." } else { "\u{25b6}" }}
                    </button>
                </div>

                <div class="stat-cards">
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f4e6}"</div>
                        <div class="card-info">
                            <div class="card-label">"COMMON CHESTS"</div>
                            <div class="card-value" style="color: var(--text)">{move || common_chests().to_string()}</div>
                            <div class=cs("card-sub dim")>"In Queue"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f3f0}\u{fe0f}"</div>
                        <div class="card-info">
                            <div class="card-label">"STAGE CHESTS"</div>
                            <div class="card-value" style="color: var(--text)">{move || stage_chests().to_string()}</div>
                            <div class=cs("card-sub dim")>"In Queue"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{2705}"</div>
                        <div class="card-info">
                            <div class="card-label">"CLAIMABLE"</div>
                            <div class="card-value" style="color: var(--green)">{move || claimable_chests().to_string()}</div>
                            <div class="card-sub">"Claim now!"</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{23f3}"</div>
                        <div class="card-info">
                            <div class="card-label">"NEXT UNLOCK"</div>
                            <div class="card-value" style="color: var(--purple)">{next_unlock}</div>
                            <div class=cs("card-sub dim")>{next_unlock_sub}</div>
                        </div>
                    </div>
                    <div class="stat-card">
                        <div class="card-icon">"\u{1f451}"</div>
                        <div class="card-info">
                            <div class="card-label">"BEST RARITY"</div>
                            <div class="card-value" style:color=best_rarity_color>{best_rarity}</div>
                            <div class=cs("card-sub dim")>{best_rarity_sub}</div>
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
