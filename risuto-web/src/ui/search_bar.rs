use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct SearchBarProps {
}

#[function_component(SearchBar)]
pub fn search_bar(p: &SearchBarProps) -> Html {
    let is_shown = use_state(|| false);
    let toggle_shown = {
        let is_shown = is_shown.clone();
        Callback::from(move |_| is_shown.set(!*is_shown))
    };
    let is_shown = is_shown.then(|| "search-bar-shown");
    html! {
        <div class="flex-fill">
            <div class={classes!("search-bar", "m-3", is_shown)}>
                <button
                    type="button"
                    class="btn btn-light btn-circle bi-btn bi-search fs-6"
                    title="Search"
                    onclick={toggle_shown}
                >
                </button>
                <div class="search-bar-input">
                    <input
                        type="text"
                        class="w-100 h-100 px-3"
                        placeholder="Type your search here"
                    />
                </div>
            </div>
        </div>
    }
}
