use shared::UserSummary;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

const AVATAR_COLORS: [&str; 4] = ["#c05f38", "#6f8256", "#9b6a7d", "#4f7a86"];

#[function_component(UsersView)]
pub fn users_view() -> Html {
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    let busy = use_state(|| true);
    let users = use_state(Vec::<UserSummary>::new);
    let error = use_state(String::new);

    // Load the list once on mount.
    if *busy {
        let busy = busy.clone();
        let users = users.clone();
        let error = error.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api::fetch_users().await {
                Ok(list) => {
                    users.set(list);
                    busy.set(false);
                }
                Err(msg) => {
                    error.set(msg);
                    busy.set(false);
                }
            }
        });
    }

    let go_back = Callback::from(move |_| navigator.push(&Route::Admin));
    let count = users.len();
    let header = if users.is_empty() {
        "No accounts yet.".to_owned()
    } else {
        format!("{} account{}", count, if count == 1 { "" } else { "s" })
    };

    html! {
        <div class="screen sb-scroll">
            <div style="display:flex;align-items:center;gap:10px;margin-bottom:6px">
                <button class="alt-auth-btn" onclick={go_back}>
                    <span class="mi">{"arrow_back"}</span>{"Back"}
                </button>
                <div class="screen-title" style="font-size:24px">{"Users"}</div>
            </div>

            if !error.is_empty() {
                <div class="auth-note" style="color:var(--danger);margin-bottom:12px">{(*error).clone()}</div>
            }

            if *busy {
                <div class="auth-note">{"Loading…"}</div>
            } else {
                <div class="screen-sub" style="margin-bottom:14px">{header}</div>

                if users.is_empty() {
                    <div class="auth-note">{"No accounts yet."}</div>
                } else {
                    <div style="margin-top:4px">
                        { for users.iter().enumerate().map(|(i, u)| {
                            let color = AVATAR_COLORS[i % AVATAR_COLORS.len()];
                            let initial = u.username.chars().next().unwrap_or('S').to_ascii_uppercase();
                            html! {
                                <div style="display:flex;align-items:center;gap:12px;padding:13px 14px;border-bottom:1px solid var(--border)">
                                    <div
                                        class="avatar"
                                        style={format!("background:{color}")}
                                    >{initial}</div>
                                    <div style="flex:1;min-width:0">
                                        <div class="friend-name">{&u.username}</div>
                                        <div class="friend-sub">
                                            {&u.created_at[..10]}
                                            if u.is_admin {
                                                <span style="margin-left:8px;color:var(--accent);font-weight:700;font-size:11px">{"admin"}</span>
                                            }
                                        </div>
                                    </div>
                                </div>
                            }
                        }) }
                    </div>
                }
            }
        </div>
    }
}
