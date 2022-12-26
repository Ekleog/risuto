use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct SearchBarProps {
}

#[function_component(SearchBar)]
pub fn search_bar(p: &SearchBarProps) -> Html {
    html! {
        <div class="search-bar m-3">
            <button
                type="button"
                class="btn btn-light btn-circle bi-btn bi-search fs-6"
                title="Search"
                data-bs-toggle="collapse"
                data-bs-target="#search-bar"
            >
            </button>
            <div id="search-bar" class="collapse collapse-horizontal">
                <div class="d-flex flex-align-center h-100">
                    <input
                        type="text"
                        class="ms-2 me-3"
                        placeholder="Type your search here"
                    />
                </div>
            </div>
        </div>
    }
}
