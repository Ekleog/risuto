use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
            <h1>{ "Tasks" }</h1>
            <ul>
                <li>{"task 1"}</li>
                <li>{"task 2"}</li>
            </ul>
        </>
    }
}

fn main() {
    yew::start_app::<App>();
}
