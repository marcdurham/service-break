mod api;
mod app;
mod components;
mod glue;
mod maps_link;
mod route;

use yew::prelude::*;
use yew_router::prelude::*;

use app::App;

#[function_component(Root)]
fn root() -> Html {
    html! {
        <BrowserRouter>
            <App />
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<Root>::new().render();
}
