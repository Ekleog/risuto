use risuto_api::{AuthInfo, Tag, TagId, UserId};
use std::collections::HashMap;
use yew::prelude::*;

use crate::util;

#[derive(Clone, PartialEq, Properties)]
pub struct TagListProps {
    pub tags: HashMap<TagId, (Tag, AuthInfo)>,
    pub current_user: UserId,
    pub active: Option<TagId>,
    pub on_select_tag: Callback<Option<TagId>>,
}

#[function_component(TagList)]
pub fn tag_list(p: &TagListProps) -> Html {
    let mut tags = p.tags.iter().collect();
    util::sort_tags(&p.current_user, &mut tags);
    let list_items = tags
        .iter()
        .map(|(id, t)| (Some((*id).clone()), t.0.name.clone()))
        .chain(std::iter::once((None, String::from(":untagged"))))
        .map(|(id, tag)| {
            let id = id.clone();
            let is_active = match id == p.active {
                true => "active",
                false => "",
            };
            let on_select_tag = p.on_select_tag.reform(move |_| id);
            html! {
                <li class={classes!("nav-item", is_active, "border-bottom", "p-2")}>
                    <a
                        class={classes!("nav-link", is_active)}
                        href={format!("#tag-{}", tag)}
                        onclick={on_select_tag}
                    >
                        { tag }
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
