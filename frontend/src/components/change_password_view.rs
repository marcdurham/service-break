use shared::{AuthSession, ChangePassword};
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

/// Whether the signed-in account has a password to verify before setting a
/// new one. Google-only accounts don't, so they get the "set" flow instead
/// of "change".
async fn fetch_has_password() -> bool {
    api::get_me()
        .await
        .ok()
        .and_then(|p| p.get("has_password").and_then(|v| v.as_bool()))
        .unwrap_or(true)
}

#[derive(Properties, PartialEq)]
pub struct ChangePasswordViewProps {
    pub auth: Option<AuthSession>,
    pub on_toast: Callback<String>,
}

/// A text input with a small eye button that toggles between password and
/// plain text. The `show` flag controls the visible type; `on_toggle` flips it.
pub(crate) fn password_field(show: UseStateHandle<String>, show_pw: bool, on_toggle: Callback<MouseEvent>) -> Html {
    let input_type = if show_pw { "text" } else { "password" };
    html! {
        <div class="input-row">
            <span class="mi">{"lock_outline"}</span>
            <input
                type={input_type}
                placeholder="Enter password"
                value={(*show).clone()}
                oninput={{
                    let show = show.clone();
                    Callback::from(move |e: InputEvent| {
                        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                            show.set(el.value());
                        }
                    })
                }}
            />
            <button
                class="use-loc"
                onclick={on_toggle.clone()}
                title={if show_pw { "Hide" } else { "Show" }}
            >
                <span class="mi">{if show_pw { "visibility_off" } else { "visibility" }}</span>
            </button>
        </div>
    }
}

/// The Change Password page. Shows the current/new/confirm password fields
/// and submits them via the API. Requires an active session.
#[function_component(ChangePasswordView)]
pub fn change_password_view(props: &ChangePasswordViewProps) -> Html {
    let cur_pw = use_state(String::new);
    let new_pw = use_state(String::new);
    let confirm_pw = use_state(String::new);
    let pw_busy = use_state(|| false);
    let show_cur = use_state(|| false);
    let show_new = use_state(|| false);
    let show_confirm = use_state(|| false);
    let has_password = use_state(|| true);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    {
        let has_password = has_password.clone();
        let signed_in = props.auth.is_some();
        use_effect_with(signed_in, move |signed_in| {
            if *signed_in {
                wasm_bindgen_futures::spawn_local(async move {
                    has_password.set(fetch_has_password().await);
                });
            }
        });
    }

    if props.auth.is_none() {
        // Not signed in — bounce back to account so they can sign in.
        return html! { <div class="screen sb-scroll"><div class="screen-title">{"Change password"}</div></div> };
    }

    let toggle_show_cur = { let show_cur = show_cur.clone(); Callback::from(move |_| show_cur.set(!*show_cur)) };
    let toggle_show_new = { let show_new = show_new.clone(); Callback::from(move |_| show_new.set(!*show_new)) };
    let toggle_show_confirm = { let show_confirm = show_confirm.clone(); Callback::from(move |_| show_confirm.set(!*show_confirm)) };

    let change_password = {
        let cur_pw = cur_pw.clone();
        let new_pw = new_pw.clone();
        let confirm_pw = confirm_pw.clone();
        let pw_busy = pw_busy.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if (*cur_pw).is_empty() || (*new_pw).is_empty() || (*confirm_pw).is_empty() {
                on_toast.emit("Fill in all three fields".to_owned());
                return;
            }
            if !(*new_pw).eq(&*confirm_pw) {
                on_toast.emit("New passwords don't match".to_owned());
                return;
            }
            if *pw_busy {
                return;
            }
            pw_busy.set(true);
            let cur = cur_pw.clone();
            let new_p = new_pw.clone();
            let confirm_pw_local = confirm_pw.clone();
            let cur_pw_local = cur_pw.clone();
            let new_pw_local = new_pw.clone();
            let on_toast = on_toast.clone();
            let pw_busy = pw_busy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::change_password(&ChangePassword {
                    current_password: (*cur).clone(),
                    new_password: (*new_p).clone(),
                })
                .await
                {
                    Ok(()) => {
                        cur_pw_local.set(String::new());
                        new_pw_local.set(String::new());
                        confirm_pw_local.set(String::new());
                        on_toast.emit("Password changed".to_owned());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                pw_busy.set(false);
            });
        })
    };

    let set_password = {
        let new_pw = new_pw.clone();
        let confirm_pw = confirm_pw.clone();
        let pw_busy = pw_busy.clone();
        let on_toast = props.on_toast.clone();
        let has_password = has_password.clone();
        Callback::from(move |_| {
            if (*new_pw).is_empty() || (*confirm_pw).is_empty() {
                on_toast.emit("Fill in both fields".to_owned());
                return;
            }
            if !(*new_pw).eq(&*confirm_pw) {
                on_toast.emit("New passwords don't match".to_owned());
                return;
            }
            if *pw_busy {
                return;
            }
            pw_busy.set(true);
            let new_p = new_pw.clone();
            let confirm_pw_local = confirm_pw.clone();
            let new_pw_local = new_pw.clone();
            let on_toast = on_toast.clone();
            let pw_busy = pw_busy.clone();
            let has_password = has_password.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::set_password(&new_p).await {
                    Ok(()) => {
                        new_pw_local.set(String::new());
                        confirm_pw_local.set(String::new());
                        has_password.set(true);
                        on_toast.emit("Password set".to_owned());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
                pw_busy.set(false);
            });
        })
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">
                {if *has_password { "Change password" } else { "Set a password" }}
            </div>
            <button class="alt-auth-btn" onclick={{
                let navigator = navigator.clone();
                Callback::from(move |_| navigator.push(&Route::Account))
            }}>
                <span class="mi">{"arrow_back"}</span>{"Back to account"}
            </button>

            if *has_password {
                <div class="field-label">{"Current password"}</div>
                {password_field(cur_pw.clone(), *show_cur, toggle_show_cur)}
                <div class="field-label">{"New password"}</div>
                {password_field(new_pw.clone(), *show_new, toggle_show_new)}
                <div class="field-label">{"Confirm new password"}</div>
                {password_field(confirm_pw.clone(), *show_confirm, toggle_show_confirm)}
                <button
                    class="submit-btn"
                    onclick={change_password}
                    disabled={*pw_busy}
                >
                    <span class="mi">{"lock_reset"}</span>
                    {if *pw_busy { "One moment…" } else { "Update password" }}
                </button>
            } else {
                <div class="screen-sub mb-sm">
                    {"Your account signs in with Google. Set a password to also sign in \
                      with your username."}
                </div>
                <div class="field-label">{"New password"}</div>
                {password_field(new_pw.clone(), *show_new, toggle_show_new)}
                <div class="field-label">{"Confirm new password"}</div>
                {password_field(confirm_pw.clone(), *show_confirm, toggle_show_confirm)}
                <button
                    class="submit-btn"
                    onclick={set_password}
                    disabled={*pw_busy}
                >
                    <span class="mi">{"lock_reset"}</span>
                    {if *pw_busy { "One moment…" } else { "Set password" }}
                </button>
            }
        </div>
    }
}
