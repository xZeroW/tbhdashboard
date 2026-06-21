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
    FarmRanking,
    Requests,
    Settings,
}

const TAB_DEFS: &[(Tab, &str, &str, bool)] = &[
    (Tab::ChestQueue, "Queue", "\u{1f4e6}", false),
    (Tab::RerollPreview, "Reroll", "\u{1f3b2}", true),
    (Tab::FarmRanking, "Farming", "\u{1f33e}", false),
    (Tab::Requests, "Requests", "\u{1f50e}", false),
    (Tab::Settings, "Settings", "\u{1f527}", false),
];

fn tab_title(tab: &Tab) -> &'static str {
    TAB_DEFS
        .iter()
        .find(|(t, _, _, _)| *t == *tab)
        .map(|(_, l, _, _)| *l)
        .unwrap_or("")
}

fn cs(classes: &str) -> String {
    classes.to_string()
}

#[component]
pub fn App() -> impl IntoView {
    let (checking_session, set_checking_session) = signal(true);
    let (current_user, set_current_user) = signal(None::<invoke::AuthUser>);

    Effect::new(move |_| {
        spawn_local(async move {
            set_current_user.set(invoke::invoke_get_current_user().await);
            set_checking_session.set(false);
        });
    });

    let on_login = move |user: invoke::AuthUser| {
        set_current_user.set(Some(user));
    };

    let on_logout = move |_| {
        spawn_local(async move {
            let _ = invoke::invoke_logout().await;
            set_current_user.set(None);
        });
    };

    view! {
        <Show
            when=move || !checking_session.get()
            fallback=move || view! { <AuthSplash message="Checking session..."/> }
        >
            <Show
                when=move || current_user.get().is_some()
                fallback=move || view! { <LoginScreen on_login/> }
            >
                <Dashboard user=Signal::derive(move || current_user.get().unwrap_or_default()) on_logout/>
            </Show>
        </Show>
    }
}

