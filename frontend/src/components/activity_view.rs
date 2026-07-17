use shared::ActivityEntry;
use uuid::Uuid;
use yew::prelude::*;
use yew_router::hooks::use_navigator;

use crate::api;
use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct ActivityViewProps {
    /// `None`: the signed-in user's own activity (`GET /api/auth/activity`).
    /// `Some(id)`: an admin viewing another account's activity.
    #[prop_or_default]
    pub user_id: Option<Uuid>,
    /// Where the back button returns to. Defaults to the Account page for
    /// the self view, or the Users list for the admin view.
    #[prop_or_default]
    pub back_route: Option<Route>,
}

/// Icon for one activity kind — see [`shared::ActivityEntry::kind`].
fn icon_for(kind: &str) -> &'static str {
    match kind {
        "login" => "login",
        "failed_login" => "gpp_maybe",
        "profile_change" => "manage_accounts",
        "place_change" => "edit_location_alt",
        "rating" => "star_rate",
        _ => "history",
    }
}

/// Shows the last 50 logins, failed logins, profile changes, place edits
/// and ratings for an account — either the signed-in user's own (reached
/// from the Account page) or, for admins, any account's (reached from the
/// Edit user page).
#[function_component(ActivityView)]
pub fn activity_view(props: &ActivityViewProps) -> Html {
    let entries = use_state(Vec::<ActivityEntry>::new);
    let loading = use_state(|| true);
    let error = use_state(String::new);
    let navigator = use_navigator().expect("BrowserRouter provides a navigator");

    {
        let entries = entries.clone();
        let loading = loading.clone();
        let error = error.clone();
        let user_id = props.user_id;
        use_effect_with(user_id, move |user_id| {
            let entries = entries.clone();
            let loading = loading.clone();
            let error = error.clone();
            let user_id = *user_id;
            loading.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let result = match user_id {
                    Some(id) => api::fetch_user_activity(id).await,
                    None => api::fetch_my_activity().await,
                };
                match result {
                    Ok(list) => entries.set(list),
                    Err(msg) => error.set(msg),
                }
                loading.set(false);
            });
            || ()
        });
    }

    let default_back = if props.user_id.is_some() { Route::Users } else { Route::Account };
    let go_back = {
        let navigator = navigator.clone();
        let back_route = props.back_route.unwrap_or(default_back);
        Callback::from(move |_| navigator.push(&back_route))
    };

    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Activity"}</div>
            <button class="alt-auth-btn mb-md" onclick={go_back}>
                <span class="mi">{"arrow_back"}</span>{"Back"}
            </button>

            <div class="screen-sub mb-md">
                {"The last 50 logins, failed logins, profile changes, place edits and ratings."}
            </div>

            if !error.is_empty() {
                <div class="auth-note" style="color:var(--danger);margin-bottom:12px">{(*error).clone()}</div>
            }

            if *loading {
                <div class="auth-note">{"Loading…"}</div>
            } else if entries.is_empty() {
                <div class="auth-note">{"No activity yet."}</div>
            } else {
                <div class="cards pb-24">
                    { for entries.iter().map(|e| html! {
                        <div class="review-card" key={format!("{}-{}-{}", e.created_at, e.kind, e.summary)}>
                            <div class="review-name">
                                <span class="mi">{icon_for(&e.kind)}</span>
                                {" "}{&e.summary}
                            </div>
                            <div class="review-time">
                                {format!("by {} · {}", e.actor, e.time_ago)}
                            </div>
                        </div>
                    }) }
                </div>
            }
        </div>
    }
}
