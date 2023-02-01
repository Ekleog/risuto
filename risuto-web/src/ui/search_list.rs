use std::iter;

use risuto_client::api::{Search, SearchId, Tag, TagId, UserId};
use yew::prelude::*;

use crate::util;

#[derive(Clone, PartialEq, Properties)]
pub struct SearchListProps {
    pub searches: im::HashMap<SearchId, Search>,
    pub tags: im::HashMap<TagId, Tag>,
    pub current_user: UserId,
    pub active_search: SearchId,
    pub on_select_search: Callback<Search>,
}

enum Item {
    Search(Search),
    Separator(&'static str),
}

#[function_component(SearchList)]
pub fn search_list(p: &SearchListProps) -> Html {
    let mut searches = p.searches.values().collect::<Vec<_>>();
    searches.sort_by_key(|s| (s.priority, &s.name, s.id));
    let mut tags = p.tags.values().collect::<Vec<_>>();
    util::sort_tags(&p.current_user, &mut tags, |t| t);
    let list_items = iter::once(Item::Search(Search::today(util::local_tz())))
        .chain(iter::once(Item::Separator("Custom Searches")))
        .chain(p.searches.values().cloned().map(Item::Search))
        .chain(iter::once(Item::Separator("Tags")))
        .chain(tags.into_iter().map(Search::for_tag).map(Item::Search))
        .chain(iter::once(Item::Search(Search::untagged())))
        .map(|it| match it {
            Item::Separator(name) => html! {
                <li class="category border-bottom p-1">
                    { name }
                </li>
            },
            Item::Search(search) => {
                let is_active = match search.id == p.active_search {
                    true => "active",
                    false => "",
                };
                let on_select_tag = {
                    let search = search.clone();
                    p.on_select_search.reform(move |_| search.clone())
                };
                html! {
                    <li class={classes!(is_active, "border-bottom", "p-2")}>
                        <a
                            class={classes!("nav-link", is_active)}
                            href={format!("#search-{}", js_sys::encode_uri(&search.name))}
                            onclick={on_select_tag}
                        >
                            { search.name.clone() }
                        </a>
                    </li>
                }
            }
        });
    html! {
        <ul class="nav flex-column">
            { for list_items }
        </ul>
    }
}
