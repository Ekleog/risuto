use risuto_client::{
    api::{Event, TagId},
    DbDump, Task,
};
use std::{rc::Rc, sync::Arc};
use yew::prelude::*;

use crate::ui;

#[derive(Clone, PartialEq, Properties)]
pub struct TaskListProps {
    pub ref_this: NodeRef,
    pub db: Rc<DbDump>,
    pub current_tag: Option<TagId>,
    pub user_knows_current_tag: bool,
    pub tasks: Rc<Vec<Arc<Task>>>,
    pub now: chrono::DateTime<chrono::Utc>,
    pub timezone: chrono_tz::Tz,
    pub on_event: Callback<Event>,
}

#[function_component(TaskList)]
pub fn task_list(p: &TaskListProps) -> Html {
    // First, build the list items
    let list_items = p.tasks.iter().map(|t| {
        html! {
            <ui::TaskListItem
                task={ t.clone() }
                db={ p.db.clone() }
                current_tag={ p.current_tag.clone() }
                user_knows_current_tag={ p.user_knows_current_tag }
                now={ p.now.clone() }
                timezone={ p.timezone.clone() }
                on_event={ p.on_event.clone() }
            />
        }
    });

    // Then, put everything together
    html! {
        <ul ref={p.ref_this.clone()} class="task-list list-group">
            { for list_items }
        </ul>
    }
}
