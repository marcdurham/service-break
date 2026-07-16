//! The "Create an account" page. Reached from the Account page or straight
//! from an invitation link (`/register?code=XYZ`), which pre-fills the
//! invite code.

use shared::{query_param, validate_password, validate_username, AuthSession, Credentials};
use web_sys::{HtmlInputElement, MouseEvent};
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct RegisterViewProps {
    pub auth: Option<AuthSession>,
    pub on_login: Callback<AuthSession>,
    pub on_toast: Callback<String>,
    /// Whether the server has a Google OAuth client configured.
    #[prop_or_default]
    pub google_enabled: bool,
}

/// The `code` query parameter of the current URL, e.g. from a shared
/// invitation link.
fn code_from_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|search| query_param(&search, "code"))
        .unwrap_or_default()
}

/// A labelled text input bound to a state handle.
fn field(
    label: &str,
    placeholder: &str,
    password: bool,
    value: &UseStateHandle<String>,
) -> Html {
    let oninput = {
        let value = value.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                value.set(el.value());
            }
        })
    };
    html! {
        <>
            <div class="field-label">{label.to_owned()}</div>
            <input
                class="input"
                type={if password { "password" } else { "text" }}
                placeholder={placeholder.to_owned()}
                value={(**value).clone()}
                {oninput}
            />
        </>
    }
}

/// A labelled text input with an eye button that toggles between password and
/// plain text. `show` is the current value, `show_pw` controls visibility.
fn pw_field(
    label: &str,
    placeholder: &str,
    show: UseStateHandle<String>,
    show_pw: bool,
    on_toggle: Callback<MouseEvent>,
) -> Html {
    let input_type = if show_pw { "text" } else { "password" };
    html! {
        <>
            <div class="field-label">{label.to_owned()}</div>
            <div class="input-row">
                <span class="mi">{"lock_outline"}</span>
                <input
                    type={input_type}
                    placeholder={placeholder.to_owned()}
                    value={(*show).clone()}
                    oninput={{
                        let show = show.clone();
                        Callback::from(move |e: InputEvent| {
                            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                                show.set(el.value());
                            }
                        })
                    }}
                />
                <button
                    class="use-loc"
                    onclick={on_toggle}
                    title={if show_pw { "Hide" } else { "Show" }}
                >
                    <span class="mi">{if show_pw { "visibility_off" } else { "visibility" }}</span>
                </button>
            </div>
        </>
    }
}

#[function_component(RegisterView)]
pub fn register_view(props: &RegisterViewProps) -> Html {
    let username = use_state(String::new);
    let password = use_state(String::new);
    let confirm = use_state(String::new);
    let invite_code = use_state(code_from_url);
    let busy = use_state(|| false);
    let show_pw = use_state(|| false);
    let show_confirm = use_state(|| false);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    let create_account = {
        let username = username.clone();
        let password = password.clone();
        let confirm = confirm.clone();
        let invite_code = invite_code.clone();
        let busy = busy.clone();
        let navigator = navigator.clone();
        let on_login = props.on_login.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let creds = Credentials {
                username: username.trim().to_owned(),
                password: (*password).clone(),
                invite_code: invite_code.trim().to_owned(),
            };
            if let Err(e) = validate_username(&creds.username)
                .and_then(|()| validate_password(&creds.password))
            {
                on_toast.emit(e.to_owned());
                return;
            }
            if *password != *confirm {
                on_toast.emit("Passwords don't match".to_owned());
                return;
            }
            if creds.invite_code.is_empty() {
                on_toast.emit("Enter the invite code a friend gave you".to_owned());
                return;
            }
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let navigator = navigator.clone();
            let on_login = on_login.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::register(&creds).await {
                    Ok(session) => {
                        on_login.emit(session);
                        navigator.push(&Route::Account);
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                busy.set(false);
            });
        })
    };

    let go_to_account = {
        let navigator = navigator.clone();
        Callback::from(move |_| navigator.push(&Route::Account))
    };

    let continue_with_google = {
        let invite_code = invite_code.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let code = invite_code.trim().to_owned();
            if code.is_empty() {
                on_toast.emit("Enter the invite code a friend gave you first".to_owned());
                return;
            }
            api::start_google_auth("register", &code);
        })
    };

    let toggle_pw = { let s = show_pw.clone(); Callback::from(move |_| s.set(!*s)) };
    let toggle_confirm = { let s = show_confirm.clone(); Callback::from(move |_| s.set(!*s)) };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Create an account"}</div>
            if props.auth.is_some() {
                <div class="screen-sub mb-md">
                    {"You're already signed in."}
                </div>
                <button class="alt-auth-btn" onclick={go_to_account}>
                    <span class="mi">{"person"}</span>{"Go to Account"}
                </button>
            } else {
                <div class="screen-sub mb-sm">
                    {"Registration is invite-only — you'll need a code from a friend \
                      who's already a scout."}
                </div>

                { field("Username", "e.g. trail-scout", false, &username) }
                { pw_field("Password", "At least 8 characters", password.clone(), *show_pw, toggle_pw) }
                { pw_field("Confirm password", "Type it again", confirm.clone(), *show_confirm, toggle_confirm) }
                { field("Invite code", "e.g. A1B2C3D4", false, &invite_code) }

                <button class="submit-btn" onclick={create_account} disabled={*busy}>
                    <span class="mi">{"person_add"}</span>
                    {if *busy { "One moment…" } else { "Create an account" }}
                </button>
                if props.google_enabled {
                    <button class="alt-auth-btn" onclick={continue_with_google}>
                        <span class="mi">{"login"}</span>{"Continue with Google"}
                    </button>
                    <div class="auth-note">
                        {"Uses the invite code above — no username or password needed."}
                    </div>
                }
                <button class="alt-auth-btn" onclick={go_to_account}>
                    <span class="mi">{"login"}</span>{"Already have an account? Sign in"}
                </button>
            }
        </div>
    }
}
