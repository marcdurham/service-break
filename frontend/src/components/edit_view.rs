//! Full-screen editor for an existing place. Any logged-in user can edit;
//! the backend records every changed field in the place's audit log.

use std::collections::HashSet;

use shared::{Amenity, Parking, PlaceDetail, PlaceType, Requirement, UpdatePlace};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::api;

#[derive(Clone, PartialEq)]
struct Form {
    name: String,
    address: String,
    place_type: PlaceType,
    amenities: HashSet<Amenity>,
    door_ft: String,
    door_note: String,
    parking: Parking,
    purchase: Requirement,
    code: Requirement,
    hours: String,
}

impl Form {
    fn from_detail(d: &PlaceDetail) -> Self {
        let p = &d.summary;
        Form {
            name: p.name.clone(),
            address: p.address.clone(),
            place_type: p.place_type,
            amenities: p.amenities.iter().copied().collect(),
            door_ft: p.door_ft.to_string(),
            door_note: d.door_note.clone(),
            parking: p.parking,
            purchase: p.purchase_required,
            code: p.code_required,
            hours: d.hours.clone().unwrap_or_default(),
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct EditViewProps {
    pub detail: PlaceDetail,
    pub on_close: Callback<()>,
    /// Emits the updated detail after a successful save.
    pub on_saved: Callback<PlaceDetail>,
    pub on_toast: Callback<String>,
}

#[function_component(EditView)]
pub fn edit_view(props: &EditViewProps) -> Html {
    let form = use_state({
        let detail = props.detail.clone();
        move || Form::from_detail(&detail)
    });
    let submitting = use_state(|| false);

    let close = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let text_input = |form: &UseStateHandle<Form>, update: fn(&mut Form, String)| {
        let form = form.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                let mut next = (*form).clone();
                update(&mut next, el.value());
                form.set(next);
            }
        })
    };
    let on_name = text_input(&form, |f, v| f.name = v);
    let on_address = text_input(&form, |f, v| f.address = v);
    let on_door_ft = text_input(&form, |f, v| f.door_ft = v);
    let on_door_note = text_input(&form, |f, v| f.door_note = v);
    let on_hours = text_input(&form, |f, v| f.hours = v);