#[component]
fn AuthSplash(message: &'static str) -> impl IntoView {
    view! {
        <div class="login-root">
            <div class="login-card compact">
                <img class="login-logo" src="logo.png" alt="TaskBarHero"/>
                <div class="login-title">"TASKBAR HERO"</div>
                <div class="login-subtitle">"DASHBOARD"</div>
                <div class="login-message muted">{message}</div>
            </div>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    SignIn,
    CreateAccount,
}

#[component]
fn LoginScreen<F>(on_login: F) -> impl IntoView
where
    F: Fn(invoke::AuthUser) + Copy + 'static,
{
    let (auth_mode, set_auth_mode) = signal(AuthMode::SignIn);
    let (server_url, set_server_url) = signal("http://127.0.0.1:3000".to_string());
    let (username, set_username) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (logging_in, set_logging_in) = signal(false);
    let (message, set_message) = signal(String::new());
    let (message_is_error, set_message_is_error) = signal(false);

    Effect::new(move |_| {
        spawn_local(async move {
            let settings = invoke::invoke_get_settings().await;
            set_server_url.set(settings.server_url);
        });
    });

    let submit = move || {
        if logging_in.get() {
            return;
        }

        set_logging_in.set(true);
        set_message.set(String::new());
        set_message_is_error.set(false);
        let mode = auth_mode.get();
        let server = server_url.get();
        let name = username.get();
        let email_address = email.get();
        let pass = password.get();

        spawn_local(async move {
            match mode {
                AuthMode::SignIn => {
                    let result = invoke::invoke_login(&server, &name, &pass).await;
                    if result.ok {
                        set_logging_in.set(false);
                        if let Some(user) = result.user {
                            on_login(user);
                        }
                        return;
                    }

                    if result.status != Some(403) {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message.set(result.message);
                        return;
                    }

                    set_message.set("Account is not active. Looking up checkout...".to_string());
                    let checkout =
                        invoke::invoke_get_inactive_checkout(&server, &name, &pass).await;

                    if !checkout.ok {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        let msg = if checkout.message.is_empty() {
                            "Account is not active. If you just paid, wait a few seconds and try again.".to_string()
                        } else {
                            checkout.message
                        };
                        set_message.set(format!(
                            "{msg}\n\nIf you have not completed payment, switch to \"Create account\" and register again."
                        ));
                        return;
                    }

                    let (user_id, checkout_email, checkout_cfg, resp_username) = match (
                        checkout.user_id,
                        checkout.email,
                        checkout.checkout,
                    ) {
                        (Some(uid), Some(em), Some(co)) => (uid, em, co, checkout.username),
                        _ => {
                            set_logging_in.set(false);
                            set_message_is_error.set(true);
                            set_message.set("Server returned incomplete account details. Try again or contact support.".to_string());
                            return;
                        }
                    };

                    if let Some(registered_username) = resp_username {
                        set_username.set(registered_username);
                    }

                    set_message.set("Opening Paddle checkout...".to_string());
                    let checkout_result = invoke::invoke_open_paddle_checkout(
                        &checkout_cfg,
                        &checkout_email,
                        &user_id,
                    )
                    .await;

                    if !checkout_result.opened {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message.set(
                            checkout_result
                                .message
                                .unwrap_or_else(|| "Failed to open Paddle checkout.".to_string()),
                        );
                        return;
                    }

                    if !checkout_result.completed {
                        set_logging_in.set(false);
                        set_password.set(String::new());
                        set_message_is_error.set(true);
                        set_message.set(
                            "Checkout was not completed. You can try again or sign in after payment."
                                .to_string(),
                        );
                        return;
                    }

                    set_message.set("Verifying payment...".to_string());
                    for attempt in 0..30 {
                        let activation =
                            invoke::invoke_get_activation_status(&server, &user_id).await;
                        if !activation.ok {
                            set_logging_in.set(false);
                            set_password.set(String::new());
                            set_message_is_error.set(false);
                            set_message.set("Payment complete. You can now sign in. If login fails, wait a few seconds for payment processing.".to_string());
                            return;
                        }

                        if activation.active {
                            let login = invoke::invoke_login(&server, &name, &pass).await;
                            if login.ok {
                                if let Some(user) = login.user {
                                    set_logging_in.set(false);
                                    on_login(user);
                                    return;
                                }
                            }

                            if login.status == Some(401) {
                                set_logging_in.set(false);
                                set_password.set(String::new());
                                set_message_is_error.set(true);
                                set_message.set(
                                    "Automatic sign-in failed. Sign in manually with your account."
                                        .to_string(),
                                );
                                return;
                            }

                            if login.status != Some(403) {
                                set_logging_in.set(false);
                                set_password.set(String::new());
                                set_message_is_error.set(true);
                                set_message.set(format!(
                                    "Payment verified, but automatic sign-in failed: {}",
                                    login.message
                                ));
                                return;
                            }
                        }

                        if attempt < 29 {
                            gloo_timers::future::TimeoutFuture::new(2_000).await;
                        }
                    }

                    set_logging_in.set(false);
                    set_password.set(String::new());
                    set_message_is_error.set(false);
                    set_message.set(
                        "Payment received. Verification is still processing. Try signing in in a moment."
                            .to_string(),
                    );
                }
                AuthMode::CreateAccount => {
                    let result =
                        invoke::invoke_register(&server, &name, &email_address, &pass).await;
                    if !result.ok {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        let msg = if result.message.contains("already registered") {
                            "An account with this username or email already exists. Switch to \"Sign in\" to complete payment."
                        } else {
                            &result.message
                        };
                        set_message.set(msg.to_string());
                        return;
                    }

                    let Some(user_id) = result.user_id else {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message
                            .set("Registration response did not include a user ID.".to_string());
                        return;
                    };
                    let Some(checkout_email) = result.email else {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message
                            .set("Registration response did not include an email.".to_string());
                        return;
                    };
                    let Some(checkout) = result.checkout else {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message.set(
                            "Registration response did not include checkout details.".to_string(),
                        );
                        return;
                    };

                    if let Some(registered_username) = result.username {
                        set_username.set(registered_username);
                    }

                    set_message.set("Opening Paddle checkout...".to_string());
                    let checkout_result =
                        invoke::invoke_open_paddle_checkout(&checkout, &checkout_email, &user_id)
                            .await;

                    if !checkout_result.opened {
                        set_logging_in.set(false);
                        set_message_is_error.set(true);
                        set_message.set(
                            checkout_result
                                .message
                                .unwrap_or_else(|| "Failed to open Paddle checkout.".to_string()),
                        );
                        return;
                    }

                    if !checkout_result.completed {
                        set_logging_in.set(false);
                        set_auth_mode.set(AuthMode::SignIn);
                        set_password.set(String::new());
                        set_message_is_error.set(true);
                        set_message.set(
                            "Checkout was not completed. You can try again or sign in after payment."
                                .to_string(),
                        );
                        return;
                    }

                    set_message.set("Verifying payment...".to_string());
                    for attempt in 0..30 {
                        let activation =
                            invoke::invoke_get_activation_status(&server, &user_id).await;
                        if !activation.ok {
                            set_logging_in.set(false);
                            set_auth_mode.set(AuthMode::SignIn);
                            set_password.set(String::new());
                            set_message_is_error.set(false);
                            set_message.set("Payment complete. You can now sign in. If login fails, wait a few seconds for payment processing.".to_string());
                            return;
                        }

                        if activation.active {
                            let login = invoke::invoke_login(&server, &name, &pass).await;
                            if login.ok {
                                if let Some(user) = login.user {
                                    set_logging_in.set(false);
                                    on_login(user);
                                    return;
                                }
                            }

                            if login.status == Some(401) {
                                set_logging_in.set(false);
                                set_auth_mode.set(AuthMode::SignIn);
                                set_password.set(String::new());
                                set_message_is_error.set(true);
                                set_message.set(
                                    "Automatic sign-in failed. Sign in manually with your new account."
                                        .to_string(),
                                );
                                return;
                            }

                            if login.status != Some(403) {
                                set_logging_in.set(false);
                                set_auth_mode.set(AuthMode::SignIn);
                                set_password.set(String::new());
                                set_message_is_error.set(true);
                                set_message.set(format!(
                                    "Payment verified, but automatic sign-in failed: {}",
                                    login.message
                                ));
                                return;
                            }
                        }

                        if attempt < 29 {
                            gloo_timers::future::TimeoutFuture::new(2_000).await;
                        }
                    }

                    set_logging_in.set(false);
                    set_auth_mode.set(AuthMode::SignIn);
                    set_password.set(String::new());
                    set_message_is_error.set(false);
                    set_message.set(
                        "Payment received. Verification is still processing. Try signing in in a moment."
                            .to_string(),
                    );
                }
            }
        });
    };

    view! {
        <div class="login-root">
            <div class="login-card">
                <div class="login-brand">
                    <img class="login-logo" src="logo.png" alt="TaskBarHero"/>
                    <div>
                        <div class="login-title">"TASKBAR HERO"</div>
                        <div class="login-subtitle">"DASHBOARD"</div>
                    </div>
                </div>
                <div class="login-mode-toggle">
                    <button class:active=move || auth_mode.get() == AuthMode::SignIn
                        disabled=move || logging_in.get()
                        on:click=move |_| {
                            set_auth_mode.set(AuthMode::SignIn);
                            set_message.set(String::new());
                            set_message_is_error.set(false);
                        }
                    >"Sign in"</button>
                    <button class:active=move || auth_mode.get() == AuthMode::CreateAccount
                        disabled=move || logging_in.get()
                        on:click=move |_| {
                            set_auth_mode.set(AuthMode::CreateAccount);
                            set_message.set(String::new());
                            set_message_is_error.set(false);
                        }
                    >"Create account"</button>
                </div>
                <div class="login-panel-title">
                    {move || if auth_mode.get() == AuthMode::SignIn { "Sign in" } else { "Create account" }}
                </div>
                <Show when=move || auth_mode.get() == AuthMode::CreateAccount>
                    <div class="login-panel-copy">
                        "Create an account, then complete Paddle checkout to activate it."
                    </div>
                </Show>
                <label class="login-label">"Username"</label>
                <input class="login-input" type="text" prop:value=username
                    autocomplete="username"
                    on:input=move |ev| set_username.set(event_target_value(&ev))
                />

                <Show when=move || auth_mode.get() == AuthMode::CreateAccount>
                    <label class="login-label">"Email"</label>
                    <input class="login-input" type="email" prop:value=email
                        autocomplete="email"
                        on:input=move |ev| set_email.set(event_target_value(&ev))
                    />
                </Show>

                <label class="login-label">"Password"</label>
                <input class="login-input" type="password" prop:value=password
                    autocomplete=move || if auth_mode.get() == AuthMode::SignIn { "current-password" } else { "new-password" }
                    on:input=move |ev| set_password.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            submit();
                        }
                    }
                />

                <button class="login-button" disabled=move || logging_in.get() on:click=move |_| submit()>
                    {move || match (auth_mode.get(), logging_in.get()) {
                        (AuthMode::SignIn, true) => "Signing in...",
                        (AuthMode::SignIn, false) => "Login",
                        (AuthMode::CreateAccount, true) => "Creating account...",
                        (AuthMode::CreateAccount, false) => "Create account",
                    }}
                </button>

                <div class="login-divider">
                    <span>"or"</span>
                </div>

                <button class="login-button secondary" disabled=move || logging_in.get() on:click=move |_| {
                    set_logging_in.set(true);
                    set_message.set(String::new());
                    spawn_local(async move {
                        let user = invoke::invoke_skip_login().await;
                        set_logging_in.set(false);
                        on_login(user);
                    });
                }>
                    {move || if logging_in.get() { "Entering offline mode..." } else { "Offline Mode (skip login)" }}
                </button>

                <div class="login-message"
                    class:error=move || message_is_error.get() && !message.get().is_empty()
                    class:muted=move || !message_is_error.get()
                >
                    {message}
                </div>
            </div>
        </div>
    }
}

