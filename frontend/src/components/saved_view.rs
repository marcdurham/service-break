use shared::PlaceSummary;
use uuid::Uuid;
use yew::prelude::*;

use crate::components::ui;

#[derive(Properties, PartialEq)]
pub struct SavedViewProps {
    pub places: Vec<PlaceSummary>,
    pub on_open: Callback<Uuid>,
}

#[function_component(SavedView)]
pub fn saved_view(props: &SavedViewProps) -> Html {
    html! {
        <div class="screen sb-scroll">
            <div class="screen-title">{"Saved"}</div>
            <div class="screen-sub" style="margin-bottom:18px">
                {format!("{} spots you can count on.", props.places.len())}
            </div>
            if props.places.is_empty() {
                <div class="empty">
                    <span class="mi">{"bookmark_border"}</span>
                    <div class="empty-title">{"Nothing saved yet"}</div>
                    <div class="empty-sub">{"Tap the bookmark on any place to keep it here."}</div>
                </div>
            } else {
                <div class="cards">
                    { for props.places.iter().map(|p| saved_card(p, props)) }
                </div>
            }
        </div>
    }
}

fn saved_card(p: &PlaceSummary, props: &SavedViewProps) -> Html {
    let open = {
        let cb = props.on_open.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    let directions = {
        let place = p.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            ui::open_directions(&place);
        })
    };
    let sub = match p.distance_mi {
        Some(d) => format!("{} · {} mi away", p.place_type.label(), shared::fmt_distance_mi(d)),
        None => p.place_type.label().to_owned(),
    };
    html! {
        <button class="card" key={p.id.to_string()} onclick={open} style="align-items:center">
            { ui::badge(p.place_type) }
            <div class="card-main">
                <div class="card-name">{&p.name}</div>
                <div class="card-type" style="margin-top:2px">{sub}</div>
            </div>
            <span class="card-dir-btn" onclick={directions} role="button">
                <span class="mi">{"directions"}</span>
            </span>
        </button>
    }
}
