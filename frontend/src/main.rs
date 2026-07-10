mod api;
mod app;
mod components;
mod glue;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