#[component]
fn Dashboard(
    user: Signal<invoke::AuthUser>,
    on_logout: impl Fn(()) + Copy + 'static,
) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(Tab::ChestQueue);
    let (tick, set_tick) = signal(0u32);
    let (chest_rows, set_chest_rows) = signal(Vec::<invoke::ChestRow>::new());
    let (launching_game, set_launching_game) = signal(false);
    let (launch_status, set_launch_status) = signal(None::<invoke::LaunchGameResult>);
    let (show_launch_setup, set_show_launch_setup) = signal(false);
    let (steam_launch_options, set_steam_launch_options) = signal(String::new());

    Effect::new(move |_| {
        spawn_local(async move {
            let settings = invoke::invoke_get_settings().await;
            set_steam_launch_options.set(settings.steam_launch_options.clone());
            if !settings.steam_launch_options_prompted
                && !settings.steam_launch_options.trim().is_empty()
            {
                set_show_launch_setup.set(true);
            }
        });
    });

    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(500).await;
            set_tick.update(|n| *n += 1);
        }
    });

    Effect::new(move |_| {
        tick.get();
        spawn_local(async move {
            let rows = invoke::invoke_get_chest_rows(false).await;
            let has_claimable = rows.iter().any(|row| row.remaining <= 0.0 && !row.is_get);
            set_chest_rows.set(rows);
            if has_claimable {
                let _ = invoke::invoke_upload_claimable_reward_observations().await;
            }
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

    let acknowledge_launch_setup = move |_| {
        set_show_launch_setup.set(false);
        spawn_local(async move {
            let mut settings = invoke::invoke_get_settings().await;
            settings.steam_launch_options_prompted = true;
            settings.include_steam_launch_options = true;
            invoke::invoke_set_settings(settings).await;
        });
    };

    view! {
        <div class="app-root">
            <div style:display=move || if show_launch_setup.get() { "flex" } else { "none" }
                style="position: fixed; inset: 0; z-index: 50; align-items: center; justify-content: center; background: rgba(2, 6, 23, 0.78); padding: 24px;">
                <div style="width: min(720px, 100%); border: 1px solid var(--border); background: var(--panel); box-shadow: 0 24px 80px rgba(0,0,0,.45); padding: 24px; border-radius: 14px;">
                    <div class="panel-title" style="margin-bottom: 10px;">"Steam Setup Required"</div>
                    <p style="color: var(--text-dim); line-height: 1.6; margin-bottom: 14px;">
                        "To capture game traffic reliably, set this once in Steam: Task Bar Hero -> Properties -> Launch Options. The dashboard will not edit Steam files automatically."
                    </p>
                    <textarea readonly rows="4" prop:value=move || steam_launch_options.get()
                        style="width: 100%; resize: vertical; background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 8px; padding: 12px; font-family: var(--font-mono); font-size: 13px; margin-bottom: 14px;">
                    </textarea>
                    <p style="color: var(--amber); line-height: 1.6; margin-bottom: 16px;">
                        "After changing Steam Launch Options, restart the game. If Steam was already open and capture does not start, restart Steam once."
                    </p>
                    <div style="display: flex; justify-content: flex-end; gap: 10px;">
                        <button class="btn-action" on:click=acknowledge_launch_setup>"I Set This In Steam"</button>
                    </div>
                </div>
            </div>
            <aside class="sidebar">
                <div class="sidebar-logo">
                    <img class="logo-img" src="logo.png" alt="TaskBarHero"/>
                    <div class="logo-title">"TASKBAR"</div>
                    <div class="logo-title">"HERO"</div>
                    <div class="logo-subtitle">"\u{2014} DASHBOARD \u{2014}"</div>
                </div>
                <nav>
                    {TAB_DEFS.iter().map(|(tab, label, icon, disabled)| {
                        let t1 = tab.clone();
                        let t2 = tab.clone();
                        let l = label.to_string();
                        let ic = icon.to_string();
                        let is_disabled = *disabled;
                        let is_active = move || active_tab.get() == t1;
                        view! {
                            <button
                                class:active=is_active
                                disabled=is_disabled
                                title=if is_disabled { "Coming soon" } else { "" }
                                on:click=move |_| {
                                    if !is_disabled {
                                        set_active_tab.set(t2.clone());
                                    }
                                }
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
                    <div class="header-status">
                        <span class="header-label">"User:"</span>
                        <span class="header-val">{move || user.get().username}</span>
                    </div>
                    <div class="header-icon disabled" title="Auto-refresh is enabled" style="font-size: 24px;">"\u{21bb}"</div>
                    <div class="header-icon" title="Settings" style="cursor: pointer;" on:click=move |_| set_active_tab.set(Tab::Settings)>"\u{1f527}"</div>
                    <button class="header-icon logout-button" title="Logout" on:click=move |_| on_logout(())>"\u{23fb}"</button>
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
                    <div style:display={move || if active_tab.get() == Tab::FarmRanking { "block" } else { "none" }}>
                        <components::farm_ranking::FarmRanking tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::Requests { "block" } else { "none" }}>
                        <components::requests::RequestHistory tick/>
                    </div>
                    <div style:display={move || if active_tab.get() == Tab::Settings { "block" } else { "none" }}>
                        <components::settings::Settings tick/>
                    </div>
                </div>
            </div>
        </div>
    }
}
