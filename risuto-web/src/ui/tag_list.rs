use risuto_api::{Tag, TagId};
use yew::prelude::*;

#[derive(Clone, PartialEq, Properties)]
pub struct TagListProps {
    pub tags: Vec<(TagId, Tag)>,
    pub active: Option<TagId>,
    pub on_select_tag: Callback<Option<TagId>>,
}

#[function_component(TagList)]
pub fn tag_list(p: &TagListProps) -> Html {
    let list_items = p
        .tags
        .iter()
        .map(|(id, t)| (Some(id.clone()), t.name.clone()))
        .chain(std::iter::once((None, String::from(":untagged"))))
        .map(|(id, tag)| {
            let id = id.clone();
            let a_class = match id == p.active {
                true => "nav-link active",
                false => "nav-link",
            };
            let on_select_tag = p.on_select_tag.reform(move |_| id);
            html! {
                <li class="nav-item border-bottom p-2">
                    <a
                        class={ a_class }
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
