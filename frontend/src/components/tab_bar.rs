use yew::prelude::*;

use crate::route::Route;

#[derive(Properties, PartialEq)]
pub struct TabBarProps {
    pub route: Route,
    pub on_change: Callback<Route>,
}

#[function_component(TabBar)]
pub fn tab_bar(props: &TabBarProps) -> Html {
    let tabs = [
        (Route::Map, "Map", "map"),
        (Route::List, "List", "format_list_bulleted"),
        (Route::Add, "Add", "add"),
        (Route::Saved, "Saved", "bookmark"),
        (Route::Account, "Account", "person"),
    ];
    html! {
        <div class="tabbar">
            // Only visible when the bar renders as a nav rail on wide screens.
            <div class="rail-logo"><span class="mi">{"coffee"}</span></div>
            { for tabs.into_iter().map(|(r, label, icon)| {
                let on = props.route == r;
                let onclick = {
                    // Use window.location.href instead of Yew's navigator
                    // because the client-side router can fail to re-render
                    // components when navigating between sibling routes.
                    Callback::from(move |_| {
                        if let Some(window) = web_sys::window() {
                            let location = window.location();
                            let _ = location.set_href(route_to_path(r));
                        }
                    })
                };
                html! {
                    <button class={if on { "tab on" } else { "tab" }} {onclick}>
                        <span class="mi">{icon}</span>
                        <span>{label}</span>
                    </button>
                }
            }) }
        </div>
    }
}

/// Convert a route variant to its path string for `window.location.href`.
fn route_to_path(r: Route) -> &'static str {
    match r {
        Route::Map => "/",
        Route::List => "/list",
        Route::Add => "/add",
        Route::Saved => "/saved",
        Route::Account => "/account",
        _ => "/", // fallback for routes not in the tab bar
    }
}
