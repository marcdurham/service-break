use shared::UserSummary;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

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
                    <table style="width:100%;border-collapse:collapse;font-size:14px">
                        <thead>
                            <tr style="text-align:left;border-bottom:2px solid var(--line)">
                                <th style="padding:6px 4px;color:var(--muted);font-weight:500">{"Username"}</th>
                                <th style="padding:6px 4px;color:var(--muted);font-weight:500;width:70px">{"Role"}</th>
                                <th style="padding:6px 4px;color:var(--muted);font-weight:500;width:130px">{"Joined"}</th>
                            </tr>
                        </thead>
                        <tbody>
                            { for users.iter().map(|u| html! {
                                <tr style="border-bottom:1px solid var(--line)">
                                    <td style="padding:8px 4px">
                                        <div>{&u.username}</div>
                                        <div style="font-size:11px;color:var(--muted);font-family:monospace">{&u.id[..8]}</div>
                                    </td>
                                    <td style="padding:8px 4px">
                                        if u.is_admin {
                                            <span class="badge" style="background:var(--accent);color:#fff;padding:2px 7px;border-radius:10px;font-size:11px">{"admin"}</span>
                                        } else {
                                            <span style="color:var(--muted);font-size:12px">{"user"}</span>
                                        }
                                    </td>
                                    <td style="padding:8px 4px;color:var(--muted);font-size:12px">{&u.created_at[..10]}</td>
                                </tr>
                            }) }
                        </tbody>
                    </table>
                }
            }
        </div>
    }
}
