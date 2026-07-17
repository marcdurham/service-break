//! Lands here after the Google OAuth round trip: the backend's
//! `/api/auth/google/callback` redirects the browser to `/oauth-complete`
//! with either the new session or an error message in the URL *fragment*
//! (never the query string, so it's never sent to a server or logged).
//! This component's only job is to read that fragment once, apply it, and
//! bounce on to the Account page.

use shared::{query_param, AuthSession};
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct OauthCompleteViewProps {
    pub on_login: Callback<AuthSession>,
    pub on_toast: Callback<String>,
    /// Fired instead of `on_toast` when the backend reports "no account is
    /// linked yet" — the message becomes a persistent banner on `/register`
    /// rather than a toast that vanishes before most people act on it.
    pub on_register_notice: Callback<String>,
}

fn fragment() -> String {
    web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default()
}

#[function_component(OauthCompleteView)]
pub fn oauth_complete_view(props: &OauthCompleteViewProps) -> Html {
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");
    {
        let on_login = props.on_login.clone();
        let on_toast = props.on_toast.clone();
        let on_register_notice = props.on_register_notice.clone();
        let navigator = navigator.clone();
        use_effect_with((), move |()| {
            let frag = fragment();
            let frag = frag.trim_start_matches('#');
            let mut destination = Route::Account;
            if let Some(message) = query_param(frag, "register_required") {
                on_register_notice.emit(message);
                destination = Route::Register;
            } else if let Some(message) = query_param(frag, "error") {
                on_toast.emit(message);
            } else if let Some(token) = query_param(frag, "token") {
                on_login.emit(AuthSession {
                    token,
                    username: query_param(frag, "username").unwrap_or_default(),
                    is_admin: query_param(frag, "is_admin").as_deref() == Some("true"),
                    given_name: String::new(),
                    family_name: String::new(),
                });
            } else {
                on_toast.emit("Google sign-in didn't complete".to_owned());
            }
            navigator.replace(&destination);
            || ()
        });
    }
    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Signing you in…"}</div>
        </div>
    }
}
