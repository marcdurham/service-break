use shared::{PlaceSummary, PlaceType};
use uuid::Uuid;
use yew::prelude::*;

use crate::components::ui::{PlaceCard, TypeChips};

#[derive(Properties, PartialEq)]
pub struct ListViewProps {
    pub places: Vec<PlaceSummary>,
    pub active_types: Vec<PlaceType>,
    pub on_open: Callback<Uuid>,
    pub on_open_filters: Callback<()>,
    pub on_toggle_type: Callback<PlaceType>,
    pub on_reset_filters: Callback<()>,
}

#[function_component(ListView)]
pub fn list_view(props: &ListViewProps) -> Html {
    let open_filters = {
        let cb = props.on_open_filters.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let reset = {
        let cb = props.on_reset_filters.clone();
        Callback::from(move |_| cb.emit(()))
    };
    html! {
        <div class="screen sb-scroll">
            <div class="list-head">
                <div>
                    <div class="screen-title">{"Nearby places"}</div>
                    <div class="screen-sub">
                        {format!("{} places · sorted by distance", props.places.len())}
                    </div>
                </div>
                <button class="icon-btn" onclick={open_filters}>
                    <span class="mi">{"tune"}</span>
                </button>
            </div>
            <TypeChips active={props.active_types.clone()} on_toggle={props.on_toggle_type.clone()} />
            if props.places.is_empty() {
                <div class="empty">
                    <span class="mi">{"travel_explore"}</span>
                    <div class="empty-title">{"No places match your filters"}</div>
                    <button onclick={reset}>{"Clear filters"}</button>
                </div>
            } else {
                <div class="cards">
                    { for props.places.iter().map(|p| html! {
                        <PlaceCard key={p.id.to_string()} place={p.clone()} on_open={props.on_open.clone()} />
                    }) }
                </div>
            }
        </div>
    }
}
