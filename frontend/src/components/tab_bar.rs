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
                    let cb = props.on_change.clone();
                    Callback::from(move |_| cb.emit(r))
                };
                if r == Route::Add {
                    html! {
                        <button class="tab" {onclick}>
                            <div class="tab-add-btn"><span class="mi">{"add"}</span></div>
                        </button>
                    }
                } else {
                    html! {
                        <button class={if on { "tab on" } else { "tab" }} {onclick}>
                            <span class="mi">{icon}</span>
                            <span>{label}</span>
                        </button>
                    }
                }
            }) }
        </div>
    }
}
