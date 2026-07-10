use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct OnboardingProps {
    pub on_start: Callback<()>,
}

#[function_component(Onboarding)]
pub fn onboarding(props: &OnboardingProps) -> Html {
    let onclick = {
        let cb = props.on_start.clone();
        Callback::from(move |_| cb.emit(()))
    };
    html! {
        <div class="onb">
            <div class="onb-blob-a"></div>
            <div class="onb-blob-b"></div>
            <div class="onb-body">
                <div class="onb-logo">
                    <div class="onb-logo-badge"><span class="mi">{"wc"}</span></div>
                    <div>
                        <div class="onb-app-name">{"Service Break"}</div>
                        <div class="onb-tagline">{"find a clean stop"}</div>
                    </div>
                </div>
                <h1 class="onb-h1">{"Every good trip needs a clean pit stop."}</h1>
                <p class="onb-p">
                    {"Real ratings for bathrooms at coffee shops, groceries, parks, \
                      gas stations & more — sorted by what's closest to you."}
                </p>
            </div>
            <div class="onb-foot">
                <div class="onb-features">
                    <div class="onb-feature">
                        <span class="mi">{"mop"}</span>
                        <div class="onb-feature-text">{"Cleanliness you can trust"}</div>
                    </div>
                    <div class="onb-feature">
                        <span class="mi">{"near_me"}</span>
                        <div class="onb-feature-text">{"Directions in one tap"}</div>
                    </div>
                </div>
                <button class="onb-cta" {onclick}>
                    <span class="mi">{"my_location"}</span>{"Enable location & explore"}
                </button>
                <div class="onb-note">{"We only use your location to sort nearby stops."}</div>
            </div>
        </div>
    }
}
