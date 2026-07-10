use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::app::Filters;

#[derive(Properties, PartialEq)]
pub struct FiltersSheetProps {
    pub filters: Filters,
    pub count: usize,
    pub on_change: Callback<Filters>,
    pub on_reset: Callback<()>,
    pub on_close: Callback<()>,
}

#[function_component(FiltersSheet)]
pub fn filters_sheet(props: &FiltersSheetProps) -> Html {
    let close_scrim = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let close_apply = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let reset = {
        let cb = props.on_reset.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    type Toggle = (&'static str, &'static str, bool, fn(&mut Filters));
    let toggles: [Toggle; 3] = [
        ("Clean spots only (4.0+)", "mop", props.filters.clean_only,
            |f| f.clean_only = !f.clean_only),
        ("Has parking", "local_parking", props.filters.has_parking,
            |f| f.has_parking = !f.has_parking),
        ("No purchase required", "money_off", props.filters.no_purchase,
            |f| f.no_purchase = !f.no_purchase),
    ];

    let on_radius = {
        let filters = props.filters.clone();
        let cb = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(pos) = el.value().parse::<u32>() {
                    let mut f = filters.clone();
                    f.radius_mi = shared::radius_from_slider(pos);
                    cb.emit(f);
                }
            }
        })
    };

    html! {
        <div class="sheet-scrim" onclick={close_scrim}>
            <div class="sheet" onclick={stop}>
                <div class="sheet-handle"></div>
                <div class="sheet-head">
                    <div class="sheet-title">{"Filters"}</div>
                    <button class="sheet-reset" onclick={reset}>{"Reset"}</button>
                </div>
                <div class="ftoggles">
                    { for toggles.into_iter().map(|(label, icon, on, flip)| {
                        let onclick = {
                            let filters = props.filters.clone();
                            let cb = props.on_change.clone();
                            Callback::from(move |_| {
                                let mut f = filters.clone();
                                flip(&mut f);
                                cb.emit(f);
                            })
                        };
                        html! {
                            <div class={if on { "ftoggle on" } else { "ftoggle" }} {onclick}>
                                <span class="mi">{icon}</span>
                                <span class="ftoggle-label">{label}</span>
                                <div class={if on { "switch on" } else { "switch" }}>
                                    <div class="switch-knob"></div>
                                </div>
                            </div>
                        }
                    }) }
                </div>
                <div class="field-label">{format!("Within {} mi", props.filters.radius_mi)}</div>
                <input
                    type="range"
                    class="range"
                    min="0"
                    max={shared::RADIUS_SLIDER_STEPS.to_string()}
                    step="1"
                    value={shared::radius_to_slider(props.filters.radius_mi).to_string()}
                    oninput={on_radius}
                />
                <button class="sheet-apply" onclick={close_apply}>
                    {format!("Show {} places", props.count)}
                </button>
            </div>
        </div>
    }
}
