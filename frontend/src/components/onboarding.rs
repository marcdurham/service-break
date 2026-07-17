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
                    <div class="onb-logo-badge"><span class="mi">{"coffee"}</span></div>
                    <div>
                        <div class="onb-app-name">{"Service Break"}</div>
                        <div class="onb-tagline">{"find places for refreshment"}</div>
                    </div>
                </div>
                <h1 class="onb-h1">{"Good places to take breaks"}</h1>
                <p class="onb-p">
                    {"Places with coffee, food, bathrooms, places to sit at \
                      shops, stores, malls, parks & more — sorted by what's \
                      closest to you."}
                </p>
            </div>
            <div class="onb-foot">
                <div class="onb-features">
                    <div class="onb-feature">
                        <span class="mi">{"clean_hands"}</span>
                        <div class="onb-feature-text">{"Clean, comfortable places"}</div>
                    </div>
                    <div class="onb-feature">
                        <span class="mi">{"near_me"}</span>
                        <div class="onb-feature-text">{"Directions in one tap"}</div>
                    </div>
                </div>
                <button class="onb-cta" {onclick}>
                    <span class="mi">{"my_location"}</span>{"Enable location & explore"}
                </button>
                <div class="onb-note">{"We only use your location to sort nearby places."}</div>
            </div>
        </div>
    }
}
