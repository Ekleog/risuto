use risuto_client::{api::UserId, Search};
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct SearchListProps {
    pub searches: Vec<Search>,
    pub current_user: UserId,
    pub active_search: usize,
    pub on_select_search: Callback<usize>,
}

#[function_component(SearchList)]
pub fn search_list(p: &SearchListProps) -> Html {
    let list_items = p.searches.iter().enumerate().map(|(id, s)| {
        let is_active = match id == p.active_search {
            true => "active",
            false => "",
        };
        let on_select_tag = p.on_select_search.reform(move |_| id);
        html! {
            <li class={classes!("nav-item", is_active, "border-bottom", "p-2")}>
                <a
                    class={classes!("nav-link", is_active)}
                    href={format!("#search-{}", id)} // TODO: replace with urlescape(s.name)
                    onclick={on_select_tag}
                >
                    { s.name.clone() }
                </a>
            </li>
        }
    });
    html! {
        <ul class="nav flex-column">
            { for list_items }
        </ul>
    }
}
