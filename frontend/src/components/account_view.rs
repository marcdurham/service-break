use shared::{AuthSession, Credentials, Invitation, InviteStatus, InvitesOverview};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct AccountViewProps {
    pub auth: Option<AuthSession>,
    pub on_login: Callback<AuthSession>,
    pub on_logout: Callback<()>,
    pub on_toast: Callback<String>,
}

/// One row in the friends & invitations list.
fn invite_row(inv: &Invitation) -> Html {
    let title = if !inv.name.is_empty() {
        inv.name.clone()
    } else if let Some(u) = &inv.joined_username {
        u.clone()
    } else {
        inv.code.clone()
    };
    let sub = match &inv.joined_username {
        Some(u) => format!("{} · joined as {u}", inv.code),
        None => inv.code.clone(),
    };
    let chip_class = match inv.status {
        InviteStatus::Pending => "friend-status status-pending",
        InviteStatus::Expired => "friend-status status-expired",
        InviteStatus::Joined => "friend-status status-joined",
    };
    html! {
        <div class="friend-row" key={inv.code.clone()}>
            <div>
                <div class="friend-name">{title}</div>
                <div class="friend-sub">{sub}</div>
            </div>
            <span class={chip_class}>{inv.status.label()}</span>
        </div>
    }
}

/// The Account page: a sign-in form when logged out (with a link to the
/// invite-only "create an account" page), and — when logged in — the
/// account card, an invite button, the user's editable invitation name,
/// and the list of friends and invitations.
#[function_component(AccountView)]
pub fn account_view(props: &AccountViewProps) -> Html {
    let username = use_state(String::new);
    let password = use_state(String::new);
    let busy = use_state(|| false);
    let overview = use_state(|| None::<InvitesOverview>);
    let my_name = use_state(String::new);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    // Load the invitations overview whenever we're signed in.
    {
        let overview = overview.clone();
        let my_name = my_name.clone();
        let signed_in = props.auth.is_some();
        use_effect_with(signed_in, move |signed_in| {
            if *signed_in {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(o) = api::list_invites().await {
                        my_name.set(o.my_invite_name.clone().unwrap_or_default());
                        overview.set(Some(o));
                    }
                });
            } else {
                overview.set(None);
            }
        });
    }

    let log_in = {
        let username = username.clone();
        let password = password.clone();
        let busy = busy.clone();
        let on_login = props.on_login.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let creds = Credentials {
                username: username.trim().to_owned(),
                password: (*password).clone(),
                invite_code: String::new(),
            };
            if creds.username.is_empty() || creds.password.is_empty() {
                on_toast.emit("Enter a username and password".to_owned());
                return;
            }
            if *busy {
                return;
            }
            busy.set(true);
            let password = password.clone();
            let busy = busy.clone();
            let on_login = on_login.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::login(&creds).await {
                    Ok(session) => {
                        password.set(String::new());
                        on_login.emit(session);
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                busy.set(false);
            });
        })
    };

    let go_to_register = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Register))
    };

    let go_to_invite = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Invite))
    };

    let save_name = {
        let overview = overview.clone();
        let my_name = my_name.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let Some(code) =
                overview.as_ref().and_then(|o| o.my_invite_code.clone())
            else {
                return;
            };
            let name = my_name.trim().to_owned();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::rename_invite(&code, &name).await {
                    Ok(()) => on_toast.emit("Name updated".to_owned()),
                    Err(msg) => on_toast.emit(msg),
                }
            });
        })
    };

    let sign_out = {
        let cb = props.on_logout.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    let go_to_admin = Callback::from(move |_| navigator.push(&Route::Admin));

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Account"}</div>
            if let Some(session) = &props.auth {
                <div class="screen-sub" style="margin-bottom:18px">
                    {"You're signed in and ready to scout."}
                </div>
                <div class="account-card">
                    <div class="account-avatar">
                        {session.username.chars().next().unwrap_or('S').to_ascii_uppercase()}
                    </div>
                    <div class="account-name">{&session.username}</div>
                    <div class="account-sub">
                        {"Places, reviews and saves you add are signed with this name."}
                    </div>
                    if session.is_admin {
                        <button class="alt-auth-btn" onclick={go_to_admin}>
                            <span class="mi">{"admin_panel_settings"}</span>{"Admin"}
                        </button>
                    }
                    <button class="signout-btn" onclick={sign_out}>
                        <span class="mi">{"logout"}</span>{"Sign out"}
                    </button>
                </div>

                <button class="submit-btn" onclick={go_to_invite}>
                    <span class="mi">{"group_add"}</span>{"Invite your friends"}
                </button>

                if overview.as_ref().is_some_and(|o| o.my_invite_code.is_some()) {
                    <div class="section-title">{"Your name"}</div>
                    <div class="name-edit-row">
                        <input
                            class="input"
                            style="margin-bottom:0"
                            placeholder="The name on your invitation"
                            value={(*my_name).clone()}
                            oninput={{
                                let my_name = my_name.clone();
                                Callback::from(move |e: InputEvent| {
                                    if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                                        my_name.set(el.value());
                                    }
                                })
                            }}
                        />
                        <button class="name-save-btn" onclick={save_name}>{"Save"}</button>
                    </div>
                }

                if let Some(o) = &*overview {
                    <div class="section-title">{"Friends & invitations"}</div>
                    <div class="friend-list">
                        if let Some(inviter) = &o.invited_by {
                            <div class="friend-row">
                                <div>
                                    <div class="friend-name">{inviter}</div>
                                    <div class="friend-sub">{"Invited you"}</div>
                                </div>
                                <span class="friend-status status-joined">{"Friend"}</span>
                            </div>
                        }
                        { for o.invites.iter().map(invite_row) }
                        if o.invites.is_empty() && o.invited_by.is_none() {
                            <div class="friend-row">
                                <div class="friend-sub">
                                    {"No invitations yet — invite your friends!"}
                                </div>
                            </div>
                        }
                    </div>
                }
            } else {
                <div class="screen-sub" style="margin-bottom:6px">
                    {"Sign in to add places, post reviews and save favorites."}
                </div>

                <div class="field-label">{"Username"}</div>
                <input
                    class="input"
                    placeholder="e.g. trail-scout"
                    value={(*username).clone()}
                    oninput={{
                        let username = username.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                                username.set(el.value());
                            }
                        })
                    }}
                />

                <div class="field-label">{"Password"}</div>
                <input
                    class="input"
                    type="password"
                    placeholder="At least 8 characters"
                    value={(*password).clone()}
                    oninput={{
                        let password = password.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                                password.set(el.value());
                            }
                        })
                    }}
                />

                <button class="submit-btn" onclick={log_in} disabled={*busy}>
                    <span class="mi">{"login"}</span>
                    {if *busy { "One moment…" } else { "Sign in" }}
                </button>
                <button class="alt-auth-btn" onclick={go_to_register}>
                    <span class="mi">{"person_add"}</span>{"Create an account"}
                </button>
                <div class="auth-note">
                    {"New here? You'll need an invite code from an existing scout — \
                      tap Create an account to get started."}
                </div>
            }
        </div>
    }
}
