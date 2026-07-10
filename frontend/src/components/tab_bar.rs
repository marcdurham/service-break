use yew::prelude::*;

use crate::app::Tab;

#[derive(Properties, PartialEq)]
pub struct TabBarProps {
    pub tab: Tab,
    pub on_change: Callback<Tab>,
}

#[function_component(TabBar)]
pub fn tab_bar(props: &TabBarProps) -> Html {
    let tabs = [
        (Tab::Map, "Map", "map"),
        (Tab::List, "List", "format_list_bulleted"),
        (Tab::Add, "Add", "add"),
        (Tab::Saved, "Saved", "bookmark"),
    ];
    html! {
        <div class="tabbar">
            // Only visible when the bar renders as a nav rail on wide screens.
            <div class="rail-logo"><span class="mi">{"wc"}</span></div>
            { for tabs.into_iter().map(|(t, label, icon)| {
                let on = props.tab == t;
                let onclick = {
                    let cb = props.on_change.clone();
                    Callback::from(move |_| cb.emit(t))
                };
                if t == Tab::Add {
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
