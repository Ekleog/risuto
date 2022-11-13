use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
            <h1>{ "Tasks" }</h1>
            <ul class="list-group">
                <li class="list-group-item">{"task 1"}</li>
                <li class="list-group-item">{"task 2"}</li>
            </ul>
        </>
    }
}

fn main() {
    yew::start_app::<App>();
}
