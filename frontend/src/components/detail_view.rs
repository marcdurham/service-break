use shared::{NewReview, PlaceDetail, PlaceSummary};
use uuid::Uuid;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

use crate::api;
use crate::components::ui;

const AVATAR_COLORS: [&str; 4] = ["#c05f38", "#6f8256", "#9b6a7d", "#4f7a86"];

#[derive(Properties, PartialEq)]
pub struct DetailViewProps {
    pub detail: PlaceDetail,
    pub device_id: String,
    pub saved: bool,
    pub on_close: Callback<()>,
    pub on_show_on_map: Callback<PlaceSummary>,
    pub on_toggle_save: Callback<Uuid>,
    pub on_updated: Callback<PlaceDetail>,
    pub on_toast: Callback<String>,
}

#[function_component(DetailView)]
pub fn detail_view(props: &DetailViewProps) -> Html {
    let composing = use_state(|| false);
    let clean_pick = use_state(|| 5i16);
    let text = use_state(String::new);

    let p = &props.detail.summary;
    let d = &props.detail;

    let close = {
        let cb = props.on_close.clone();
        Callback::from(move |_| cb.emit(()))
    };
    let toggle_save = {
        let cb = props.on_toggle_save.clone();
        let id = p.id;
        Callback::from(move |_| cb.emit(id))
    };
    let directions = {
        let place = p.clone();
        Callback::from(move |_| ui::open_directions(&place))
    };
    let show_on_map = {
        let cb = props.on_show_on_map.clone();
        let place = p.clone();
        Callback::from(move |_| cb.emit(place.clone()))
    };
    let open_composer = {
        let composing = composing.clone();
        Callback::from(move |_| composing.set(true))
    };
    let cancel_composer = {
        let composing = composing.clone();
        Callback::from(move |_| composing.set(false))
    };
    let send_review = {
        let composing = composing.clone();
        let clean_pick = clean_pick.clone();
        let text = text.clone();
        let device = props.device_id.clone();
        let id = p.id;
        let on_updated = props.on_updated.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_| {
            let review = NewReview {
                device_id: device.clone(),
                clean: *clean_pick,
                text: (*text).trim().to_owned(),
            };
            let composing = composing.clone();
            let text = text.clone();
            let on_updated = on_updated.clone();
            let on_toast = on_toast.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::create_review(id, &review).await {
                    Ok(detail) => {
                        composing.set(false);
                        text.set(String::new());
                        on_updated.emit(detail);
                        on_toast.emit("Review added — thanks, scout!".to_owned());
                    }
                    Err(msg) => on_toast.emit(msg),
                }
            });
        })
    };

    let rating = p.clean_avg.unwrap_or(0.0);
    let clean_pct = (rating / 5.0 * 100.0).clamp(0.0, 100.0);
    let access = ui::access_label(p.purchase_required, p.code_required);
    let access_class = if p.purchase_required || p.code_required {
        "tag tag-code"
    } else {
        "tag tag-free"
    };

    html! {
        <div class="detail-overlay">
            <div class="detail-scroll sb-scroll">
                <div class="hero" style={format!("background:{}", p.place_type.color())}>
                    <span class="mi">{p.place_type.icon()}</span>
                    <div class="hero-btns">
                        <button class="hero-btn" onclick={close}>
                            <span class="mi">{"arrow_back"}</span>
                        </button>
                        <button class="hero-btn" onclick={toggle_save}>
                            <span class={if props.saved { "mi saved" } else { "mi" }}>
                                {if props.saved { "bookmark" } else { "bookmark_border" }}
                            </span>
                        </button>
                    </div>
                </div>
                <div class="detail-body">
                    <div class="detail-meta">
                        <span class="detail-type">{p.place_type.label()}</span>
                        if let Some(hours) = &d.hours {
                            <span class="open-dot"></span>
                            <span class="detail-hours-label">{hours.clone()}</span>
                        }
                    </div>
                    <div class="detail-name">{&p.name}</div>
                    <div class="stars-row">
                        { ui::stars(rating) }
                        <span class="rating-num">{ui::clean_label(p.clean_avg)}</span>
                        <span class="rating-count">
                            {format!("({} review{})", p.review_count, if p.review_count == 1 { "" } else { "s" })}
                        </span>
                    </div>

                    <div class="action-row">
                        <button class="directions-btn" onclick={directions}>
                            <span class="mi">{"directions"}</span>{"Directions"}
                        </button>
                        <button class="rate-btn" onclick={show_on_map}>
                            <span class="mi">{"map"}</span>{"Map"}
                        </button>
                        <button class="rate-btn" onclick={open_composer.clone()}>
                            <span class="mi">{"rate_review"}</span>{"Rate"}
                        </button>
                    </div>

                    <div class="breakdown">
                        <div class="brow">
                            <span class="mi" style="color:#6f8256">{"mop"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Cleanliness"}</div>
                                <div class="bbar">
                                    <div class="bbar-fill" style={format!("width:{clean_pct}%")}></div>
                                </div>
                            </div>
                            <span class="brow-val">{ui::clean_label(p.clean_avg)}</span>
                        </div>
                        <div class="brow">
                            <span class="mi" style="color:#b09a82">{"directions_walk"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Bathroom distance from door"}</div>
                                if !d.door_note.is_empty() {
                                    <div class="brow-sub">{d.door_note.clone()}</div>
                                }
                            </div>
                            <span class="brow-val plain">{shared::door_short(p.door_ft)}</span>
                        </div>
                        <div class="brow">
                            <span class="mi" style="color:#c08a4a">{"local_parking"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Parking"}</div>
                            </div>
                            { ui::parking_tag(p.parking) }
                        </div>
                        <div class="brow">
                            <span class="mi" style="color:#a8795a">{"key"}</span>
                            <div class="brow-main">
                                <div class="brow-title">{"Bathroom access"}</div>
                            </div>
                            <span class={access_class}>{access}</span>
                        </div>
                    </div>

                    if !p.address.is_empty() {
                        <div class="addr-card">
                            <span class="mi">{"location_on"}</span>
                            <div>
                                <div class="addr-line">{&p.address}</div>
                                if let Some(hours) = &d.hours {
                                    <div class="addr-sub">{hours.clone()}</div>
                                }
                            </div>
                        </div>
                    }

                    <div class="reviews-head">
                        <div class="reviews-title">{"Reviews"}</div>
                        <button class="use-loc" onclick={open_composer}>
                            <span class="mi">{"add"}</span>{"Add yours"}
                        </button>
                    </div>

                    if *composing {
                        <div class="composer">
                            <div class="clean-row">
                                { for (1..=5i16).map(|n| {
                                    let on = n <= *clean_pick;
                                    let onclick = {
                                        let clean_pick = clean_pick.clone();
                                        Callback::from(move |_| clean_pick.set(n))
                                    };
                                    html! {
                                        <button class={if on { "clean-pick on" } else { "clean-pick" }} {onclick}>
                                            <span class="mi">
                                                {if on { "sentiment_very_satisfied" } else { "sentiment_neutral" }}
                                            </span>
                                        </button>
                                    }
                                }) }
                            </div>
                            <textarea
                                class="textarea"
                                placeholder="How was it?"
                                value={(*text).clone()}
                                oninput={{
                                    let text = text.clone();
                                    Callback::from(move |e: InputEvent| {
                                        if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                                            text.set(el.value());
                                        }
                                    })
                                }}
                            ></textarea>
                            <div class="composer-actions">
                                <button class="composer-send" onclick={send_review}>{"Post review"}</button>
                                <button class="composer-cancel" onclick={cancel_composer}>{"Cancel"}</button>
                            </div>
                        </div>
                    }

                    <div class="cards" style="padding-bottom:24px">
                        { for d.reviews.iter().enumerate().map(|(i, r)| html! {
                            <div class="review-card" key={r.id.to_string()}>
                                <div class="review-top">
                                    <div
                                        class="avatar"
                                        style={format!("background:{}", AVATAR_COLORS[i % AVATAR_COLORS.len()])}
                                    >
                                        {r.author.chars().next().unwrap_or('S')}
                                    </div>
                                    <div class="review-who">
                                        <div class="review-name">{&r.author}</div>
                                        <div class="review-time">{&r.time_ago}</div>
                                    </div>
                                    { ui::stars(f64::from(r.clean)) }
                                </div>
                                if !r.text.is_empty() {
                                    <div class="review-text">{&r.text}</div>
                                }
                            </div>
                        }) }
                    </div>
                </div>
            </div>
        </div>
    }
}
