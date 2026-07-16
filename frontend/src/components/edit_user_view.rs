use shared::AuthSession;
use web_sys::{Event, HtmlInputElement};
use yew::prelude::*;
use yew_router::hooks::{use_navigator, use_route};

use crate::api;
use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct EditUserViewProps {
    pub auth: Option<AuthSession>,
    pub on_toast: Callback<String>,
}

/// Admin-only page for editing a single user account. Fields that aren't
/// touched are left alone — the form sends only what changed. Password
/// resets require re-typing to avoid typos; admin flag flips in place.
#[function_component(EditUserView)]
pub fn edit_user_view(props: &EditUserViewProps) -> Html {
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    let route = use_route::<Route>().unwrap_or(Route::Users);

    // Debug: log the current route.
    web_sys::console::log_1(&format!("EditUserView route: {route:?}").into());

    // Form state. `initial_username` is captured once so we can detect the
    // "same as before" case and skip sending it (which would trigger a
    // duplicate-username error against ourselves).
    let busy = use_state(|| false);
    let username = use_state(String::new);
    let given_name = use_state(String::new);
    let family_name = use_state(String::new);
    let is_admin = use_state(|| false);
    let password = use_state(String::new);
    let confirm_password = use_state(String::new);
    let initial_username = use_state(String::new);
    let initial_given_name = use_state(String::new);
    let initial_family_name = use_state(String::new);
    let error = use_state(String::new);

    // Pull the user id out of the URL route. Must happen after all hooks
    // are initialized so Yew's hook counter stays in sync.
    let user_id_opt: Option<uuid::Uuid> = match &route {
        Route::UserEdit { id } => {
            web_sys::console::log_1(&format!("Extracted user ID: {id}").into());
            Some(*id)
        }
        _ => {
            web_sys::console::log_1(&"Not on UserEdit route".into());
            None
        }
    };

    // Load user detail on mount.
    let username_ef = username.clone();
    let given_name_ef = given_name.clone();
    let family_name_ef = family_name.clone();
    let is_admin_ef = is_admin.clone();
    let initial_username_ef = initial_username.clone();
    let initial_given_name_ef = initial_given_name.clone();
    let initial_family_name_ef = initial_family_name.clone();
    let error_ef = error.clone();
    yew::use_effect_with(user_id_opt, move |user_id_opt| {
        if let Some(user_id) = user_id_opt.as_ref() {
            let user_id = *user_id;
            wasm_bindgen_futures::spawn_local(async move {
                match api::fetch_user_detail(user_id).await {
                    Ok(detail) => {
                        let username_val = detail.username;
                        let given_name_val = detail.given_name.unwrap_or_default();
                        let family_name_val = detail.family_name.unwrap_or_default();
                        username_ef.set(username_val.clone());
                        initial_given_name_ef.set(given_name_val.clone());
                        initial_family_name_ef.set(family_name_val.clone());
                        given_name_ef.set(given_name_val);
                        family_name_ef.set(family_name_val);
                        is_admin_ef.set(detail.is_admin);
                        initial_username_ef.set(username_val);
                    }
                    Err(msg) => {
                        error_ef.set(msg);
                    }
                }
            });
        }
        || ()
    });

    // All callbacks must be defined before any early return to satisfy Yew's
    // hook ordering rules.
    
    // Back button: navigate with window.location.href instead of navigator.push()
    // because Yew's client-side router can fail to re-render components when
    // navigating between sibling routes (both match `/users*`).
    let go_back = {
        Callback::from(move |_| {
            if let Some(window) = web_sys::window() {
                let location = window.location();
                let _ = location.set_href("/users");
            }
        })
    };

    // Input handlers.
    let on_username_input = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                username.set(el.value());
            }
        })
    };

    let on_given_name_input = {
        let given_name = given_name.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                given_name.set(el.value());
            }
        })
    };

    let on_family_name_input = {
        let family_name = family_name.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                family_name.set(el.value());
            }
        })
    };

    let on_password_input = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                password.set(el.value());
            }
        })
    };

    let on_confirm_password_input = {
        let confirm_password = confirm_password.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                confirm_password.set(el.value());
            }
        })
    };

    let on_is_admin_change = {
        let is_admin = is_admin.clone();
        Callback::from(move |e: Event| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                is_admin.set(el.checked());
            }
        })
    };

    // Save handler. State handles are cloned at three levels so they survive
    // the move into the async block while remaining valid for subsequent
    // clicks (Callback must be `Fn`, not `FnOnce`).
    let save = {
        let busy = busy.clone();
        let username = username.clone();
        let given_name = given_name.clone();
        let family_name = family_name.clone();
        let is_admin = is_admin.clone();
        let password = password.clone();
        let confirm_password = confirm_password.clone();
        let initial_username = initial_username.clone();
        let initial_given_name = initial_given_name.clone();
        let initial_family_name = initial_family_name.clone();
        let error = error.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if *busy {
                return;
            }
            // Password field is optional — only validate when present.
            if !password.is_empty() && *password != *confirm_password {
                error.set("passwords don't match".to_owned());
                return;
            }
            busy.set(true);
            let user_id = user_id_opt.unwrap();
            let username_val = (*username).clone();
            let given_name_val = (*given_name).clone();
            let family_name_val = (*family_name).clone();
            let is_admin_val = *is_admin;
            let password_val = if password.is_empty() {
                String::new()
            } else {
                (*password).clone()
            };
            let initial_username_val = (*initial_username).clone();
            let initial_given_name_val = (*initial_given_name).clone();
            let initial_family_name_val = (*initial_family_name).clone();
            let busy = busy.clone();
            let error = error.clone();
            let on_toast = on_toast.clone();
            // Clone state handles for the async block — they need to be
            // `'static` and we can't move them from the outer closure.
            let password_async = password.clone();
            let confirm_password_async = confirm_password.clone();
            let error_async = error.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut update = api::AdminUserUpdate {
                    username: None,
                    password: None,
                    is_admin: None,
                    given_name: None,
                    family_name: None,
                };
                if username_val != initial_username_val {
                    update.username = Some(username_val);
                }
                if !password_val.is_empty() {
                    update.password = Some(password_val);
                }
                update.is_admin = Some(is_admin_val);
                if given_name_val != initial_given_name_val {
                    update.given_name = Some(given_name_val);
                }
                if family_name_val != initial_family_name_val {
                    update.family_name = Some(family_name_val);
                }

                match api::update_user(user_id, &update).await {
                    Ok(_) => {
                        on_toast.emit("User updated".to_owned());
                        // Clear the password field on success.
                        password_async.set(String::new());
                        confirm_password_async.set(String::new());
                        error_async.set(String::new());
                    }
                    Err(msg) => error_async.set(msg),
                }
                busy.set(false);
            });
        })
    };

    let delete_navigator = navigator.clone();
    // Delete handler.
    let delete = {
        let busy = busy.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            if *busy {
                return;
            }
            let user_id = user_id_opt.unwrap();
            let busy = busy.clone();
            let on_toast = on_toast.clone();
            let delete_navigator = delete_navigator.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::delete_user(user_id).await {
                    Ok(_) => {
                        on_toast.emit("Account deleted".to_owned());
                        delete_navigator.push(&Route::Users);
                    }
                    Err(msg) => {
                        busy.set(false);
                        on_toast.emit(msg);
                    }
                }
            });
        })
    };

    // Early return if not on the UserEdit route — all hooks are already
    // initialized above so Yew's hook counter stays in sync.
    let user_id = match user_id_opt {
        Some(id) => id,
        None => return html! {},
    };

    let has_changes = || {
        *username != *initial_username
            || !password.is_empty()
            || *is_admin
            || (*given_name) != (*initial_given_name)
            || (*family_name) != (*initial_family_name)
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Edit user"}</div>
            <button class="alt-auth-btn" onclick={go_back}>
                <span class="mi">{"arrow_back"}</span>{"Back to users"}
            </button>

            if !error.is_empty() {
                <div class="auth-note" style="color:var(--danger);margin-bottom:12px">{(*error).clone()}</div>
            }

            if username.is_empty() {
                <div class="auth-note">{"Loading…"}</div>
            } else {
                <div class="field-label">{"Username"}</div>
                <input
                    class="input"
                    type="text"
                    value={(*username).clone()}
                    oninput={on_username_input}
                    style="margin-bottom:14px"
                />

                <div class="field-label">{"Given name"}</div>
                <input
                    class="input"
                    type="text"
                    value={(*given_name).clone()}
                    oninput={on_given_name_input}
                    style="margin-bottom:14px"
                />

                <div class="field-label">{"Family name"}</div>
                <input
                    class="input"
                    type="text"
                    value={(*family_name).clone()}
                    oninput={on_family_name_input}
                    style="margin-bottom:14px"
                />

                <label style="display:flex;align-items:center;gap:8px;margin-bottom:18px">
                    <input
                        type="checkbox"
                        checked={*is_admin}
                        onchange={on_is_admin_change}
                    />
                    {"Admin account"}
                </label>

                <div class="field-label">{"Reset password"}</div>
                <div class="auth-note" style="margin-bottom:8px">
                    {"Leave blank to keep the current password."}
                </div>
                <input
                    class="input"
                    type="password"
                    placeholder={if password.is_empty() { "New password" } else { "" }}
                    value={(*password).clone()}
                    oninput={on_password_input}
                    style="margin-bottom:8px"
                />
                <input
                    class="input"
                    type="password"
                    placeholder={"Confirm new password"}
                    value={(*confirm_password).clone()}
                    oninput={on_confirm_password_input}
                    style="margin-bottom:18px"
                />

                <button
                    class="submit-btn"
                    onclick={save}
                    disabled={!has_changes() || *busy}
                >
                    <span class="mi">{"save"}</span>
                    {if *busy { "One moment…" } else { "Save changes" }}
                </button>

                <div style="margin-top:28px;padding-top:18px;border-top:1px solid var(--border)">
                    <div class="field-label" style="color:var(--danger)">{"Danger zone"}</div>
                    <div class="auth-note" style="margin-bottom:12px">
                        {"Deleting this account removes all places, reviews and saved lists \
                         tied to it. The invitation history stays for audit purposes."}
                    </div>
                    <button
                        class="alt-auth-btn"
                        onclick={delete}
                        disabled={*busy}
                        style={"color:var(--danger);border-color:var(--danger)"}
                    >
                        <span class="mi">{"delete_forever"}</span>{"Delete account"}
                    </button>
                </div>
            }
        </div>
    }
}
