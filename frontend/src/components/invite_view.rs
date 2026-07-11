use shared::AuthSession;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::route::Route;
use crate::{api, glue};

#[derive(Properties, PartialEq)]
pub struct InviteViewProps {
    pub auth: Option<AuthSession>,
    pub on_toast: Callback<String>,
}

/// Loads this account's current unredeemed invite, minting a fresh one if
/// none exists yet.
async fn load_or_create_code() -> Result<String, String> {
    let existing =
        api::list_invites().await?.into_iter().find(|i| !i.redeemed).map(|i| i.code);
    match existing {
        Some(code) => Ok(code),
        None => api::create_invite().await.map(|i| i.code),
    }
}

#[function_component(InviteView)]
pub fn invite_view(props: &InviteViewProps) -> Html {
    let code = use_state(|| None::<String>);
    let busy = use_state(|| false);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    {
        let code = code.clone();
        let busy = busy.clone();
        let on_toast = props.on_toast.clone();
        let signed_in = props.auth.is_some();
        use_effect_with(signed_in, move |signed_in| {
            if *signed_in {
                busy.set(true);
                let code = code.clone();
                let busy = busy.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match load_or_create_code().await {
                        Ok(c) => code.set(Some(c)),
                        Err(msg) => on_toast.emit(msg),
                    }
                    busy.set(false);
                });
            } else {
                code.set(None);
            }
            || ()
        });
    }

    let copy_code = {
        let code = (*code).clone();
        let toast = props.on_toast.clone();
        Callback::from(move |_| {
            if let Some(code) = &code {
                glue::sb_copy_text(code);
                toast.emit("Invite code copied".to_owned());
            }
        })
    };

    let share = {
        let code = (*code).clone();
        let toast = props.on_toast.clone();
        Callback::from(move |_| {
            let Some(code) = code.clone() else { return };
            let text =
                format!("Join me on Service Break — use code {code} to find clean bathrooms nearby.");
            let shared = glue::sb_share_invite(&text, &code);
            let msg = if shared { "Opening share…" } else { "Invite link copied" };
            toast.emit(msg.to_owned());
        })
    };

    let new_code = {
        let code = code.clone();
        let busy = busy.clone();
        let toast = props.on_toast.clone();
        Callback::from(move |_| {
            if *busy {
                return;
            }
            busy.set(true);
            let code = code.clone();
            let busy = busy.clone();
            let toast = toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::create_invite().await {
                    Ok(inv) => code.set(Some(inv.code)),
                    Err(msg) => toast.emit(msg),
                }
                busy.set(false);
            });
        })
    };

    let go_to_account = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Account))
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Invite"}</div>
            <div class="screen-sub" style="margin-bottom:18px">
                {"Every scout makes the map better."}
            </div>
            <div class="invite-card">
                <div class="invite-badge"><span class="mi">{"group_add"}</span></div>
                <div class="invite-h1">{"Invite your fellow travelers"}</div>
                if props.auth.is_some() {
                    <div class="invite-p">
                        {"Share your code and help friends find a clean place to stop. \
                          Registration is invite-only, so this is the only way in."}
                    </div>
                    <div class="invite-code-row">
                        <div>
                            <div class="invite-code-label">{"Your code"}</div>
                            <div class="invite-code">
                                {(*code).clone().unwrap_or_else(|| "…".to_owned())}
                            </div>
                        </div>
                        <button class="invite-copy-btn" onclick={copy_code} disabled={code.is_none()}>
                            {"Copy"}
                        </button>
                    </div>
                    <button class="invite-share-btn" onclick={share} disabled={code.is_none()}>
                        <span class="mi">{"ios_share"}</span>{"Share invite"}
                    </button>
                    <button class="alt-auth-btn" onclick={new_code} disabled={*busy}>
                        <span class="mi">{"refresh"}</span>{"Generate a new code"}
                    </button>
                } else {
                    <div class="invite-p">
                        {"Sign in to get your invite code — registration is invite-only, \
                          so friends need a code from you to join."}
                    </div>
                    <button class="alt-auth-btn" onclick={go_to_account}>
                        <span class="mi">{"login"}</span>{"Go to Account"}
                    </button>
                }
            </div>
        </div>
    }
}