    let save = {
        let form = form.clone();
        let submitting = submitting.clone();
        let id = props.detail.summary.id;
        let on_saved = props.on_saved.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let f = (*form).clone();
            if f.name.trim().is_empty() {
                on_toast.emit("The place needs a name".to_owned());
                return;
            }
            if f.address.trim().is_empty() {
                on_toast.emit("Add an address or lat, lng".to_owned());
                return;
            }
            if *submitting {
                return;
            }
            submitting.set(true);
            let hours = f.hours.trim().to_owned();
            let update = UpdatePlace {
                name: f.name.trim().to_owned(),
                place_type: f.place_type,
                lat: None,
                lng: None,
                address: Some(f.address.trim().to_owned()),
                door_ft: f.door_ft.trim().parse().unwrap_or(0),
                door_note: f.door_note.trim().to_owned(),
                parking: f.parking,
                purchase_required: f.purchase,
                code_required: f.code,
                amenities: f.amenities.iter().copied().collect(),
                hours: (!hours.is_empty()).then_some(hours),
            };
            let submitting = submitting.clone();
            let on_saved = on_saved.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::update_place(id, &update).await {
                    Ok(detail) => on_saved.emit(detail),
                    Err(msg) => on_toast.emit(msg),
                }
                submitting.set(false);
            });
        })
    };

    html! {
        <div class="detail-overlay">
            <div class="detail-scroll sb-scroll">
                <div class="detail-body pt-18">
                    <div class="reviews-head">
                        <div class="screen-title">{"Edit place"}</div>
                        <button class="hero-btn" onclick={close.clone()}>
                            <span class="mi">{"close"}</span>
                        </button>
                    </div>
                    <div class="screen-sub">
                        {"Fix anything that's off — every change is logged with your name."}
                    </div>

                    <div class="field-label">{"Place name"}</div>
                    <input class="input" value={form.name.clone()} oninput={on_name} />

                    <div class="field-label">{"Address "}<span class="lite">{"· or lat, lng"}</span></div>
                    <div class="input-row">
                        <span class="mi">{"location_on"}</span>
                        <input value={form.address.clone()} oninput={on_address} />
                    </div>

                    <div class="field-label">{"Type of place"}</div>
                    <div class="type-grid">
                        { for PlaceType::ALL.into_iter().map(|t| {
                            let on = form.place_type == t;
                            let onclick = {
                                let form = form.clone();
                                Callback::from(move |_| {
                                    let mut next = (*form).clone();
                                    next.place_type = t;
                                    form.set(next);
                                })
                            };
                            html! {
                                <button class={if on { "tile on" } else { "tile" }} {onclick}>
                                    <span class="mi">{t.icon()}</span>
                                    <span>{t.label()}</span>
                                </button>
                            }
                        }) }
                    </div>

                    <div class="field-label">{"This place has"}</div>
                    <div class="type-grid">
                        { for Amenity::ALL.into_iter().map(|a| {
                            let on = form.amenities.contains(&a);
                            let onclick = {
                                let form = form.clone();
                                Callback::from(move |_| {
                                    let mut next = (*form).clone();
                                    if !next.amenities.remove(&a) {
                                        next.amenities.insert(a);
                                    }
                                    form.set(next);
                                })
                            };
                            html! {
                                <button class={if on { "tile on" } else { "tile" }} {onclick}>
                                    <span class="mi">{a.icon()}</span>
                                    <span>{a.label()}</span>
                                </button>
                            }
                        }) }
                    </div>

                    <div class="field-label">{"Distance to restroom (ft)"}</div>
                    <input
                        class="input"
                        type="number"
                        min="0"
                        value={form.door_ft.clone()}
                        oninput={on_door_ft}
                    />

                    <div class="field-label">{"Bathroom directions"}</div>
                    <input
                        class="input"
                        placeholder="e.g. Right past the counter"
                        value={form.door_note.clone()}
                        oninput={on_door_note}
                    />

                    <div class="field-label">{"Parking"}</div>
                    <div class="pick-grid-3">
                        { for [Parking::Easy, Parking::Street, Parking::None].into_iter().map(|p| {
                            let on = form.parking == p;
                            let label = if p == Parking::None { "None" } else { p.label() };
                            let onclick = {
                                let form = form.clone();
                                Callback::from(move |_| {
                                    let mut next = (*form).clone();
                                    next.parking = p;
                                    form.set(next);
                                })
                            };
                            html! {
                                <button class={if on { "pill on" } else { "pill" }} {onclick}>{label}</button>
                            }
                        }) }
                    </div>

                    { requirement_row(&form, "Purchase required?", |f| f.purchase, |f, v| f.purchase = v) }
                    { requirement_row(&form, "Code required?", |f| f.code, |f, v| f.code = v) }

                    <div class="field-label">{"Hours "}<span class="lite">{"· optional"}</span></div>
                    <input
                        class="input"
                        placeholder="e.g. Open · closes 8:00 PM"
                        value={form.hours.clone()}
                        oninput={on_hours}
                    />

                    <button class="submit-btn" onclick={save} disabled={*submitting}>
                        <span class="mi">{"save"}</span>
                        {if *submitting { "Saving…" } else { "Save changes" }}
                    </button>
                    <div class="composer-actions pb-24">
                        <button class="composer-cancel" onclick={close}>{"Cancel"}</button>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn requirement_row(
    form: &UseStateHandle<Form>,
    title: &'static str,
    get: fn(&Form) -> Requirement,
    set: fn(&mut Form, Requirement),
) -> Html {
    let current = get(form);
    html! {
        <>
            <div class="field-label">{title}</div>
            <div class="pick-grid-3">
                { for Requirement::ALL.into_iter().map(|r| {
                    let on = current == r;
                    let onclick = {
                        let form = form.clone();
                        Callback::from(move |_| {
                            let mut next = (*form).clone();
                            set(&mut next, r);
                            form.set(next);
                        })
                    };
                    html! {
                        <button class={if on { "pill on" } else { "pill" }} {onclick}>{r.label()}</button>
                    }
                }) }
            </div>
        </>
    }
}
