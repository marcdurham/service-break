use shared::{validate_password, validate_username, AuthSession, Credentials};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::api;

#[derive(Properties, PartialEq)]
pub struct AccountViewProps {
    pub auth: Option<AuthSession>,
    pub on_login: Callback<AuthSession>,
    pub on_logout: Callback<()>,
    pub on_toast: Callback<String>,
}

/// The Account page: a sign-in / create-account form when logged out, and
/// the signed-in identity with a sign-out button when logged in.
#[function_component(AccountView)]
pub fn account_view(props: &AccountViewProps) -> Html {
    let username = use_state(String::new);
    let password = use_state(String::new);
    let busy = use_state(|| false);

    let submit = |registering: bool| {
        let username = username.clone();
        let password = password.clone();
        let busy = busy.clone();
        let on_login = props.on_login.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let creds = Credentials {
                username: username.trim().to_owned(),
                password: (*password).clone(),
            };
            if creds.username.is_empty() || creds.password.is_empty() {
                on_toast.emit("Enter a username and password".to_owned());
                return;
            }
            if registering {
                if let Err(e) = validate_username(&creds.username)
                    .and_then(|()| validate_password(&creds.password))
                {
                    on_toast.emit(e.to_owned());
                    return;
                }
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
                let res = if registering {
                    api::register(&creds).await
                } else {
                    api::login(&creds).await
                };
                match res {
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
    let log_in = submit(false);
    let create_account = submit(true);

    let sign_out = {
        let cb = props.on_logout.clone();
        Callback::from(move |_| cb.emit(()))
    };

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
                    <button class="signout-btn" onclick={sign_out}>
                        <span class="mi">{"logout"}</span>{"Sign out"}
                    </button>
                </div>
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
                <button class="alt-auth-btn" onclick={create_account} disabled={*busy}>
                    <span class="mi">{"person_add"}</span>{"Create an account"}
                </button>
                <div class="auth-note">
                    {"New here? Pick a username, type a password and tap Create an account."}
                </div>
            }
        </div>
    }
}
