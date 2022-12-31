use std::{rc::Rc, sync::Arc};

use risuto_client::{api::Query, DbDump, Order, OrderType, QueryExt, Search, Task};
use yew::prelude::*;

use crate::util;

#[derive(Clone, PartialEq, Properties)]
pub struct SearchBarProps {
    pub db: Rc<DbDump>,
}

#[function_component(SearchBar)]
pub fn search_bar(p: &SearchBarProps) -> Html {
    // whether the search bar is shown at all
    let bar_shown = use_state(|| false);
    let toggle_shown = {
        let bar_shown = bar_shown.clone();
        Callback::from(move |_| bar_shown.set(!*bar_shown))
    };
    let bar_shown = bar_shown.then(|| "shown");

    // the results, local or fetched from the server
    let results = use_state(|| None::<SearchResults>);
    let query_local = {
        let db = p.db.clone();
        let results = results.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            let search: web_sys::HtmlInputElement = e.target_unchecked_into();
            let search = search.value();
            let search = search.trim();
            results.set(match search.len() {
                0 => None,
                _ => {
                    let filter = Query::from_search(&db, &util::local_tz(), search.trim());
                    tracing::debug!("searching with query {:?}", filter);
                    tracing::debug!("(parsed from {:?})", search.trim());
                    let search = Search {
                        name: String::from("Search Bar"),
                        filter,
                        order: Order::LastEventDate(OrderType::Desc),
                    };
                    Some(SearchResults::Local(db.search(&search)))
                }
            });
        })
    };
    let query_server = Callback::from(|_| {
        // TODO: clear results if it was a previous server search, and trigger a search on the remote server
        // this behavior should be shown by a last line on the search results hinting at it
        // eg: on local search results page, display "local results only" and add ", press enter to search from server" if a server search is not already ongoing
        //     on server search results page, display "server results" and add ", press enter to search again" if a server search is not already ongoing
        // also remind the user of the search results currently displayed, as they can differ from the search bar?
    });
    let results_shown = (bar_shown.is_some() && results.is_some()).then(|| "shown");
    let results = match results.as_ref().map(|r| r.results()) {
        None => html!(),
        Some(v) if v.is_empty() => html! {
            <li class="list-group-item"><em>{ "No results" }</em></li>
        },
        Some(v) => v
            .iter()
            .map(|t| {
                html! {
                    <li class="list-group-item">{ t.current_title.clone() }</li>
                }
            })
            .collect(),
    };

    html! {
        <div class="flex-fill">
            <div class={classes!("search-bar", "m-3", bar_shown)}>
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
                        oninput={query_local}
                        onchange={query_server}
                    />
                </div>
                <div class={classes!("search-results", results_shown)}>
                    <ul class="list-group">
                        { results }
                    </ul>
                </div>
            </div>
        </div>
    }
}

enum SearchResults {
    Local(Vec<Arc<Task>>),
    // Server(Vec<Arc<Task>>),
}

impl SearchResults {
    fn results(&self) -> &Vec<Arc<Task>> {
        match self {
            SearchResults::Local(v) => v,
        }
    }
}
